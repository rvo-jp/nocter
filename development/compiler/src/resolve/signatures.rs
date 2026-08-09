use super::GenericRequirements;
use super::{
    AssociatedFunctionSignature, ConstructionEntry, ConstructionEntryKind, ConstructionSurface,
    DropSignature, EnumVariantSignature, FunctionSignature, MethodSignature, ParameterSignature,
    StructFieldSignature, TypeSymbol, TypeSymbolKind,
};
use crate::ast::{
    AstFile, FunctionDecl, GenericParamList, ImplDecl, ImplMember, InterfaceDecl, MethodDecl,
    Parameter, PrimitiveDecl, StructField, TypeAliasDecl, TypeExpr,
};
use crate::diagnostics::Diagnostic;
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashMap;

use super::conformance::interface_conformance;
use super::diagnostics::duplicate_inherent_member_name_diagnostic;

pub(super) fn attach_inherent_impl_members_to_symbol(
    symbol: &mut TypeSymbol,
    ast: &AstFile,
    type_name: &str,
) {
    if !type_symbol_accepts_inherent_impl(symbol) {
        return;
    }

    symbol
        .interface_conformances
        .extend(ast.items.iter().filter_map(|item| {
            let crate::ast::Item::Impl(impl_) = item else {
                return None;
            };
            (impl_target_type_name(&impl_.target_ty) == Some(type_name))
                .then(|| interface_conformance(impl_))
                .flatten()
        }));

    symbol
        .associated_functions
        .extend(top_level_associated_function_signatures(ast, type_name));

    for item in &ast.items {
        let crate::ast::Item::Impl(impl_) = item else {
            continue;
        };
        if impl_.interface_ty.is_some()
            || impl_target_type_name(&impl_.target_ty) != Some(type_name)
        {
            continue;
        }

        symbol.methods.extend(method_signatures(impl_));
        if symbol.drop_member.is_none() {
            symbol.drop_member = drop_signature(impl_);
        }
    }
}

pub(super) fn type_symbol_accepts_inherent_impl(symbol: &TypeSymbol) -> bool {
    matches!(symbol.kind, TypeSymbolKind::Struct | TypeSymbolKind::Enum)
        || type_symbol_is_error_alias(symbol)
}

fn type_symbol_is_error_alias(symbol: &TypeSymbol) -> bool {
    if !matches!(symbol.kind, TypeSymbolKind::Alias) {
        return false;
    }

    matches!(
        symbol.alias_target.as_ref(),
        Some(TypeExpr::Reference(reference)) if reference.name == "error"
    )
}

pub(super) fn top_level_associated_function_signatures<'a>(
    ast: &'a AstFile,
    type_name: &'a str,
) -> impl Iterator<Item = AssociatedFunctionSignature> + 'a {
    ast.items.iter().flat_map(move |item| {
        let functions: Box<dyn Iterator<Item = &FunctionDecl>> = match item {
            crate::ast::Item::Function(function) => Box::new(std::iter::once(function)),
            crate::ast::Item::Construct(construct) => {
                Box::new(construct.functions().map(|(_, function)| function))
            }
            _ => Box::new(std::iter::empty()),
        };
        functions.filter_map(move |function| {
            let owner = function.owner.as_ref()?;
            (owner.name == type_name).then(|| associated_function_signature(function))
        })
    })
}

pub(super) fn associated_function_signature(
    function: &FunctionDecl,
) -> AssociatedFunctionSignature {
    AssociatedFunctionSignature {
        name: function.member_name.clone(),
        target_name: function.name.clone(),
        name_span: function.member_name_span,
        visibility: function.visibility,
        is_accessible: true,
        signature: function_signature(function),
    }
}

pub(super) fn method_signatures(impl_: &ImplDecl) -> impl Iterator<Item = MethodSignature> + '_ {
    impl_.members.iter().filter_map(|member| match member {
        ImplMember::Method(method) => Some(method_signature_in_impl(method, impl_)),
        ImplMember::AssociatedType(_) | ImplMember::Drop(_) => None,
    })
}

