use std::collections::{BTreeMap, BTreeSet};

use nocter_mir::{MirBody, MirConstant, MirOperationKind, MirProgram, MirRoot};
use nocter_model::{ConstantValue, ExecutableStaticId, FrozenValue, TypeId};
use nocter_runtime_contract::{RuntimePrimitive, RuntimeType};

use crate::identity::{MachineId, MachineTable};
use crate::{
    MachineDataId, MachineEndianness, MachineLayoutKind, MachineLayoutPlan, MachineProgramError,
    MachineTarget,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineDataRelocation {
    offset: u64,
    target: MachineDataId,
}

impl MachineDataRelocation {
    #[must_use]
    pub const fn offset(self) -> u64 {
        self.offset
    }

    #[must_use]
    pub const fn target(self) -> MachineDataId {
        self.target
    }
}

/// One canonical immutable data object in the final machine program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineData {
    bytes: Box<[u8]>,
    alignment: u64,
    relocations: Box<[MachineDataRelocation]>,
}

impl MachineData {
    #[must_use]
    pub const fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    #[must_use]
    pub const fn relocations(&self) -> &[MachineDataRelocation] {
        &self.relocations
    }
}

/// Final immutable data in deterministic key order.
///
/// Literal text is keyed by content. Static objects are keyed first by their dense executable
/// identity so distinct declarations cannot acquire one shared address.
#[derive(Debug)]
pub struct MachineDataTable {
    entries: MachineTable<MachineDataId, MachineData>,
}

