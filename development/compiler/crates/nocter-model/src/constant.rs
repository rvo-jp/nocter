/// One storage-independent value proven by compile-time constant evaluation.
///
/// This representation is shared only after source expressions have been typed and evaluated.
/// It contains no syntax, evaluator state, or runtime storage identity.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ConstantValue {
    Bool(bool),
    Character(u32),
    Integer(i128),
    Text(Box<str>),
}

/// One recursively typed, storage-bearing value frozen during semantic construction.
///
/// Scalar leaves deliberately reuse [`ConstantValue`], keeping arithmetic and literal semantics
/// under the constant evaluator's single authority. Physical size, alignment, byte order, and
/// relocation remain absent until machine layout.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum FrozenValue {
    Scalar(ConstantValue),
    FixedArray(Box<[FrozenValue]>),
}
