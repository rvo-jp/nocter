//! Minimal executable MIR model. New execution forms extend this model before
//! their AST-driven lowering path is removed.

use super::ids::{BasicBlockId, DropPlanId, LoanId, LocalId, ProjectionPathId, ScopeId};
use super::locals::{Local, OwnershipKind, ScalarType, ValueRepresentation};
use crate::semantic::{BodyId, DefId, ExprId, TyId};
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Body {
    pub(crate) source_body: BodyId,
    pub(crate) source_span: ByteSpan,
    pub(crate) return_local: LocalId,
    pub(crate) return_mode: ReturnMode,
    pub(crate) root_scope: ScopeId,
    pub(crate) scopes: Vec<super::scopes::Scope>,
    pub(crate) locals: Vec<Local>,
    pub(crate) entry: BasicBlockId,
    pub(crate) blocks: Vec<BasicBlock>,
    pub(crate) loop_regions: Vec<LoopRegion>,
    pub(crate) loans: Vec<Loan>,
    pub(crate) projections: Vec<ProjectionPath>,
    pub(crate) drop_plans: Vec<super::drop_plans::DropPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectionPath {
    pub(crate) id: ProjectionPathId,
    pub(crate) base: LocalId,
    pub(crate) parent: Option<ProjectionPathId>,
    pub(crate) element: ProjectionElement,
    pub(crate) ty: TyId,
    pub(crate) representation: ValueRepresentation,
    pub(crate) ownership: OwnershipKind,
    pub(crate) drop_plan: Option<DropPlanId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProjectionElement {
    Field {
        offset: u32,
    },
    #[allow(
        dead_code,
        reason = "aggregate route construction follows the projected-place validation checkpoint"
    )]
    Index {
        index: Operand,
        length: u64,
        stride: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Loan {
    pub(crate) id: LoanId,
    pub(crate) source: Place,
    pub(crate) destination: LocalId,
    pub(crate) kind: BorrowKind,
    pub(crate) scope: ScopeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BorrowKind {
    Readonly,
    Readwrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReturnMode {
    Plain,
    Fallible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LoopRegion {
    pub(crate) header: BasicBlockId,
    pub(crate) condition: BasicBlockId,
    pub(crate) body: BasicBlockId,
    pub(crate) continue_target: BasicBlockId,
    pub(crate) exit: BasicBlockId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    Expression(ExprId),
    Desugared(ByteSpan),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BasicBlock {
    pub(crate) scope: ScopeId,
    pub(crate) statements: Vec<Statement>,
    pub(crate) terminator: Terminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Statement {
    Assign {
        destination: Place,
        value: Rvalue,
        origin: Origin,
    },
    BeginLoan {
        loan: LoanId,
        origin: Origin,
    },
    EndLoan {
        loan: LoanId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Place {
    pub(crate) local: LocalId,
    pub(crate) projection: Option<ProjectionPathId>,
}

impl Place {
    pub(crate) const fn local(local: LocalId) -> Self {
        Self {
            local,
            projection: None,
        }
    }

    pub(crate) const fn projected(local: LocalId, projection: ProjectionPathId) -> Self {
        Self {
            local,
            projection: Some(projection),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Rvalue {
    Use(Operand),
    Unary {
        operator: UnaryOperator,
        operand: Operand,
        ty: TyId,
    },
    Cast {
        operand: Operand,
        source_ty: TyId,
        source_scalar: ScalarType,
        target_ty: TyId,
        target_scalar: ScalarType,
    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnaryOperator {
    Negate,
    LogicalNot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Operand {
    Constant(Constant),
    Copy(Place),
    #[allow(
        dead_code,
        reason = "owned aggregate lowering will construct move operands next"
    )]
    Move(Place),
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
    pub(crate) representation: ValueRepresentation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CallContinuation {
    Return {
        destination: Place,
        target: BasicBlockId,
    },
    Outcome {
        destination: Place,
        success: BasicBlockId,
        failure: BasicBlockId,
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
    ShiftLeft,
    ShiftRight,
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
        origin: Origin,
        callee: DefId,
        arguments: Vec<CallArgument>,
        continuation: CallContinuation,
    },
    /// Destroy one initialized owned place, then continue along `target`.
    /// Cleanup is explicit control flow so every kind of scope exit shares
    /// the same path-sensitive ownership model.
    Drop {
        place: Place,
        plan: DropPlanId,
        target: BasicBlockId,
    },
    Trap,
    PropagateFailure,
    Return,
}
