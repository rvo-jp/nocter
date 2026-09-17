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
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
