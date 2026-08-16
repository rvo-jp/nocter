use nocter_model::{
    BodyNodeId, BorrowCapability, CaptureId, FieldId, LocalBindingId, ParameterId, TypeId,
};

use super::StaticSelection;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PlaceRoot {
    Parameter(ParameterId),
    Local(LocalBindingId),
    Capture(CaptureId),
}

/// Storage authority retained by a checked place.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PlaceAccess {
    Owned,
    Borrowed(BorrowCapability),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlaceProjection {
    Field(FieldId),
    /// An implicit dereference required to continue projecting through a borrow value.
    BorrowDeref {
        capability: BorrowCapability,
    },
    BuiltinIndex {
        index: BodyNodeId,
    },
    CoercedBuiltinIndex {
        index: BodyNodeId,
        receiver_coercion: StaticSelection,
    },
    SelectedIndex {
        index: BodyNodeId,
        operation: StaticSelection,
        receiver_coercion: Option<StaticSelection>,
    },
}

/// One fully classified place. Move eligibility is further restricted to field-only projections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedPlace {
    root: PlaceRoot,
    projections: Box<[PlaceProjection]>,
    ty: TypeId,
    access: PlaceAccess,
    writable: bool,
}

impl CheckedPlace {
    pub(super) fn new(
        root: PlaceRoot,
        projections: impl Into<Box<[PlaceProjection]>>,
        ty: TypeId,
        access: PlaceAccess,
        writable: bool,
    ) -> Self {
        Self {
            root,
            projections: projections.into(),
            ty,
            access,
            writable,
        }
    }

    #[must_use]
    pub const fn root(&self) -> PlaceRoot {
        self.root
    }

    #[must_use]
    pub const fn projections(&self) -> &[PlaceProjection] {
        &self.projections
    }

    #[must_use]
    pub const fn ty(&self) -> TypeId {
        self.ty
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
                .all(|projection| matches!(projection, PlaceProjection::Field(_)))
    }

    pub(crate) fn evaluation_nodes(&self) -> impl Iterator<Item = BodyNodeId> + '_ {
        self.projections
            .iter()
            .filter_map(|projection| match projection {
                PlaceProjection::BuiltinIndex { index }
                | PlaceProjection::CoercedBuiltinIndex { index, .. }
                | PlaceProjection::SelectedIndex { index, .. } => Some(*index),
                PlaceProjection::Field(_) | PlaceProjection::BorrowDeref { .. } => None,
            })
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
            projections: Box::new([PlaceProjection::Field(field)]),
            ty: types.builtin(BuiltinType::I32),
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
