//! Serialize established literal DAGs with the common stored layout and byte-order authority.
use super::Failure;
use crate::identity::MachineId;
use crate::{
    MachineData, MachineDataId, MachineDataRelocation, MachineDataTable, MachineLayoutPlan,
};
use nocter_model::FrozenData;
use nocter_runtime_contract::RuntimeDataId;
use nocter_runtime_contract::RuntimeEnvironment;
use std::collections::{BTreeMap, HashMap};
mod encoding;

pub(super) struct Data {
    values: Vec<MachineData>,
    statics: BTreeMap<nocter_model::ValueStatic, (RuntimeDataId, MachineDataId)>,
    ids: HashMap<(RuntimeDataId, FrozenData), MachineDataId>,
    texts: BTreeMap<Box<str>, MachineDataId>,
    remaining: usize,
}
impl Default for Data {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            statics: BTreeMap::new(),
            ids: HashMap::new(),
            texts: BTreeMap::new(),
            remaining: 16 * 1024 * 1024,
        }
    }
}
impl Data {
    pub fn build(
        program: &nocter_mir::value_access::Program,
        layouts: &MachineLayoutPlan,
    ) -> Result<Self, Failure> {
        let mut data = Self::default();
        for (&id, source) in program.statics() {
            let object = data.encode(program.runtime(), layouts, source.data, &source.value)?;
            data.statics.insert(id, (source.data, object));
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
        ty: RuntimeDataId,
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
        ty: RuntimeDataId,
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
        let mut relocations = Vec::new();
        encoding::write(runtime, layouts, ty, value, &mut bytes, |offset, text| {
            let target = self.text(ty, text)?;
            relocations.push(MachineDataRelocation::new(offset, target));
            Ok(())
        })?;
        relocations.sort_unstable_by_key(|relocation| relocation.offset());
        let id = MachineDataId::new(self.values.len());
        self.values.push(MachineData::relocated(
            bytes.into_boxed_slice(),
            layout.alignment(),
            relocations.into_boxed_slice(),
        ));
        Ok(id)
    }
    fn text(&mut self, owner: RuntimeDataId, text: &str) -> Result<MachineDataId, Failure> {
        if let Some(&id) = self.texts.get(text) {
            return Ok(id);
        }
        let size = text.len().max(1);
        self.remaining = self
            .remaining
            .checked_sub(size)
            .ok_or(Failure::DataLimit(owner))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size)
            .map_err(|_| Failure::DataLimit(owner))?;
        bytes.extend_from_slice(text.as_bytes());
        if bytes.is_empty() {
            bytes.push(0);
        }
        let id = MachineDataId::new(self.values.len());
        self.values
            .push(MachineData::literal(bytes.into_boxed_slice(), 1));
        self.texts.insert(text.into(), id);
        Ok(id)
    }
    pub fn global(&self, id: nocter_model::ValueStatic) -> Option<(RuntimeDataId, MachineDataId)> {
        self.statics.get(&id).copied()
    }
    pub fn get(&self, ty: RuntimeDataId, value: &FrozenData) -> Option<MachineDataId> {
        self.ids.get(&(ty, value.clone())).copied()
    }
    pub fn finish(self) -> MachineDataTable {
        MachineDataTable::from_values(self.values)
    }
}
