//! Source-declared construction and behavior surfaces for compiler-owned types.
//!
//! Built-in types are type syntax, not ordinary named declarations. Their
//! methods still need stable source identities, so the resolver retains a
//! separate surface instead of inserting synthetic structs into the value/type
//! symbol table.

use super::constructions::{append_construction_entries, success_payload_is_self};
use super::signatures::{
    associated_function_signature, duplicate_inherent_member_name_diagnostics,
    instance_method_signatures,
};
use super::{ConstructionSurface, Resolver, TypeSymbol, TypeSymbolKind};
use crate::ast::{
    AstFile, ConstructDecl, ConstructMemberDecl, InstanceDecl, Item, MethodReceiverMode, TypeExpr,
    Visibility,
};
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
    pub(super) fn collect_builtin_source_surfaces(&mut self, root: &AstFile) {
        self.collect_builtin_instance_surfaces(root);
        self.collect_builtin_construction_surfaces(root);
    }

    fn collect_builtin_instance_surfaces(&mut self, root: &AstFile) {
        let mut instances =
            self.module_index
                .asts()
                .flat_map(|ast| {
                    ast.items.iter().filter_map(move |item| match item {
                        Item::Instance(instance) => builtin_instance_owner(instance)
                            .map(|owner| (ast, owner, instance.clone())),
                        _ => None,
                    })
                })
                .collect::<Vec<_>>();
        instances.sort_by_key(|(_, _, instance)| {
            (
                instance.span.source == root.span.source,
                instance.span.source.raw(),
                instance.span.start,
            )
        });

        for (ast, owner, instance) in instances {
            if let Err(message) = builtin_instance_shape(owner, &instance) {
                if instance.span.source == root.span.source {
                    self.output
                        .diagnostics
                        .push(invalid_builtin_instance_diagnostic(
                            self.sources,
                            instance.target_ty.span(),
                            owner,
                            message,
                        ));
                }
                continue;
            }

            let mut methods = instance_method_signatures(&instance).collect::<Vec<_>>();
            self.prepare_builtin_surface_methods(
                ast,
                owner.source_authority().module,
                methods.as_mut_slice(),
            );

            let surface = self
                .output
                .builtin_type_surfaces
                .entry(owner)
                .or_insert_with(|| BuiltinTypeSurface {
                    owner,
                    declaration_span: instance.target_ty.span(),
                    symbol: empty_builtin_symbol(owner, &instance),
                });

            if instance.span.source == root.span.source {
                self.output
                    .diagnostics
                    .extend(duplicate_inherent_member_name_diagnostics(
                        self.sources,
                        owner.canonical_name(),
                        &surface.symbol,
                        &instance,
                    ));
            }
            surface.symbol.methods.extend(methods);
        }
    }

    fn collect_builtin_construction_surfaces(&mut self, root: &AstFile) {
        let mut constructs = self
            .module_index
            .asts()
            .flat_map(|ast| {
                ast.items.iter().filter_map(move |item| match item {
                    Item::Construct(construct) => {
                        BuiltinTypeOwner::from_construction_target(&construct.target)
                            .map(|owner| (ast, owner, construct.clone()))
                    }
                    _ => None,
                })
            })
            .collect::<Vec<_>>();
        constructs.sort_by_key(|(_, owner, construct)| {
            (
                owner.canonical_name(),
                construct.span.source == root.span.source,
                construct.span.source.raw(),
                construct.span.start,
            )
        });

        for (ast, owner, construct) in constructs {
            if let Err(message) = builtin_construction_shape(owner, &construct) {
                if construct.span.source == root.span.source {
                    self.output
                        .diagnostics
                        .push(invalid_builtin_surface_diagnostic(
                            self.sources,
                            construct.target.span(),
                            owner,
                            message,
                        ));
                }
                continue;
            }

            let mut contribution = empty_builtin_type_symbol(owner);
            contribution.associated_functions.extend(
                construct
                    .functions()
                    .map(|(_, function)| associated_function_signature(function)),
            );
            contribution.construction.declaration_span = Some(construct.span);
            let explicit_defaults = append_construction_entries(&mut contribution, &construct);
            if let Some(&(entry, _)) = explicit_defaults.first() {
                contribution.construction.default_entry = Some(entry);
            }
            self.prepare_builtin_surface_symbol(
                ast,
                owner.source_authority().module,
                &mut contribution,
            );

            let surface = self
                .output
                .builtin_type_surfaces
                .entry(owner)
                .or_insert_with(|| BuiltinTypeSurface {
                    owner,
                    declaration_span: construct.target.span(),
                    symbol: empty_builtin_type_symbol(owner),
                });
            if surface.symbol.construction.declaration_span.is_some() {
                if construct.span.source == root.span.source {
                    self.output
                        .diagnostics
                        .push(invalid_builtin_surface_diagnostic(
                            self.sources,
                            construct.target.span(),
                            owner,
                            "the built-in type already has a construct declaration",
                        ));
                }
                continue;
            }
            surface
                .symbol
                .associated_functions
                .extend(contribution.associated_functions);
            surface.symbol.construction = contribution.construction;
        }
    }
}

fn builtin_instance_owner(instance: &InstanceDecl) -> Option<BuiltinTypeOwner> {
    BuiltinTypeOwner::from_instance_target(&instance.target_ty)
}

