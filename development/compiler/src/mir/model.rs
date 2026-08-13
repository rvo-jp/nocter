//! Minimal executable MIR model. New execution forms extend this model before
//! their AST-driven lowering path is removed.

use super::ids::{BasicBlockId, LocalId};
use crate::resolve::LocalSymbolId;
use crate::semantic::{BodyId, DefId, ExprId, TyId};
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Body {
    pub(crate) source_body: BodyId,
    pub(crate) source_span: ByteSpan,
    pub(crate) return_local: LocalId,
    pub(crate) locals: Vec<Local>,
    pub(crate) entry: BasicBlockId,
    pub(crate) blocks: Vec<BasicBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Local {
    pub(crate) ty: TyId,
    pub(crate) scalar: ScalarType,
    pub(crate) source: LocalSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScalarType {
    I32,
    Usize,
    Bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalSource {
    Return,
    Parameter { symbol: LocalSymbolId, index: usize },
    Binding(LocalSymbolId),
    Temporary(ExprId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BasicBlock {
    pub(crate) statements: Vec<Statement>,
    pub(crate) terminator: Terminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Statement {
    Assign {
        destination: Place,
        value: Rvalue,
        source: ExprId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Place {
    pub(crate) local: LocalId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Rvalue {
    Use(Operand),
    Binary {
        operator: BinaryOperator,
        left: Operand,
        right: Operand,
        ty: TyId,
    },
    Compare {
        operator: ComparisonOperator,
        left: Operand,
        right: Operand,
        operand_ty: TyId,
        operand_scalar: ScalarType,
        result_ty: TyId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Operand {
    Constant(Constant),
    Copy(Place),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Constant {
    pub(crate) ty: TyId,
    pub(crate) scalar: ScalarType,
    pub(crate) value: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CallArgument {
    pub(crate) operand: Operand,
    pub(crate) ty: TyId,
    pub(crate) scalar: ScalarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CallContinuation {
    Return {
        destination: Place,
        target: BasicBlockId,
    },
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComparisonOperator {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Terminator {
    Goto {
        target: BasicBlockId,
    },
    Switch {
        condition: Operand,
        then_target: BasicBlockId,
        else_target: BasicBlockId,
    },
    Call {
        callee: DefId,
        arguments: Vec<CallArgument>,
        continuation: CallContinuation,
    },
    Return,
}