pub(super) fn drop_signature(impl_: &ImplDecl) -> Option<DropSignature> {
    let target_name = impl_target_type_name(&impl_.target_ty)?;
    impl_.members.iter().find_map(|member| match member {
        ImplMember::Drop(drop_) => Some(DropSignature {
            name_span: drop_.name_span,
            target_name: drop_function_name(target_name),
            binding: parameter_signature(&drop_.binding),
        }),
        ImplMember::AssociatedType(_) | ImplMember::Method(_) => None,
    })
}

pub(super) fn duplicate_inherent_member_name_diagnostics(
    sources: &SourceMap,
    target_name: &str,
    type_symbol: &TypeSymbol,
    impl_: &ImplDecl,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut seen = HashMap::<&str, ByteSpan>::new();
    for variant in &type_symbol.variants {
        seen.entry(variant.name.as_str())
            .or_insert(variant.name_span);
    }
    for function in &type_symbol.associated_functions {
        seen.entry(function.name.as_str())
            .or_insert(function.name_span);
    }
    for method in &type_symbol.methods {
        seen.entry(method.name.as_str()).or_insert(method.name_span);
    }

    for member in &impl_.members {
        let (name, span) = match member {
            ImplMember::Method(method) => (method.name.as_str(), method.name_span),
            ImplMember::AssociatedType(_) | ImplMember::Drop(_) => continue,
        };
        match seen.entry(name) {
            std::collections::hash_map::Entry::Occupied(first) => {
                diagnostics.push(duplicate_inherent_member_name_diagnostic(
                    sources,
                    target_name,
                    name,
                    *first.get(),
                    span,
                ));
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(span);
            }
        }
    }

    diagnostics
}

pub(super) fn duplicate_inherent_drop_diagnostics(
    sources: &SourceMap,
    target_name: &str,
    type_symbol: &TypeSymbol,
    impl_: &ImplDecl,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let Some(drop_) = impl_.members.iter().find_map(|member| match member {
        ImplMember::Drop(drop_) => Some(drop_),
        ImplMember::AssociatedType(_) | ImplMember::Method(_) => None,
    }) else {
        return diagnostics;
    };

    if let Some(existing) = &type_symbol.drop_member {
        diagnostics.push(duplicate_inherent_member_name_diagnostic(
            sources,
            target_name,
            "drop",
            existing.name_span,
            drop_.name_span,
        ));
    }

    diagnostics
}

pub(super) fn impl_target_type_name(ty: &TypeExpr) -> Option<&str> {
    match ty {
        TypeExpr::Callable(_) | TypeExpr::Closure(_) | TypeExpr::Projection(_) => None,
        TypeExpr::Reference(reference) => Some(&reference.name),
        TypeExpr::Generic(generic) => Some(&generic.name),
        TypeExpr::Pointer(_)
        | TypeExpr::Borrow(_)
        | TypeExpr::View(_)
        | TypeExpr::Array(_)
        | TypeExpr::Optional(_)
        | TypeExpr::Fallible(_) => None,
    }
}

pub(crate) fn drop_function_name(type_name: &str) -> String {
    format!("{type_name}.drop")
}

pub(super) fn function_signature(function: &FunctionDecl) -> FunctionSignature {
    callable_signature(
        &function.generics,
        &function.parameters.parameters,
        function.return_type.clone(),
        function.result_provenance.clone(),
        function.requirements.as_ref(),
    )
}

pub(super) fn primitive_signature(primitive: &PrimitiveDecl) -> FunctionSignature {
    callable_signature(
        &primitive.generics,
        &primitive.parameters.parameters,
        primitive.return_type.clone(),
        primitive.result_provenance.clone(),
        primitive.requirements.as_ref(),
    )
}

