use std::fmt;

use nocter_arm64::{Arm64Program, Arm64ProgramError};

use nocter_hash::sha256;

const MACH_HEADER_SIZE: u64 = 32;
const VM_BASE: u64 = 0x1_0000_0000;
const SEGMENT_ALIGNMENT: u64 = 0x4000;
const CODE_HASH_PAGE_SIZE: usize = 0x1000;
const CODE_SIGNATURE_ALIGNMENT: u64 = 16;

const LC_SEGMENT_64: u32 = 0x19;
const LC_LOAD_DYLINKER: u32 = 0x0e;
const LC_BUILD_VERSION: u32 = 0x32;
const LC_MAIN: u32 = 0x8000_0028;
const LC_LOAD_DYLIB: u32 = 0x0c;
const LC_CODE_SIGNATURE: u32 = 0x1d;
const LC_UUID: u32 = 0x1b;
const LC_DYLD_INFO_ONLY: u32 = 0x8000_0022;

const REBASE_TYPE_POINTER: u8 = 1;
const REBASE_OPCODE_SET_TYPE_IMM: u8 = 0x10;
const REBASE_OPCODE_SET_SEGMENT_AND_OFFSET_ULEB: u8 = 0x20;
const REBASE_OPCODE_DO_REBASE_IMM_TIMES: u8 = 0x50;
const DATA_CONST_SEGMENT_INDEX: u8 = 2;

const SEGMENT_COMMAND_SIZE: u32 = 72;
const SECTION_SIZE: u32 = 80;
const LINKEDIT_DATA_COMMAND_SIZE: u32 = 16;
const MAIN_COMMAND_SIZE: u32 = 24;
const BUILD_VERSION_COMMAND_SIZE: u32 = 24;
const UUID_COMMAND_SIZE: u32 = 24;
const DYLD_INFO_COMMAND_SIZE: u32 = 48;

const CODE_DIRECTORY_MAGIC: u32 = 0xfade_0c02;
const EMBEDDED_SIGNATURE_MAGIC: u32 = 0xfade_0cc0;
const CODE_DIRECTORY_VERSION: u32 = 0x0002_0400;
const CODE_SIGNATURE_FLAGS: u32 = 0x0002_0002;
const SHA256_HASH_SIZE: u8 = 32;
const SHA256_HASH_TYPE: u8 = 2;
const CODE_HASH_PAGE_SHIFT: u8 = 12;
const EXECUTABLE_SEGMENT_MAIN_BINARY: u64 = 1;
const CODE_DIRECTORY_HEADER_SIZE: u32 = 88;
const SUPERBLOB_HEADER_SIZE: u32 = 20;
const SIGNATURE_IDENTIFIER: &[u8] = b"nocter\0";

/// A complete executable file image. Writing it to an executable path requires no assembler,
/// linker, code-signing process, or runtime bundled with Nocter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachOImage {
    bytes: Box<[u8]>,
}

