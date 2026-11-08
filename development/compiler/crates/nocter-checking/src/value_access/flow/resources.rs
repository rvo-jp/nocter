//! Finite logical ownership and ordered cleanup for the closed first-fragment data contracts.
//! These transitions retain obligations; they do not choose physical storage or an ABI.
use super::{IntegrityError, Location};
use crate::value_access::{Query as TypedQuery, ValueType};
use nocter_model::{
    AccessExpressionId, AccessLocalId, BuiltinType, TypeId, ValueAccessMode as Mode,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Holder {
    Local(AccessLocalId),
    Expression(AccessExpressionId),
    Prepared {
        call: AccessExpressionId,
        index: usize,
    },
    Temporary {
        call: AccessExpressionId,
        index: usize,
    },
    Return,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Disposal {
    Trivial,
    OwnedError,
}
/// An exhaustive concrete data contract. Independence is never inferred from Copy or ownership.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Payload {
    data: TypeId,
    disposal: Disposal,
}
impl Payload {
    #[must_use]
    pub const fn data(self) -> TypeId {
        self.data
    }
    #[must_use]
    pub const fn disposal(self) -> Disposal {
        self.disposal
    }
    /// Only the admitted scalar/unit and owned-error snapshot contracts have empty D/S here.
    #[must_use]
    pub const fn has_external_dependencies(self) -> bool {
        false
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Step {
    Construct {
        destination: Holder,
        payload: Payload,
    },
    /// Receive the callee's existing result obligation, without acquiring another resource.
    Receive {
        call: AccessExpressionId,
        destination: Holder,
        payload: Payload,
    },
    Move {
        source: Holder,
        destination: Holder,
        payload: Payload,
    },
    Dispose {
        source: Holder,
        payload: Payload,
    },
    Invoke {
        source: Holder,
        call: AccessExpressionId,
        index: usize,
        payload: Payload,
    },
    /// Replacement through a caller-owned U input; the old payload is initialized by contract.
    ReplaceInput {
        parameter: nocter_model::AccessParameterId,
        source: Holder,
        payload: Payload,
    },
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Frame {
    id: AccessExpressionId,
    temporaries: Vec<Holder>,
}
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct State {
    owned: BTreeMap<Holder, Payload>,
    frames: Vec<Frame>,
}
/// One reachable ownership alternative. Private fields prevent unvalidated assembly.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Transition {
    before: State,
    steps: Box<[Step]>,
    after: State,
}
impl Transition {
    #[must_use]
    pub const fn steps(&self) -> &[Step] {
        &self.steps
    }
    #[must_use]
    pub fn initialized_before(&self, holder: Holder) -> bool {
        self.before.owned.contains_key(&holder)
    }
    #[must_use]
    pub fn initialized_after(&self, holder: Holder) -> bool {
        self.after.owned.contains_key(&holder)
    }
}
/// Source-neutral transition alternatives keyed by the original operation graph.
#[derive(Clone, Debug, Default)]
pub struct Report {
    transitions: BTreeMap<Location, BTreeSet<Transition>>,
}
impl Report {
    pub fn at(&self, at: Location) -> impl Iterator<Item = &Transition> {
        self.transitions.get(&at).into_iter().flatten()
    }
    pub(super) fn record(&mut self, at: Location, before: &State, steps: &[Step], after: &State) {
        self.transitions.entry(at).or_default().insert(Transition {
            before: before.clone(),
            steps: steps.into(),
            after: after.clone(),
        });
    }
}
const INVALID: IntegrityError = IntegrityError::InconsistentTypedBody;
type Result<T> = std::result::Result<T, IntegrityError>;
pub(super) fn payload(typed: &TypedQuery, data: TypeId) -> Result<Payload> {
    let disposal = match typed.program().data_kind(data).ok_or(INVALID)? {
        BuiltinType::Bool | BuiltinType::I32 | BuiltinType::Void => Disposal::Trivial,
        BuiltinType::Error => Disposal::OwnedError,
        _ => return Err(INVALID),
    };
    Ok(Payload { data, disposal })
}
pub(super) fn expression_payload(typed: &TypedQuery, id: AccessExpressionId) -> Result<Payload> {
    let ValueType::Value(ty) = typed.body().expressions().get(id).ok_or(INVALID)?.ty else {
        return Err(INVALID);
    };
    payload(typed, ty.data())
}
impl State {
    pub fn entry(typed: &TypedQuery) -> Result<Self> {
        let mut state = Self::default();
        for (id, local) in typed.body().locals().iter() {
            if matches!(local.origin, crate::value_access::LocalOrigin::Parameter(_))
                && local.ty.mode() == Mode::Own
            {
                state
                    .owned
                    .insert(Holder::Local(id), payload(typed, local.ty.data())?);
            }
        }
        Ok(state)
    }
    pub fn contains(&self, holder: Holder) -> bool {
        self.owned.contains_key(&holder)
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
    pub fn ending(&self, id: AccessExpressionId) -> Result<Vec<Holder>> {
        let frame = self.frames.last().ok_or(INVALID)?;
        if frame.id != id {
            return Err(INVALID);
        }
        Ok(frame.temporaries.iter().rev().copied().collect())
    }
    pub fn end(&mut self, id: AccessExpressionId) -> Result<()> {
        let frame = self.frames.pop().ok_or(INVALID)?;
        if frame.id != id || !frame.temporaries.is_empty() {
            return Err(INVALID);
        }
        Ok(())
    }
    fn insert(&mut self, holder: Holder, value: Payload, outer: bool) -> Result<()> {
        if self.owned.insert(holder, value).is_some() {
            return Err(INVALID);
        }
        if matches!(holder, Holder::Expression(_) | Holder::Temporary { .. }) {
            let index = self
                .frames
                .len()
                .checked_sub(if outer { 2 } else { 1 })
                .ok_or(INVALID)?;
            self.frames
                .get_mut(index)
                .ok_or(INVALID)?
                .temporaries
                .push(holder);
        }
        Ok(())
    }
    pub fn take(&mut self, holder: Holder) -> Result<Payload> {
        let value = self.owned.remove(&holder).ok_or(INVALID)?;
        for frame in &mut self.frames {
            frame.temporaries.retain(|&item| item != holder);
        }
        Ok(value)
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
    pub fn receive(
        &mut self,
        call: AccessExpressionId,
        value: Payload,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        let destination = Holder::Expression(call);
        self.insert(destination, value, false)?;
        steps.push(Step::Receive {
            call,
            destination,
            payload: value,
        });
        Ok(())
    }
    pub fn move_to(
        &mut self,
        source: Holder,
        destination: Holder,
        outer: bool,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        let value = self.take(source)?;
        self.insert(destination, value, outer)?;
        steps.push(Step::Move {
            source,
            destination,
            payload: value,
        });
        Ok(())
    }
    pub fn dispose(&mut self, source: Holder, steps: &mut Vec<Step>) -> Result<()> {
        let value = self.take(source)?;
        steps.push(Step::Dispose {
            source,
            payload: value,
        });
        Ok(())
    }
    pub fn invoke(
        &mut self,
        source: Holder,
        call: AccessExpressionId,
        index: usize,
        steps: &mut Vec<Step>,
    ) -> Result<()> {
        let value = self.take(source)?;
        steps.push(Step::Invoke {
            source,
            call,
            index,
            payload: value,
        });
        Ok(())
    }
    pub fn returned(&self, owns: bool) -> bool {
        self.frames.is_empty()
            && self.owned.len() == usize::from(owns)
            && (!owns || self.owned.contains_key(&Holder::Return))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_access::flow::tests::queries;

    #[test]
    fn ownership_mutants_cannot_duplicate_destroy_twice_or_escape_without_a_return_holder() {
        let queries = queries("func run(value: ^error): ^error { ^value }\n");
        let query = queries[0].typed();
        let local = query.body().locals().iter().next().unwrap().0;
        let mut state = State::entry(query).unwrap();
        let owned = payload(query, query.body().locals().get(local).unwrap().ty.data()).unwrap();
        assert!(!state.returned(true));
        assert!(
            state
                .construct(Holder::Local(local), owned, &mut Vec::new())
                .is_err()
        );
        let mut state = State::entry(query).unwrap();
        state
            .dispose(Holder::Local(local), &mut Vec::new())
            .unwrap();
        assert!(
            state
                .dispose(Holder::Local(local), &mut Vec::new())
                .is_err()
        );
        assert!(!state.returned(true));
        let mut state = State::entry(query).unwrap();
        state
            .move_to(Holder::Local(local), Holder::Return, false, &mut Vec::new())
            .unwrap();
        assert!(state.returned(true));
        assert!(
            state
                .move_to(Holder::Local(local), Holder::Return, false, &mut Vec::new())
                .is_err()
        );
    }
}
