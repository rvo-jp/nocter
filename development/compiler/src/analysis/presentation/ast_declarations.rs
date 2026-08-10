//! Normalized syntax presentation used only when semantic resolution is unavailable.

use super::{CallablePresentation, LiteralPresentation};
use crate::ast::{
    CoercionEntry, DropDecl, EnumDecl, FunctionDecl, GenericParam, InterfaceDecl, LiteralDecl,
    LiteralShape, MethodDecl, Parameter, PrimitiveDecl, StructDecl, TypeAliasDecl,
};

pub(crate) fn ast_type_alias_presentation(alias: &TypeAliasDecl) -> String {
    nominal_type(
        "type",
        &alias.name,
        &alias.generics.parameters,
        false,
        Some(&alias.target),
        alias.requirements.as_ref(),
    )
}

pub(crate) fn ast_struct_presentation(struct_: &StructDecl) -> String {
    nominal_type(
        "struct",
        &struct_.name,
        &struct_.generics.parameters,
        struct_.is_copy,
        None,
        struct_.requirements.as_ref(),
    )
}

pub(crate) fn ast_enum_presentation(enum_: &EnumDecl) -> String {
    nominal_type(
        "enum",
        &enum_.name,
        &enum_.generics.parameters,
        false,
        None,
        enum_.requirements.as_ref(),
    )
}

pub(crate) fn ast_interface_presentation(interface: &InterfaceDecl) -> String {
    nominal_type(
        "interface",
        &interface.name,
        &interface.generics.parameters,
        false,
        None,
        interface.requirements.as_ref(),
    )
}

pub(crate) fn ast_function_presentation(function: &FunctionDecl) -> String {
    callable(
        "func",
        &function.name,
        &function.generics.parameters,
        &function.parameters.parameters,
        &function.return_type,
        function.result_provenance.as_ref(),
        function.requirements.as_ref(),
    )
}

pub(crate) fn ast_primitive_presentation(primitive: &PrimitiveDecl) -> String {
    callable(
        "primitive",
        &primitive.name,
        &primitive.generics.parameters,
        &primitive.parameters.parameters,
        &primitive.return_type,
        primitive.result_provenance.as_ref(),
        primitive.requirements.as_ref(),
    )
}

pub(crate) fn ast_method_presentation(method: &MethodDecl) -> String {
    callable(
        "method",
        &format!(
            "{}self.{}",
            method.receiver.mode.source_prefix(),
            method.name
        ),
        &method.generics.parameters,
        &method.parameters.parameters,
        &method.return_type,
        method.result_provenance.as_ref(),
        method.requirements.as_ref(),
    )
}

pub(crate) fn ast_coercion_presentation(entry: &CoercionEntry) -> String {
    let visibility = if entry.visibility.is_private() {
        String::new()
    } else {
        format!("{} ", entry.visibility.source_notation())
    };
    let provenance = super::result_origin_labels(entry.result_provenance.as_ref());
    let provenance = if provenance.is_empty() {
        String::new()
    } else {
        format!(" from {}", provenance.join(" | "))
    };
    format!(
        "{visibility}{}self as {}{provenance}",
        entry.receiver.mode.source_prefix(),
        crate::ast::canonical_type_expr(&entry.target),
    )
}

pub(crate) fn ast_literal_presentation(literal: &LiteralDecl) -> String {
    let parameters = if let Some(capture) = &literal.capture {
        vec![format!(
            "...{}: {}",
            capture.name,
            crate::ast::canonical_type_expr(&capture.element_type)
        )]
    } else {
        ast_parameter_labels(&literal.parameters.parameters)
    };
    LiteralPresentation::new(
        crate::ast::canonical_type_expr(&literal.target),
        match literal.shape {
            LiteralShape::Sequence => "[]",
            LiteralShape::String => "\"\"",
        },
        parameters,
        crate::ast::canonical_type_expr(&literal.return_type),
        super::result_origin_labels(literal.result_provenance.as_ref()),
    )
    .render()
}

pub(crate) fn ast_drop_presentation(drop_: &DropDecl) -> String {
    format!(
        "drop {}: {}",
        drop_.binding.name,
        crate::ast::canonical_type_expr(&drop_.binding.ty)
    )
}

pub(crate) fn ast_parameter_labels(parameters: &[Parameter]) -> Vec<String> {
    parameters
        .iter()
        .map(|parameter| {
            format!(
                "{}: {}",
                parameter.name,
                crate::ast::canonical_type_expr(&parameter.ty)
            )
        })
        .collect()
}

fn callable(
    kind: &str,
    name: &str,
    generics: &[GenericParam],
    parameters: &[Parameter],
    return_type: &crate::ast::TypeExpr,
    result_provenance: Option<&crate::ast::ResultProvenanceClause>,
    requirements: Option<&crate::ast::WhereClause>,
) -> String {
    CallablePresentation::new(
        kind,
        name,
        generics.iter().map(generic_label).collect(),
        ast_parameter_labels(parameters),
        crate::ast::canonical_type_expr(return_type),
        super::result_origin_labels(result_provenance),
        super::canonical_where_predicate_labels(requirements),
    )
    .render()
}

fn generic_label(parameter: &GenericParam) -> String {
    parameter.name.clone()
}

fn nominal_type(
    keyword: &str,
    name: &str,
    generics: &[GenericParam],
    is_copy: bool,
    target: Option<&crate::ast::TypeExpr>,
    requirements: Option<&crate::ast::WhereClause>,
) -> String {
    let generics = generics.iter().map(generic_label).collect::<Vec<_>>();
    let generics = if generics.is_empty() {
        String::new()
    } else {
        format!("<{}>", generics.join(", "))
    };
    let copy = if is_copy { "copy " } else { "" };
    let target = target
        .map(|target| format!(" = {}", crate::ast::canonical_type_expr(target)))
        .unwrap_or_default();
    let predicates = super::canonical_where_predicate_labels(requirements);
    let requirements = if predicates.is_empty() {
        String::new()
    } else {
        format!(" where {}", predicates.join(", "))
    };
    format!("{copy}{keyword} {name}{generics}{target}{requirements}")
}
