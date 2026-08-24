/// One storage-independent value proven by compile-time constant evaluation.
///
/// This representation is shared only after source expressions have been typed and evaluated.
/// It contains no syntax, evaluator state, or runtime storage identity.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ConstantValue {
    Bool(bool),
    Integer(i128),
    Text(Box<str>),
}
