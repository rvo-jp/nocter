use nocter_model::{Arena, BodyNodeId, FieldId, TypeId};

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
    Value { node: BodyNodeId, ty: TypeId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CleanupAction {
    target: CleanupTarget,
    condition: CleanupCondition,
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

/// Cleanup actions keyed by the checked node whose outgoing edge runs them.
#[derive(Debug)]
pub struct CleanupTable {
    actions: Arena<BodyNodeId, Box<[CleanupAction]>>,
}

impl CleanupTable {
    pub(crate) const fn new(actions: Arena<BodyNodeId, Box<[CleanupAction]>>) -> Self {
        Self { actions }
    }

    #[must_use]
    pub fn get(&self, node: BodyNodeId) -> Option<&[CleanupAction]> {
        self.actions.get(node).map(Box::as_ref)
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.actions.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}
