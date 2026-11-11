//! Saved indices and prefix endpoints are scalar owners linked by sparse missing coverage.
use super::{
    Holder, INVALID, Index, OwnedValue, Path, Place, Program, Result, RetainedIndex, State, Step,
};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{BuiltinType, ValuePath, ValueProjection};
use std::collections::BTreeSet;

impl State {
    pub(super) fn validate_snapshots(&self, program: &Program, place: &Place) -> Result<()> {
        for index in place.fields().retained() {
            if self.code != Some(index.owner()) {
                return Err(INVALID);
            }
            let value = self
                .owned
                .get(&Holder::IndexSnapshot(index))
                .ok_or(INVALID)?;
            if !value.missing.is_empty()
                || program.data_kind(value.payload.data()) != Some(BuiltinType::Usize)
            {
                return Err(INVALID);
            }
        }
        Ok(())
    }
    /// Stage each evaluated index along one extraction path, from outer to inner. The caller
    /// commits all snapshots with its transfer; cancellation and rejection publish no change.
    pub(super) fn retain_source(
        &mut self,
        program: &Program,
        source: &Place,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        let fields = source.fields().fields();
        for (position, field) in fields.iter().enumerate() {
            if let ValueProjection::Array(Index::Retained(index)) = field {
                let container = Place {
                    holder: source.holder(),
                    fields: Path(ValuePath::new(fields[..position].iter().copied())),
                };
                self.retain_index(program, &container, *index, steps)?;
            }
        }
        Ok(())
    }
    fn retain_index(
        &mut self,
        program: &Program,
        container: &Place,
        index: RetainedIndex,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        if self.code != Some(index.owner()) || self.contains(Holder::IndexSnapshot(index)) {
            return Err(INVALID);
        }
        self.validate_snapshots(program, container)?;
        let owner = self.owned.get(&container.holder()).ok_or(INVALID)?;
        let data = owner.container(program, container.fields())?.data();
        let Some(PayloadSchema::Array(array)) = program.data_schema(data) else {
            return Err(INVALID);
        };
        if array.length() != index.bound() || array.is_empty() {
            return Err(INVALID);
        }
        let from = Holder::Expression(index.operand()).place();
        let payload = self.take(program, &from)?;
        if program.data_kind(payload.data()) != Some(BuiltinType::Usize) {
            return Err(INVALID);
        }
        let holder = Holder::IndexSnapshot(index);
        // Deliberately unframed: the array's missing path owns this lifetime, not the index's
        // evaluation frame. No second snapshot or ownership registry is introduced.
        self.owned.insert(holder, OwnedValue::new(payload));
        steps.push(Step::Move {
            source: from,
            destination: holder.place(),
            payload,
        });
        Ok(())
    }
    /// Called only after the operation that consumes the old path. Physical consumers still
    /// need its numeric backing during replacement or remainder cleanup.
    pub(super) fn retire_unused_backing(&mut self, steps: &mut Vec<Step>) -> Result<()> {
        let live: BTreeSet<_> = self
            .owned
            .values()
            .flat_map(|value| value.missing.iter().flat_map(super::Missing::backing))
            .collect();
        let retired: Vec<_> = self
            .owned
            .keys()
            .filter(|holder| holder.is_backing() && !live.contains(holder))
            .copied()
            .collect();
        for holder in retired {
            let value = self.remove_root(holder)?;
            steps.push(Step::Dispose {
                source: holder.place(),
                payload: value.payload,
            });
        }
        Ok(())
    }
}
