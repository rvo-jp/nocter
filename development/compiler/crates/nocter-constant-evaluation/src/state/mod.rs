//! Closed stateful plans for value-access code. Source, dispatch and ownership selection are inputs.
mod execution;
mod memory;
mod validation;
pub use execution::{Error, Executor, Rule};
use nocter_model::{AccessCallableId, AccessFlowPointId, Arena};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    Unit,
    Bool,
    I32,
    Record(Box<[usize]>),
}
impl Type {
    /// Fixed-width primitive evaluator domain; absent types require an explicit extension.
    #[must_use]
    pub const fn builtin(data: nocter_model::BuiltinType) -> Option<Self> {
        match data {
            nocter_model::BuiltinType::Bool => Some(Self::Bool),
            nocter_model::BuiltinType::I32 => Some(Self::I32),
            nocter_model::BuiltinType::Void => Some(Self::Unit),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Unit,
    Bool(bool),
    I32(i32),
    Record(Box<[Arc<Self>]>),
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
pub enum Site {
    Action {
        code: AccessCallableId,
        point: AccessFlowPointId,
        action: usize,
    },
    Expression {
        code: AccessCallableId,
        expression: nocter_model::AccessExpressionId,
    },
}
impl Site {
    #[must_use]
    pub const fn code(self) -> AccessCallableId {
        match self {
            Self::Action { code, .. } | Self::Expression { code, .. } => code,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dependency {
    pub initializer: AccessCallableId,
    pub through: Box<[Site]>,
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
    Constant {
        destination: usize,
        initializer: AccessCallableId,
    },
    Alias {
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
        target: AccessCallableId,
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
    pub result: usize,
    pub initializer: bool,
    pub dependencies: Box<[Dependency]>,
    pub entry: AccessFlowPointId,
    pub points: Arena<AccessFlowPointId, Point>,
}
/// Closed immutable table. Public parts are validated before becoming executable plans.
#[derive(Clone, Debug)]
pub struct Program {
    types: Box<[Type]>,
    plans: BTreeMap<AccessCallableId, Plan>,
}
impl Program {
    /// # Errors
    /// Rejects invalid types, slots, control, call contracts or resource-plan references.
    pub fn new(
        types: impl Into<Box<[Type]>>,
        plans: BTreeMap<AccessCallableId, Plan>,
    ) -> Result<Self, Rule> {
        let program = Self {
            types: types.into(),
            plans,
        };
        validation::validate(&program)?;
        Ok(program)
    }
}

#[cfg(test)]
mod tests;
