//! Source-declared inherent method surfaces for compiler built-in unsized types.
//!
//! Built-in types are type syntax, not ordinary named declarations. Their
//! methods still need stable source identities, so the resolver retains a
//! separate surface instead of inserting synthetic structs into the value/type
//! symbol table.

use super::signatures::{duplicate_inherent_member_name_diagnostics, method_signatures};
use super::{ConstructionSurface, Resolver, TypeSymbol, TypeSymbolKind};
use crate::ast::{AstFile, ImplDecl, ImplMember, Item, TypeExpr};
use crate::source::ByteSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinTypeOwner {
    Str,
    Slice,
}

impl BuiltinTypeOwner {
    pub(crate) const fn canonical_name(self) -> &'static str {
        match self {
            Self::Str => "str",
            Self::Slice => "[T]",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuiltinTypeSurface {
    pub(crate) owner: BuiltinTypeOwner,
    pub(crate) declaration_span: ByteSpan,
    pub(crate) symbol: TypeSymbol,
}

impl Resolver<'_> {
    pub(super) fn collect_builtin_impl_surfaces(&mut self, root: &AstFile) {
        let impls = self
            .module_index
            .asts()
            .flat_map(|ast| ast.items.iter())
            .filter_map(|item| match item {
                Item::Impl(impl_) if impl_.interface_ty.is_none() => {
                    builtin_impl_owner(impl_).map(|owner| (owner, impl_.clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        for (owner, impl_) in impls {
            if !builtin_impl_shape_is_valid(owner, &impl_) {
                continue;
            }

            let surface = self
                .output
                .builtin_type_surfaces
                .entry(owner)
                .or_insert_with(|| BuiltinTypeSurface {
                    owner,
                    declaration_span: impl_.target_ty.span(),
                    symbol: empty_builtin_symbol(owner, &impl_),
                });

            if impl_.span.source == root.span.source {
                self.output
                    .diagnostics
                    .extend(duplicate_inherent_member_name_diagnostics(
                        self.sources,
                        owner.canonical_name(),
                        &surface.symbol,
                        &impl_,
                    ));
            }
            surface.symbol.methods.extend(method_signatures(&impl_));
        }
    }
}

fn builtin_impl_owner(impl_: &ImplDecl) -> Option<BuiltinTypeOwner> {
    match &impl_.target_ty {
        TypeExpr::Reference(reference) if reference.name == "str" => Some(BuiltinTypeOwner::Str),
        TypeExpr::View(_) => Some(BuiltinTypeOwner::Slice),
        _ => None,
    }
}

fn builtin_impl_shape_is_valid(owner: BuiltinTypeOwner, impl_: &ImplDecl) -> bool {
    let generics_match = match owner {
        BuiltinTypeOwner::Str => impl_.generics.parameters.is_empty(),
        BuiltinTypeOwner::Slice => impl_.generics.parameters.len() == 1,
    };
    generics_match
        && impl_
            .members
            .iter()
            .all(|member| matches!(member, ImplMember::Method(method) if method.body.is_some()))
}

fn empty_builtin_symbol(owner: BuiltinTypeOwner, impl_: &ImplDecl) -> TypeSymbol {
    TypeSymbol {
        // The surface is kept outside the nominal symbol table. `Struct` here
        // only reuses the common method container; it never grants fields,
        // construction, coercion ownership, or drop.
        kind: TypeSymbolKind::Struct,
        canonical_name: owner.canonical_name().to_string(),
        generic_parameters: impl_
            .generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect(),
        generic_parameter_bounds: impl_
            .generics
            .parameters
            .iter()
            .map(|parameter| parameter.bounds.clone())
            .collect(),
        generic_arity: impl_.generics.parameters.len(),
        is_copy: false,
        alias_target: None,
        fields: Vec::new(),
        variants: Vec::new(),
        associated_functions: Vec::new(),
        methods: Vec::new(),
        interface_conformances: Vec::new(),
        drop_member: None,
        literals: Vec::new(),
        coercions: Vec::new(),
        construction: ConstructionSurface::default(),
    }
}
