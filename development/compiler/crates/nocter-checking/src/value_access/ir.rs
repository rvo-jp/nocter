use nocter_model::{
    AccessBlockId, AccessCallableId, AccessExpressionId, AccessLocalId, AccessLoopId,
    AccessParameterId, AccessStatementId, Arena, TypeId, ValueAccessMode, ValueAccessType,
};

/// A type-only value proof; never is control interruption, not a storable data shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueType {
    Never,
    Value(ValueAccessType),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalOrigin {
    Parameter(AccessParameterId),
    Binding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Local {
    pub ty: ValueAccessType,
    pub origin: LocalOrigin,
}

/// Construction/materialization at a storage or return boundary. None silently moves a place.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryAction {
    ReadPlace,
    DelegateUpdate,
    ReadAccess,
    CopyPayload,
    TakeValue,
}

/// Preparation of a selected direct call, before its atomic invocation commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgumentAction {
    ReserveRead(AccessLocalId),
    ReserveUpdate(AccessLocalId),
    ReserveTransfer(AccessLocalId),
    ReadValue,
    /// Read a fresh owned temporary without Copy; its caller retains cleanup responsibility.
    ReadOwnedValue,
    ReadAccessValue,
    UpdateValue,
    CopyPayload,
    TakeValue,
    Interrupted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Argument {
    pub value: AccessExpressionId,
    pub action: ArgumentAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unary {
    Not,
    CheckedNegate,
}

/// Integer operations retain their checked semantics; this is not permission to remove traps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Binary {
    CheckedAdd,
    CheckedSubtract,
    CheckedMultiply,
    CheckedDivide,
    CheckedRemainder,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpressionKind {
    Integer(i32),
    Boolean(bool),
    Unit,
    Place(AccessLocalId),
    Update(AccessLocalId),
    Transfer(AccessLocalId),
    Boundary {
        source: AccessExpressionId,
        action: BoundaryAction,
    },
    Unary {
        operation: Unary,
        operand: AccessExpressionId,
    },
    Binary {
        operation: Binary,
        left: AccessExpressionId,
        right: AccessExpressionId,
    },
    Call {
        target: AccessCallableId,
        arguments: Box<[Argument]>,
    },
    If {
        condition: AccessExpressionId,
        then_block: AccessBlockId,
        else_block: AccessBlockId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Expression {
    pub ty: ValueType,
    pub kind: ExpressionKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Statement {
    Bind {
        local: AccessLocalId,
        value: AccessExpressionId,
    },
    Discard(AccessExpressionId),
    Evaluate(AccessExpressionId),
    Assign {
        target: AccessLocalId,
        value: AccessExpressionId,
    },
    CompoundAssign {
        target: AccessLocalId,
        operation: Binary,
        value: AccessExpressionId,
    },
    Drop(AccessLocalId),
    Return(AccessExpressionId),
    Break(AccessLoopId),
    Continue(AccessLoopId),
    While {
        loop_id: AccessLoopId,
        condition: AccessExpressionId,
        body: AccessBlockId,
    },
    Loop {
        loop_id: AccessLoopId,
        body: AccessBlockId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub statements: Box<[AccessStatementId]>,
    pub result: Option<AccessExpressionId>,
    pub ty: ValueType,
}

/// Type/call evidence only. This cannot be passed to native/checked-program consumers: initialization,
/// access liveness, escaping roots, cleanup schedules and numeric safety proofs remain unvalidated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedBody {
    pub(super) root: AccessBlockId,
    pub(super) locals: Arena<AccessLocalId, Local>,
    pub(super) expressions: Arena<AccessExpressionId, Expression>,
    pub(super) statements: Arena<AccessStatementId, Statement>,
    pub(super) blocks: Arena<AccessBlockId, Block>,
    pub(super) loops: Arena<AccessLoopId, ()>,
}

impl TypedBody {
    #[must_use]
    pub const fn root(&self) -> AccessBlockId {
        self.root
    }
    #[must_use]
    pub const fn locals(&self) -> &Arena<AccessLocalId, Local> {
        &self.locals
    }
    #[must_use]
    pub const fn expressions(&self) -> &Arena<AccessExpressionId, Expression> {
        &self.expressions
    }
    #[must_use]
    pub const fn statements(&self) -> &Arena<AccessStatementId, Statement> {
        &self.statements
    }
    #[must_use]
    pub const fn blocks(&self) -> &Arena<AccessBlockId, Block> {
        &self.blocks
    }
    #[must_use]
    pub const fn loops(&self) -> &Arena<AccessLoopId, ()> {
        &self.loops
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Boundary {
    Binding(ValueAccessType),
    Return(ValueAccessType),
}
impl Boundary {
    pub(super) const fn ty(self) -> ValueAccessType {
        match self {
            Self::Binding(ty) | Self::Return(ty) => ty,
        }
    }
}

pub(super) const fn value_type(data: TypeId, mode: ValueAccessMode) -> ValueType {
    ValueType::Value(ValueAccessType::new(data, mode))
}
