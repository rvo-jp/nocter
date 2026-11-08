use nocter_model::{
    AccessBlockId, AccessCallableId, AccessExpressionId, AccessFieldId, AccessLocalId,
    AccessLoopId, AccessParameterId, AccessPlaceId, AccessStatementId, Arena, FieldPath, TypeId,
    ValueAccess, ValueAccessMode, ValueBindingKind, ValueInputOperation,
};

/// Known result data/access and normal completion are independent typing facts.
/// An interrupted operation can retain its declared result shape without producing a value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueType {
    access: Option<ValueAccess>,
    may_complete: bool,
}
impl ValueType {
    /// No known result shape and no normal continuation; never a storable data type.
    pub const NEVER: Self = Self {
        access: None,
        may_complete: false,
    };
    #[must_use]
    pub const fn known(access: ValueAccess) -> Self {
        Self {
            access: Some(access),
            may_complete: true,
        }
    }
    /// The statically known result operation, including on an interrupted evaluation.
    #[must_use]
    pub const fn access(self) -> Option<ValueAccess> {
        self.access
    }
    /// Typing has not excluded a normal continuation. This is not a termination guarantee.
    #[must_use]
    pub const fn may_complete(self) -> bool {
        self.may_complete
    }
    /// Known result access when a normal continuation remains possible.
    #[must_use]
    pub const fn normal(self) -> Option<ValueAccess> {
        if self.may_complete { self.access } else { None }
    }
    /// Restrict continuation without erasing a known result shape.
    #[must_use]
    pub const fn with_continuation(self, may_complete: bool) -> Self {
        Self {
            access: self.access,
            may_complete: self.may_complete && may_complete,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalOrigin {
    Parameter(AccessParameterId),
    Binding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Local {
    pub data: TypeId,
    pub kind: ValueBindingKind,
    pub origin: LocalOrigin,
}

impl Local {
    #[must_use]
    pub const fn access(&self) -> ValueAccess {
        ValueAccess::new(self.data, self.kind.access_mode())
    }
}

/// One resolved logical selection. Field types are selected once by the declaration owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Place {
    pub local: AccessLocalId,
    pub fields: FieldPath,
    pub data: TypeId,
}

/// Selected construction/delegation at a value or scoped-alias boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoundaryAction {
    ReadPlace,
    DelegateUpdate,
    CopyPayload,
    TakeValue,
}

/// A computed record's selected component supplies a snapshot or a transferred value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionAction {
    CopyPayload,
    MovePayload,
}

/// Preparation of a selected direct call, before its atomic invocation commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgumentAction {
    ReserveRead(AccessPlaceId),
    ReserveUpdate(AccessPlaceId),
    ReserveTransfer(AccessPlaceId),
    /// Read a fresh owned temporary without Copy; its caller retains cleanup responsibility.
    ReadOwnedValue,
    CopyPayload,
    TakeValue,
    Interrupted,
}

/// A selected formal-to-actual edge for later substitution, never runtime-erased dispatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WitnessBinding {
    pub parameter: AccessParameterId,
    pub witness: nocter_declarations::value_access::FunctionWitness,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Argument {
    pub operation: ValueInputOperation,
    /// Present for a callable formal with known source data; a terminating unknown source binds none.
    pub binding: Option<WitnessBinding>,
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

/// One selected component, retained in authored evaluation order, not declaration order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldValue {
    pub field: AccessFieldId,
    pub value: AccessExpressionId,
}

/// Whether the frozen argument bundle starts with a runtime environment input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallKind {
    Direct,
    Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpressionKind {
    Function(AccessCallableId),
    AdaptFunction {
        source: AccessExpressionId,
    },
    Integer(i32),
    Boolean(bool),
    Unit,
    Record {
        fields: Box<[FieldValue]>,
    },
    Select {
        source: AccessExpressionId,
        fields: FieldPath,
        action: SelectionAction,
    },
    Place(AccessPlaceId),
    Move(AccessPlaceId),
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
        kind: CallKind,
        target: nocter_model::ValueFunctionOrigin,
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
        target: AccessPlaceId,
        value: AccessExpressionId,
    },
    CompoundAssign {
        target: AccessPlaceId,
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
    pub(super) places: Arena<AccessPlaceId, Place>,
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
    pub const fn places(&self) -> &Arena<AccessPlaceId, Place> {
        &self.places
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
    Binding(ValueAccess),
    Value(TypeId),
    Result(TypeId),
}
impl Boundary {
    pub(super) const fn ty(self) -> ValueAccess {
        match self {
            Self::Binding(ty) => ty,
            Self::Value(data) | Self::Result(data) => ValueAccess::new(data, ValueAccessMode::Own),
        }
    }
}

pub(super) const fn value_type(data: TypeId, mode: ValueAccessMode) -> ValueType {
    ValueType::known(ValueAccess::new(data, mode))
}
