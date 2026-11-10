//! Serialize established literal DAGs with the common stored layout and byte-order authority.
use super::Failure;
use crate::identity::MachineId;
use crate::{MachineData, MachineDataId, MachineDataTable, MachineLayoutPlan, MachineTarget};
use nocter_model::{FrozenData, FrozenNode, TypeId};
use nocter_runtime_contract::{RuntimeEnvironment, RuntimeTypeRepresentation};
use std::collections::{BTreeMap, HashMap};

pub(super) struct Data {
    values: Vec<MachineData>,
    statics: BTreeMap<nocter_model::ValueStatic, MachineDataId>,
    ids: HashMap<(TypeId, FrozenData), MachineDataId>,
    remaining: usize,
}
impl Data {
    pub fn build(
        program: &nocter_mir::value_access::Program,
        layouts: &MachineLayoutPlan,
    ) -> Result<Self, Failure> {
        let mut data = Self {
            values: Vec::new(),
            statics: BTreeMap::new(),
            ids: HashMap::new(),
            remaining: 16 * 1024 * 1024,
        };
        for (&id, source) in program.statics() {
            let object = data.encode(program.runtime(), layouts, source.data, &source.value)?;
            data.statics.insert(id, object);
        }
        for body in program.bodies().values() {
            for action in body.actions().values() {
                for operation in &action.operations {
                    if let nocter_mir::value_access::Operation::Frozen { destination, value } =
                        operation
                    {
                        data.insert(program.runtime(), layouts, destination.data, value)?;
                    }
                }
            }
        }
        Ok(data)
    }
    fn insert(
        &mut self,
        runtime: &RuntimeEnvironment,
        layouts: &MachineLayoutPlan,
        ty: TypeId,
        value: &FrozenData,
    ) -> Result<(), Failure> {
        let key = (ty, value.clone());
        if self.ids.contains_key(&key) {
            return Ok(());
        }
        let layout = layouts.get(ty).ok_or(Failure::InvalidRoot)?;
        let size = usize::try_from(layout.size()).map_err(|_| Failure::DataLimit(ty))?;
        if size == 0 {
            return Ok(());
        }
        let id = self.encode(runtime, layouts, ty, value)?;
        self.ids.insert(key, id);
        Ok(())
    }
    fn encode(
        &mut self,
        runtime: &RuntimeEnvironment,
        layouts: &MachineLayoutPlan,
        ty: TypeId,
        value: &FrozenData,
    ) -> Result<MachineDataId, Failure> {
        let layout = layouts.get(ty).ok_or(Failure::InvalidRoot)?;
        // An addressable empty static still receives distinct physical backing.
        let size = usize::try_from(layout.size().max(1)).map_err(|_| Failure::DataLimit(ty))?;
        self.remaining = self
            .remaining
            .checked_sub(size)
            .ok_or(Failure::DataLimit(ty))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size)
            .map_err(|_| Failure::DataLimit(ty))?;
        bytes.resize(size, 0);
        let mut pending = vec![(ty, value.root(), 0u64)];
        let target = MachineTarget::select(runtime.abi());
        while let Some((ty, node, offset)) = pending.pop() {
            let layout = layouts.get(ty).ok_or(Failure::InvalidRoot)?;
            if layout.size() == 0 {
                continue;
            }
            match &value.nodes()[node] {
                FrozenNode::Scalar(value) => {
                    let bits = crate::data::scalar_bits(value).ok_or(Failure::InvalidRoot)?;
                    crate::data::write_integer_bytes(
                        &mut bytes,
                        offset,
                        bits,
                        layout.size(),
                        target,
                    )
                    .ok_or(Failure::InvalidRoot)?;
                }
                FrozenNode::Aggregate(values) => {
                    let Some(RuntimeTypeRepresentation::ValueAggregate { fields }) =
                        runtime.type_representations().get(ty)
                    else {
                        return Err(Failure::InvalidRoot);
                    };
                    if fields.len() != values.len() {
                        return Err(Failure::InvalidRoot);
                    }
                    for (field, &node) in fields.iter().zip(values).rev() {
                        let member = layouts
                            .value_field(ty, field.field())
                            .ok_or(Failure::InvalidRoot)?;
                        pending.push((
                            member.ty(),
                            node,
                            offset
                                .checked_add(member.offset())
                                .ok_or(Failure::InvalidRoot)?,
                        ));
                    }
                }
                FrozenNode::Unit | FrozenNode::Repeated { .. } => return Err(Failure::InvalidRoot),
            }
        }
        let id = MachineDataId::new(self.values.len());
        self.values.push(MachineData::literal(
            bytes.into_boxed_slice(),
            layout.alignment(),
        ));
        Ok(id)
    }
    pub fn global(&self, id: nocter_model::ValueStatic) -> Option<MachineDataId> {
        self.statics.get(&id).copied()
    }
    pub fn get(&self, ty: TypeId, value: &FrozenData) -> Option<MachineDataId> {
        self.ids.get(&(ty, value.clone())).copied()
    }
    pub fn finish(self) -> MachineDataTable {
        MachineDataTable::from_values(self.values)
    }
}
