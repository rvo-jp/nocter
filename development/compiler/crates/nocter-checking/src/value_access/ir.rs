use nocter_model::{
    AccessBlockId, AccessCodeId, AccessExpressionId, AccessLocalId, AccessLoopId,
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
    /// A bounded call-input view into an owned expression payload.
    TemporaryPlace,
    DelegateUpdate,
    CopyPayload,
    /// Intermediate value supply; result identity must close before Copy or move is selected.
    PendingValue,
    /// Closed structural supply in unreachable source; no value or ownership operation exists.
    Unproduced,
    TakeValue,
}

/// A selected component either identifies existing storage or supplies a computed value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionAction {
    Access,
    /// Project an existing place whose address was evaluated earlier in this body.
    PreparedAccess,
    /// Supply one leaf of a carrier already acquired by a take pattern.
    PreparedValue {
        copy: bool,
    },
    CopyPayload,
    MovePayload,
    /// Resolve the remaining temporary-field supply after its result data is established.
    PendingPayload,
}
impl SelectionAction {
    #[must_use]
    pub const fn is_place(self) -> bool {
        matches!(self, Self::Access | Self::PreparedAccess)
    }
}

/// One existing logical referent. Evaluated accesses retain a selected address, not a recipe to
/// reload the user's index. This identity belongs to the owning typed body.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Referent {
    Place(AccessPlaceId),
    Evaluated(AccessExpressionId),
}
impl Referent {
    #[must_use]
    pub const fn place(self) -> Option<AccessPlaceId> {
        match self {
            Self::Place(place) => Some(place),
            Self::Evaluated(_) => None,
        }
    }
    pub(super) fn value_type<'a>(
        self,
        place: impl Fn(AccessPlaceId) -> Option<&'a Place>,
        expression: impl Fn(AccessExpressionId) -> Option<&'a Expression>,
    ) -> Option<ValueType> {
        match self {
            Self::Place(id) => Some(value_type(place(id)?.data, ValueAccessMode::Read)),
            Self::Evaluated(id) => {
                let expression = expression(id)?;
                (expression.referent(id) == Some(self)).then_some(expression.ty)
            }
        }
    }
    /// Resolve only a unique existing base authority; never guess among alternate places.
    pub(super) fn static_base<'a>(
        self,
        expression: impl Fn(AccessExpressionId) -> Option<&'a Expression>,
        block: impl Fn(AccessBlockId) -> Option<&'a Block>,
    ) -> Option<AccessPlaceId> {
        let bases = self.static_bases(expression, block)?;
        (bases.len() == 1).then(|| bases[0])
    }
    pub(super) fn static_bases<'a>(
        self,
        expression: impl Fn(AccessExpressionId) -> Option<&'a Expression>,
        block: impl Fn(AccessBlockId) -> Option<&'a Block>,
    ) -> Option<Vec<AccessPlaceId>> {
        self.static_roots(expression, block)?
            .into_iter()
            .map(|root| match root {
                super::paths::ProjectionRoot::Place(place) => Some(place),
                super::paths::ProjectionRoot::Temporary(_) => None,
            })
            .collect()
    }
    pub(super) fn static_roots<'a>(
        self,
        expression: impl Fn(AccessExpressionId) -> Option<&'a Expression>,
        block: impl Fn(AccessBlockId) -> Option<&'a Block>,
    ) -> Option<Vec<super::paths::ProjectionRoot>> {
        let mut roots: Vec<_> = self
            .projections(
                expression,
                block,
                super::paths::ProjectionRoots::All,
                |_| (),
                |_, _| Some(()),
            )?
            .into_iter()
            .map(|(root, _)| root)
            .collect();
        roots.sort_unstable();
        roots.dedup();
        Some(roots)
    }
}
impl From<AccessPlaceId> for Referent {
    fn from(place: AccessPlaceId) -> Self {
        Self::Place(place)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexAction {
    Access,
    CopyPayload,
}

/// Preparation of a selected direct call, before its atomic invocation commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArgumentAction {
    ReserveRead(Referent),
    ReserveUpdate(Referent),
    ReserveTransfer {
        place: Referent,
        /// Selected callable views and implicit data wrappers delayed until commit, in materialization order.
        adapters: Vec<AccessExpressionId>,
    },
    /// Lend read/update access to a produced value; its caller retains cleanup responsibility.
    AccessOwnedValue,
    /// Lend the same read channel after the preceding call committed in this evaluation.
    ReuseRead {
        call: AccessExpressionId,
        index: usize,
    },
    /// Snapshot a selected place at this input preparation, without a retained access.
    CopyPlace(Referent),
    /// Intermediate direct supply; cannot enter initialization or accepted products.
    PendingValue(Referent),
    /// Closed structural supply that cannot reach argument preparation.
    Unproduced,
    TakeValue,
    Interrupted,
}

/// A selected formal-to-actual edge for later substitution, never runtime-erased dispatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WitnessBinding {
    pub parameter: nocter_model::ValueParameter,
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

/// Call slots are indexed by the selected contract. Preparation and cleanup may traverse them
/// in a different order, while keeping the same slot identities in every phase.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallArguments {
    formal: Box<[Argument]>,
    reverse_preparation: bool,
    retain_reads: bool,
}
impl CallArguments {
    pub(super) fn in_order(formal: impl Into<Box<[Argument]>>) -> Self {
        Self {
            formal: formal.into(),
            reverse_preparation: false,
            retain_reads: false,
        }
    }
    pub(super) fn reverse_pair([left, right]: [Argument; 2]) -> Self {
        Self {
            formal: Box::new([right, left]),
            reverse_preparation: true,
            retain_reads: false,
        }
    }
    pub(super) fn retain_read_pair(&mut self) -> bool {
        if self.formal.len() != 2
            || self
                .formal
                .iter()
                .any(|arg| arg.operation != ValueInputOperation::Read)
        {
            return false;
        }
        self.retain_reads = true;
        true
    }
    pub(super) const fn retains_reads(&self) -> bool {
        self.retain_reads
    }
    pub(super) fn preparation_order(
        &self,
    ) -> impl DoubleEndedIterator<Item = (usize, &Argument)> + ExactSizeIterator + '_ {
        let len = self.formal.len();
        (0..len).map(move |position| {
            let index = if self.reverse_preparation {
                len - 1 - position
            } else {
                position
            };
            (index, &self.formal[index])
        })
    }
    pub(super) fn prepared_prefix(
        &self,
        count: usize,
    ) -> Option<impl DoubleEndedIterator<Item = (usize, &Argument)> + '_> {
        (count <= self.formal.len()).then(|| self.preparation_order().take(count))
    }
}
impl AsRef<[Argument]> for CallArguments {
    fn as_ref(&self) -> &[Argument] {
        &self.formal
    }
}
impl std::ops::Deref for CallArguments {
    type Target = [Argument];
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}
impl<'a> IntoIterator for &'a CallArguments {
    type Item = &'a Argument;
    type IntoIter = std::slice::Iter<'a, Argument>;
    fn into_iter(self) -> Self::IntoIter {
        self.formal.iter()
    }
}
impl<'a> IntoIterator for &'a mut CallArguments {
    type Item = &'a mut Argument;
    type IntoIter = std::slice::IterMut<'a, Argument>;
    fn into_iter(self) -> Self::IntoIter {
        self.formal.iter_mut()
    }
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
    pub(super) const fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::Equal
                | Self::NotEqual
                | Self::Less
                | Self::LessEqual
                | Self::Greater
                | Self::GreaterEqual
        )
    }

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

