use std::fmt;
use std::sync::Arc;

use crate::code::Arm64CodeFixup;
use crate::{
    Arm64Code, Arm64DataId, Arm64EncodingError, Arm64FunctionId, Arm64Instruction, Arm64Register,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64FunctionRange {
    offset: u64,
    size: u64,
}

impl Arm64FunctionRange {
    #[must_use]
    pub const fn offset(self) -> u64 {
        self.offset
    }

    #[must_use]
    pub const fn size(self) -> u64 {
        self.size
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DataRange {
    offset: u64,
    size: u64,
    alignment: u64,
}

impl Arm64DataRange {
    #[must_use]
    pub const fn offset(self) -> u64 {
        self.offset
    }

    #[must_use]
    pub const fn size(self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn alignment(self) -> u64 {
        self.alignment
    }
}

/// One data-address pair awaiting final text and read-only-section virtual addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DataAddressFixup {
    instruction_offset: u64,
    target_offset: u64,
    destination: Arm64Register,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Arm64FunctionAddressFixup {
    instruction_offset: u64,
    target_offset: u64,
    destination: Arm64Register,
}

impl Arm64DataAddressFixup {
    #[must_use]
    pub const fn instruction_offset(self) -> u64 {
        self.instruction_offset
    }

    #[must_use]
    pub const fn target_offset(self) -> u64 {
        self.target_offset
    }

    #[must_use]
    pub const fn destination(self) -> Arm64Register {
        self.destination
    }
}

/// One fully laid-out ARM64 text section plus its independently laid-out read-only data section.
/// Only page-address fixups remain because their displacement depends on Mach-O virtual addresses.
#[derive(Debug, Eq, PartialEq)]
struct Arm64ProgramContents {
    text: Box<[u8]>,
    read_only_data: Box<[u8]>,
    functions: Box<[Arm64FunctionRange]>,
    data: Box<[Arm64DataRange]>,
    data_alignment: u64,
    function_address_fixups: Box<[Arm64FunctionAddressFixup]>,
    data_fixups: Box<[Arm64DataAddressFixup]>,
}

/// One independently executable entry over immutable target code and data. Test entries from the
/// same machine program share the completed contents and differ only in their native entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64Program {
    contents: Arc<Arm64ProgramContents>,
    entry: Arm64FunctionId,
}

impl Arm64Program {
    #[must_use]
    pub fn text(&self) -> &[u8] {
        &self.contents.text
    }

    #[must_use]
    pub fn read_only_data(&self) -> &[u8] {
        &self.contents.read_only_data
    }

    #[must_use]
    pub fn function(&self, id: Arm64FunctionId) -> Option<Arm64FunctionRange> {
        self.contents.functions.get(id.0).copied()
    }

    #[must_use]
    pub fn data(&self, id: Arm64DataId) -> Option<Arm64DataRange> {
        self.contents.data.get(id.0).copied()
    }

    #[must_use]
    pub fn data_fixups(&self) -> &[Arm64DataAddressFixup] {
        &self.contents.data_fixups
    }

    #[must_use]
    pub fn read_only_data_alignment(&self) -> u64 {
        self.contents.data_alignment
    }

    #[must_use]
    pub const fn entry(&self) -> Arm64FunctionId {
        self.entry
    }

    pub(crate) fn with_entry(&self, entry: Arm64FunctionId) -> Result<Self, Arm64ProgramError> {
        if self.contents.functions.get(entry.0).is_none() {
            return Err(Arm64ProgramError::UnknownFunction(entry));
        }
        Ok(Self {
            contents: Arc::clone(&self.contents),
            entry,
        })
    }

    /// Resolves every function- and data-address pair for final section virtual addresses.
    ///
    /// # Errors
    ///
    /// Rejects virtual-address arithmetic overflow, a malformed fixup offset, or a page
    /// displacement outside ARM64's signed 21-bit page range.
    pub fn relocate_addresses(
        &self,
        text_virtual_address: u64,
        data_virtual_address: u64,
    ) -> Result<Box<[u8]>, Arm64ProgramError> {
        let mut text = self.contents.text.to_vec();
        for fixup in &self.contents.function_address_fixups {
            let instruction_address = text_virtual_address
                .checked_add(fixup.instruction_offset)
                .ok_or(Arm64ProgramError::AddressOverflow)?;
            let target_address = text_virtual_address
                .checked_add(fixup.target_offset)
                .ok_or(Arm64ProgramError::AddressOverflow)?;
            patch_address_pair(
                &mut text,
                fixup.instruction_offset,
                instruction_address,
                target_address,
                fixup.destination,
            )?;
        }
        for fixup in &self.contents.data_fixups {
            let instruction_address = text_virtual_address
                .checked_add(fixup.instruction_offset)
                .ok_or(Arm64ProgramError::AddressOverflow)?;
            let target_address = data_virtual_address
                .checked_add(fixup.target_offset)
                .ok_or(Arm64ProgramError::AddressOverflow)?;
            patch_address_pair(
                &mut text,
                fixup.instruction_offset,
                instruction_address,
                target_address,
                fixup.destination,
            )?;
        }
        Ok(text.into_boxed_slice())
    }
}

struct DataDefinition {
    bytes: Box<[u8]>,
    alignment: u64,
}

/// Consuming builder for a single native executable image.
#[derive(Default)]
pub struct Arm64ProgramBuilder {
    functions: Vec<Option<Arm64Code>>,
    data: Vec<DataDefinition>,
    entry: Option<Arm64FunctionId>,
}

impl Arm64ProgramBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            functions: Vec::new(),
            data: Vec::new(),
            entry: None,
        }
    }

    /// Allocates a stable function identity before any bodies need to reference it.
    #[must_use]
    pub fn declare_function(&mut self) -> Arm64FunctionId {
        let id = Arm64FunctionId(self.functions.len());
        self.functions.push(None);
        id
    }

    /// Defines one previously declared function.
    ///
    /// # Errors
    ///
    /// Rejects an identity outside this builder or a second definition.
    pub fn define_function(
        &mut self,
        id: Arm64FunctionId,
        code: Arm64Code,
    ) -> Result<(), Arm64ProgramError> {
        let definition = self
            .functions
            .get_mut(id.0)
            .ok_or(Arm64ProgramError::UnknownFunction(id))?;
        if definition.is_some() {
            return Err(Arm64ProgramError::DuplicateFunction(id));
        }
        *definition = Some(code);
        Ok(())
    }

    /// Adds read-only data in deterministic caller order.
    ///
    /// # Errors
    ///
    /// Rejects zero or non-power-of-two alignment.
    pub fn add_data(
        &mut self,
        bytes: impl Into<Box<[u8]>>,
        alignment: u64,
    ) -> Result<Arm64DataId, Arm64ProgramError> {
        if !alignment.is_power_of_two() {
            return Err(Arm64ProgramError::InvalidDataAlignment(alignment));
        }
        let id = Arm64DataId(self.data.len());
        self.data.push(DataDefinition {
            bytes: bytes.into(),
            alignment,
        });
        Ok(id)
    }

    /// Selects the unique native entry function.
    ///
    /// # Errors
    ///
    /// Rejects a second entry selection.
    pub fn set_entry(&mut self, entry: Arm64FunctionId) -> Result<(), Arm64ProgramError> {
        if self.entry.is_some() {
            return Err(Arm64ProgramError::DuplicateEntry);
        }
        self.entry = Some(entry);
        Ok(())
    }

    /// Finalizes section-local layout, resolves every function branch, and validates all function
    /// and data address references before exposing an immutable program.
    ///
    /// # Errors
    ///
    /// Rejects missing definitions or entry, foreign targets, arithmetic overflow, an out-of-range
    /// function branch, or an invalid instruction placeholder.
    pub fn finish(self) -> Result<Arm64Program, Arm64ProgramError> {
        let entry = self.entry.ok_or(Arm64ProgramError::MissingEntry)?;
        if self.functions.get(entry.0).is_none() {
            return Err(Arm64ProgramError::UnknownFunction(entry));
        }

        let mut text = Vec::new();
        let mut functions = Vec::with_capacity(self.functions.len());
        let mut code_fixups = Vec::new();
        for (index, definition) in self.functions.into_iter().enumerate() {
            let id = Arm64FunctionId(index);
            let code = definition.ok_or(Arm64ProgramError::MissingFunction(id))?;
            let offset =
                u64::try_from(text.len()).map_err(|_| Arm64ProgramError::OffsetOverflow)?;
            let (bytes, fixups) = code.into_parts();
            let size = u64::try_from(bytes.len()).map_err(|_| Arm64ProgramError::OffsetOverflow)?;
            text.extend_from_slice(&bytes);
            functions.push(Arm64FunctionRange { offset, size });
            for fixup in fixups {
                code_fixups.push(offset_fixup(fixup, offset)?);
            }
        }

        let laid_out_data = layout_data(self.data)?;
        let read_only_data = laid_out_data.bytes;
        let data = laid_out_data.ranges;
        let data_alignment = laid_out_data.alignment;
        let mut function_address_fixups = Vec::new();
        let mut data_fixups = Vec::new();
        for fixup in code_fixups {
            match fixup {
                Arm64CodeFixup::FunctionBranch {
                    offset,
                    target,
                    link,
                } => {
                    let target = functions
                        .get(target.0)
                        .ok_or(Arm64ProgramError::UnknownFunction(target))?;
                    patch_function_branch(&mut text, offset, target.offset, link)?;
                }
                Arm64CodeFixup::FunctionAddress {
                    offset,
                    target,
                    destination,
                } => {
                    let target = functions
                        .get(target.0)
                        .ok_or(Arm64ProgramError::UnknownFunction(target))?;
                    function_address_fixups.push(Arm64FunctionAddressFixup {
                        instruction_offset: offset,
                        target_offset: target.offset,
                        destination,
                    });
                }
                Arm64CodeFixup::DataAddress {
                    offset,
                    target,
                    destination,
                } => {
                    let target = data
                        .get(target.0)
                        .ok_or(Arm64ProgramError::UnknownData(target))?;
                    data_fixups.push(Arm64DataAddressFixup {
                        instruction_offset: offset,
                        target_offset: target.offset,
                        destination,
                    });
                }
            }
        }

        Ok(Arm64Program {
            contents: Arc::new(Arm64ProgramContents {
                text: text.into_boxed_slice(),
                read_only_data,
                functions: functions.into_boxed_slice(),
                data,
                data_alignment,
                function_address_fixups: function_address_fixups.into_boxed_slice(),
                data_fixups: data_fixups.into_boxed_slice(),
            }),
            entry,
        })
    }
}

