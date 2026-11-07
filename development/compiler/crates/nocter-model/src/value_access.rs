use crate::TypeId;

/// Source-independent operation permission. This is not a borrow capability or a coercion rank.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueAccessMode {
    Read,
    Update,
    Own,
}

/// Access to one resolved data shape at a value boundary.
///
/// The owner of the surrounding semantic product validates `data` against its type authority.
/// This descriptor does not flatten an outcome of access into access to an outcome, grant Copy
/// evidence, or describe a physical pointer/storage layout.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueAccessType {
    data: TypeId,
    mode: ValueAccessMode,
}

impl ValueAccessType {
    #[must_use]
    pub const fn new(data: TypeId, mode: ValueAccessMode) -> Self {
        Self { data, mode }
    }

    #[must_use]
    pub const fn data(self) -> TypeId {
        self.data
    }

    #[must_use]
    pub const fn mode(self) -> ValueAccessMode {
        self.mode
    }
}
