use nocter_model::{Arena, BodyNodeId, FieldId, PlaceId, TypeId};

use super::PlaceRoot;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanupCondition {
    Always,
    IfInitialized,
}

/// One owned storage path whose remaining live value is destroyed on an outgoing edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupPath {
    root: PlaceRoot,
    fields: Box<[FieldId]>,
    ty: TypeId,
}

impl CleanupPath {
    pub(crate) fn new(root: PlaceRoot, fields: impl Into<Box<[FieldId]>>, ty: TypeId) -> Self {
        Self {
            root,
            fields: fields.into(),
            ty,
        }
    }

    #[must_use]
    pub const fn root(&self) -> PlaceRoot {
        self.root
    }

    #[must_use]
    pub const fn fields(&self) -> &[FieldId] {
        &self.fields
    }

    #[must_use]
    pub const fn ty(&self) -> TypeId {
        self.ty
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CleanupTarget {
    Path(CleanupPath),
    Place { place: PlaceId, ty: TypeId },
    Value { node: BodyNodeId, ty: TypeId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupAction {
    target: CleanupTarget,
    condition: CleanupCondition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanupTiming {
    /// Runs after child evaluation and before the node performs its outgoing control transfer.
    BeforeTransfer,
    /// Runs after right-hand-side and target evaluation, immediately before assignment storage.
    BeforeStore,
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
    pub(crate) const fn new(target: CleanupTarget, condition: CleanupCondition) -> Self {
        Self { target, condition }
    }

    #[must_use]
    pub const fn target(&self) -> &CleanupTarget {
        &self.target
    }

    #[must_use]
    pub const fn condition(&self) -> CleanupCondition {
        self.condition
    }
}

/// Cleanup schedules keyed by the checked node that owns their exact execution point.
#[derive(Debug)]
pub struct CleanupTable {
    schedules: Arena<BodyNodeId, Option<CleanupSchedule>>,
}

impl CleanupTable {
    pub(crate) const fn new(schedules: Arena<BodyNodeId, Option<CleanupSchedule>>) -> Self {
        Self { schedules }
    }

    #[must_use]
    pub fn schedule(&self, node: BodyNodeId) -> Option<&CleanupSchedule> {
        self.schedules.get(node).and_then(Option::as_ref)
    }

    /// Returns the actions for a known node, including an empty slice when it has no schedule.
    #[must_use]
    pub fn actions(&self, node: BodyNodeId) -> Option<&[CleanupAction]> {
        self.schedules
            .get(node)
            .map(|schedule| schedule.as_ref().map_or(&[][..], CleanupSchedule::actions))
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