/// Identify the frozen callable-value prefix separately from explicit source inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallKind {
    Direct,
    Value,
}
impl CallKind {
    /// Decode the frozen callable view from the explicit call role, never from argument count.
    #[must_use]
    pub fn invocation(self, arguments: &[Argument]) -> Option<nocter_model::ValueInvocation> {
        use nocter_model::ValueInvocation;
        match self {
            Self::Direct => Some(ValueInvocation::Read),
            Self::Value => Some(match self.environment_argument(arguments)?.operation {
                ValueInputOperation::Read => ValueInvocation::Read,
                ValueInputOperation::Edit => ValueInvocation::Edit,
                ValueInputOperation::Take => ValueInvocation::Take,
            }),
        }
    }
    #[must_use]
    pub fn environment_argument(self, arguments: &[Argument]) -> Option<&Argument> {
        match self {
            Self::Direct => None,
            Self::Value => arguments.first(),
        }
    }
    /// Ordinary inputs exclude a value call's prepared callable, even when it has zero-byte data.
    #[must_use]
    pub fn explicit_arguments(self, arguments: &[Argument]) -> Option<&[Argument]> {
        match self {
            Self::Direct => Some(arguments),
            Self::Value => arguments.get(1..),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpressionKind {
    /// Named function value with its declaration-owned witness.
    Function(AccessCodeId),
    Closure {
        closure: nocter_model::ValueClosure,
        capture: AccessExpressionId,
    },
    AdaptFunction {
        source: AccessExpressionId,
    },
    /// Form a source once, then forward the existing place selected by its projection/control.
    /// Normal alternatives retain their own source authority, address and lifetime.
    Scoped {
        source: AccessExpressionId,
        projected: AccessExpressionId,
    },
    /// Check and retain the active tag of an already formed carrier place.
    GuardView {
        source: Referent,
        primary: bool,
    },
    /// An ordinary lexical value scope, including compiler-generated outcome expansion.
    Block {
        block: AccessBlockId,
    },
    /// Observe a stored carrier tag without transferring or retaining its payload.
    PrimaryCondition {
        source: Referent,
    },
    /// Require the primary branch before forwarding this owned carrier to payload selection.
    RequirePrimary {
        source: AccessExpressionId,
    },
    Constant(nocter_model::FrozenData),
    Integer(i128),
    Boolean(bool),
    Unit,
    Aggregate {
        enum_case: Option<u64>,
        fields: Box<[FieldValue]>,
    },
    Select {
        source: AccessExpressionId,
        fields: FieldPath,
        action: SelectionAction,
    },
    Repeat {
        seed: AccessExpressionId,
    },
    Index {
        base: AccessExpressionId,
        index: AccessExpressionId,
        action: IndexAction,
        bounds: nocter_model::IndexBoundsCheck,
    },
    Place(AccessPlaceId),
    Move(Referent),
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
        arguments: CallArguments,
    },
    /// Consume a future and deliver its output on completion.
    Await {
        future: AccessExpressionId,
    },
    If {
        condition: AccessExpressionId,
        then_block: AccessBlockId,
        else_block: AccessBlockId,
    },
    EnumMatch {
        source: AccessExpressionId,
        operation: ValueInputOperation,
        cases: Box<[(u64, AccessBlockId)]>,
        fallback: Option<AccessBlockId>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Expression {
    pub ty: ValueType,
    pub kind: ExpressionKind,
    /// Result shared by normal completions; this grants no omission of evaluation or its effects.
    pub(super) normal_value: Option<NormalValue>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NormalValue {
    Boolean(bool),
    Integer(i128),
}
impl Expression {
    pub(super) fn normal_boolean(&self) -> Option<bool> {
        match self.normal_value {
            Some(NormalValue::Boolean(value)) => Some(value),
            _ => None,
        }
    }
    pub(super) fn normal_integer(&self) -> Option<i128> {
        match self.normal_value {
            Some(NormalValue::Integer(value)) => Some(value),
            _ => None,
        }
    }
    #[must_use]
    pub fn referent(&self, id: AccessExpressionId) -> Option<Referent> {
        match self.kind {
            ExpressionKind::Place(place) => Some(Referent::Place(place)),
            ExpressionKind::Block { .. } | ExpressionKind::If { .. }
                if self
                    .ty
                    .normal()
                    .is_some_and(|ty| ty.mode() != ValueAccessMode::Own) =>
            {
                Some(Referent::Evaluated(id))
            }
            ExpressionKind::Boundary {
                action: BoundaryAction::TemporaryPlace,
                ..
            }
            | ExpressionKind::Scoped { .. }
            | ExpressionKind::GuardView { .. }
            | ExpressionKind::Index {
                action: IndexAction::Access,
                ..
            }
            | ExpressionKind::Select {
                action: SelectionAction::Access | SelectionAction::PreparedAccess,
                ..
            } => Some(Referent::Evaluated(id)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Statement {
    Bind {
        local: AccessLocalId,
        value: AccessExpressionId,
    },
    Discard(AccessExpressionId),
    /// Retire an already acquired carrier without evaluating the scrutinee again.
    DiscardPrepared(AccessExpressionId),
    Evaluate(AccessExpressionId),
    /// Evaluate an existing place once and keep its address for the following projections.
    PreparePlace(AccessExpressionId),
    Assign {
        target: Referent,
        value: AccessExpressionId,
    },
    CompoundAssign {
        target: Referent,
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
    /// Existing storage authority for a referent; no concrete index or alias-disjointness proof.
    /// The returned place belongs to this body's arena and may select a component of its local.
    #[must_use]
    pub fn referent_base(&self, referent: Referent) -> Option<AccessPlaceId> {
        referent.static_base(|id| self.expressions.get(id), |id| self.blocks.get(id))
    }
    #[must_use]
    pub fn referent_data(&self, referent: Referent) -> Option<TypeId> {
        referent
            .value_type(|id| self.places.get(id), |id| self.expressions.get(id))?
            .access()
            .map(ValueAccess::data)
    }

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
    pub(super) fn call_arguments(&self, call: AccessExpressionId) -> Option<&CallArguments> {
        match &self.expressions.get(call)?.kind {
            ExpressionKind::Call { arguments, .. } => Some(arguments),
            _ => None,
        }
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

#[derive(Clone, Copy)]
pub(super) struct Projection {
    pub mode: ValueAccessMode,
    pub temporary: bool,
}
impl Projection {
    pub const fn place(mode: ValueAccessMode) -> Self {
        Self {
            mode,
            temporary: false,
        }
    }
    pub const fn input(mode: ValueAccessMode) -> Self {
        Self {
            mode,
            temporary: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Boundary {
    Binding(ValueAccess),
    /// Demand an existing place without activating a read/update grant during preparation.
    Place(ValueAccess),
    /// A direct scoped input can also lend a fresh owned value within its evaluation frame.
    Input(ValueAccess),
    Value(TypeId),
    Result(TypeId),
}
impl Boundary {
    pub(super) const fn ty(self) -> ValueAccess {
        match self {
            Self::Binding(ty) | Self::Place(ty) | Self::Input(ty) => ty,
            Self::Value(data) | Self::Result(data) => ValueAccess::new(data, ValueAccessMode::Own),
        }
    }
}

pub(super) const fn value_type(data: TypeId, mode: ValueAccessMode) -> ValueType {
    ValueType::known(ValueAccess::new(data, mode))
}
