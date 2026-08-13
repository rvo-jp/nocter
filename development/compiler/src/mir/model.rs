//! Minimal executable MIR model. New execution forms extend this model before
//! their AST-driven lowering path is removed.

use super::ids::{BasicBlockId, LocalId};
use crate::semantic::{BodyId, ExprId, TyId};
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
    pub(crate) source: Option<ByteSpan>,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Operand {
    Constant(Constant),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Constant {
    pub(crate) ty: TyId,
    pub(crate) value: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Terminator {
    Goto { target: BasicBlockId },
    Return,
}

impl Body {
    pub(crate) fn return_type(&self) -> Option<TyId> {
        self.locals
            .get(self.return_local.index())
            .map(|local| local.ty)
    }
}

impl Rvalue {
    pub(crate) const fn ty(&self) -> TyId {
        match self {
            Self::Use(Operand::Constant(constant)) => constant.ty,
        }
    }
}
