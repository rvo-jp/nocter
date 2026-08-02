//! Resolved callable presentation for hover and LSP signature help.

use super::call_sites::{CallCursorRegion, call_at_offset};
use super::call_specializations::impl_substitutions_for_self_ty;
use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{
    CallExpr, Expr, FunctionDecl, ImplDecl, ImplMember, Item, MethodDecl, Parameter, PrimitiveDecl,
    TypeExpr, substitute_type_expr_parameters,
};
use crate::comments::{DocumentationTarget, attach_documentation};
use crate::source::{ByteSpan, SourceMap};
use crate::typecheck::type_expr_presentation_label;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SignatureHelpInfo {
    pub(crate) label: String,
    pub(crate) parameters: Vec<SignatureParameterInfo>,
    pub(crate) active_parameter: usize,
    pub(crate) documentation: Option<String>,
    pub(crate) is_specialized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SignatureParameterInfo {
    pub(crate) label: String,
    pub(crate) documentation: Option<String>,
    pub(crate) ty: TypeExpr,
}

pub(crate) fn signature_help_for_file_analysis(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<SignatureHelpInfo> {
    if let Some(literal) = crate::analysis::literals::literal_editor_info_at_offset(
        analysis,
        file,
        offset,
        crate::analysis::literals::LiteralCursorRegion::Arguments,
    ) {
        return Some(SignatureHelpInfo {
            label: literal.label,
            parameters: literal
                .parameters
                .into_iter()
                .map(|parameter| SignatureParameterInfo {
                    label: parameter.label,
                    documentation: None,
                    ty: parameter.ty,
                })
                .collect(),
            active_parameter: 0,
            documentation: crate::analysis::hover::target_documentation(
                sources,
                analysis,
                literal.declaration_shape_span,
            ),
            is_specialized: literal.is_specialized,
        });
    }
    let call = call_at_offset(&file.ast, offset, CallCursorRegion::Arguments)?;
    signature_info_for_call(sources, analysis, file, call, offset)
}

pub(crate) fn call_signature_at_offset(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<SignatureHelpInfo> {
    let call = call_at_offset(&file.ast, offset, CallCursorRegion::FullCall)?;
    signature_info_for_call(sources, analysis, file, call, offset)
}

fn signature_info_for_call(
    sources: &SourceMap,
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    call: &CallExpr,
    offset: usize,
) -> Option<SignatureHelpInfo> {
    let call_target = call_target(file, call)?;
    let declaration = callable_declaration(analysis, call_target)?;
    let mut substitutions = HashMap::new();

    if let Some(specialization) = file.typecheck_facts.function_call_specialization(call.span) {
        substitutions.extend(specialization.substitutions.clone());
    }

    if let Some(member_span) = call_member_span(call)
        && let Some(specialization) = file.typecheck_facts.method_call_specialization(member_span)
    {
        if let CallableDeclaration::Method { impl_, .. } = declaration
            && let Some(impl_substitutions) =
                impl_substitutions_for_self_ty(impl_, &specialization.self_ty)
        {
            substitutions.extend(impl_substitutions);
        }
        substitutions.extend(specialization.substitutions.clone());
        substitutions.insert("Self".to_string(), specialization.self_ty.clone());
    }

    let (kind, name, parameters, return_type, generic_parameters, receiver) = match declaration {
        CallableDeclaration::Function(function) => (
            "func",
            function.name.as_str(),
            function.parameters.parameters.as_slice(),
            &function.return_type,
            function
                .generics
                .parameters
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<Vec<_>>(),
            None,
        ),
        CallableDeclaration::Primitive(primitive) => (
            "primitive",
            primitive.name.as_str(),
            primitive.parameters.parameters.as_slice(),
            &primitive.return_type,
            primitive
                .generics
                .parameters
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<Vec<_>>(),
            None,
        ),
        CallableDeclaration::Method { method, .. } => (
            "method",
            method.name.as_str(),
            method.parameters.parameters.as_slice(),
            &method.return_type,
            Vec::new(),
            Some(&method.receiver),
        ),
    };

    let specialized_parameters = parameters
        .iter()
        .map(|parameter| {
            (
                parameter_label(parameter, &substitutions, &file.resolved),
                substitute_type_expr_parameters(&parameter.ty, &substitutions),
            )
        })
        .collect::<Vec<_>>();
    let return_type = substitute_type_expr_parameters(return_type, &substitutions);
    let return_label = type_expr_presentation_label(&return_type, &file.resolved);
    let name = specialized_callable_name(name, &generic_parameters, &substitutions, &file.resolved);
    let callable = match receiver {
        Some(receiver) => format!(
            "{}.{name}",
            receiver_presentation(receiver, &substitutions, &file.resolved)
        ),
        None => name,
    };
    let label = format!(
        "{kind} {callable}({}): {return_label}",
        specialized_parameters
            .iter()
            .map(|(label, _)| label.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    Some(SignatureHelpInfo {
        label,
        parameters: specialized_parameters
            .into_iter()
            .map(|(label, ty)| SignatureParameterInfo {
                label,
                documentation: None,
                ty,
            })
            .collect(),
        active_parameter: active_parameter(call, offset, parameters.len()),
        documentation: callable_documentation(sources, declaration, call_target),
        is_specialized: !substitutions.is_empty(),
    })
}

fn call_target(file: &FileAnalysis, call: &CallExpr) -> Option<ByteSpan> {
    match call.callee.without_groups() {
        Expr::Identifier(identifier) => file.typecheck_facts.function_call_target(identifier.span),
        Expr::Member(member) => file
            .typecheck_facts
            .function_call_target(member.member_span)
            .or_else(|| file.typecheck_facts.method_call_target(member.member_span))
            .or_else(|| {
                file.typecheck_facts
                    .associated_function_target(member.member_span)
            }),
        _ => None,
    }
}

fn call_member_span(call: &CallExpr) -> Option<ByteSpan> {
    let Expr::Member(member) = call.callee.without_groups() else {
        return None;
    };
    Some(member.member_span)
}

enum CallableDeclaration<'a> {
    Function(&'a FunctionDecl),
    Primitive(&'a PrimitiveDecl),
    Method {
        impl_: &'a ImplDecl,
        method: &'a MethodDecl,
    },
}

impl CallableDeclaration<'_> {
    fn span(&self) -> ByteSpan {
        match self {
            Self::Function(function) => function.span,
            Self::Primitive(primitive) => primitive.span,
            Self::Method { method, .. } => method.span,
        }
    }
}

fn callable_declaration(
    analysis: &CompileUnitAnalysis,
    target: ByteSpan,
) -> Option<CallableDeclaration<'_>> {
    let file = analysis.file_by_source(target.source)?;
    file.ast.items.iter().find_map(|item| match item {
        Item::Function(function)
            if function.name_span == target || function.member_name_span == target =>
        {
            Some(CallableDeclaration::Function(function))
        }
        Item::Primitive(primitive) if primitive.name_span == target => {
            Some(CallableDeclaration::Primitive(primitive))
        }
        Item::Impl(impl_) => impl_.members.iter().find_map(|member| {
            let ImplMember::Method(method) = member else {
                return None;
            };
            (method.name_span == target).then_some(CallableDeclaration::Method { impl_, method })
        }),
        _ => None,
    })
}

fn callable_documentation(
    sources: &SourceMap,
    declaration: CallableDeclaration<'_>,
    target: ByteSpan,
) -> Option<String> {
    let source = sources.get(target.source)?;
    let text = source.text();
    let documentation = attach_documentation(
        target.source,
        text,
        &[DocumentationTarget::new(
            declaration_line_start(text, declaration.span().start),
            target.start,
        )],
    );
    documentation.get(target.start).map(str::to_string)
}

fn declaration_line_start(text: &str, node_start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut line_start = node_start.min(bytes.len());
    while line_start > 0 && bytes[line_start - 1] != b'\n' {
        line_start -= 1;
    }
    while line_start < node_start && matches!(bytes[line_start], b' ' | b'\t') {
        line_start += 1;
    }
    line_start
}

fn parameter_label(
    parameter: &Parameter,
    substitutions: &HashMap<String, TypeExpr>,
    resolved: &crate::resolve::ResolveOutput,
) -> String {
    let ty = substitute_type_expr_parameters(&parameter.ty, substitutions);
    format!(
        "{}: {}",
        parameter.name,
        type_expr_presentation_label(&ty, resolved)
    )
}

fn specialized_callable_name(
    name: &str,
    generic_parameters: &[&str],
    substitutions: &HashMap<String, TypeExpr>,
    resolved: &crate::resolve::ResolveOutput,
) -> String {
    if generic_parameters.is_empty() {
        return name.to_string();
    }
    let arguments = generic_parameters
        .iter()
        .map(|parameter| substitutions.get(*parameter))
        .collect::<Option<Vec<_>>>();
    let Some(arguments) = arguments else {
        return name.to_string();
    };
    let arguments = arguments
        .into_iter()
        .map(|argument| type_expr_presentation_label(argument, resolved))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}<{arguments}>")
}

fn receiver_presentation(
    receiver: &Parameter,
    substitutions: &HashMap<String, TypeExpr>,
    resolved: &crate::resolve::ResolveOutput,
) -> String {
    let ty = substitute_type_expr_parameters(&receiver.ty, substitutions);
    match ty {
        TypeExpr::Borrow(borrow) if borrow.is_readwrite => format!(
            "&+{}",
            type_expr_presentation_label(&borrow.inner, resolved)
        ),
        TypeExpr::Borrow(borrow) => {
            format!("&{}", type_expr_presentation_label(&borrow.inner, resolved))
        }
        ty => type_expr_presentation_label(&ty, resolved),
    }
}

fn active_parameter(call: &CallExpr, offset: usize, parameter_count: usize) -> usize {
    if parameter_count == 0 {
        return 0;
    }
    for (index, argument) in call.arguments.iter().enumerate() {
        if offset <= argument.span().end {
            return index.min(parameter_count - 1);
        }
    }
    call.arguments.len().min(parameter_count - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::test_support::analyze_namespace_import_text;
    use crate::analysis::test_support::analyze_text;

    #[test]
    fn presents_sequence_literal_element_pack_signature() {
        let text = r#"struct Bucket<T> { length: usize }

literal Bucket<T> [](...items: T): Self {
    return Bucket<T> { length: items.len() }
}

func main(): i32 {
    let values = Bucket [20, 22]
    return 0
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("22]").unwrap();

        let signature = signature_help_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected signature help");

        assert_eq!(
            signature.label,
            "literal Bucket<i32> [](...items: i32): Bucket<i32>"
        );
        assert_eq!(signature.parameters[0].label, "...items: i32");
        assert_eq!(signature.active_parameter, 0);
    }

    #[test]
    fn presents_imported_generic_call_with_specialized_types() {
        let root = r#"use lib/math

func main(): i32 {
    return math.identity(42)
}
"#;
        let module = r#"/// Returns its input.
pub func identity<T>(value: T): T {
    return value
}
"#;
        let (sources, analysis) = analyze_namespace_import_text(root, module);
        let file = analysis.root_file().expect("expected root file");
        let offset = root.find("42").expect("expected argument");

        let signature = signature_help_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected signature help");

        assert_eq!(signature.label, "func identity<i32>(value: i32): i32");
        assert_eq!(signature.active_parameter, 0);
        assert_eq!(
            signature.documentation.as_deref(),
            Some("Returns its input.")
        );
    }

    #[test]
    fn selects_active_parameter_between_arguments() {
        let root = r#"use lib/math

func main(): i32 {
    return math.add(20, 22)
}
"#;
        let module = r#"pub func add(left: i32, right: i32): i32 {
    return left + right
}
"#;
        let (sources, analysis) = analyze_namespace_import_text(root, module);
        let file = analysis.root_file().expect("expected root file");
        let offset = root.find("22").expect("expected second argument");

        let signature = signature_help_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected signature help");

        assert_eq!(signature.active_parameter, 1);
        assert_eq!(
            signature
                .parameters
                .iter()
                .map(|parameter| parameter.label.as_str())
                .collect::<Vec<_>>(),
            vec!["left: i32", "right: i32"]
        );
    }
}
