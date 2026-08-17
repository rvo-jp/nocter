use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64DataId, Arm64DataSize, Arm64EncodingError, Arm64FunctionId, Arm64Instruction,
    Arm64Register,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Arm64LabelId(usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CodeItem {
    Instruction(Arm64Instruction),
    Branch {
        target: Arm64LabelId,
        link: bool,
    },
    ConditionalBranch {
        target: Arm64LabelId,
        condition: Arm64BranchCondition,
    },
    FunctionBranch {
        target: Arm64FunctionId,
        link: bool,
    },
    DataAddress {
        target: Arm64DataId,
        destination: Arm64Register,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Arm64CodeFixup {
    FunctionBranch {
        offset: u64,
        target: Arm64FunctionId,
        link: bool,
    },
    DataAddress {
        offset: u64,
        target: Arm64DataId,
        destination: Arm64Register,
    },
}

/// Builder for one function's local ARM64 control-flow domain.
#[derive(Default)]
pub struct Arm64CodeBuilder {
    items: Vec<CodeItem>,
    labels: Vec<Option<usize>>,
}

impl Arm64CodeBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            items: Vec::new(),
            labels: Vec::new(),
        }
    }

    #[must_use]
    pub fn create_label(&mut self) -> Arm64LabelId {
        let label = Arm64LabelId(self.labels.len());
        self.labels.push(None);
        label
    }

    /// Binds a label to the next instruction.
    ///
    /// # Errors
    ///
    /// Rejects binding the same label twice.
    pub fn bind(&mut self, label: Arm64LabelId) -> Result<(), Arm64CodeError> {
        let binding = self
            .labels
            .get_mut(label.0)
            .ok_or(Arm64CodeError::UnknownLabel(label))?;
        if binding.replace(self.items.len()).is_some() {
            return Err(Arm64CodeError::DuplicateLabel(label));
        }
        Ok(())
    }

    pub fn append(&mut self, instruction: Arm64Instruction) {
        self.items.push(CodeItem::Instruction(instruction));
    }

    pub fn branch(&mut self, target: Arm64LabelId, link: bool) {
        self.items.push(CodeItem::Branch { target, link });
    }

    pub fn branch_conditional(&mut self, target: Arm64LabelId, condition: Arm64BranchCondition) {
        self.items
            .push(CodeItem::ConditionalBranch { target, condition });
    }

    /// Emits a whole-program function reference to be resolved after text layout.
    pub fn branch_function(&mut self, target: Arm64FunctionId, link: bool) {
        self.items.push(CodeItem::FunctionBranch { target, link });
    }

    /// Emits a linked whole-program function call.
    pub fn call(&mut self, target: Arm64FunctionId) {
        self.branch_function(target, true);
    }

    /// Materializes one read-only-data address through a page and page-offset fixup pair.
    pub fn load_data_address(&mut self, target: Arm64DataId, destination: Arm64Register) {
        self.items.push(CodeItem::DataAddress {
            target,
            destination,
        });
    }

    /// Resolves all labels and relaxes out-of-range conditional branches before encoding.
    ///
    /// # Errors
    ///
    /// Rejects unknown or unbound labels, offset overflow, out-of-range unconditional branches,
    /// and invalid concrete instruction encodings.
    pub fn finish(self) -> Result<Arm64Code, Arm64CodeError> {
        let mut expanded = vec![false; self.items.len()];
        loop {
            let item_offsets = item_offsets(&self.items, &expanded)?;
            let labels = resolve_labels(&self.labels, &item_offsets)?;
            let mut changed = false;
            for (index, item) in self.items.iter().enumerate() {
                let CodeItem::ConditionalBranch { target, condition } = item else {
                    continue;
                };
                if expanded[index] {
                    continue;
                }
                let displacement =
                    displacement(item_offsets[index], label_offset(&labels, *target)?)?;
                match (Arm64Instruction::BranchConditional {
                    displacement,
                    condition: *condition,
                })
                .encode()
                {
                    Ok(_) => {}
                    Err(Arm64EncodingError::BranchOutOfRange) => {
                        expanded[index] = true;
                        changed = true;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            if !changed {
                return encode_items(self.items, &expanded, &item_offsets, labels);
            }
        }
    }
}

/// Final local code bytes and resolved label offsets for one ARM64 function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64Code {
    bytes: Box<[u8]>,
    label_offsets: Box<[u64]>,
    fixups: Box<[Arm64CodeFixup]>,
    instruction_count: usize,
}

impl Arm64Code {
    #[must_use]
    pub const fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub fn label_offset(&self, label: Arm64LabelId) -> Option<u64> {
        self.label_offsets.get(label.0).copied()
    }

    #[must_use]
    pub const fn instruction_count(&self) -> usize {
        self.instruction_count
    }

    pub(crate) fn into_parts(self) -> (Box<[u8]>, Box<[Arm64CodeFixup]>) {
        (self.bytes, self.fixups)
    }
}

fn item_offsets(items: &[CodeItem], expanded: &[bool]) -> Result<Vec<u64>, Arm64CodeError> {
    let mut offsets = Vec::with_capacity(items.len() + 1);
    let mut offset = 0_u64;
    for (index, item) in items.iter().enumerate() {
        offsets.push(offset);
        let size = match item {
            CodeItem::ConditionalBranch { .. } if expanded[index] => 8,
            CodeItem::DataAddress { .. } => 8,
            _ => 4,
        };
        offset = offset
            .checked_add(size)
            .ok_or(Arm64CodeError::OffsetOverflow)?;
    }
    offsets.push(offset);
    Ok(offsets)
}

fn resolve_labels(
    bindings: &[Option<usize>],
    item_offsets: &[u64],
) -> Result<Vec<u64>, Arm64CodeError> {
    bindings
        .iter()
        .enumerate()
        .map(|(index, binding)| {
            let label = Arm64LabelId(index);
            let item = binding.ok_or(Arm64CodeError::UnboundLabel(label))?;
            item_offsets
                .get(item)
                .copied()
                .ok_or(Arm64CodeError::OffsetOverflow)
        })
        .collect()
}

fn encode_items(
    items: Vec<CodeItem>,
    expanded: &[bool],
    offsets: &[u64],
    labels: Vec<u64>,
) -> Result<Arm64Code, Arm64CodeError> {
    let capacity = usize::try_from(*offsets.last().ok_or(Arm64CodeError::OffsetOverflow)?)
        .map_err(|_| Arm64CodeError::OffsetOverflow)?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut fixups = Vec::new();
    for (index, item) in items.into_iter().enumerate() {
        match item {
            CodeItem::Instruction(instruction) => append(&mut bytes, instruction)?,
            CodeItem::Branch { target, link } => append(
                &mut bytes,
                Arm64Instruction::Branch {
                    displacement: displacement(offsets[index], label_offset(&labels, target)?)?,
                    link,
                },
            )?,
            CodeItem::ConditionalBranch { target, condition } if expanded[index] => {
                append(
                    &mut bytes,
                    Arm64Instruction::BranchConditional {
                        displacement: 8,
                        condition: condition.invert(),
                    },
                )?;
                append(
                    &mut bytes,
                    Arm64Instruction::Branch {
                        displacement: displacement(
                            offsets[index]
                                .checked_add(4)
                                .ok_or(Arm64CodeError::OffsetOverflow)?,
                            label_offset(&labels, target)?,
                        )?,
                        link: false,
                    },
                )?;
            }
            CodeItem::ConditionalBranch { target, condition } => append(
                &mut bytes,
                Arm64Instruction::BranchConditional {
                    displacement: displacement(offsets[index], label_offset(&labels, target)?)?,
                    condition,
                },
            )?,
            CodeItem::FunctionBranch { target, link } => {
                append(
                    &mut bytes,
                    Arm64Instruction::Branch {
                        displacement: 0,
                        link,
                    },
                )?;
                fixups.push(Arm64CodeFixup::FunctionBranch {
                    offset: offsets[index],
                    target,
                    link,
                });
            }
            CodeItem::DataAddress {
                target,
                destination,
            } => {
                append(
                    &mut bytes,
                    Arm64Instruction::AddressPage {
                        destination,
                        displacement: 0,
                    },
                )?;
                append(
                    &mut bytes,
                    Arm64Instruction::AddSubtractImmediate {
                        size: Arm64DataSize::Bits64,
                        operation: Arm64AddSubtract::Add,
                        set_flags: false,
                        destination: Arm64AddSubtractDestination::General(destination),
                        source: Arm64BaseRegister::General(destination),
                        immediate: 0,
                        shift_12: false,
                    },
                )?;
                fixups.push(Arm64CodeFixup::DataAddress {
                    offset: offsets[index],
                    target,
                    destination,
                });
            }
        }
    }
    let instruction_count = bytes.len() / 4;
    Ok(Arm64Code {
        bytes: bytes.into_boxed_slice(),
        label_offsets: labels.into_boxed_slice(),
        fixups: fixups.into_boxed_slice(),
        instruction_count,
    })
}

fn label_offset(labels: &[u64], label: Arm64LabelId) -> Result<u64, Arm64CodeError> {
    labels
        .get(label.0)
        .copied()
        .ok_or(Arm64CodeError::UnknownLabel(label))
}

fn append(bytes: &mut Vec<u8>, instruction: Arm64Instruction) -> Result<(), Arm64CodeError> {
    bytes.extend_from_slice(&instruction.encode()?);
    Ok(())
}

fn displacement(source: u64, target: u64) -> Result<i64, Arm64CodeError> {
    let source = i64::try_from(source).map_err(|_| Arm64CodeError::OffsetOverflow)?;
    let target = i64::try_from(target).map_err(|_| Arm64CodeError::OffsetOverflow)?;
    target
        .checked_sub(source)
        .ok_or(Arm64CodeError::OffsetOverflow)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64CodeError {
    UnknownLabel(Arm64LabelId),
    DuplicateLabel(Arm64LabelId),
    UnboundLabel(Arm64LabelId),
    OffsetOverflow,
    Encoding(Arm64EncodingError),
}

impl From<Arm64EncodingError> for Arm64CodeError {
    fn from(error: Arm64EncodingError) -> Self {
        Self::Encoding(error)
    }
}
