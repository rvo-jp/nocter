use nocter_model::{
    Arena, BodyNodeId, DropId, FieldId, LocalBindingId, ParameterId, PlaceId, TypeId, VariantId,
};

use super::PlaceRoot;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanupCondition {
    Always,
    IfInitialized,
}

/// One statically selected aggregate step and the checked result type reached after projecting it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanupProjection {
    Field { field: FieldId, ty: TypeId },
    TupleElement { index: usize, ty: TypeId },
}

impl CleanupProjection {
    #[must_use]
    pub const fn named_field(field: FieldId, ty: TypeId) -> Self {
        Self::Field { field, ty }
    }

    #[must_use]
    pub const fn field(self) -> Option<FieldId> {
        match self {
            Self::Field { field, .. } => Some(field),
            Self::TupleElement { .. } => None,
        }
    }

    #[must_use]
    pub const fn tuple_element(index: usize, ty: TypeId) -> Self {
        Self::TupleElement { index, ty }
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        match self {
            Self::Field { ty, .. } | Self::TupleElement { ty, .. } => ty,
        }
    }
}

/// One owned storage path whose remaining live value is destroyed on an outgoing edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupPath {
    root: PlaceRoot,
    root_ty: TypeId,
    projections: Box<[CleanupProjection]>,
}

impl CleanupPath {
    pub(crate) fn new(
        root: PlaceRoot,
        root_ty: TypeId,
        projections: impl Into<Box<[CleanupProjection]>>,
    ) -> Self {
        Self {
            root,
            root_ty,
            projections: projections.into(),
        }
    }

    #[must_use]
    pub const fn root(&self) -> PlaceRoot {
        self.root
    }

    #[must_use]
    pub const fn projections(&self) -> &[CleanupProjection] {
        &self.projections
    }

    #[must_use]
    pub fn ty(&self) -> TypeId {
        match self.projections.last() {
            Some(projection) => projection.ty(),
            None => self.root_ty,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CleanupTarget {
    Path(CleanupPath),
    Place {
        place: PlaceId,
        ty: TypeId,
    },
    Value {
        node: BodyNodeId,
        ty: TypeId,
    },
    /// The still-initialized payload fields left after an owned enum pattern transfers bindings.
    EnumResidual {
        subject: BodyNodeId,
        variant: VariantId,
        payload: Box<[ParameterId]>,
        ty: TypeId,
    },
    /// Releases one compiler-created lexical child allocation context after its body values.
    /// `parent` identifies the retained allocator/context loan that ends at this action.
    Region {
        binding: LocalBindingId,
        parent: BodyNodeId,
    },
}

/// Allocation-effect dependencies selected together with one checked cleanup action.
///
/// Ownership is the sole authority that decides whether and what a cleanup destroys. Effect
/// analysis consumes this frozen contract and never reconstructs a value's recursive shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CleanupEffect {
    drops: Box<[DropId]>,
    unknown_destruction: bool,
}

impl CleanupEffect {
    pub(crate) fn new(drops: impl Into<Box<[DropId]>>, unknown_destruction: bool) -> Self {
        Self {
            drops: drops.into(),
            unknown_destruction,
        }
    }

    pub(crate) fn allocation_free() -> Self {
        Self::new([], false)
    }

    #[must_use]
    pub(crate) const fn drops(&self) -> &[DropId] {
        &self.drops
    }

    #[must_use]
    pub(crate) const fn has_unknown_destruction(&self) -> bool {
        self.unknown_destruction
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupAction {
    target: CleanupTarget,
    condition: CleanupCondition,
    effect: CleanupEffect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanupTiming {
    /// Runs after a destructuring binding has transferred every named leaf and before the binding
    /// statement completes. Discarded leaves are destroyed through the ordinary cleanup contract.
    DuringBinding,
    /// Runs after one complete statement has committed its result or destination.
    AtStatementEnd,
    /// Runs after a boolean control header and before selecting or entering its branch/body.
    AtControlHeaderEnd,
    /// Runs after child evaluation and before the node performs its outgoing control transfer.
    BeforeTransfer,
    /// Runs after right-hand-side and target evaluation, immediately before assignment storage.
    BeforeStore,
    /// Runs only on the absence/failure branch selected by postfix `?`, before it returns.
    OnOutcomePropagation,
}

/// One ordered cleanup event attached to an exact checked operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupSchedule {
    timing: CleanupTiming,
    actions: Box<[CleanupAction]>,
}

impl CleanupSchedule {
    pub(crate) fn new(timing: CleanupTiming, actions: impl Into<Box<[CleanupAction]>>) -> Self {
        Self {
            timing,
            actions: actions.into(),
        }
    }

    #[must_use]
    pub const fn timing(&self) -> CleanupTiming {
        self.timing
    }

    #[must_use]
    pub const fn actions(&self) -> &[CleanupAction] {
        &self.actions
    }
}

impl CleanupAction {
    pub(crate) const fn new(
        target: CleanupTarget,
        condition: CleanupCondition,
        effect: CleanupEffect,
    ) -> Self {
        Self {
            target,
            condition,
            effect,
        }
    }

    #[must_use]
    pub const fn target(&self) -> &CleanupTarget {
        &self.target
    }

    #[must_use]
    pub const fn condition(&self) -> CleanupCondition {
        self.condition
    }

    #[must_use]
    pub(crate) const fn effect(&self) -> &CleanupEffect {
        &self.effect
    }

    pub(crate) fn with_condition(&self, condition: CleanupCondition) -> Self {
        Self {
            target: self.target.clone(),
            condition,
            effect: self.effect.clone(),
        }
    }
}

/// Cleanup events keyed by the checked node that owns their exact execution point.
#[derive(Clone, Debug)]
pub struct CleanupTable {
    schedules: Arena<BodyNodeId, Box<[CleanupSchedule]>>,
}

impl CleanupTable {
    pub(crate) const fn new(schedules: Arena<BodyNodeId, Box<[CleanupSchedule]>>) -> Self {
        Self { schedules }
    }

    #[must_use]
    pub fn schedules(&self, node: BodyNodeId) -> Option<&[CleanupSchedule]> {
        self.schedules.get(node).map(AsRef::as_ref)
    }

    #[must_use]
    pub fn schedule(&self, node: BodyNodeId, timing: CleanupTiming) -> Option<&CleanupSchedule> {
        self.schedules.get(node).and_then(|schedules| {
            schedules
                .iter()
                .find(|schedule| schedule.timing() == timing)
        })
    }

    #[must_use]
    pub fn actions(&self, node: BodyNodeId, timing: CleanupTiming) -> Option<&[CleanupAction]> {
        self.schedule(node, timing).map(CleanupSchedule::actions)
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.schedules.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.schedules.is_empty()
    }
}