fn offset_fixup(
    fixup: Arm64CodeFixup,
    function_offset: u64,
) -> Result<Arm64CodeFixup, Arm64ProgramError> {
    match fixup {
        Arm64CodeFixup::FunctionBranch {
            offset,
            target,
            link,
        } => Ok(Arm64CodeFixup::FunctionBranch {
            offset: function_offset
                .checked_add(offset)
                .ok_or(Arm64ProgramError::OffsetOverflow)?,
            target,
            link,
        }),
        Arm64CodeFixup::FunctionAddress {
            offset,
            target,
            destination,
        } => Ok(Arm64CodeFixup::FunctionAddress {
            offset: function_offset
                .checked_add(offset)
                .ok_or(Arm64ProgramError::OffsetOverflow)?,
            target,
            destination,
        }),
        Arm64CodeFixup::DataAddress {
            offset,
            target,
            destination,
        } => Ok(Arm64CodeFixup::DataAddress {
            offset: function_offset
                .checked_add(offset)
                .ok_or(Arm64ProgramError::OffsetOverflow)?,
            target,
            destination,
        }),
    }
}

fn patch_address_pair(
    text: &mut [u8],
    instruction_offset: u64,
    instruction_address: u64,
    target_address: u64,
    destination: Arm64Register,
) -> Result<(), Arm64ProgramError> {
    let instruction_page = instruction_address & !0x0fff;
    let target_page = target_address & !0x0fff;
    let displacement = i128::from(target_page) - i128::from(instruction_page);
    let displacement =
        i64::try_from(displacement).map_err(|_| Arm64ProgramError::AddressOverflow)?;
    patch_word(
        text,
        instruction_offset,
        Arm64Instruction::AddressPage {
            destination,
            displacement,
        },
    )?;
    patch_word(
        text,
        instruction_offset
            .checked_add(4)
            .ok_or(Arm64ProgramError::OffsetOverflow)?,
        Arm64Instruction::AddSubtractImmediate {
            size: crate::Arm64DataSize::Bits64,
            operation: crate::Arm64AddSubtract::Add,
            set_flags: false,
            destination: crate::Arm64AddSubtractDestination::General(destination),
            source: crate::Arm64BaseRegister::General(destination),
            immediate: u16::try_from(target_address & 0x0fff)
                .map_err(|_| Arm64ProgramError::AddressOverflow)?,
            shift_12: false,
        },
    )
}

