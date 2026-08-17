use std::collections::BTreeMap;
use std::fmt;

use nocter_mir::{MirBody, MirLocalKind, MirValueDefinition};
use nocter_model::{
    ExecutableItemId, MirBlockId, MirDropFlagId, MirLocalId, MirOperationId, MirValueId,
};

use super::MachineProgramError;
use super::control::lower_blocks;
use super::operation::lower_operations;
use crate::identity::{MachineId, MachineTable};
use crate::{
    MachineBlockId, MachineDataTable, MachineDropFlag, MachineDropFlagId, MachineFunctionId,
    MachineLayoutStore, MachineLinkageId, MachineOperationId, MachineStackId, MachineStackObject,
    MachineStackPurpose, MachineValue, MachineValueDefinition, MachineValueId,
};

pub(super) fn lower_body(
    owner: MachineLinkageId,
    body: &MirBody,
    layouts: &MachineLayoutStore,
    data: &MachineDataTable,
    functions: &BTreeMap<ExecutableItemId, MachineFunctionId>,
) -> Result<crate::MachineBody, MachineProgramError> {
    let ids = BodyIdentities::new(owner, body);
    let stack = lower_stack(body, layouts)?;
    let parameters = body
        .parameters()
        .iter()
        .map(|parameter| ids.stack(*parameter))
        .collect::<Result<Vec<_>, _>>()?;
    let drop_flags = body
        .drop_flags()
        .iter()
        .map(|(_, flag)| MachineDropFlag::new(flag.initially_initialized()))
        .collect::<Vec<_>>();
    let values = lower_values(body, layouts, &ids)?;
    let operations = lower_operations(body, data, functions, &ids)?;
    let blocks = lower_blocks(body, &ids)?;

    Ok(crate::MachineBody::new(
        parameters,
        MachineTable::from_values(stack),
        MachineTable::from_values(drop_flags),
        MachineTable::from_values(values),
        MachineTable::from_values(operations),
        MachineTable::from_values(blocks),
        ids.block(body.entry())?,
    ))
}

fn lower_stack(
    body: &MirBody,
    layouts: &MachineLayoutStore,
) -> Result<Vec<MachineStackObject>, MachineProgramError> {
    body.locals()
        .iter()
        .map(|(_, local)| {
            let layout = layouts
                .get(local.ty())
                .ok_or(MachineProgramError::MissingStoredLayout(local.ty()))?;
            let purpose = match local.kind() {
                MirLocalKind::Parameter { position } => MachineStackPurpose::Parameter { position },
                MirLocalKind::User => MachineStackPurpose::User,
                MirLocalKind::Temporary => MachineStackPurpose::Temporary,
                MirLocalKind::Region => MachineStackPurpose::Region,
            };
            Ok(MachineStackObject::new(
                local.ty(),
                layout.size(),
                layout.alignment(),
                purpose,
            ))
        })
        .collect()
}

fn lower_values(
    body: &MirBody,
    layouts: &MachineLayoutStore,
    ids: &BodyIdentities,
) -> Result<Vec<MachineValue>, MachineProgramError> {
    body.values()
        .iter()
        .map(|(_, value)| {
            if layouts.get(value.ty()).is_none() {
                return Err(MachineProgramError::MissingStoredLayout(value.ty()));
            }
            let definition = match value.definition() {
                MirValueDefinition::BlockParameter { block, position } => {
                    MachineValueDefinition::BlockParameter {
                        block: ids.block(block)?,
                        position,
                    }
                }
                MirValueDefinition::Operation(operation) => {
                    MachineValueDefinition::Operation(ids.operation(operation)?)
                }
            };
            Ok(MachineValue::new(value.ty(), definition))
        })
        .collect()
}

pub(super) struct BodyIdentities {
    owner: MachineLinkageId,
    stack: BTreeMap<MirLocalId, MachineStackId>,
    drop_flags: BTreeMap<MirDropFlagId, MachineDropFlagId>,
    values: BTreeMap<MirValueId, MachineValueId>,
    operations: BTreeMap<MirOperationId, MachineOperationId>,
    blocks: BTreeMap<MirBlockId, MachineBlockId>,
}

impl BodyIdentities {
    fn new(owner: MachineLinkageId, body: &MirBody) -> Self {
        Self {
            owner,
            stack: assign_ids::<MirLocalId, MachineStackId, _>(body.locals().iter()),
            drop_flags: assign_ids::<MirDropFlagId, MachineDropFlagId, _>(body.drop_flags().iter()),
            values: assign_ids::<MirValueId, MachineValueId, _>(body.values().iter()),
            operations: assign_ids::<MirOperationId, MachineOperationId, _>(
                body.operations().iter(),
            ),
            blocks: assign_ids::<MirBlockId, MachineBlockId, _>(body.blocks().iter()),
        }
    }

    pub(super) const fn owner(&self) -> MachineLinkageId {
        self.owner
    }

    fn stack(&self, source: MirLocalId) -> Result<MachineStackId, MachineProgramError> {
        self.require(&self.stack, source)
    }

    pub(super) fn drop_flag(
        &self,
        source: MirDropFlagId,
    ) -> Result<MachineDropFlagId, MachineProgramError> {
        self.require(&self.drop_flags, source)
    }

    pub(super) fn value(&self, source: MirValueId) -> Result<MachineValueId, MachineProgramError> {
        self.require(&self.values, source)
    }

    pub(super) fn operation(
        &self,
        source: MirOperationId,
    ) -> Result<MachineOperationId, MachineProgramError> {
        self.require(&self.operations, source)
    }

    pub(super) fn block(&self, source: MirBlockId) -> Result<MachineBlockId, MachineProgramError> {
        self.require(&self.blocks, source)
    }

    fn require<K: Copy + Ord + fmt::Debug, I: Copy>(
        &self,
        ids: &BTreeMap<K, I>,
        source: K,
    ) -> Result<I, MachineProgramError> {
        ids.get(&source)
            .copied()
            .ok_or(MachineProgramError::MissingBodyIdentity {
                owner: self.owner,
                source: format!("{source:?}").into_boxed_str(),
            })
    }
}

fn assign_ids<K: Copy + Ord, I: MachineId, V>(
    values: impl ExactSizeIterator<Item = (K, V)>,
) -> BTreeMap<K, I> {
    values
        .enumerate()
        .map(|(index, (source, _))| (source, I::new(index)))
        .collect()
}
