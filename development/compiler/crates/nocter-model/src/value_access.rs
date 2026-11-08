use crate::TypeId;

/// Source-independent operation permission. This is not a borrow capability or a coercion rank.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueAccessMode {
    Read,
    Update,
    Own,
}

/// Transient operation access to one resolved data shape, not a qualified semantic data type.
///
/// The owner of the surrounding semantic product validates `data` against its type authority.
/// This descriptor does not flatten an outcome of access into access to an outcome, grant Copy
/// evidence, or describe a physical pointer/storage layout.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ValueAccess {
    data: TypeId,
    mode: ValueAccessMode,
}

impl ValueAccess {
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

/// Operation selected by a callable input, independently of the input's data type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueInputOperation {
    Read,
    Edit,
    Take,
}
impl ValueInputOperation {
    #[must_use]
    pub const fn access_mode(self) -> ValueAccessMode {
        match self {
            Self::Read => ValueAccessMode::Read,
            Self::Edit => ValueAccessMode::Update,
            Self::Take => ValueAccessMode::Own,
        }
    }
    #[must_use]
    pub const fn binding_kind(self) -> ValueBindingKind {
        match self {
            Self::Read => ValueBindingKind::ReadAlias,
            Self::Edit => ValueBindingKind::EditAlias,
            Self::Take => ValueBindingKind::MutableValue,
        }
    }
}

/// How a local stores a value or accesses a separate existing place.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueBindingKind {
    FixedValue,
    MutableValue,
    ReadAlias,
    EditAlias,
}
impl ValueBindingKind {
    #[must_use]
    pub const fn access_mode(self) -> ValueAccessMode {
        match self {
            Self::FixedValue | Self::MutableValue => ValueAccessMode::Own,
            Self::ReadAlias => ValueAccessMode::Read,
            Self::EditAlias => ValueAccessMode::Update,
        }
    }
    #[must_use]
    pub const fn writable(self) -> bool {
        matches!(self, Self::MutableValue | Self::EditAlias)
    }
}