fn patch_function_branch(
    text: &mut [u8],
    source: u64,
    target: u64,
    link: bool,
) -> Result<(), Arm64ProgramError> {
    let source_signed = i64::try_from(source).map_err(|_| Arm64ProgramError::OffsetOverflow)?;
    let target_signed = i64::try_from(target).map_err(|_| Arm64ProgramError::OffsetOverflow)?;
    let displacement = target_signed
        .checked_sub(source_signed)
        .ok_or(Arm64ProgramError::OffsetOverflow)?;
    patch_word(
        text,
        source,
        Arm64Instruction::Branch { displacement, link },
    )
}

fn patch_word(
    text: &mut [u8],
    instruction_offset: u64,
    instruction: Arm64Instruction,
) -> Result<(), Arm64ProgramError> {
    let encoded = instruction.encode()?;
    let offset =
        usize::try_from(instruction_offset).map_err(|_| Arm64ProgramError::OffsetOverflow)?;
    let end = offset
        .checked_add(4)
        .ok_or(Arm64ProgramError::OffsetOverflow)?;
    let destination = text
        .get_mut(offset..end)
        .ok_or(Arm64ProgramError::InvalidFixupOffset(instruction_offset))?;
    destination.copy_from_slice(&encoded);
    Ok(())
}

struct LaidOutData {
    bytes: Box<[u8]>,
    ranges: Box<[Arm64DataRange]>,
    alignment: u64,
}

