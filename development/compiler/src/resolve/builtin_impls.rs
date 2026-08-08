//! Source-declared inherent method surfaces for compiler built-in unsized types.
//!
//! Built-in types are type syntax, not ordinary named declarations. Their
//! methods still need stable source identities, so the resolver retains a
//! separate surface instead of inserting synthetic structs into the value/type
//! symbol table.

use super::signatures::{duplicate_inherent_member_name_diagnostics, method_signatures};
use super::{ConstructionSurface, Resolver, TypeSymbol, TypeSymbolKind};
use crate::ast::{AstFile, ImplDecl, ImplMember, Item, MethodReceiverMode, TypeExpr, Visibility};
use crate::builtin_types::BuiltinTypeOwner;
use crate::diagnostics::Diagnostic;
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuiltinTypeSurface {
    pub(crate) owner: BuiltinTypeOwner,
    pub(crate) declaration_span: ByteSpan,
    pub(crate) symbol: TypeSymbol,
}

impl Resolver<'_> {
    pub(super) fn collect_builtin_impl_surfaces(&mut self, root: &AstFile) {
        let mut impls = self
            .module_index
            .asts()
            .flat_map(|ast| {
                ast.items.iter().filter_map(move |item| match item {
                    Item::Impl(impl_) => {
                        builtin_impl_owner(&impl_).map(|owner| (ast, owner, impl_.clone()))
                    }
                    _ => None,
                })
            })
            .collect::<Vec<_>>();
        impls.sort_by_key(|(_, _, impl_)| {
            (
                impl_.span.source == root.span.source,
                impl_.span.source.raw(),
                impl_.span.start,
            )
        });

        for (ast, owner, impl_) in impls {
            if let Err(message) = builtin_impl_shape(owner, &impl_) {
                if impl_.span.source == root.span.source {
                    self.output
                        .diagnostics
                        .push(invalid_builtin_impl_diagnostic(
                            self.sources,
                            impl_.target_ty.span(),
                            owner,
                            message,
                        ));
                }
                continue;
            }

            let mut methods = method_signatures(&impl_).collect::<Vec<_>>();
            self.prepare_builtin_surface_methods(
                ast,
                owner.implementation_module(),
                methods.as_mut_slice(),
            );

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
            surface.symbol.methods.extend(methods);
        }
    }
}

fn builtin_impl_owner(impl_: &ImplDecl) -> Option<BuiltinTypeOwner> {
    BuiltinTypeOwner::from_impl_target(&impl_.target_ty)
}

fn builtin_impl_shape(owner: BuiltinTypeOwner, impl_: &ImplDecl) -> Result<(), &'static str> {
    if impl_.interface_ty.is_some() {
        return Err("built-in types cannot implement project interfaces");
    }
    match owner {
        BuiltinTypeOwner::Str if !impl_.generics.parameters.is_empty() => {
            return Err("`impl str` cannot declare generic parameters");
        }
        BuiltinTypeOwner::Slice => {
            let TypeExpr::View(view) = &impl_.target_ty else {
                unreachable!("slice owner always has a view target");
            };
            if view.is_readwrite {
                return Err("the built-in slice owner must be written `[T]`, not `&+[T]`");
            }
            let [parameter] = impl_.generics.parameters.as_slice() else {
                return Err("the built-in slice owner requires exactly one generic parameter");
            };
            let TypeExpr::Reference(element) = view.element.as_ref() else {
                return Err("the built-in slice element must name its generic parameter");
            };
            if element.name != parameter.name {
                return Err("the slice element must match the implementation generic parameter");
            }
        }
        BuiltinTypeOwner::Str => {}
    }
    if impl_.members.iter().any(|member| {
        !matches!(
            member,
            ImplMember::Method(method)
                if method.body.is_some()
                    && method.visibility == Visibility::Public
                    && method.receiver.mode != MethodReceiverMode::Owned
        )
    }) {
        return Err("built-in implementations contain only public borrowed methods with bodies");
    }
    Ok(())
}

fn invalid_builtin_impl_diagnostic(
    sources: &crate::source::SourceMap,
    span: ByteSpan,
    owner: BuiltinTypeOwner,
    message: &'static str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0417",
        format!(
            "invalid implementation surface for built-in type `{}`: {message}",
            owner.canonical_name()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "built-in surfaces must keep their canonical generic target and public borrowed source methods"
            .to_string(),
    );
    diagnostic
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
