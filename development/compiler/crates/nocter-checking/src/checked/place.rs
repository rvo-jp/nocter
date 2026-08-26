use nocter_model::{
    BodyNodeId, BorrowCapability, CaptureId, FieldId, LocalBindingId, ParameterId, TypeId,
};

use super::StaticSelection;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PlaceRoot {
    Parameter(ParameterId),
    Local(LocalBindingId),
    Capture(CaptureId),
    /// A checked expression whose result is a borrow value projected as a place.
    Value(BodyNodeId),
}

/// Storage authority retained by a checked place.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PlaceAccess {
    Owned,
    Borrowed(BorrowCapability),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlaceProjection {
    Field {
        field: FieldId,
        ty: TypeId,
    },
    /// An implicit dereference required to continue projecting through a borrow value.
    BorrowDeref {
        capability: BorrowCapability,
        ty: TypeId,
    },
    BuiltinIndex {
        index: BodyNodeId,
        ty: TypeId,
    },
    CoercedBuiltinIndex {
        index: BodyNodeId,
        receiver_coercion: StaticSelection,
        ty: TypeId,
    },
    SelectedIndex {
        index: BodyNodeId,
        operation: StaticSelection,
        receiver_coercion: Option<StaticSelection>,
        ty: TypeId,
    },
}

impl PlaceProjection {
    /// The checked result type after applying this projection.
    #[must_use]
    pub const fn ty(&self) -> TypeId {
        match self {
            Self::Field { ty, .. }
            | Self::BorrowDeref { ty, .. }
            | Self::BuiltinIndex { ty, .. }
            | Self::CoercedBuiltinIndex { ty, .. }
            | Self::SelectedIndex { ty, .. } => *ty,
        }
    }
}

/// One fully classified place. Move eligibility is further restricted to field-only projections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedPlace {
    root: PlaceRoot,
    root_ty: TypeId,
    projections: Box<[PlaceProjection]>,
    access: PlaceAccess,
    writable: bool,
}

impl CheckedPlace {
    pub(super) fn new(
        root: PlaceRoot,
        root_ty: TypeId,
        projections: impl Into<Box<[PlaceProjection]>>,
        access: PlaceAccess,
        writable: bool,
    ) -> Self {
        Self {
            root,
            root_ty,
            projections: projections.into(),
            access,
            writable,
        }
    }

    #[must_use]
    pub const fn root(&self) -> PlaceRoot {
        self.root
    }

    #[must_use]
    pub const fn root_ty(&self) -> TypeId {
        self.root_ty
    }

    #[must_use]
    pub const fn projections(&self) -> &[PlaceProjection] {
        &self.projections
    }

    #[must_use]
    pub fn ty(&self) -> TypeId {
        match self.projections.last() {
            Some(projection) => projection.ty(),
            None => self.root_ty,
        }
    }

    #[must_use]
    pub const fn access(&self) -> PlaceAccess {
        self.access
    }

    #[must_use]
    pub const fn is_writable(&self) -> bool {
        self.writable
    }

    #[must_use]
    pub fn is_move_source(&self) -> bool {
        self.access == PlaceAccess::Owned
            && self
                .projections
                .iter()
                .all(|projection| matches!(projection, PlaceProjection::Field { .. }))
    }

    pub(crate) fn evaluation_nodes(&self) -> impl Iterator<Item = BodyNodeId> + '_ {
        let root = match self.root {
            PlaceRoot::Value(value) => Some(value),
            PlaceRoot::Parameter(_) | PlaceRoot::Local(_) | PlaceRoot::Capture(_) => None,
        };
        root.into_iter().chain(
            self.projections
                .iter()
                .filter_map(|projection| match projection {
                    PlaceProjection::BuiltinIndex { index, .. }
                    | PlaceProjection::CoercedBuiltinIndex { index, .. }
                    | PlaceProjection::SelectedIndex { index, .. } => Some(*index),
                    PlaceProjection::Field { .. } | PlaceProjection::BorrowDeref { .. } => None,
                }),
        )
    }

    pub(crate) fn has_dynamic_evaluation(&self) -> bool {
        self.evaluation_nodes().next().is_some()
    }
}

#[cfg(test)]
mod tests {
    use nocter_model::{ArenaBuilder, BuiltinType, FieldId, LocalBindingId, TypeStore};

    use super::{CheckedPlace, PlaceAccess, PlaceProjection, PlaceRoot};

    #[test]
    fn only_owned_named_field_paths_are_move_sources() {
        let mut locals = ArenaBuilder::<LocalBindingId, _>::new();
        let local = locals.insert(());
        let _ = locals.finish();
        let mut fields = ArenaBuilder::<FieldId, _>::new();
        let field = fields.insert(());
        let _ = fields.finish();
        let types = TypeStore::new();
        let owned = CheckedPlace {
            root: PlaceRoot::Local(local),
            root_ty: types.builtin(BuiltinType::I32),
            projections: Box::new([PlaceProjection::Field {
                field,
                ty: types.builtin(BuiltinType::I32),
            }]),
            access: PlaceAccess::Owned,
            writable: true,
        };
        let borrowed = CheckedPlace {
            access: PlaceAccess::Borrowed(nocter_model::BorrowCapability::ReadWrite),
            ..owned.clone()
        };

        assert!(owned.is_move_source());
        assert!(!borrowed.is_move_source());
    }
}
