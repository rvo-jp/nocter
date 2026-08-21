use crate::FieldId;

/// One compiler-defined field whose source surface is part of a built-in type contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BuiltinField {
    ErrorCode,
    ErrorMessage,
}

/// Stable identity of a source-selectable field.
///
/// Authored nominal fields retain their declaration ID. Built-in fields remain a closed semantic
/// domain instead of receiving synthetic declarations or being recovered from backend offsets.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FieldIdentity {
    Declared(FieldId),
    Builtin(BuiltinField),
}

impl From<FieldId> for FieldIdentity {
    fn from(field: FieldId) -> Self {
        Self::Declared(field)
    }
}
