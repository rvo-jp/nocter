//! Canonical identities and source authorities for compiler built-in types.
//!
//! Built-in types are syntax-level identities rather than nominal symbols.
//! This registry is the single boundary shared by frontend loading, resolver
//! surface collection, type checking, and editor analysis.

use crate::ast::TypeExpr;

pub(crate) const STR_IMPLEMENTATION_MODULE: &str = "std/str";
pub(crate) const SLICE_IMPLEMENTATION_MODULE: &str = "std/slice";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinTypeOwner {
    Str,
    Slice,
}

impl BuiltinTypeOwner {
    pub(crate) const ALL: [Self; 2] = [Self::Str, Self::Slice];

    pub(crate) const fn canonical_name(self) -> &'static str {
        match self {
            Self::Str => "str",
            Self::Slice => "[T]",
        }
    }

    pub(crate) const fn implementation_module(self) -> &'static str {
        match self {
            Self::Str => STR_IMPLEMENTATION_MODULE,
            Self::Slice => SLICE_IMPLEMENTATION_MODULE,
        }
    }

    pub(crate) fn from_instance_target(target: &TypeExpr) -> Option<Self> {
        match target {
            TypeExpr::Reference(reference) if reference.name == "str" => Some(Self::Str),
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
        let authorities = BuiltinTypeOwner::ALL.map(BuiltinTypeOwner::implementation_module);
        assert_eq!(authorities, ["std/str", "std/slice"]);
        assert_ne!(authorities[0], authorities[1]);
    }
}