pub(super) fn alias_type_symbol(alias: &TypeAliasDecl) -> TypeSymbol {
    TypeSymbol {
        kind: TypeSymbolKind::Alias,
        canonical_name: alias.name.clone(),
        generic_parameters: alias
            .generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect(),
        generic_parameter_requirements: alias
            .generics
            .parameters
            .iter()
            .map(GenericRequirements::from_parameter)
            .collect(),
        generic_arity: alias.generics.parameters.len(),
        is_copy: false,
        alias_target: Some(alias.target.clone()),
        fields: Vec::new(),
        variants: Vec::new(),
        associated_types: Vec::new(),
        associated_functions: Vec::new(),
        methods: Vec::new(),
        interface_conformances: Vec::new(),
        drop_member: None,
        literals: Vec::new(),
        coercions: Vec::new(),
        construction: ConstructionSurface::default(),
    }
}

pub(super) fn interface_type_symbol(interface: &InterfaceDecl) -> TypeSymbol {
    TypeSymbol {
        kind: TypeSymbolKind::Interface,
        canonical_name: interface.name.clone(),
        generic_parameters: interface
            .generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect(),
        generic_parameter_requirements: interface
            .generics
            .parameters
            .iter()
            .map(GenericRequirements::from_parameter)
            .collect(),
        generic_arity: interface.generics.parameters.len(),
        is_copy: false,
        alias_target: None,
        fields: Vec::new(),
        variants: Vec::new(),
        associated_types: interface
            .associated_types
            .iter()
            .map(|associated_type| super::AssociatedTypeSignature {
                name: associated_type.name.clone(),
                name_span: associated_type.name_span,
                declaration_span: associated_type.span,
                requirements: GenericRequirements::from_bounds(&associated_type.bounds),
            })
            .collect(),
        associated_functions: Vec::new(),
        methods: interface
            .methods
            .iter()
            .map(|method| method_signature_inner(method, None, &interface.generics))
            .collect(),
        interface_conformances: Vec::new(),
        drop_member: None,
        literals: Vec::new(),
        coercions: Vec::new(),
        construction: ConstructionSurface::default(),
    }
}

pub(super) fn struct_type_symbol(
    struct_: &crate::ast::StructDecl,
    is_copy: bool,
    fields: &[StructField],
) -> TypeSymbol {
    TypeSymbol {
        kind: TypeSymbolKind::Struct,
        canonical_name: struct_.name.clone(),
        generic_parameters: struct_
            .generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect(),
        generic_parameter_requirements: struct_
            .generics
            .parameters
            .iter()
            .map(GenericRequirements::from_parameter)
            .collect(),
        generic_arity: struct_.generics.parameters.len(),
        is_copy,
        alias_target: None,
        fields: fields
            .iter()
            .map(|field| StructFieldSignature {
                name: field.name.clone(),
                name_span: field.name_span,
                visibility: field.visibility,
                is_accessible: true,
                ty: field.ty.clone(),
            })
            .collect(),
        variants: Vec::new(),
        associated_types: Vec::new(),
        associated_functions: Vec::new(),
        methods: Vec::new(),
        interface_conformances: Vec::new(),
        drop_member: None,
        literals: Vec::new(),
        coercions: Vec::new(),
        construction: {
            let mut surface = ConstructionSurface::default();
            surface.entries.push(ConstructionEntry {
                kind: ConstructionEntryKind::Structural,
                declaration_span: struct_.span,
                focus_span: struct_.name_span,
                is_accessible: true,
            });
            surface.default_entry = Some(0);
            surface
        },
    }
}

