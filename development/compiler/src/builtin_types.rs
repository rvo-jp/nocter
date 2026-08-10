//! Canonical identities and source authorities for compiler built-in types.
//!
//! Built-in types are syntax-level identities rather than nominal symbols.
//! This registry is the single boundary shared by frontend loading, resolver
//! surface collection, type checking, and editor analysis.

use crate::ast::TypeExpr;
use crate::integer::IntegerType;

pub(crate) const STR_IMPLEMENTATION_MODULE: &str = "std/str";
pub(crate) const SLICE_IMPLEMENTATION_MODULE: &str = "std/slice";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinTypeOwner {
    Str,
    Slice,
    Bool,
    Integer(IntegerType),
}

impl BuiltinTypeOwner {
    pub(crate) const INSTANCE_OWNERS: [Self; 2] = [Self::Str, Self::Slice];

    pub(crate) const fn canonical_name(self) -> &'static str {
        match self {
            Self::Str => "str",
            Self::Slice => "[T]",
            Self::Bool => "bool",
            Self::Integer(kind) => kind.name(),
        }
    }

    pub(crate) const fn instance_module(self) -> Option<&'static str> {
        match self {
            Self::Str => Some(STR_IMPLEMENTATION_MODULE),
            Self::Slice => Some(SLICE_IMPLEMENTATION_MODULE),
            Self::Bool | Self::Integer(_) => None,
        }
    }

    pub(crate) fn from_instance_target(target: &TypeExpr) -> Option<Self> {
        match target {
            TypeExpr::Reference(reference) if reference.name == "str" => Some(Self::Str),
            TypeExpr::View(_) => Some(Self::Slice),
            _ => None,
        }
    }

    pub(crate) fn from_conformance_target(target: &TypeExpr) -> Option<Self> {
        match target {
            TypeExpr::Reference(reference) if reference.name == "str" => Some(Self::Str),
            TypeExpr::Reference(reference) if reference.name == "bool" => Some(Self::Bool),
            TypeExpr::Reference(reference) => {
                IntegerType::from_name(&reference.name).map(Self::Integer)
            }
            TypeExpr::View(_) => Some(Self::Slice),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BuiltinTypeOwner;

    #[test]
    fn every_builtin_owner_has_one_distinct_source_authority() {
        let authorities = BuiltinTypeOwner::INSTANCE_OWNERS.map(BuiltinTypeOwner::instance_module);
        assert_eq!(authorities, [Some("std/str"), Some("std/slice")]);
        assert_ne!(authorities[0], authorities[1]);
    }

    #[test]
    fn scalar_builtins_have_conformance_identity_without_inherent_authority() {
        assert_eq!(BuiltinTypeOwner::Bool.canonical_name(), "bool");
        assert_eq!(
            BuiltinTypeOwner::Integer(crate::integer::IntegerType::I64).canonical_name(),
            "i64"
        );
        assert_eq!(BuiltinTypeOwner::Bool.instance_module(), None);
    }
}
