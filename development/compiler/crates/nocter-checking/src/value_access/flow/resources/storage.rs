//! One ownership ledger: whole values, selected fields, and lexical/evaluation cleanup frames.
use super::{
    Disposal, Frame, Holder, INVALID, Missing, OwnedValue, Payload, Place, Program, Result, State,
    Step, TypedQuery, payload,
};
use nocter_model::{AccessExpressionId, TypeId, ValueAccessMode as Mode};

impl State {
    pub fn entry(typed: &TypedQuery) -> Result<Self> {
        let mut state = Self {
            code: Some(typed.code()),
            ..Self::default()
        };
        for (id, local) in typed.body().locals().iter() {
            if local.origin.is_input() && local.access().mode() == Mode::Own {
                state.owned.insert(
                    Holder::Local(id),
                    OwnedValue::new(payload(typed.program(), local.data)?),
                );
            }
        }
        Ok(state)
    }
    /// Presence of an owning container, including one with unavailable fields.
    pub fn contains(&self, holder: Holder) -> bool {
        self.owned.contains_key(&holder)
    }
    pub(in crate::value_access::flow) fn missing_point(&self, source: &Place) -> bool {
        self.owned.get(&source.holder()).is_some_and(|value| {
            value
                .missing
                .iter()
                .any(|hole| hole.value() == Some(source.fields()))
        })
    }
    /// A complete-or-point proof can project only the actual matching missing descendant.
    /// This inspects no index value and introduces no independent ownership state.
    pub(in crate::value_access::flow) fn unique_array_point(
        &self,
        array: &Place,
        suffix: &nocter_model::FieldPath,
    ) -> Result<Option<Place>> {
        let owner = self.owned.get(&array.holder()).ok_or(INVALID)?;
        let mut result = None;
        for missing in &owner.missing {
            if !missing.overlaps(array.fields()) {
                continue;
            }
            let Some(path) = missing.value() else {
                return Err(INVALID);
            };
            if !path.matches_array_element_shape(array.fields(), suffix) || result.is_some() {
                return Err(INVALID);
            }
            result = Some(array.holder().place().with_fields(path.clone()));
        }
        Ok(result)
    }
    pub fn initialized_root(&self, holder: Holder) -> bool {
        self.owned
            .get(&holder)
            .is_some_and(|value| value.missing.is_empty())
    }
    pub fn initialized(&self, program: &Program, place: &Place) -> Result<bool> {
        let Some(value) = self.owned.get(&place.holder()) else {
            return Ok(false);
        };
        self.validate_snapshots(program, place)?;
        value.selected(program, place.fields())?;
        Ok(value.complete(place.fields()))
    }
    pub fn begin(&mut self, id: AccessExpressionId) -> Result<()> {
        if self.frames.iter().any(|frame| frame.id == id) {
            return Err(INVALID);
        }
        self.frames.push(Frame {
            id,
            temporaries: Vec::new(),
        });
        Ok(())
    }
    /// Protect the actual backing of a forwarded alias before this result frame ends.
    /// An owner already held by an outer frame stays there; no physical move is introduced.
    pub(in crate::value_access::flow) fn protect_expression(
        &mut self,
        source: AccessExpressionId,
        result_frame: AccessExpressionId,
    ) -> Result<()> {
        let holder = Holder::Expression(source);
        if !self.owned.contains_key(&holder) {
            return Err(INVALID);
        }
        let current = self.frames.len().checked_sub(1).ok_or(INVALID)?;
        if self.frames[current].id != result_frame {
            return Err(INVALID);
        }
        let owning = self
            .frames
            .iter()
            .position(|frame| frame.temporaries.contains(&holder))
            .ok_or(INVALID)?;
        if owning == current {
            let outer = current.checked_sub(1).ok_or(INVALID)?;
            self.frames[current]
                .temporaries
                .retain(|&item| item != holder);
            self.frames[outer].temporaries.push(holder);
        }
        Ok(())
    }
    pub fn ending(&self, id: AccessExpressionId) -> Result<Vec<Holder>> {
        let frame = self.frames.last().ok_or(INVALID)?;
        if frame.id != id {
            return Err(INVALID);
        }
        Ok(frame.temporaries.iter().rev().copied().collect())
    }
    pub fn end(&mut self, id: AccessExpressionId) -> Result<()> {
        let frame = self.frames.last().ok_or(INVALID)?;
        if frame.id != id || !frame.temporaries.is_empty() {
            return Err(INVALID);
        }
        self.frames.pop();
        Ok(())
    }
    pub(super) fn insert(&mut self, holder: Holder, value: Payload, outer: bool) -> Result<()> {
        if holder.is_backing() || self.owned.contains_key(&holder) {
            return Err(INVALID);
        }
        let frame = if matches!(holder, Holder::Expression(_) | Holder::Temporary { .. }) {
            let index = self
                .frames
                .len()
                .checked_sub(if outer { 2 } else { 1 })
                .ok_or(INVALID)?;
            self.frames.get(index).ok_or(INVALID)?;
            Some(index)
        } else {
            None
        };
        self.owned.insert(holder, OwnedValue::new(value));
        if let Some(index) = frame {
            self.frames[index].temporaries.push(holder);
        }
        Ok(())
    }
    pub(super) fn remove_root(&mut self, holder: Holder) -> Result<OwnedValue> {
        let value = self.owned.remove(&holder).ok_or(INVALID)?;
        for frame in &mut self.frames {
            frame.temporaries.retain(|&item| item != holder);
        }
        Ok(value)
    }
    pub fn take(&mut self, program: &Program, source: &Place) -> Result<Payload> {
        if source.holder().is_backing() {
            return Err(INVALID);
        }
        self.validate_snapshots(program, source)?;
        let value = self.owned.get(&source.holder()).ok_or(INVALID)?;
        let selected = value.selected(program, source.fields())?;
        if !value.complete(source.fields()) {
            return Err(INVALID);
        }
        if source.fields().is_root() {
            self.remove_root(source.holder())?;
        } else {
            self.owned
                .get_mut(&source.holder())
                .ok_or(INVALID)?
                .remove(source.fields());
        }
        Ok(selected)
    }
    fn install(
        &mut self,
        program: &Program,
        destination: &Place,
        value: Payload,
        outer: bool,
    ) -> Result<()> {
        self.validate_snapshots(program, destination)?;
        if destination.fields().is_root() {
            return self.insert(destination.holder(), value, outer);
        }
        let owner = self.owned.get_mut(&destination.holder()).ok_or(INVALID)?;
        if owner.container(program, destination.fields())? != value
            || !owner
                .missing
                .contains(&Missing::Value(destination.fields().clone()))
        {
            return Err(INVALID);
        }
        owner
            .missing
            .remove(&Missing::Value(destination.fields().clone()));
        Ok(())
    }
    pub fn construct(
        &mut self,
        holder: Holder,
        value: Payload,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        self.insert(holder, value, false)?;
        steps.push(Step::Construct {
            destination: holder,
            payload: value,
        });
        Ok(())
    }
    pub fn drive_future(
        &mut self,
        program: &Program,
        future: AccessExpressionId,
        output: TypeId,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        let holder = Holder::Expression(future);
        let value = self.owned.get(&holder).ok_or(INVALID)?;
        let Disposal::UnstartedFuture(producer) = value.payload.disposal() else {
            return Err(INVALID);
        };
        if program.code_execution(producer)
            != Some(nocter_declarations::CallableExecution::Deferred { output })
        {
            return Err(INVALID);
        }
        let source = holder.place();
        let payload = self.take(program, &source)?;
        steps.push(Step::DriveFuture { source, payload });
        Ok(())
    }
    pub fn construct_activation(
        &mut self,
        program: &Program,
        destination: Holder,
        payload: Payload,
        sources: &[(Place, TypeId)],
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        let mut staged = self.clone();
        let mut inputs = Vec::with_capacity(sources.len());
        let mut changes = Vec::new();
        for (source, expected) in sources {
            staged.retain_source(program, source, &mut changes)?;
            let received = staged.take(program, source)?;
            if received.data() != *expected || received.disposal() != Disposal::Trivial {
                return Err(INVALID);
            }
            inputs.push((source.clone(), received));
        }
        staged.insert(destination, payload, false)?;
        changes.push(Step::ConstructActivation {
            destination,
            payload,
            inputs: inputs.into(),
        });
        staged.retire_unused_backing(&mut changes)?;
        *self = staged;
        steps.extend(changes);
        Ok(())
    }
    pub fn receive(
        &mut self,
        result: AccessExpressionId,
        value: Payload,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        self.insert(Holder::Expression(result), value, false)?;
        steps.push(Step::Receive {
            result,
            payload: value,
        });
        Ok(())
    }
    /// Transfer only a complete selected value into an unavailable destination.
    pub fn move_to(
        &mut self,
        program: &Program,
        source: &Place,
        destination: &Place,
        outer: bool,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        self.transfer(program, source, destination, outer, false, steps)
    }
    /// The replacement is already fully evaluated. Clean the old initialized remainder once.
    pub fn replace(
        &mut self,
        program: &Program,
        source: &Place,
        destination: &Place,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        self.transfer(program, source, destination, false, true, steps)
    }
    /// Replace an initialized scoped referent without changing its enclosing owner's state.
    /// The caller supplies the resolved alias-relative target and its checked data identity.
    pub fn replace_referent(
        &mut self,
        program: &Program,
        source: &Place,
        destination: &Place,
        data: TypeId,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        let mut staged = self.clone();
        let payload = staged.take(program, source)?;
        if payload.data() != data {
            return Err(INVALID);
        }
        *self = staged;
        steps.push(Step::ReplaceReferent {
            destination: destination.clone(),
            source: source.clone(),
            retired: payload,
            payload,
        });
        Ok(())
    }
    fn transfer(
        &mut self,
        program: &Program,
        source: &Place,
        destination: &Place,
        outer: bool,
        replace: bool,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        // Integrity rejection cannot publish half a transfer, cleanup, or frame registration.
        let mut staged = self.clone();
        let mut changes = Vec::new();
        staged.retain_source(program, source, &mut changes)?;
        let value = staged.take(program, source)?;
        if let Some(owner) = staged.owned.get(&destination.holder())
            && owner.container(program, destination.fields())? != value
        {
            return Err(INVALID);
        }
        if replace {
            staged.clear(program, destination, true, &mut changes)?;
        }
        staged.install(program, destination, value, outer)?;
        changes.push(Step::Move {
            source: source.clone(),
            destination: destination.clone(),
            payload: value,
        });
        staged.retire_unused_backing(&mut changes)?;
        *self = staged;
        steps.extend(changes);
        Ok(())
    }
    fn clear(
        &mut self,
        program: &Program,
        source: &Place,
        allow_missing: bool,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        if source.holder().is_backing() {
            return Err(INVALID);
        }
        self.validate_snapshots(program, source)?;
        let Some(value) = self.owned.get(&source.holder()) else {
            return if allow_missing && source.fields().is_root() {
                Ok(())
            } else {
                Err(INVALID)
            };
        };
        if !allow_missing
            && value
                .missing
                .contains(&Missing::Value(source.fields().clone()))
        {
            return Err(INVALID);
        }
        let cleanup = value.cleanup(program, source)?;
        if source.fields().is_root() {
            self.remove_root(source.holder())?;
        } else {
            self.owned
                .get_mut(&source.holder())
                .ok_or(INVALID)?
                .remove(source.fields());
        }
        steps.extend(cleanup);
        Ok(())
    }
    /// Cleanup disposes the remaining initialized regions of a present, possibly incomplete owner.
    pub fn dispose(
        &mut self,
        program: &Program,
        source: &Place,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        let mut staged = self.clone();
        let mut changes = Vec::new();
        staged.clear(program, source, false, &mut changes)?;
        staged.retire_unused_backing(&mut changes)?;
        *self = staged;
        steps.extend(changes);
        Ok(())
    }
    pub fn invoke(
        &mut self,
        program: &Program,
        source: &Place,
        call: AccessExpressionId,
        index: usize,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        let mut staged = self.clone();
        let mut changes = Vec::new();
        staged.retain_source(program, source, &mut changes)?;
        let value = staged.take(program, source)?;
        changes.push(Step::Invoke {
            source: source.clone(),
            call,
            index,
            payload: value,
        });
        staged.retire_unused_backing(&mut changes)?;
        *self = staged;
        steps.extend(changes);
        Ok(())
    }
    pub fn returned(&self, program: &Program, data: TypeId) -> bool {
        self.returned_with(data, |actual, expected| {
            program.matches_uninhabited_metadata(actual, expected)
        })
    }
    pub fn returned_with(&self, data: TypeId, compatible: impl Fn(TypeId, TypeId) -> bool) -> bool {
        self.frames.is_empty()
            && self.owned.len() == 1
            && self.owned.get(&Holder::Return).is_some_and(|value| {
                compatible(value.payload.data(), data) && value.missing.is_empty()
            })
    }
}
