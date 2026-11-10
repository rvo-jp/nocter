//! Closed stateful plans for value-access code. Source, dispatch and ownership selection are inputs.
mod budget;
mod execution;
mod memory;
mod validation;
mod value;
pub use budget::{Budget, Limits};
pub use execution::{Error, Executor, Rule};
use nocter_model::{AccessCodeId, AccessFlowPointId, Arena, ValueCodeRoot};
use std::{collections::BTreeMap, sync::Arc};
pub use value::{Data, Value};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    /// A declared data identity without a payload representation in this table. No Value validates
    /// against it; an empty array can still retain it as its uninstantiated element identity.
    Unavailable,
    Unit,
    Bool,
    I32,
    Usize,
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
            Self::Array { element, length } if u64::try_from(index).ok()? < *length => {
                Some(*element)
            }
            _ => None,
        }
    }
    fn component_count(&self) -> Option<u64> {
        match self {
            Self::Record(fields) => u64::try_from(fields.len()).ok(),
            Self::Array { length, .. } => Some(*length),
            _ => None,
        }
    }
    #[must_use]
    pub const fn scalar(&self) -> Option<crate::ConstantScalarType> {
        match self {
            Self::Bool => Some(crate::ConstantScalarType::Bool),
            Self::I32 => Some(crate::ConstantScalarType::Integer(
                nocter_model::BuiltinType::I32,
            )),
            Self::Usize => Some(crate::ConstantScalarType::Integer(
                nocter_model::BuiltinType::Usize,
            )),
            _ => None,
        }
    }
    /// Fixed-width primitive evaluator domain; absent types require an explicit extension.
    #[must_use]
    pub const fn builtin(data: nocter_model::BuiltinType) -> Option<Self> {
        match data {
            nocter_model::BuiltinType::Bool => Some(Self::Bool),
            nocter_model::BuiltinType::I32 => Some(Self::I32),
            nocter_model::BuiltinType::Usize => Some(Self::Usize),
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
    pub fields: Box<[usize]>,
}
impl Place {
    #[must_use]
    pub fn root(slot: usize) -> Self {
        Self {
            slot,
            fields: Box::new([]),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Site {
    pub code: ValueCodeRoot,
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
    Dispose(Place),
    Release(Place),
    Assemble {
        destination: usize,
        fields: Box<[(usize, Place)]>,
    },
    Replace {
        source: Place,
        destination: Place,
    },
    Invoke(Place),
}
/// Exact sparse owning state before the action; aliases and scalar snapshots are not owners.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Alternative {
    pub owned: BTreeMap<usize, Box<[Box<[usize]>]>>,
    pub before: Box<[Effect]>,
    pub after: Box<[Effect]>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operation {
    None,
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
        target: AccessCodeId,
        inputs: Box<[Input]>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Input {
    pub operation: nocter_model::ValueInputOperation,
    pub place: Place,
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
    plans: BTreeMap<ValueCodeRoot, Plan>,
}
impl Program {
    /// # Errors
    /// Rejects invalid types, slots, control, call contracts or resource-plan references.
    pub fn new(
        types: impl Into<Box<[Type]>>,
        plans: BTreeMap<ValueCodeRoot, Plan>,
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
