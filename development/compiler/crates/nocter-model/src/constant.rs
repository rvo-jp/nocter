/// One storage-independent value proven by compile-time constant evaluation.
///
/// This representation is shared only after source expressions have been typed and evaluated.
/// It contains no syntax, evaluator state, or runtime storage identity.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ConstantValue {
    Bool(bool),
    Character(u32),
    Float32(u32),
    Float64(u64),
    Integer(i128),
    Text(Box<str>),
}

/// One syntax-independent `usize` term used by structural type identity.
///
/// A declaration may retain its own constant parameter until specialization. Concrete layout and
/// ABI consumers must call [`Self::closed_value`] and reject an unclosed term; they never receive
/// source syntax or evaluator state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UsizeTerm {
    Value(u64),
    Parameter(crate::GenericParameterId),
}

impl UsizeTerm {
    #[must_use]
    pub const fn closed_value(&self) -> Option<u64> {
        match self {
            Self::Value(value) => Some(*value),
            Self::Parameter(_) => None,
        }
    }

    #[must_use]
    pub const fn parameter(&self) -> Option<crate::GenericParameterId> {
        match self {
            Self::Value(_) => None,
            Self::Parameter(parameter) => Some(*parameter),
        }
    }
}

impl From<u64> for UsizeTerm {
    fn from(value: u64) -> Self {
        Self::Value(value)
    }
}

/// One normalized value in an ordered generic application.
///
/// The declaration schema determines which variant is valid at each position. Keeping both
/// domains in one sequence prevents type identity, substitution, and presentation from inventing
/// parallel ordering rules for type and constant parameters.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GenericValue {
    Type(crate::TypeId),
    Usize(UsizeTerm),
}

impl GenericValue {
    #[must_use]
    pub const fn as_type(&self) -> Option<crate::TypeId> {
        match self {
            Self::Type(ty) => Some(*ty),
            Self::Usize(_) => None,
        }
    }

    #[must_use]
    pub const fn as_usize(&self) -> Option<&UsizeTerm> {
        match self {
            Self::Type(_) => None,
            Self::Usize(value) => Some(value),
        }
    }
}

impl From<crate::TypeId> for GenericValue {
    fn from(ty: crate::TypeId) -> Self {
        Self::Type(ty)
    }
}

impl From<UsizeTerm> for GenericValue {
    fn from(value: UsizeTerm) -> Self {
        Self::Usize(value)
    }
}

/// The authored-order values of one generic application.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GenericApplication(Box<[GenericValue]>);

impl GenericApplication {
    #[must_use]
    pub fn new(values: impl Into<Box<[GenericValue]>>) -> Self {
        Self(values.into())
    }

    #[must_use]
    pub fn from_types(types: impl IntoIterator<Item = crate::TypeId>) -> Self {
        Self(
            types
                .into_iter()
                .map(GenericValue::Type)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )
    }

    #[must_use]
    pub const fn as_slice(&self) -> &[GenericValue] {
        &self.0
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, GenericValue> {
        self.0.iter()
    }

    pub fn type_values(&self) -> impl Iterator<Item = crate::TypeId> + '_ {
        self.0.iter().filter_map(GenericValue::as_type)
    }

    #[must_use]
    pub fn type_at(&self, index: usize) -> Option<crate::TypeId> {
        self.0.get(index).and_then(GenericValue::as_type)
    }

    /// Maps only type values while preserving constant values and authored order.
    ///
    /// # Errors
    ///
    /// Returns the first error produced by `map`.
    pub fn try_map_types<E>(
        &self,
        mut map: impl FnMut(crate::TypeId) -> Result<crate::TypeId, E>,
    ) -> Result<Self, E> {
        self.0
            .iter()
            .map(|value| match value {
                GenericValue::Type(ty) => map(*ty).map(GenericValue::Type),
                GenericValue::Usize(value) => Ok(GenericValue::Usize(*value)),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Self::new)
    }
}

impl From<Box<[crate::TypeId]>> for GenericApplication {
    fn from(types: Box<[crate::TypeId]>) -> Self {
        Self::from_types(types)
    }
}

impl From<Vec<crate::TypeId>> for GenericApplication {
    fn from(types: Vec<crate::TypeId>) -> Self {
        Self::from_types(types)
    }
}

impl std::iter::FromIterator<crate::TypeId> for GenericApplication {
    fn from_iter<T: IntoIterator<Item = crate::TypeId>>(types: T) -> Self {
        Self::from_types(types)
    }
}

/// One recursively typed, storage-bearing value frozen during semantic construction.
///
/// Scalar leaves deliberately reuse [`ConstantValue`], keeping arithmetic and literal semantics
/// under the constant evaluator's single authority. Physical size, alignment, byte order, and
/// relocation remain absent until machine layout.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum FrozenValue {
    Scalar(ConstantValue),
    Tuple(Box<[FrozenValue]>),
    FixedArray(Box<[FrozenValue]>),
}
