use nocter_model::{
    AccessBlockId, AccessCallableId, AccessExpressionId, AccessLocalId, AccessLoopId,
    AccessParameterId, AccessPlaceId, AccessStatementId, Arena, FieldPath, TypeId, ValueAccess,
    ValueAccessMode, ValueBindingKind, ValueField, ValueInputOperation,
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
    /// A body-local address view of one immutable global place; never a callable input.
    Static(nocter_model::ValueStatic),
    Parameter(AccessParameterId),
    ClosureParameter,
    Capture,
    Binding,
}
impl LocalOrigin {
    #[must_use]
    pub const fn initialized_on_entry(self) -> bool {
        self.is_input() || matches!(self, Self::Static(_))
    }
    #[must_use]
    pub const fn is_input(self) -> bool {
        matches!(
            self,
            Self::Parameter(_) | Self::ClosureParameter | Self::Capture
        )
    }
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
    /// Intermediate value supply; result identity must close before Copy or move is selected.
    PendingValue,
    /// Closed structural supply in unreachable source; no value or ownership operation exists.
    Unproduced,
    TakeValue,
}

/// A computed record's selected component supplies a snapshot or a transferred value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionAction {
    CopyPayload,
    MovePayload,
}

/// Preparation of a selected direct call, before its atomic invocation commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArgumentAction {
    ReserveRead(AccessPlaceId),
    ReserveUpdate(AccessPlaceId),
    ReserveTransfer {
        place: AccessPlaceId,
        /// Selected adapter operations delayed until commit, in materialization order.
        adapters: Vec<AccessExpressionId>,
    },
    /// Lend read/update access to a produced value; its caller retains cleanup responsibility.
    AccessOwnedValue,
    /// Snapshot a selected place at this input preparation, without a retained access.
    CopyPlace(AccessPlaceId),
    /// Intermediate direct supply; cannot enter initialization or accepted products.
    PendingValue(AccessPlaceId),
    /// Closed structural supply that cannot reach argument preparation.
    Unproduced,
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
    UnitEqual,
    UnitNotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}

impl Binary {
    /// A selected unit comparison's result after both operands complete. Evaluation is retained.
    #[must_use]
    pub const fn fixed_boolean_result(self) -> Option<bool> {
        match self {
            Self::UnitEqual => Some(true),
            Self::UnitNotEqual => Some(false),
            _ => None,
        }
    }
}

/// One selected component, retained in authored evaluation order, not declaration order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldValue {
    pub field: ValueField,
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
    /// Named function value with its declaration-owned witness.
    Function(AccessCallableId),
    Closure {
        closure: nocter_model::ValueClosure,
        capture: AccessExpressionId,
    },
    AdaptFunction {
        source: AccessExpressionId,
    },
    Constant(nocter_model::FrozenData),
    Integer(i32),
    Boolean(bool),
    Unit,
    Aggregate {
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
    /// Result shared by normal completions; this grants no omission of evaluation or its effects.
    pub(super) normal_boolean: Option<bool>,
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

/// A selected body input operation and its exact local slot, independent of physical ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BodyInput {
    pub local: AccessLocalId,
    pub operation: ValueInputOperation,
}
/// Environment and explicit inputs are distinct channels with frozen source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodyInputs {
    pub(super) environment: Option<BodyInput>,
    pub(super) explicit: Box<[BodyInput]>,
}
impl BodyInputs {
    #[must_use]
    pub const fn environment(&self) -> Option<BodyInput> {
        self.environment
    }
    #[must_use]
    pub fn explicit(&self) -> &[BodyInput] {
        &self.explicit
    }
    pub fn ordered(&self) -> impl Iterator<Item = BodyInput> + '_ {
        self.environment
            .into_iter()
            .chain(self.explicit.iter().copied())
    }
}

/// Type/call evidence only. This cannot be passed to native/checked-program consumers: initialization,
/// access liveness, escaping roots, cleanup schedules and numeric safety proofs remain unvalidated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedBody {
    pub(super) inputs: BodyInputs,
    pub(super) root: AccessBlockId,
    pub(super) locals: Arena<AccessLocalId, Local>,
    pub(super) expressions: Arena<AccessExpressionId, Expression>,
    pub(super) places: Arena<AccessPlaceId, Place>,
    pub(super) statements: Arena<AccessStatementId, Statement>,
    pub(super) blocks: Arena<AccessBlockId, Block>,
    pub(super) loops: Arena<AccessLoopId, ()>,
}

impl TypedBody {
    /// Body-local address views, independent of callable input channels and source spelling.
    pub fn static_views(
        &self,
    ) -> impl Iterator<Item = (AccessLocalId, nocter_model::ValueStatic)> + '_ {
        self.locals
            .iter()
            .filter_map(|(local, declaration)| match declaration.origin {
                LocalOrigin::Static(id) => Some((local, id)),
                _ => None,
            })
    }
    #[must_use]
    pub const fn inputs(&self) -> &BodyInputs {
        &self.inputs
    }
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
