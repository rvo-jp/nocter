//! Type-checker access to source-backed compiler built-in type surfaces.

use super::model::Type;
use crate::builtin_types::BuiltinTypeOwner;
use crate::integer::IntegerType;
use crate::resolve::{ResolveOutput, TypeSymbol};

pub(super) fn owner_for_type(ty: &Type) -> Option<BuiltinTypeOwner> {
    match ty {
        Type::StrData | Type::Str => Some(BuiltinTypeOwner::Str),
        Type::ArrayData { .. } | Type::View { .. } => Some(BuiltinTypeOwner::Slice),
        Type::Error => Some(BuiltinTypeOwner::Error),
        Type::I32 => Some(BuiltinTypeOwner::Integer(IntegerType::I32)),
        Type::Primitive(name) if name == "bool" => Some(BuiltinTypeOwner::Bool),
        Type::Primitive(name) => IntegerType::from_name(name).map(BuiltinTypeOwner::Integer),
        _ => None,
    }
}

pub(super) fn self_type(owner: BuiltinTypeOwner, symbol: &TypeSymbol) -> Type {
    match owner {
        BuiltinTypeOwner::Str => Type::Str,
        BuiltinTypeOwner::Slice => Type::View {
            element: Box::new(Type::Parameter(
                symbol
                    .generic_parameters
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "T".to_string()),
            )),
            is_readwrite: false,
        },
        BuiltinTypeOwner::Error => Type::Error,
        BuiltinTypeOwner::Bool => Type::Primitive("bool".to_string()),
        BuiltinTypeOwner::Integer(IntegerType::I32) => Type::I32,
        BuiltinTypeOwner::Integer(kind) => Type::Primitive(kind.name().to_string()),
    }
}

pub(super) fn symbol_for_type<'a>(
    ty: &Type,
    resolved: &'a ResolveOutput,
) -> Option<&'a TypeSymbol> {
    if let Some(owner) = owner_for_type(ty) {
        return resolved
            .builtin_type_surface(owner)
            .map(|surface| &surface.symbol);
    }
    ty.nominal_name()
        .and_then(|name| resolved.type_symbol_by_canonical_name(name))
}
