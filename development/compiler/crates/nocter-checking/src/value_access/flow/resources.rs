//! Finite logical ownership and sparse partial-payload cleanup for the closed data contracts.
//! These transitions retain obligations; they do not choose physical storage or an ABI.
use super::{IntegrityError, Location};
use crate::value_access::Query as TypedQuery;
use nocter_declarations::value_access::{PayloadSchema, Program};
use nocter_model::{AccessExpressionId, AccessLocalId, BuiltinType, FieldPath, TypeId};
use std::collections::{BTreeMap, BTreeSet};

mod assembly;
mod function_view;
mod payload;
mod storage;
#[cfg(test)]
mod tests;
use payload::OwnedValue;

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
impl Holder {
    #[must_use]
    pub fn place(self) -> Place {
        Place {
            holder: self,
            fields: FieldPath::default(),
        }
    }
}

/// A logical selection within one resource holder, not a physical address or an access grant.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Place {
    holder: Holder,
    fields: FieldPath,
}
impl Place {
    #[must_use]
    pub const fn holder(&self) -> Holder {
        self.holder
    }
    #[must_use]
    pub const fn fields(&self) -> &FieldPath {
        &self.fields
    }
    #[must_use]
    pub fn project(&self, field: nocter_model::AccessFieldId) -> Self {
        Self {
            holder: self.holder,
            fields: self.fields.project(field),
        }
    }
    #[must_use]
    pub fn project_path(&self, suffix: &FieldPath) -> Self {
        Self {
            holder: self.holder,
            fields: self.fields.concat(suffix),
        }
    }
    /// Relation for declaration-validated selections; not an availability or permission proof.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.holder == other.holder && self.fields.overlaps(&other.fields)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Disposal {
    /// The supplied environment's destructor, retained symbolically until witness specialization.
    FunctionEnvironment,
    Trivial,
    OwnedError,
    Record,
}
/// A schema-selected cleanup obligation, concrete or tied to a formal environment witness.
/// Independence is never inferred from Copy, ownership or the absence of known captures.
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
    /// The admitted builtin leaves and their closed record compositions have empty D/S here.
    #[must_use]
    pub const fn has_external_dependencies(self) -> bool {
        false
    }
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Step {
    /// Reclassify one selected named witness without constructing or duplicating its value.
    AdaptFunction {
        source: Place,
        destination: Holder,
        from: Payload,
        payload: Payload,
    },
    /// Transfer complete component obligations into one record; no second value is constructed.
    Assemble {
        destination: Holder,
        payload: Payload,
        fields: Box<[(nocter_model::AccessFieldId, Place)]>,
    },
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
        source: Place,
        destination: Place,
        payload: Payload,
    },
    Dispose {
        source: Place,
        payload: Payload,
    },
    /// End a partial record's containing slot after cleaning its surviving regions. This does
    /// not destroy its payload again or prescribe physical allocation/deallocation.
    ReleaseContainer {
        source: Place,
        payload: Payload,
    },
    Invoke {
        source: Place,
        call: AccessExpressionId,
        index: usize,
        payload: Payload,
    },
    /// Replacement through a caller-owned U input; the old payload is initialized by contract.
    ReplaceInput {
        parameter: nocter_model::AccessParameterId,
        fields: FieldPath,
        source: Place,
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
    owned: BTreeMap<Holder, OwnedValue>,
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
        self.before.initialized_root(holder)
    }
    #[must_use]
    pub fn initialized_after(&self, holder: Holder) -> bool {
        self.after.initialized_root(holder)
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
/// Project the declaration-owned schema into its logical cleanup obligation.
/// This grants no initialized owner, acceptance, layout or native execution evidence.
/// # Errors
/// Rejects a type outside the supplied private program's supported payload domain.
pub fn payload(program: &Program, data: TypeId) -> Result<Payload> {
    let disposal = match program.data_schema(data).ok_or(INVALID)? {
        PayloadSchema::Builtin(BuiltinType::Bool | BuiltinType::I32 | BuiltinType::Void) => {
            Disposal::Trivial
        }
        PayloadSchema::Builtin(BuiltinType::Error) => Disposal::OwnedError,
        PayloadSchema::Record(_) => Disposal::Record,
        PayloadSchema::Function(witness) => match witness.origin {
            nocter_model::ValueFunctionOrigin::Declaration(_) => Disposal::Trivial,
            nocter_model::ValueFunctionOrigin::Parameter(_) => Disposal::FunctionEnvironment,
        },
        PayloadSchema::Builtin(_) => return Err(INVALID),
    };
    Ok(Payload { data, disposal })
}
pub(super) fn expression_payload(typed: &TypedQuery, id: AccessExpressionId) -> Result<Payload> {
    let Some(ty) = typed
        .body()
        .expressions()
        .get(id)
        .ok_or(INVALID)?
        .ty
        .normal()
    else {
        return Err(INVALID);
    };
    payload(typed.program(), ty.data())
}