impl MachOImage {
    /// Serializes one ARM64 program as a deterministic ad-hoc-signed Mach-O executable.
    ///
    /// # Errors
    ///
    /// Rejects offset/address overflow, section alignment beyond Mach-O's 32-bit exponent field,
    /// relocation failure, and file layouts that exceed 32-bit Mach-O command offsets.
    pub fn build(program: &Arm64Program) -> Result<Self, MachOError> {
        let layout = ImageLayout::new(program)?;
        let relocated = program.relocate_sections(
            checked_add(VM_BASE, layout.text_offset)?,
            checked_add(VM_BASE, layout.data_offset)?,
        )?;
        let uuid = image_uuid(
            relocated.text(),
            relocated.read_only_data(),
            layout.entry_offset,
        );
        let mut bytes = Vec::with_capacity(layout.file_size()?);
        write_header(&mut bytes, &layout);
        write_load_commands(&mut bytes, &layout, uuid);
        resize_to(&mut bytes, layout.text_offset)?;
        bytes.extend_from_slice(relocated.text());
        resize_to(&mut bytes, layout.data_offset)?;
        bytes.extend_from_slice(relocated.read_only_data());
        if !layout.rebase_info.is_empty() {
            resize_to(&mut bytes, layout.rebase_offset)?;
            bytes.extend_from_slice(&layout.rebase_info);
        }
        resize_to(&mut bytes, layout.signature_offset)?;
        let signature = code_signature(
            &bytes,
            layout.signature_offset,
            u64::try_from(program.text().len()).map_err(|_| MachOError::OffsetOverflow)?,
        )?;
        debug_assert_eq!(signature.len(), layout.signature_size as usize);
        bytes.extend_from_slice(&signature);
        Ok(Self {
            bytes: bytes.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub fn into_bytes(self) -> Box<[u8]> {
        self.bytes
    }
}

struct ImageLayout {
    command_count: u32,
    command_size: u32,
    text_offset: u64,
    text_size: u64,
    data_offset: u64,
    data_size: u64,
    data_alignment_power: u32,
    rebase_offset: u64,
    rebase_info: Box<[u8]>,
    linkedit_offset: u64,
    signature_offset: u64,
    signature_size: u32,
    linkedit_file_size: u64,
    linkedit_virtual_size: u64,
    entry_offset: u64,
    dylinker_command_size: u32,
    dylib_command_size: u32,
}

impl ImageLayout {
    fn new(program: &Arm64Program) -> Result<Self, MachOError> {
        let dylinker_command_size = string_command_size(12, b"/usr/lib/dyld\0")?;
        let dylib_command_size = string_command_size(24, b"/usr/lib/libSystem.B.dylib\0")?;
        let command_count = 11;
        let command_size = SEGMENT_COMMAND_SIZE
            .checked_add(SEGMENT_COMMAND_SIZE + SECTION_SIZE)
            .and_then(|size| size.checked_add(SEGMENT_COMMAND_SIZE + SECTION_SIZE))
            .and_then(|size| size.checked_add(SEGMENT_COMMAND_SIZE))
            .and_then(|size| size.checked_add(dylinker_command_size))
            .and_then(|size| size.checked_add(BUILD_VERSION_COMMAND_SIZE))
            .and_then(|size| size.checked_add(UUID_COMMAND_SIZE))
            .and_then(|size| size.checked_add(MAIN_COMMAND_SIZE))
            .and_then(|size| size.checked_add(dylib_command_size))
            .and_then(|size| size.checked_add(DYLD_INFO_COMMAND_SIZE))
            .and_then(|size| size.checked_add(LINKEDIT_DATA_COMMAND_SIZE))
            .ok_or(MachOError::OffsetOverflow)?;
        let text_offset = align_up(checked_add(MACH_HEADER_SIZE, u64::from(command_size))?, 16)?;
        let text_size =
            u64::try_from(program.text().len()).map_err(|_| MachOError::OffsetOverflow)?;
        let text_end = checked_add(text_offset, text_size)?;
        let data_alignment = program.read_only_data_alignment();
        if !data_alignment.is_power_of_two() {
            return Err(MachOError::InvalidDataAlignment(data_alignment));
        }
        let data_alignment_power = data_alignment.trailing_zeros();
        let data_offset = align_up(text_end, SEGMENT_ALIGNMENT)?;
        let data_size = u64::try_from(program.read_only_data().len())
            .map_err(|_| MachOError::OffsetOverflow)?;
        let content_end = checked_add(data_offset, data_size)?;
        let linkedit_offset = align_up(content_end, SEGMENT_ALIGNMENT)?;
        let rebase_info = encode_rebase_info(program);
        let rebase_offset = if rebase_info.is_empty() {
            0
        } else {
            linkedit_offset
        };
        let rebase_end = checked_add(
            linkedit_offset,
            u64::try_from(rebase_info.len()).map_err(|_| MachOError::OffsetOverflow)?,
        )?;
        let signature_offset = align_up(rebase_end, CODE_SIGNATURE_ALIGNMENT)?;
        require_u32(signature_offset)?;
        require_u32(rebase_offset)?;
        require_u32(u64::try_from(rebase_info.len()).map_err(|_| MachOError::OffsetOverflow)?)?;
        let signature_size = signature_size(signature_offset)?;
        let linkedit_file_size = checked_add(signature_offset, u64::from(signature_size))?
            .checked_sub(linkedit_offset)
            .ok_or(MachOError::OverlappingLayout)?;
        let linkedit_virtual_size = align_up(linkedit_file_size, SEGMENT_ALIGNMENT)?;
        let entry = program
            .function(program.entry())
            .ok_or(MachOError::MissingEntryFunction)?;
        let entry_offset = checked_add(text_offset, entry.offset())?;
        Ok(Self {
            command_count,
            command_size,
            text_offset,
            text_size,
            data_offset,
            data_size,
            data_alignment_power,
            rebase_offset,
            rebase_info,
            linkedit_offset,
            signature_offset,
            signature_size,
            linkedit_file_size,
            linkedit_virtual_size,
            entry_offset,
            dylinker_command_size,
            dylib_command_size,
        })
    }

    fn file_size(&self) -> Result<usize, MachOError> {
        usize::try_from(checked_add(
            self.signature_offset,
            u64::from(self.signature_size),
        )?)
        .map_err(|_| MachOError::OffsetOverflow)
    }
}

fn write_header(bytes: &mut Vec<u8>, layout: &ImageLayout) {
    push_u32(bytes, 0xfeed_facf);
    push_u32(bytes, 0x0100_000c);
    push_u32(bytes, 0);
    push_u32(bytes, 2);
    push_u32(bytes, layout.command_count);
    push_u32(bytes, layout.command_size);
    push_u32(bytes, 0x0020_0085);
    push_u32(bytes, 0);
}

fn write_load_commands(bytes: &mut Vec<u8>, layout: &ImageLayout, uuid: [u8; 16]) {
    write_segment(
        bytes,
        SegmentCommand {
            name: "__PAGEZERO",
            virtual_address: 0,
            virtual_size: VM_BASE,
            file_offset: 0,
            file_size: 0,
            maximum_protection: 0,
            initial_protection: 0,
            section_count: 0,
            flags: 0,
        },
    );
    write_text_segment(bytes, layout);
    write_data_const_segment(bytes, layout);
    write_segment(
        bytes,
        SegmentCommand {
            name: "__LINKEDIT",
            virtual_address: checked_add(VM_BASE, layout.linkedit_offset)
                .expect("validated layout"),
            virtual_size: layout.linkedit_virtual_size,
            file_offset: layout.linkedit_offset,
            file_size: layout.linkedit_file_size,
            maximum_protection: 1,
            initial_protection: 1,
            section_count: 0,
            flags: 0,
        },
    );
    write_string_command(
        bytes,
        LC_LOAD_DYLINKER,
        layout.dylinker_command_size,
        12,
        b"/usr/lib/dyld\0",
    );
    push_u32(bytes, LC_BUILD_VERSION);
    push_u32(bytes, BUILD_VERSION_COMMAND_SIZE);
    push_u32(bytes, 1);
    push_u32(bytes, 0x000d_0000);
    push_u32(bytes, 0);
    push_u32(bytes, 0);
    push_u32(bytes, LC_UUID);
    push_u32(bytes, UUID_COMMAND_SIZE);
    bytes.extend_from_slice(&uuid);
    push_u32(bytes, LC_MAIN);
    push_u32(bytes, MAIN_COMMAND_SIZE);
    push_u64(bytes, layout.entry_offset);
    push_u64(bytes, 0);
    write_dylib_command(bytes, layout.dylib_command_size);
    push_u32(bytes, LC_DYLD_INFO_ONLY);
    push_u32(bytes, DYLD_INFO_COMMAND_SIZE);
    push_u32(
        bytes,
        u32::try_from(layout.rebase_offset).expect("validated 32-bit rebase offset"),
    );
    push_u32(
        bytes,
        u32::try_from(layout.rebase_info.len()).expect("validated 32-bit rebase size"),
    );
    for _ in 0..8 {
        push_u32(bytes, 0);
    }
    push_u32(bytes, LC_CODE_SIGNATURE);
    push_u32(bytes, LINKEDIT_DATA_COMMAND_SIZE);
    push_u32(
        bytes,
        u32::try_from(layout.signature_offset).expect("validated 32-bit file offset"),
    );
    push_u32(bytes, layout.signature_size);
}

/// Encodes the classic dyld rebase stream for absolute pointers in `__DATA_CONST,__const`.
///
/// Code references remain PC-relative and need no loader metadata. Each data pointer has already
/// been relocated to its preferred virtual address by ARM64 lowering; dyld adds the single image
/// slide described by this stream when loading a PIE image.
fn encode_rebase_info(program: &Arm64Program) -> Box<[u8]> {
    if program.data_pointer_fixups().is_empty() {
        return Box::new([]);
    }

    let mut output = vec![REBASE_OPCODE_SET_TYPE_IMM | REBASE_TYPE_POINTER];
    for fixup in program.data_pointer_fixups() {
        output.push(REBASE_OPCODE_SET_SEGMENT_AND_OFFSET_ULEB | DATA_CONST_SEGMENT_INDEX);
        push_uleb128(&mut output, fixup.location_offset());
        output.push(REBASE_OPCODE_DO_REBASE_IMM_TIMES | 1);
    }
    output.push(0);
    output.into_boxed_slice()
}

fn push_uleb128(output: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        output.push(byte);
        if value == 0 {
            return;
        }
    }
}

fn image_uuid(text: &[u8], data: &[u8], entry_offset: u64) -> [u8; 16] {
    let mut content = Vec::with_capacity(text.len() + data.len() + 8);
    content.extend_from_slice(text);
    content.extend_from_slice(data);
    content.extend_from_slice(&entry_offset.to_le_bytes());
    let digest = sha256(&content);
    let mut uuid: [u8; 16] = digest[..16].try_into().expect("digest prefix has 16 bytes");
    uuid[6] = (uuid[6] & 0x0f) | 0x30;
    uuid[8] = (uuid[8] & 0x3f) | 0x80;
    uuid
}

fn write_text_segment(bytes: &mut Vec<u8>, layout: &ImageLayout) {
    write_segment(
        bytes,
        SegmentCommand {
            name: "__TEXT",
            virtual_address: VM_BASE,
            virtual_size: layout.data_offset,
            file_offset: 0,
            file_size: layout.data_offset,
            maximum_protection: 5,
            initial_protection: 5,
            section_count: 1,
            flags: 0,
        },
    );
    write_section(
        bytes,
        SectionRecord {
            section: "__text",
            segment: "__TEXT",
            address: VM_BASE + layout.text_offset,
            size: layout.text_size,
            offset: layout.text_offset,
            alignment_power: 2,
            flags: 0x8000_0400,
        },
    );
}

fn write_data_const_segment(bytes: &mut Vec<u8>, layout: &ImageLayout) {
    write_segment(
        bytes,
        SegmentCommand {
            name: "__DATA_CONST",
            virtual_address: VM_BASE + layout.data_offset,
            virtual_size: layout.linkedit_offset - layout.data_offset,
            file_offset: layout.data_offset,
            file_size: layout.linkedit_offset - layout.data_offset,
            maximum_protection: 3,
            initial_protection: 3,
            section_count: 1,
            flags: 0x10,
        },
    );
    write_section(
        bytes,
        SectionRecord {
            section: "__const",
            segment: "__DATA_CONST",
            address: VM_BASE + layout.data_offset,
            size: layout.data_size,
            offset: layout.data_offset,
            alignment_power: layout.data_alignment_power,
            flags: 0,
        },
    );
}

#[derive(Clone, Copy)]
struct SegmentCommand<'name> {
    name: &'name str,
    virtual_address: u64,
    virtual_size: u64,
    file_offset: u64,
    file_size: u64,
    maximum_protection: u32,
    initial_protection: u32,
    section_count: u32,
    flags: u32,
}

fn write_segment(bytes: &mut Vec<u8>, segment: SegmentCommand<'_>) {
    push_u32(bytes, LC_SEGMENT_64);
    push_u32(
        bytes,
        SEGMENT_COMMAND_SIZE + segment.section_count * SECTION_SIZE,
    );
    push_name(bytes, segment.name);
    push_u64(bytes, segment.virtual_address);
    push_u64(bytes, segment.virtual_size);
    push_u64(bytes, segment.file_offset);
    push_u64(bytes, segment.file_size);
    push_u32(bytes, segment.maximum_protection);
    push_u32(bytes, segment.initial_protection);
    push_u32(bytes, segment.section_count);
    push_u32(bytes, segment.flags);
}

#[derive(Clone, Copy)]
struct SectionRecord<'name> {
    section: &'name str,
    segment: &'name str,
    address: u64,
    size: u64,
    offset: u64,
    alignment_power: u32,
    flags: u32,
}

