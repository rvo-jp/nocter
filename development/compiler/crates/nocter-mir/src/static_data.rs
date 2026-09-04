use nocter_model::{FrozenValue, TypeId};

/// One reachable immutable static before ABI layout and byte encoding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStatic {
    ty: TypeId,
    value: FrozenValue,
}

impl MirStatic {
    #[must_use]
    pub const fn new(ty: TypeId, value: FrozenValue) -> Self {
        Self { ty, value }
    }

    #[must_use]
    pub const fn ty(&self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn value(&self) -> &FrozenValue {
        &self.value
    }
}
