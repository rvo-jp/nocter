use nocter_model::{
    BorrowCapability, FieldId, MirLocalId, MirValueId, OpaqueTypeId, ParameterId, TypeId, VariantId,
};

/// Why one function-local storage slot exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirLocalKind {
    Parameter { position: usize },
    User,
    Temporary,
    Region,
}

/// One concrete function-local storage slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirLocal {
    ty: TypeId,
    kind: MirLocalKind,
    mutable: bool,
}

impl MirLocal {
    #[must_use]
    pub const fn new(ty: TypeId, kind: MirLocalKind, mutable: bool) -> Self {
        Self { ty, kind, mutable }
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn kind(self) -> MirLocalKind {
        self.kind
    }

    #[must_use]
    pub const fn is_mutable(self) -> bool {
        self.mutable
    }
}

/// The storage authority from which a place starts.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MirPlaceRoot {
    Local(MirLocalId),
    Dereference {
        value: MirValueId,
        capability: BorrowCapability,
    },
}

/// One typed storage projection. `ty` is the type after applying `kind`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MirProjection {
    kind: MirProjectionKind,
    ty: TypeId,
}

impl MirProjection {
    #[must_use]
    pub const fn new(kind: MirProjectionKind, ty: TypeId) -> Self {
        Self { kind, ty }
    }

    #[must_use]
    pub const fn kind(self) -> MirProjectionKind {
        self.kind
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

/// A backend-independent projection with no source-expression behavior left to recover.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MirProjectionKind {
    Field(FieldId),
    VariantPayload {
        variant: VariantId,
        parameter: ParameterId,
    },
    BorrowDereference(BorrowCapability),
    FixedIndex(u64),
    DynamicIndex(MirValueId),
    OptionalPayload,
    FallibleSuccess,
    FallibleFailure,
    OpaqueWitness(OpaqueTypeId),
}

/// One interned typed path to storage.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MirPlace {
    root: MirPlaceRoot,
    projections: Box<[MirProjection]>,
    ty: TypeId,
}

impl MirPlace {
    #[must_use]
    pub fn new(
        root: MirPlaceRoot,
        projections: impl Into<Box<[MirProjection]>>,
        ty: TypeId,
    ) -> Self {
        Self {
            root,
            projections: projections.into(),
            ty,
        }
    }

    #[must_use]
    pub const fn root(&self) -> MirPlaceRoot {
        self.root
    }

    #[must_use]
    pub const fn projections(&self) -> &[MirProjection] {
        &self.projections
    }

    #[must_use]
    pub const fn ty(&self) -> TypeId {
        self.ty
    }
}