pub(super) fn enum_type_symbol(enum_: &crate::ast::EnumDecl) -> TypeSymbol {
    TypeSymbol {
        kind: TypeSymbolKind::Enum,
        canonical_name: enum_.name.clone(),
        generic_parameters: enum_
            .generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect(),
        generic_parameter_requirements: enum_
            .generics
            .parameters
            .iter()
            .map(GenericRequirements::from_parameter)
            .collect(),
        generic_arity: enum_.generics.parameters.len(),
        is_copy: false,
        alias_target: None,
        fields: Vec::new(),
        variants: enum_
            .variants
            .iter()
            .map(|variant| EnumVariantSignature {
                name: variant.name.clone(),
                name_span: variant.name_span,
                payload: variant.payload.iter().map(parameter_signature).collect(),
            })
            .collect(),
        associated_types: Vec::new(),
        associated_functions: Vec::new(),
        methods: Vec::new(),
        interface_conformances: Vec::new(),
        drop_member: None,
        literals: Vec::new(),
        coercions: Vec::new(),
        construction: ConstructionSurface {
            declaration_span: None,
            entries: enum_
                .variants
                .iter()
                .map(|variant| ConstructionEntry {
                    kind: ConstructionEntryKind::Variant(variant.name.clone()),
                    declaration_span: variant.span,
                    focus_span: variant.name_span,
                    is_accessible: true,
                })
                .collect(),
            default_entry: None,
        },
    }
}

fn method_signature_in_impl(method: &MethodDecl, impl_: &ImplDecl) -> MethodSignature {
    method_signature_inner(method, Some(impl_.target_ty.clone()), &impl_.generics)
}

fn method_signature_inner(
    method: &MethodDecl,
    impl_target_ty: Option<TypeExpr>,
    generics: &GenericParamList,
) -> MethodSignature {
    let has_default_body = method.body.is_some() && impl_target_ty.is_none();
    MethodSignature {
        name: method.name.clone(),
        name_span: method.name_span,
        visibility: method.visibility,
        is_accessible: true,
        impl_target_ty,
        has_default_body,
        owner_generic_count: generics.parameters.len(),
        receiver: method.receiver.clone(),
        signature: method_callable_signature(
            generics,
            &method.generics,
            &method.parameters.parameters,
            method.return_type.clone(),
            method.result_provenance.clone(),
            method.requirements.as_ref(),
        ),
    }
}

fn method_callable_signature(
    owner_generics: &GenericParamList,
    method_generics: &GenericParamList,
    parameters: &[Parameter],
    return_type: TypeExpr,
    result_provenance: Option<crate::ast::ResultProvenanceClause>,
    requirements: Option<&crate::ast::WhereClause>,
) -> FunctionSignature {
    let generic_parameters = owner_generics
        .parameters
        .iter()
        .chain(&method_generics.parameters)
        .collect::<Vec<_>>();
    FunctionSignature {
        generic_parameters: generic_parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect(),
        generic_parameter_requirements: generic_parameters
            .iter()
            .map(|parameter| {
                let mut resolved = GenericRequirements::from_parameter(parameter);
                resolved.extend_from_clause(&parameter.name, requirements);
                resolved
            })
            .collect(),
        where_clause: requirements.cloned(),
        parameters: parameters.iter().map(parameter_signature).collect(),
        return_type,
        result_provenance,
    }
}

fn callable_signature(
    generics: &GenericParamList,
    parameters: &[Parameter],
    return_type: TypeExpr,
    result_provenance: Option<crate::ast::ResultProvenanceClause>,
    requirements: Option<&crate::ast::WhereClause>,
) -> FunctionSignature {
    FunctionSignature {
        generic_parameters: generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect(),
        generic_parameter_requirements: generics
            .parameters
            .iter()
            .map(|parameter| {
                let mut resolved = GenericRequirements::from_parameter(parameter);
                resolved.extend_from_clause(&parameter.name, requirements);
                resolved
            })
            .collect(),
        where_clause: requirements.cloned(),
        parameters: parameters.iter().map(parameter_signature).collect(),
        return_type,
        result_provenance,
    }
}

fn parameter_signature(parameter: &Parameter) -> ParameterSignature {
    ParameterSignature {
        name: parameter.name.clone(),
        name_span: parameter.name_span,
        ty: parameter.ty.clone(),
    }
}
