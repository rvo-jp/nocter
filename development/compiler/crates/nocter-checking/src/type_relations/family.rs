use nocter_model::{BuiltinType, NominalTypeId, TypeId, TypeKind, TypeStore};

/// The declaration family that may own inherent operations for one type.
///
/// Generic arguments deliberately do not participate in this identity. They are matched against
/// the selected declaration after the bounded family lookup.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum InherentTypeFamily {
    Nominal(NominalTypeId),
    Builtin(BuiltinType),
    Slice,
}

impl InherentTypeFamily {
    #[must_use]
    pub(crate) fn of(types: &TypeStore, target: TypeId) -> Option<Self> {
        match types.get(target)? {
            TypeKind::Nominal { definition, .. } => Some(Self::Nominal(*definition)),
            TypeKind::Builtin(builtin) => Some(Self::Builtin(*builtin)),
            TypeKind::Slice(_) => Some(Self::Slice),
            _ => None,
        }
    }
}