fn write_section(bytes: &mut Vec<u8>, section: SectionRecord<'_>) {
    push_name(bytes, section.section);
    push_name(bytes, section.segment);
    push_u64(bytes, section.address);
    push_u64(bytes, section.size);
    push_u32(
        bytes,
        u32::try_from(section.offset).expect("validated 32-bit section offset"),
    );
    push_u32(bytes, section.alignment_power);
    push_u32(bytes, 0);
    push_u32(bytes, 0);
    push_u32(bytes, section.flags);
    push_u32(bytes, 0);
    push_u32(bytes, 0);
    push_u32(bytes, 0);
}

fn write_dylib_command(bytes: &mut Vec<u8>, command_size: u32) {
    push_u32(bytes, LC_LOAD_DYLIB);
    push_u32(bytes, command_size);
    push_u32(bytes, 24);
    push_u32(bytes, 2);
    push_u32(bytes, 0);
    push_u32(bytes, 0x0001_0000);
    push_padded(bytes, b"/usr/lib/libSystem.B.dylib\0", command_size - 24);
}

fn write_string_command(
    bytes: &mut Vec<u8>,
    command: u32,
    command_size: u32,
    string_offset: u32,
    value: &[u8],
) {
    push_u32(bytes, command);
    push_u32(bytes, command_size);
    push_u32(bytes, string_offset);
    push_padded(bytes, value, command_size - string_offset);
}

