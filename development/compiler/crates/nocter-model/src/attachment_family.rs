use crate::{BuiltinType, NominalTypeId, TypeId, TypeKind, TypeStore};

/// Canonical identity of a type family that may own construction, inherent operations, explicit
/// interface implementations, or other type-attached declarations.
///
/// Generic arguments deliberately do not participate in family identity. Policy such as which
/// module may attach an operation remains the responsibility of declaration validation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AttachmentFamily {
    Nominal(NominalTypeId),
    Builtin(BuiltinType),
    Slice,
}

impl AttachmentFamily {
    #[must_use]
    pub fn of(types: &TypeStore, target: TypeId) -> Option<Self> {
        match types.get(target)? {
            TypeKind::Nominal { definition, .. } => Some(Self::Nominal(*definition)),
            TypeKind::Builtin(builtin) => Some(Self::Builtin(*builtin)),
            TypeKind::Slice(_) => Some(Self::Slice),
            _ => None,
        }
    }
}
