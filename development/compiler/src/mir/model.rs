//! Checked executable MIR shared by buildability and machine-IR projection.

use super::ids::{
    AllocationOverrideId, BasicBlockId, DropPlanId, LoanId, LocalId, ProjectionPathId, RegionId,
    ScopeId,
};
use super::locals::{Local, OwnershipKind, ScalarType, ValueRepresentation, ViewKind};
use crate::semantic::{BodyId, DefId, ExprId, TyId};
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Body {
    pub(crate) source_body: BodyId,
    pub(crate) source_span: ByteSpan,
    pub(crate) return_local: LocalId,
    pub(crate) return_mode: ReturnMode,
    pub(crate) outcome_contract: Option<OutcomeContract>,
    pub(crate) root_scope: ScopeId,
    pub(crate) scopes: Vec<super::scopes::Scope>,
    pub(crate) locals: Vec<Local>,
    pub(crate) entry: BasicBlockId,
    pub(crate) blocks: Vec<BasicBlock>,
    pub(crate) loop_regions: Vec<LoopRegion>,
    pub(crate) allocation_regions: Vec<AllocationRegion>,
    pub(crate) allocation_overrides: Vec<AllocationContextOverride>,
    pub(crate) loans: Vec<Loan>,
    pub(crate) projections: Vec<ProjectionPath>,
    pub(crate) drop_plans: Vec<super::drop_plans::DropPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutcomeContract {
    pub(crate) layers: Vec<crate::outcomes::OutcomeLayer>,
    pub(crate) payload_ty: TyId,
    pub(crate) payload_representation: ValueRepresentation,
    pub(crate) payload_borrow_readwrite: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AllocationRegion {
    pub(crate) id: RegionId,
    pub(crate) scope: ScopeId,
    pub(crate) allocator: LocalId,
    pub(crate) parent: Place,
    pub(crate) state: LocalId,
    pub(crate) parent_state: LocalId,
    pub(crate) parent_kind: LocalId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AllocationContextOverride {
    pub(crate) id: AllocationOverrideId,
    pub(crate) scope: ScopeId,
    pub(crate) allocator: Place,
    pub(crate) parent_state: LocalId,
    pub(crate) parent_kind: LocalId,
    pub(crate) selected_state: LocalId,
    pub(crate) selected_kind: LocalId,
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
    Index {
        index: Operand,
        length: u64,
        stride: u32,
    },
    ViewIndex {
        index: Operand,
    },
    Dereference,
    ErrorField(crate::builtin_types::BuiltinErrorField),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Loan {
    pub(crate) id: LoanId,
    pub(crate) source: Place,
    pub(crate) destination: LocalId,
    pub(crate) kind: BorrowKind,
    pub(crate) scope: ScopeId,
    pub(crate) lifetime: LoanLifetime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BorrowKind {
    Readonly,
    Readwrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoanLifetime {
    Scope,
    Call,
    /// The loan is transferred through a function return. Cleanup must retain
    /// it until the return operand has been materialized by the backend.
    Return,
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
    BeginAggregate {
        destination: Place,
        origin: Origin,
    },
    FinishAggregate {
        destination: Place,
        fields: Vec<ProjectionPathId>,
        origin: Origin,
    },
    Assign {
        destination: Place,
        value: Rvalue,
        origin: Origin,
    },
    Intrinsic {
        intrinsic: crate::intrinsics::IntrinsicId,
        arguments: Vec<CallArgument>,
        type_arguments: Vec<TyId>,
        origin: Origin,
    },
    DropAtPointer {
        pointer: Operand,
        offset: Operand,
        ty: TyId,
        plan: DropPlanId,
        origin: Origin,
    },
    BeginLoan {
        loan: LoanId,
        origin: Origin,
    },
    EndLoan {
        loan: LoanId,
    },
    EnterRegion {
        region: RegionId,
        origin: Origin,
    },
    ExitRegion {
        region: RegionId,
    },
    EnterAllocationContext {
        override_: AllocationOverrideId,
        origin: Origin,
    },
    ExitAllocationContext {
        override_: AllocationOverrideId,
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
    /// Constructs stored outcome storage from a success value. The backend
    /// derives tag and payload offsets from the destination type; MIR retains
    /// only the semantic value being wrapped.
    OutcomeSuccess {
        value: CallArgument,
    },
    /// Constructs the first optional failure reachable through successful
    /// outer layers of the destination outcome.
    OutcomeNone,
    /// Constructs the first fallible failure reachable through successful
    /// outer layers of the destination outcome.
    OutcomeFailure {
        code: Operand,
        message: Operand,
    },
    Error {
        code: Operand,
        message: Operand,
    },
    Variant {
        variant: DefId,
        leaves: Vec<AggregateLeaf>,
    },
    Discriminant {
        source: Operand,
        enum_ty: TyId,
        result_ty: TyId,
    },
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
    ViewCast {
        source: Operand,
        source_ty: TyId,
        target_ty: TyId,
        kind: super::ViewKind,
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
    ViewCompare {
        operator: ComparisonOperator,
        left: Operand,
        right: Operand,
        kind: ViewKind,
        result_ty: TyId,
    },
    Intrinsic {
        intrinsic: crate::intrinsics::IntrinsicId,
        arguments: Vec<CallArgument>,
        type_arguments: Vec<TyId>,
        result_ty: TyId,
        representation: super::ValueRepresentation,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AggregateLeaf {
    pub(crate) path: Vec<AggregateElement>,
    pub(crate) ty: TyId,
    pub(crate) representation: ValueRepresentation,
    pub(crate) operand: Operand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AggregateElement {
    Field(usize),
    Index(usize),
    VariantPayload(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnaryOperator {
    Negate,
    LogicalNot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Operand {
    Constant(Constant),
    StaticStr { ty: TyId, bytes: Vec<u8> },
    Copy(Place),
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
    Continue {
        target: BasicBlockId,
    },
    Return {
        destination: Place,
        target: BasicBlockId,
    },
    Outcome {
        destination: Place,
        success: BasicBlockId,
        failure: BasicBlockId,
        failure_payload: Option<LocalId>,
    },
    OutcomeEffect {
        success: BasicBlockId,
        failure: BasicBlockId,
        failure_payload: Option<LocalId>,
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
        /// Structured continuation chosen before cleanup elaboration.
        /// Loop conditions and fully terminating arms have no join.
        join_target: Option<BasicBlockId>,
    },
    Call {
        origin: Origin,
        callee: super::calls::CallInstance,
        arguments: Vec<CallArgument>,
        continuation: CallContinuation,
    },
    /// Branch on the outer layer of a stored outcome and project its success
    /// payload into `destination`. Recursive tag and payload offsets remain a
    /// backend concern derived from the source local's checked `TyId`.
    InspectOutcome {
        origin: Origin,
        source: Operand,
        layer: crate::outcomes::OutcomeLayer,
        destination: Place,
        success: BasicBlockId,
        failure: BasicBlockId,
        failure_payload: Option<LocalId>,
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
    /// Return one already stored optional/fallible value without unpacking it.
    /// The operand retains copy/move ownership semantics; recursive ABI
    /// storage remains a backend projection of its checked type.
    ReturnOutcome {
        source: Operand,
    },
    /// Return a recoverable failure value. Code and message remain logical
    /// string-view operands until machine-storage projection.
    ReturnFailure {
        code: Operand,
        message: Operand,
    },
    /// Return successful payload storage for an optional/fallible result.
    ReturnOutcomeSuccess {
        source: Operand,
    },
    /// Return absence through the optional layer in the result contract.
    ReturnOptionalNone,
    /// Return an ordinary semantic value. ABI return storage is written only
    /// when this terminal edge is projected to machine IR.
    ReturnValue {
        source: Operand,
    },
    Return,
}