fn builtin_instance_shape(
    owner: BuiltinTypeOwner,
    instance: &InstanceDecl,
) -> Result<(), &'static str> {
    if !owner.source_authority().instance {
        return Err("this built-in type does not accept instance declarations");
    }
    match owner {
        BuiltinTypeOwner::Str if !instance.generics.parameters.is_empty() => {
            return Err("`instance str` cannot declare generic parameters");
        }
        BuiltinTypeOwner::Slice => {
            let TypeExpr::View(view) = &instance.target_ty else {
                unreachable!("slice owner always has a view target");
            };
            if view.is_readwrite {
                return Err("the built-in slice owner must be written `[T]`, not `&+[T]`");
            }
            let [parameter] = instance.generics.parameters.as_slice() else {
                return Err("the built-in slice owner requires exactly one generic parameter");
            };
            let TypeExpr::Reference(element) = view.element.as_ref() else {
                return Err("the built-in slice element must name its generic parameter");
            };
            if element.name != parameter.name {
                return Err("the slice element must match the instance generic parameter");
            }
        }
        BuiltinTypeOwner::Str
        | BuiltinTypeOwner::Error
        | BuiltinTypeOwner::Bool
        | BuiltinTypeOwner::Integer(_) => {
            if !instance.generics.parameters.is_empty() {
                return Err("scalar built-in instances cannot declare generic parameters");
            }
        }
    }
    if instance.callables().any(|method| {
        method.body.is_none()
            || method.visibility != Visibility::Public
            || method.receiver.mode == MethodReceiverMode::Owned
    }) {
        return Err("built-in instances contain only public borrowed methods with bodies");
    }
    Ok(())
}

fn builtin_construction_shape(
    owner: BuiltinTypeOwner,
    construct: &ConstructDecl,
) -> Result<(), &'static str> {
    if !owner.source_authority().construction {
        return Err("this built-in type does not accept construct declarations");
    }
    let TypeExpr::Reference(reference) = &construct.target else {
        return Err("built-in construct targets must use their canonical scalar spelling");
    };
    if reference.name != owner.canonical_name() {
        return Err("built-in construct target does not match its canonical spelling");
    }
    if construct.members.is_empty() {
        return Err("built-in construct declarations must expose at least one public member");
    }
    if construct
        .members
        .iter()
        .any(|member| match &member.declaration {
            ConstructMemberDecl::Function(function) => {
                function.visibility != Visibility::Public || function.body.is_none()
            }
            ConstructMemberDecl::Literal(literal) => {
                literal.visibility != Visibility::Public || literal.body.is_none()
            }
        })
    {
        return Err("built-in construct members must be public and have bodies");
    }
    if construct
        .members
        .iter()
        .any(|member| match &member.declaration {
            ConstructMemberDecl::Function(function) => {
                !success_payload_is_self(&function.return_type)
            }
            ConstructMemberDecl::Literal(literal) => !success_payload_is_self(&literal.return_type),
        })
    {
        return Err("built-in construct members must produce `Self`");
    }
    let default_count = construct
        .members
        .iter()
        .filter(|member| member.default_span.is_some())
        .count();
    if default_count > 1 {
        return Err("built-in construct declarations may have only one default member");
    }
    Ok(())
}

fn invalid_builtin_instance_diagnostic(
    sources: &crate::source::SourceMap,
    span: ByteSpan,
    owner: BuiltinTypeOwner,
    message: &'static str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0417",
        format!(
            "invalid instance surface for built-in type `{}`: {message}",
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

fn invalid_builtin_surface_diagnostic(
    sources: &crate::source::SourceMap,
    span: ByteSpan,
    owner: BuiltinTypeOwner,
    message: &'static str,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0417",
        format!(
            "invalid source surface for built-in type `{}`: {message}",
            owner.canonical_name()
        ),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "built-in surfaces must use their canonical target and public source-backed members"
            .to_string(),
    );
    diagnostic
}

fn empty_builtin_symbol(owner: BuiltinTypeOwner, instance: &InstanceDecl) -> TypeSymbol {
    let mut symbol = empty_builtin_type_symbol(owner);
    symbol.generic_parameters = instance
        .generics
        .parameters
        .iter()
        .map(|parameter| parameter.name.clone())
        .collect();
    symbol.generic_parameter_requirements = instance
        .generics
        .parameters
        .iter()
        .map(|parameter| {
            super::GenericRequirements::for_parameter(
                &parameter.name,
                instance.requirements.as_ref(),
            )
        })
        .collect();
    symbol.where_clause = instance.requirements.clone();
    symbol.generic_arity = instance.generics.parameters.len();
    symbol
}

pub(super) fn empty_builtin_type_symbol(owner: BuiltinTypeOwner) -> TypeSymbol {
    TypeSymbol {
        // The surface is kept outside the nominal symbol table. `Struct` only
        // selects the shared member container; authority controls which member
        // categories source may contribute for each compiler-owned type.
        kind: TypeSymbolKind::Struct,
        canonical_name: owner.canonical_name().to_string(),
        generic_parameters: Vec::new(),
        generic_parameter_requirements: Vec::new(),
        where_clause: None,
        generic_arity: 0,
        is_copy: false,
        alias_target: None,
        fields: Vec::new(),
        variants: Vec::new(),
        associated_types: Vec::new(),
        associated_functions: Vec::new(),
        methods: Vec::new(),
        interface_conformances: Vec::new(),
        destructor: None,
        literals: Vec::new(),
        coercions: Vec::new(),
        construction: ConstructionSurface::default(),
    }
}