fn code_signature(
    code: &[u8],
    code_limit: u64,
    executable_size: u64,
) -> Result<Vec<u8>, MachOError> {
    let slot_count = code_slot_count(code_limit)?;
    let hash_offset = CODE_DIRECTORY_HEADER_SIZE
        .checked_add(
            u32::try_from(SIGNATURE_IDENTIFIER.len()).map_err(|_| MachOError::OffsetOverflow)?,
        )
        .ok_or(MachOError::OffsetOverflow)?;
    let directory_length = hash_offset
        .checked_add(
            slot_count
                .checked_mul(u32::from(SHA256_HASH_SIZE))
                .ok_or(MachOError::OffsetOverflow)?,
        )
        .ok_or(MachOError::OffsetOverflow)?;
    let blob_length = SUPERBLOB_HEADER_SIZE
        .checked_add(directory_length)
        .ok_or(MachOError::OffsetOverflow)?;
    let mut signature = Vec::with_capacity(
        usize::try_from(align_up(u64::from(blob_length), CODE_SIGNATURE_ALIGNMENT)?)
            .map_err(|_| MachOError::OffsetOverflow)?,
    );
    push_be_u32(&mut signature, EMBEDDED_SIGNATURE_MAGIC);
    push_be_u32(&mut signature, blob_length);
    push_be_u32(&mut signature, 1);
    push_be_u32(&mut signature, 0);
    push_be_u32(&mut signature, SUPERBLOB_HEADER_SIZE);
    write_code_directory(
        &mut signature,
        code,
        directory_length,
        hash_offset,
        slot_count,
        code_limit,
        executable_size,
    )?;
    let padded = align_up(
        u64::try_from(signature.len()).map_err(|_| MachOError::OffsetOverflow)?,
        CODE_SIGNATURE_ALIGNMENT,
    )?;
    resize_to(&mut signature, padded)?;
    Ok(signature)
}

