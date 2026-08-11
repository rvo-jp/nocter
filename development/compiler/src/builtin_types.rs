//! Canonical identities and source authorities for compiler built-in types.
//!
//! Built-in types are syntax-level identities rather than nominal symbols.
//! This registry is the single boundary shared by frontend loading, resolver
//! surface collection, type checking, and editor analysis.

use crate::ast::TypeExpr;
use crate::integer::IntegerType;

pub(crate) const STR_IMPLEMENTATION_MODULE: &str = "std/str";
pub(crate) const SLICE_IMPLEMENTATION_MODULE: &str = "std/slice";
pub(crate) const ERROR_IMPLEMENTATION_MODULE: &str = "std/error";
pub(crate) const NUM_IMPLEMENTATION_MODULE: &str = "std/num";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuiltinSurfaceAuthority {
    pub(crate) module: &'static str,
    pub(crate) instance: bool,
    pub(crate) construction: bool,
    pub(crate) conformance: bool,
    pub(crate) implicitly_loaded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinTypeOwner {
    Str,
    Slice,
    Error,
    Bool,
    Integer(IntegerType),
}

impl BuiltinTypeOwner {
    pub(crate) const ALL: [Self; 14] = [
        Self::Str,
        Self::Slice,
        Self::Error,
        Self::Bool,
        Self::Integer(IntegerType::I8),
        Self::Integer(IntegerType::I16),
        Self::Integer(IntegerType::I32),
        Self::Integer(IntegerType::I64),
        Self::Integer(IntegerType::Isize),
        Self::Integer(IntegerType::U8),
        Self::Integer(IntegerType::U16),
        Self::Integer(IntegerType::U32),
        Self::Integer(IntegerType::U64),
        Self::Integer(IntegerType::Usize),
    ];

    pub(crate) const fn canonical_name(self) -> &'static str {
        match self {
            Self::Str => "str",
            Self::Slice => "[T]",
            Self::Error => "error",
            Self::Bool => "bool",
            Self::Integer(kind) => kind.name(),
        }
    }

    pub(crate) const fn source_authority(self) -> BuiltinSurfaceAuthority {
        match self {
            Self::Str => BuiltinSurfaceAuthority {
                module: STR_IMPLEMENTATION_MODULE,
                instance: true,
                construction: false,
                conformance: true,
                implicitly_loaded: true,
            },
            Self::Slice => BuiltinSurfaceAuthority {
                module: SLICE_IMPLEMENTATION_MODULE,
                instance: true,
                construction: false,
                conformance: true,
                implicitly_loaded: true,
            },
            Self::Error => BuiltinSurfaceAuthority {
                module: ERROR_IMPLEMENTATION_MODULE,
                instance: false,
                construction: true,
                conformance: true,
                implicitly_loaded: true,
            },
            Self::Bool | Self::Integer(_) => BuiltinSurfaceAuthority {
                module: NUM_IMPLEMENTATION_MODULE,
                instance: true,
                construction: true,
                conformance: true,
                implicitly_loaded: false,
            },
        }
    }

    pub(crate) fn from_instance_target(target: &TypeExpr) -> Option<Self> {
        match target {
            TypeExpr::Reference(reference) => Self::from_reference_name(&reference.name),
            TypeExpr::View(_) => Some(Self::Slice),
            _ => None,
        }
    }

    pub(crate) fn from_construction_target(target: &TypeExpr) -> Option<Self> {
        let TypeExpr::Reference(reference) = target else {
            return None;
        };
        Self::from_reference_name(&reference.name)
    }

    pub(crate) fn from_conformance_target(target: &TypeExpr) -> Option<Self> {
        match target {
            TypeExpr::Reference(reference) => Self::from_reference_name(&reference.name),
            TypeExpr::View(_) => Some(Self::Slice),
            _ => None,
        }
    }

    pub(crate) fn from_reference_name(name: &str) -> Option<Self> {
        Some(match name {
            "str" => Self::Str,
            "error" => Self::Error,
            "bool" => Self::Bool,
            _ => Self::Integer(IntegerType::from_name(name)?),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::BuiltinTypeOwner;

    #[test]
    fn every_builtin_owner_has_one_source_authority() {
        for owner in BuiltinTypeOwner::ALL {
            assert!(!owner.source_authority().module.is_empty());
        }
        assert_eq!(
            BuiltinTypeOwner::Error.source_authority().module,
            "std/error"
        );
    }

    #[test]
    fn scalar_builtins_share_the_num_authority_without_implicit_loading() {
        assert_eq!(BuiltinTypeOwner::Bool.canonical_name(), "bool");
        assert_eq!(
            BuiltinTypeOwner::Integer(crate::integer::IntegerType::I64).canonical_name(),
            "i64"
        );
        let bool_authority = BuiltinTypeOwner::Bool.source_authority();
        let integer_authority =
            BuiltinTypeOwner::Integer(crate::integer::IntegerType::I64).source_authority();
        assert_eq!(bool_authority.module, "std/num");
        assert_eq!(bool_authority.module, integer_authority.module);
        assert!(!bool_authority.implicitly_loaded);
    }
}
