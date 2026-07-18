use super::{
    AssociatedFunctionSignature, DropSignature, EnumVariantSignature, FunctionSignature,
    MethodSignature, ParameterSignature, StructFieldSignature, TypeSymbol, TypeSymbolKind,
};
use crate::ast::{
    AstFile, FunctionDecl, GenericParamList, ImplDecl, ImplMember, InterfaceDecl, MethodDecl,
    Parameter, PrimitiveDecl, StructField, TypeAliasDecl, TypeExpr,
};
use crate::diagnostics::Diagnostic;
use crate::source::{ByteSpan, SourceMap};
use std::collections::HashMap;

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
    ast.items.iter().filter_map(move |item| {
        let crate::ast::Item::Function(function) = item else {
            return None;
        };
        let owner = function.owner.as_ref()?;
        (owner.name == type_name).then(|| associated_function_signature(function))
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
        ImplMember::Method(method) => Some(method_signature(method)),
        ImplMember::Drop(_) => None,
    })
}

pub(super) fn drop_signature(impl_: &ImplDecl) -> Option<DropSignature> {
    let target_name = impl_target_type_name(&impl_.target_ty)?;
    impl_.members.iter().find_map(|member| match member {
        ImplMember::Drop(drop_) => Some(DropSignature {
            name_span: drop_name_span(drop_.span),
            target_name: drop_function_name(target_name),
            binding: parameter_signature(&drop_.binding),
        }),
        ImplMember::Method(_) => None,
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
            ImplMember::Drop(_) => continue,
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
        ImplMember::Method(_) => None,
    }) else {
        return diagnostics;
    };

    if let Some(existing) = &type_symbol.drop_member {
        diagnostics.push(duplicate_inherent_member_name_diagnostic(
            sources,
            target_name,
            "drop",
            existing.name_span,
            drop_name_span(drop_.span),
        ));
    }

    diagnostics
}

pub(super) fn impl_target_type_name(ty: &TypeExpr) -> Option<&str> {
    let TypeExpr::Reference(reference) = ty else {
        return None;
    };

    Some(&reference.name)
}

pub(crate) fn drop_function_name(type_name: &str) -> String {
    format!("{type_name}.drop")
}

fn drop_name_span(span: ByteSpan) -> ByteSpan {
    ByteSpan::new(span.source, span.start, span.start + "drop".len())
}

pub(super) fn function_signature(function: &FunctionDecl) -> FunctionSignature {
    callable_signature(
        &function.generics,
        &function.parameters.parameters,
        function.return_type.clone(),
    )
}

pub(super) fn primitive_signature(primitive: &PrimitiveDecl) -> FunctionSignature {
    callable_signature(
        &primitive.generics,
        &primitive.parameters.parameters,
        primitive.return_type.clone(),
    )
}

pub(super) fn alias_type_symbol(alias: &TypeAliasDecl) -> TypeSymbol {
    TypeSymbol {
        kind: TypeSymbolKind::Alias,
        canonical_name: alias.name.clone(),
        generic_arity: alias.generics.parameters.len(),
        is_copy: false,
        alias_target: Some(alias.target.clone()),
        fields: Vec::new(),
        variants: Vec::new(),
        associated_functions: Vec::new(),
        methods: Vec::new(),
        drop_member: None,
    }
}

pub(super) fn interface_type_symbol(interface: &InterfaceDecl) -> TypeSymbol {
    TypeSymbol {
        kind: TypeSymbolKind::Interface,
        canonical_name: interface.name.clone(),
        generic_arity: interface.generics.parameters.len(),
        is_copy: false,
        alias_target: None,
        fields: Vec::new(),
        variants: Vec::new(),
        associated_functions: Vec::new(),
        methods: interface.methods.iter().map(method_signature).collect(),
        drop_member: None,
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
        associated_functions: Vec::new(),
        methods: Vec::new(),
        drop_member: None,
    }
}

pub(super) fn enum_type_symbol(enum_: &crate::ast::EnumDecl) -> TypeSymbol {
    TypeSymbol {
        kind: TypeSymbolKind::Enum,
        canonical_name: enum_.name.clone(),
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
        associated_functions: Vec::new(),
        methods: Vec::new(),
        drop_member: None,
    }
}

fn method_signature(method: &MethodDecl) -> MethodSignature {
    MethodSignature {
        name: method.name.clone(),
        name_span: method.name_span,
        visibility: method.visibility,
        is_accessible: true,
        receiver: parameter_signature(&method.receiver),
        signature: callable_signature(
            &GenericParamList::empty(),
            &method.parameters.parameters,
            method.return_type.clone(),
        ),
    }
}

fn callable_signature(
    generics: &GenericParamList,
    parameters: &[Parameter],
    return_type: TypeExpr,
) -> FunctionSignature {
    FunctionSignature {
        generic_parameters: generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect(),
        parameters: parameters.iter().map(parameter_signature).collect(),
        return_type,
    }
}

fn parameter_signature(parameter: &Parameter) -> ParameterSignature {
    ParameterSignature {
        name: parameter.name.clone(),
        name_span: parameter.name_span,
        ty: parameter.ty.clone(),
    }
}