fn write_code_directory(
    signature: &mut Vec<u8>,
    code: &[u8],
    directory_length: u32,
    hash_offset: u32,
    slot_count: u32,
    code_limit: u64,
    executable_size: u64,
) -> Result<(), MachOError> {
    push_be_u32(signature, CODE_DIRECTORY_MAGIC);
    push_be_u32(signature, directory_length);
    push_be_u32(signature, CODE_DIRECTORY_VERSION);
    push_be_u32(signature, CODE_SIGNATURE_FLAGS);
    push_be_u32(signature, hash_offset);
    push_be_u32(signature, CODE_DIRECTORY_HEADER_SIZE);
    push_be_u32(signature, 0);
    push_be_u32(signature, slot_count);
    push_be_u32(signature, require_u32(code_limit)?);
    signature.extend_from_slice(&[SHA256_HASH_SIZE, SHA256_HASH_TYPE, 0, CODE_HASH_PAGE_SHIFT]);
    push_be_u32(signature, 0);
    push_be_u32(signature, 0);
    push_be_u32(signature, 0);
    push_be_u32(signature, 0);
    push_be_u64(signature, code_limit);
    push_be_u64(signature, 0);
    push_be_u64(signature, executable_size);
    push_be_u64(signature, EXECUTABLE_SEGMENT_MAIN_BINARY);
    signature.extend_from_slice(SIGNATURE_IDENTIFIER);
    if u64::try_from(code.len()).map_err(|_| MachOError::OffsetOverflow)? != code_limit {
        return Err(MachOError::InvalidCodeLimit);
    }
    for page in code.chunks(CODE_HASH_PAGE_SIZE) {
        signature.extend_from_slice(&sha256(page));
    }
    Ok(())
}

