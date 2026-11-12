//! Closed stateful plans for value-access code. Source, dispatch and ownership selection are inputs.
mod budget;
mod execution;
mod memory;
mod validation;
mod value;
pub use budget::{Budget, Limits};
pub use execution::{Error, Executor, Rule};
use nocter_model::{AccessCodeId, AccessFlowPointId, Arena, ValueCode};
use std::{collections::BTreeMap, sync::Arc};
pub use value::{Data, Value};
mod projection;
pub use projection::{Missing, Projection, ProjectionPath};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    /// A declared data identity without a payload representation in this table. No Value validates
    /// against it; an empty array can still retain it as its uninstantiated element identity.
    Unavailable,
    Unit,
    /// A concrete code witness with no runtime environment; Unit is its payload.
    NamedFunction(AccessCodeId),
    /// One owned environment in internal component zero; code is static metadata, not payload.
    CapturedFunction {
        code: nocter_model::ValueClosure,
        environment: usize,
    },
    Bool,
    Integer(nocter_model::BuiltinType),
    Optional(usize),
    Record(Box<[usize]>),
    Array {
        element: usize,
        length: u64,
    },
}
impl Type {
    fn component(&self, index: usize) -> Option<usize> {
        match self {
            Self::Record(fields) => fields.get(index).copied(),
            Self::Optional(payload) if index == 0 => Some(*payload),
            Self::CapturedFunction { environment, .. } if index == 0 => Some(*environment),
            Self::Array { element, length } if u64::try_from(index).ok()? < *length => {
                Some(*element)
            }
            _ => None,
        }
    }
    fn component_count(&self) -> Option<u64> {
        match self {
            Self::Record(fields) => u64::try_from(fields.len()).ok(),
            Self::CapturedFunction { .. } => Some(1),
            Self::Array { length, .. } => Some(*length),
            _ => None,
        }
    }
    #[must_use]
    pub const fn scalar(&self) -> Option<crate::ConstantScalarType> {
        match self {
            Self::Bool => Some(crate::ConstantScalarType::Bool),
            Self::Integer(builtin) => Some(crate::ConstantScalarType::Integer(*builtin)),
            _ => None,
        }
    }
    /// Fixed-width primitive evaluator domain; absent types require an explicit extension.
    #[must_use]
    pub const fn builtin(data: nocter_model::BuiltinType) -> Option<Self> {
        match data {
            nocter_model::BuiltinType::Bool => Some(Self::Bool),
            _ if matches!(
                nocter_model::BuiltinNumericType::for_builtin(data),
                Some(nocter_model::BuiltinNumericType::Integer { .. })
            ) =>
            {
                Some(Self::Integer(data))
            }
            nocter_model::BuiltinType::Void => Some(Self::Unit),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Storage {
    Owned,
    Alias,
    Auxiliary,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Slot {
    pub data: usize,
    pub storage: Storage,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Place {
    pub slot: usize,
    pub fields: ProjectionPath,
}
impl Place {
    #[must_use]
    pub fn root(slot: usize) -> Self {
        Self {
            slot,
            fields: ProjectionPath::default(),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Site {
    pub code: ValueCode,
    pub point: AccessFlowPointId,
    pub action: usize,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    Construct(usize),
    Move {
        source: Place,
        destination: Place,
    },
    /// Transfer a frozen whole-element address under an explicitly owned array. The selected
    /// alias grants no ownership itself; execution verifies its exact containing owner.
    MoveArrayElement {
        array: Place,
        selected: Place,
        destination: usize,
    },
    /// Consume one witness payload into a checked invocation view of the same code.
    AdaptFunction {
        source: Place,
        destination: Place,
    },
    Dispose(Place),
    /// End the initialized remainder of an array in this closed effect-free payload domain.
    /// Its exact missing coverage is supplied by the action's ordinary ownership guard.
    DisposeArrayRemainder(Place),
    /// Retire an intact half-open element interval without cardinality-sized state.
    DisposeArrayElements {
        source: Place,
        range: nocter_model::ArrayRange<usize>,
    },
    /// Consolidate the last moved element with its contiguous existing missing prefix.
    /// The endpoint is an ordinary owned usize snapshot; this neither moves nor destroys data.
    ConsolidateArrayPrefix {
        source: Place,
        endpoint: usize,
    },
    Release(Place),
    Assemble {
        destination: usize,
        fields: Box<[(usize, Place)]>,
    },
    Replace {
        source: Place,
        destination: Place,
    },
    Invoke {
        source: Place,
        /// Taking input positions and their exact payload places at this owner handoff.
        inputs: Box<[(usize, Place)]>,
    },
}
/// Exact sparse owning state before the action, including retained index snapshots.
/// Aliases and ordinary auxiliary operand snapshots are not owners.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Alternative {
    pub owned: BTreeMap<usize, Box<[Missing]>>,
    pub before: Box<[Effect]>,
    pub after: Box<[Effect]>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operation {
    None,
    PrimaryCondition {
        source: Place,
        destination: usize,
    },
    RequirePrimary {
        source: Place,
        view: Option<usize>,
    },
    /// A source-proven terminal bounds failure, after all preceding operand effects.
    BoundsTrap,
    Literal {
        destination: usize,
        value: Value,
    },
    Alias {
        destination: usize,
        source: Place,
    },
    /// Capture one checked array element address, never the mutable index slot itself.
    Index {
        destination: usize,
        base: Place,
        index: Place,
    },
    /// Snapshot a selected Copy component; the producer has already established Copy evidence.
    IndexCopy {
        destination: usize,
        base: Place,
        index: Place,
    },
    /// Compact snapshots into a fixed array with length greater than one. The result schema
    /// owns the count and element identity; the producer owns source-context Copy evidence.
    Repeat {
        destination: usize,
        source: Place,
    },
    Copy {
        destination: Place,
        source: Place,
    },
    Unary {
        destination: usize,
        operation: crate::CompileTimeUnaryOperation,
        operand: Place,
    },
    Binary {
        destination: Place,
        operation: crate::CompileTimeBinaryOperation,
        left: Place,
        right: Place,
    },
    Compare {
        destination: usize,
        operation: crate::CompileTimeComparisonOperation,
        left: Place,
        right: Place,
    },
    Call {
        destination: usize,
        target: ValueCode,
        inputs: Box<[Input]>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Input {
    Read(Place),
    Edit(Place),
    /// Exact data identity. Its physical source is frozen by the selected Invoke effect,
    /// after any input wrappers have been materialized and before owner reclamation.
    Take(usize),
}
impl Input {
    #[must_use]
    pub const fn operation(&self) -> nocter_model::ValueInputOperation {
        match self {
            Self::Read(_) => nocter_model::ValueInputOperation::Read,
            Self::Edit(_) => nocter_model::ValueInputOperation::Edit,
            Self::Take(_) => nocter_model::ValueInputOperation::Take,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Action {
    pub operation: Operation,
    pub alternatives: Box<[Alternative]>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Terminator {
    Jump(AccessFlowPointId),
    Branch {
        condition: Place,
        yes: AccessFlowPointId,
        no: AccessFlowPointId,
    },
    Return(Place),
    Unreachable,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Point {
    pub actions: Box<[Action]>,
    pub terminator: Terminator,
}
#[derive(Clone, Debug)]
pub struct Plan {
    pub slots: Arc<[Slot]>,
    pub inputs: Box<[(usize, nocter_model::ValueInputOperation)]>,
    /// Stable auxiliary storage for a taking view of this captured read/edit body.
    /// It is adapter storage, not another body-owned input or invocation frame.
    pub environment_backing: Option<usize>,
    pub statics: Box<[(usize, nocter_model::ValueStatic)]>,
    pub result: usize,
    pub initializer: bool,
    pub entry: AccessFlowPointId,
    pub points: Arena<AccessFlowPointId, Point>,
}
/// Established immutable global data and its evaluator-domain shape.
#[derive(Clone, Debug)]
pub struct Static {
    pub data: usize,
    pub value: Value,
}
/// Closed immutable table of plans and established immutable global data.
#[derive(Clone, Debug)]
pub struct Program {
    types: Box<[Type]>,
    statics: BTreeMap<nocter_model::ValueStatic, Static>,
    plans: BTreeMap<ValueCode, Plan>,
}
impl Program {
    /// # Errors
    /// Rejects invalid types, slots, control, call contracts or resource-plan references.
    pub fn new(
        types: impl Into<Box<[Type]>>,
        plans: BTreeMap<ValueCode, Plan>,
        statics: BTreeMap<nocter_model::ValueStatic, Static>,
    ) -> Result<Self, Rule> {
        let program = Self {
            types: types.into(),
            statics,
            plans,
        };
        validation::validate(&program)?;
        Ok(program)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod storage_tests;