fn layout_data(definitions: Vec<DataDefinition>) -> Result<LaidOutData, Arm64ProgramError> {
    let mut bytes = Vec::new();
    let mut ranges = Vec::with_capacity(definitions.len());
    let mut section_alignment = 1;
    for definition in definitions {
        section_alignment = section_alignment.max(definition.alignment);
        let current = u64::try_from(bytes.len()).map_err(|_| Arm64ProgramError::OffsetOverflow)?;
        let offset = align_up(current, definition.alignment)?;
        let offset_usize =
            usize::try_from(offset).map_err(|_| Arm64ProgramError::OffsetOverflow)?;
        bytes.resize(offset_usize, 0);
        let size =
            u64::try_from(definition.bytes.len()).map_err(|_| Arm64ProgramError::OffsetOverflow)?;
        bytes.extend_from_slice(&definition.bytes);
        ranges.push(Arm64DataRange {
            offset,
            size,
            alignment: definition.alignment,
        });
    }
    Ok(LaidOutData {
        bytes: bytes.into_boxed_slice(),
        ranges: ranges.into_boxed_slice(),
        alignment: section_alignment,
    })
}

fn align_up(value: u64, alignment: u64) -> Result<u64, Arm64ProgramError> {
    let mask = alignment - 1;
    value
        .checked_add(mask)
        .map(|value| value & !mask)
        .ok_or(Arm64ProgramError::OffsetOverflow)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64ProgramError {
    UnknownFunction(Arm64FunctionId),
    DuplicateFunction(Arm64FunctionId),
    MissingFunction(Arm64FunctionId),
    UnknownData(Arm64DataId),
    InvalidDataAlignment(u64),
    MissingEntry,
    DuplicateEntry,
    InvalidFixupOffset(u64),
    OffsetOverflow,
    AddressOverflow,
    Encoding(Arm64EncodingError),
}

impl fmt::Display for Arm64ProgramError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 program construction failed: {self:?}")
    }
}

impl std::error::Error for Arm64ProgramError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encoding(error) => Some(error),
            Self::UnknownFunction(_)
            | Self::DuplicateFunction(_)
            | Self::MissingFunction(_)
            | Self::UnknownData(_)
            | Self::InvalidDataAlignment(_)
            | Self::MissingEntry
            | Self::DuplicateEntry
            | Self::InvalidFixupOffset(_)
            | Self::OffsetOverflow
            | Self::AddressOverflow => None,
        }
    }
}

impl From<Arm64EncodingError> for Arm64ProgramError {
    fn from(error: Arm64EncodingError) -> Self {
        Self::Encoding(error)
    }
}