fn signature_size(code_limit: u64) -> Result<u32, MachOError> {
    let slots = code_slot_count(code_limit)?;
    let directory = CODE_DIRECTORY_HEADER_SIZE
        .checked_add(
            u32::try_from(SIGNATURE_IDENTIFIER.len()).map_err(|_| MachOError::OffsetOverflow)?,
        )
        .and_then(|size| size.checked_add(slots.checked_mul(32)?))
        .ok_or(MachOError::OffsetOverflow)?;
    let blob = SUPERBLOB_HEADER_SIZE
        .checked_add(directory)
        .ok_or(MachOError::OffsetOverflow)?;
    require_u32(align_up(u64::from(blob), CODE_SIGNATURE_ALIGNMENT)?)
}

fn code_slot_count(code_limit: u64) -> Result<u32, MachOError> {
    let page_size = u64::try_from(CODE_HASH_PAGE_SIZE).expect("page size fits u64");
    let slots = code_limit.div_ceil(page_size);
    u32::try_from(slots).map_err(|_| MachOError::OffsetOverflow)
}

fn string_command_size(prefix: u32, value: &[u8]) -> Result<u32, MachOError> {
    let raw = u64::from(prefix)
        .checked_add(u64::try_from(value.len()).map_err(|_| MachOError::OffsetOverflow)?)
        .ok_or(MachOError::OffsetOverflow)?;
    require_u32(align_up(raw, 8)?)
}

fn push_padded(bytes: &mut Vec<u8>, value: &[u8], field_size: u32) {
    bytes.extend_from_slice(value);
    let padding = usize::try_from(field_size).expect("command field size fits usize") - value.len();
    bytes.resize(bytes.len() + padding, 0);
}

fn push_name(bytes: &mut Vec<u8>, name: &str) {
    let name = name.as_bytes();
    debug_assert!(name.len() <= 16);
    bytes.extend_from_slice(name);
    bytes.resize(bytes.len() + 16 - name.len(), 0);
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_be_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn push_be_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn resize_to(bytes: &mut Vec<u8>, size: u64) -> Result<(), MachOError> {
    let size = usize::try_from(size).map_err(|_| MachOError::OffsetOverflow)?;
    if bytes.len() > size {
        return Err(MachOError::OverlappingLayout);
    }
    bytes.resize(size, 0);
    Ok(())
}

fn checked_add(left: u64, right: u64) -> Result<u64, MachOError> {
    left.checked_add(right).ok_or(MachOError::OffsetOverflow)
}

fn align_up(value: u64, alignment: u64) -> Result<u64, MachOError> {
    if !alignment.is_power_of_two() {
        return Err(MachOError::InvalidDataAlignment(alignment));
    }
    let mask = alignment - 1;
    value
        .checked_add(mask)
        .map(|value| value & !mask)
        .ok_or(MachOError::OffsetOverflow)
}

fn require_u32(value: u64) -> Result<u32, MachOError> {
    u32::try_from(value).map_err(|_| MachOError::FileOffsetOutOfRange(value))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachOError {
    MissingEntryFunction,
    InvalidDataAlignment(u64),
    FileOffsetOutOfRange(u64),
    OverlappingLayout,
    InvalidCodeLimit,
    OffsetOverflow,
    Arm64(Arm64ProgramError),
}

impl fmt::Display for MachOError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEntryFunction => {
                formatter.write_str("ARM64 program has no entry function")
            }
            Self::InvalidDataAlignment(alignment) => {
                write!(formatter, "invalid Mach-O data alignment {alignment}")
            }
            Self::FileOffsetOutOfRange(offset) => {
                write!(
                    formatter,
                    "Mach-O file offset {offset} exceeds its encoded domain"
                )
            }
            Self::OverlappingLayout => formatter.write_str("Mach-O sections overlap"),
            Self::InvalidCodeLimit => formatter.write_str("Mach-O code signature limit is invalid"),
            Self::OffsetOverflow => formatter.write_str("Mach-O layout offset overflowed"),
            Self::Arm64(error) => write!(formatter, "ARM64 relocation failed: {error}"),
        }
    }
}

impl std::error::Error for MachOError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Arm64(error) => Some(error),
            Self::MissingEntryFunction
            | Self::InvalidDataAlignment(_)
            | Self::FileOffsetOutOfRange(_)
            | Self::OverlappingLayout
            | Self::InvalidCodeLimit
            | Self::OffsetOverflow => None,
        }
    }
}

impl From<Arm64ProgramError> for MachOError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Arm64(error)
    }
}