impl MachineDataTable {
    #[must_use]
    pub fn get(&self, id: MachineDataId) -> Option<&MachineData> {
        self.entries.get(id)
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (MachineDataId, &MachineData)> {
        self.entries.iter()
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.len() == 0
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DataKey {
    Text(Box<str>),
    Static {
        identity: ExecutableStaticId,
        alignment: u64,
        bytes: Box<[u8]>,
        text_relocations: Box<[(u64, Box<str>)]>,
    },
}

struct PendingData {
    bytes: Box<[u8]>,
    alignment: u64,
    text_relocations: Box<[(u64, Box<str>)]>,
}

/// Serialized objects whose final dense identities and relocation targets are not assigned yet.
struct PendingDataTable {
    entries: BTreeMap<DataKey, PendingData>,
    statics: BTreeMap<ExecutableStaticId, DataKey>,
}

/// Construction-only lookup from MIR identities to final data identities.
#[derive(Debug)]
pub(crate) struct MachineDataPlan {
    entries: MachineTable<MachineDataId, MachineData>,
    text_ids: BTreeMap<Box<str>, MachineDataId>,
    statics: BTreeMap<ExecutableStaticId, MachineDataId>,
}

impl MachineDataPlan {
    pub(crate) fn build(
        program: &MirProgram,
        layouts: &MachineLayoutPlan,
    ) -> Result<Self, MachineProgramError> {
        PendingDataTable::build(program, layouts)?.finish()
    }

    pub(crate) fn text(&self, text: &str) -> Option<MachineDataId> {
        self.text_ids.get(text).copied()
    }

    pub(crate) fn static_value(&self, id: ExecutableStaticId) -> Option<MachineDataId> {
        self.statics.get(&id).copied()
    }

    pub(crate) fn finish(self) -> MachineDataTable {
        MachineDataTable {
            entries: self.entries,
        }
    }
}

impl PendingDataTable {
    fn build(
        program: &MirProgram,
        layouts: &MachineLayoutPlan,
    ) -> Result<Self, MachineProgramError> {
        let mut texts = BTreeSet::new();
        for (_, function) in program.functions().iter() {
            collect_text(function.body(), &mut texts);
        }
        match program.root() {
            MirRoot::Process(root) => collect_text(root.body(), &mut texts),
            MirRoot::Tests { cases, .. } => {
                for case in cases {
                    collect_text(case.body(), &mut texts);
                }
            }
        }
        for (_, value) in program.statics().iter() {
            collect_frozen_text(value.value(), &mut texts);
        }

        let mut entries = BTreeMap::<DataKey, PendingData>::new();
        for text in texts {
            entries.insert(
                DataKey::Text(text.clone()),
                PendingData {
                    bytes: text.as_bytes().into(),
                    alignment: 1,
                    text_relocations: Box::new([]),
                },
            );
        }

        let target = MachineTarget::select(program.runtime_abi());
        let mut static_keys = BTreeMap::new();
        for (id, value) in program.statics().iter() {
            let layout = layouts
                .get(value.ty())
                .ok_or(MachineProgramError::MissingStoredLayout(value.ty()))?;
            let size = usize::try_from(layout.size())
                .map_err(|_| MachineProgramError::InvalidStaticData(id))?;
            let mut bytes = vec![0; size];
            let mut relocations = Vec::new();
            encode_frozen(
                program,
                layouts,
                target,
                value.ty(),
                value.value(),
                0,
                &mut bytes,
                &mut relocations,
                id,
            )?;
            relocations.sort_unstable();
            let key = DataKey::Static {
                identity: id,
                alignment: layout.alignment(),
                bytes: bytes.clone().into_boxed_slice(),
                text_relocations: relocations.clone().into_boxed_slice(),
            };
            entries.entry(key.clone()).or_insert(PendingData {
                bytes: bytes.into_boxed_slice(),
                alignment: layout.alignment(),
                text_relocations: relocations.into_boxed_slice(),
            });
            static_keys.insert(id, key);
        }

        Ok(Self {
            entries,
            statics: static_keys,
        })
    }

    fn finish(self) -> Result<MachineDataPlan, MachineProgramError> {
        let key_ids = self
            .entries
            .keys()
            .cloned()
            .enumerate()
            .map(|(index, key)| (key, MachineDataId::new(index)))
            .collect::<BTreeMap<_, _>>();
        let text_ids = key_ids
            .iter()
            .filter_map(|(key, id)| match key {
                DataKey::Text(text) => Some((text.clone(), *id)),
                DataKey::Static { .. } => None,
            })
            .collect::<BTreeMap<_, _>>();
        let entries = self
            .entries
            .into_values()
            .map(|pending| {
                let relocations = pending
                    .text_relocations
                    .iter()
                    .map(|(offset, text)| {
                        text_ids
                            .get(text)
                            .copied()
                            .map(|target| MachineDataRelocation {
                                offset: *offset,
                                target,
                            })
                            .ok_or_else(|| MachineProgramError::MissingStaticText(text.clone()))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(MachineData {
                    bytes: pending.bytes,
                    alignment: pending.alignment,
                    relocations: relocations.into_boxed_slice(),
                })
            })
            .collect::<Result<Vec<_>, MachineProgramError>>()?;
        let statics = self
            .statics
            .into_iter()
            .map(|(source, key)| {
                key_ids
                    .get(&key)
                    .copied()
                    .map(|id| (source, id))
                    .ok_or(MachineProgramError::InvalidStaticData(source))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(MachineDataPlan {
            entries: MachineTable::from_values(entries),
            text_ids,
            statics,
        })
    }
}

fn collect_text(body: &MirBody, texts: &mut BTreeSet<Box<str>>) {
    for (_, operation) in body.operations().iter() {
        if let MirOperationKind::Constant(MirConstant::Text(text)) = operation.kind() {
            texts.insert(text.clone());
        }
    }
}

fn collect_frozen_text(value: &FrozenValue, texts: &mut BTreeSet<Box<str>>) {
    match value {
        FrozenValue::Scalar(ConstantValue::Text(text)) => {
            texts.insert(text.clone());
        }
        FrozenValue::FixedArray(values) => {
            for value in values {
                collect_frozen_text(value, texts);
            }
        }
        FrozenValue::Scalar(_) => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn encode_frozen(
    program: &MirProgram,
    layouts: &MachineLayoutPlan,
    target: MachineTarget,
    ty: TypeId,
    value: &FrozenValue,
    offset: u64,
    bytes: &mut [u8],
    relocations: &mut Vec<(u64, Box<str>)>,
    owner: ExecutableStaticId,
) -> Result<(), MachineProgramError> {
    let layout = layouts
        .get(ty)
        .ok_or(MachineProgramError::MissingStoredLayout(ty))?;
    match (value, layout.kind()) {
        (FrozenValue::Scalar(ConstantValue::Bool(value)), MachineLayoutKind::Scalar(_)) => {
            write_bytes(bytes, offset, &[u8::from(*value)], owner)
        }
        (FrozenValue::Scalar(ConstantValue::Character(value)), MachineLayoutKind::Scalar(_)) => {
            write_integer(
                bytes,
                offset,
                u128::from(*value),
                layout.size(),
                target,
                owner,
            )
        }
        (FrozenValue::Scalar(ConstantValue::Integer(value)), MachineLayoutKind::Scalar(_)) => {
            write_integer(
                bytes,
                offset,
                value.cast_unsigned(),
                layout.size(),
                target,
                owner,
            )
        }
        (
            FrozenValue::Scalar(ConstantValue::Text(text)),
            MachineLayoutKind::View {
                pointer_offset,
                length_offset,
            },
        ) => {
            let Some(RuntimeType::Borrow { referent, .. }) = program.types().get(ty) else {
                return Err(MachineProgramError::InvalidStaticData(owner));
            };
            if !matches!(
                program.types().get(*referent),
                Some(RuntimeType::Primitive(RuntimePrimitive::Text))
            ) {
                return Err(MachineProgramError::InvalidStaticData(owner));
            }
            let pointer = offset
                .checked_add(*pointer_offset)
                .ok_or(MachineProgramError::InvalidStaticData(owner))?;
            let length = offset
                .checked_add(*length_offset)
                .ok_or(MachineProgramError::InvalidStaticData(owner))?;
            write_integer(
                bytes,
                length,
                text.len() as u128,
                target.word_size(),
                target,
                owner,
            )?;
            relocations.push((pointer, text.clone()));
            Ok(())
        }
        (
            FrozenValue::FixedArray(values),
            MachineLayoutKind::FixedArray {
                element,
                length,
                stride,
            },
        ) if usize::try_from(*length) == Ok(values.len()) => {
            for (index, value) in values.iter().enumerate() {
                let element_offset = offset
                    .checked_add(
                        stride
                            .checked_mul(index as u64)
                            .ok_or(MachineProgramError::InvalidStaticData(owner))?,
                    )
                    .ok_or(MachineProgramError::InvalidStaticData(owner))?;
                encode_frozen(
                    program,
                    layouts,
                    target,
                    *element,
                    value,
                    element_offset,
                    bytes,
                    relocations,
                    owner,
                )?;
            }
            Ok(())
        }
        _ => Err(MachineProgramError::InvalidStaticData(owner)),
    }
}

fn write_integer(
    bytes: &mut [u8],
    offset: u64,
    value: u128,
    size: u64,
    target: MachineTarget,
    owner: ExecutableStaticId,
) -> Result<(), MachineProgramError> {
    let encoded = match target.endianness() {
        MachineEndianness::Little => value.to_le_bytes(),
        MachineEndianness::Big => value.to_be_bytes(),
    };
    let size = usize::try_from(size).map_err(|_| MachineProgramError::InvalidStaticData(owner))?;
    let slice = match target.endianness() {
        MachineEndianness::Little => encoded.get(..size),
        MachineEndianness::Big => encoded.get(encoded.len().saturating_sub(size)..),
    }
    .ok_or(MachineProgramError::InvalidStaticData(owner))?;
    write_bytes(bytes, offset, slice, owner)
}

fn write_bytes(
    bytes: &mut [u8],
    offset: u64,
    value: &[u8],
    owner: ExecutableStaticId,
) -> Result<(), MachineProgramError> {
    let start =
        usize::try_from(offset).map_err(|_| MachineProgramError::InvalidStaticData(owner))?;
    let end = start
        .checked_add(value.len())
        .ok_or(MachineProgramError::InvalidStaticData(owner))?;
    bytes
        .get_mut(start..end)
        .ok_or(MachineProgramError::InvalidStaticData(owner))?
        .copy_from_slice(value);
    Ok(())
}
