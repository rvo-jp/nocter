//! Resolved callable presentation for hover and LSP signature help.

use super::call_sites::{CallCursorRegion, call_at_offset};
use super::call_specializations::method_owner_substitutions_for_self_ty;
use super::{CompileUnitAnalysis, FileAnalysis};
use crate::ast::{
    CallExpr, ConstructMemberDecl, Expr, FunctionDecl, Item, MethodDecl, MethodOwnerDecl,
    MethodReceiver, Parameter, PrimitiveDecl, TypeExpr, substitute_type_expr_parameters,
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
    pub(crate) result_type: TypeExpr,
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
            result_type: literal.result_type,
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
    if let Some(fact) = file.typed_hir.callable_call(call.span) {
        return callable_value_signature_info(file, call, fact, offset);
    }
    let call_target = call_target(file, call)?;
    let declaration = callable_declaration(analysis, call_target)?;
    let mut substitutions = HashMap::new();

    let construction_owner = match &declaration {
        CallableDeclaration::Function(function) => function
            .owner
            .as_ref()
            .and_then(|owner| file.resolved.type_symbol_by_name(&owner.name))
            .filter(|owner| {
                crate::analysis::constructions::construction_owns_function(
                    owner,
                    &function.member_name,
                )
            }),
        _ => None,
    };

    if let Some(specialization) = file.typed_hir.function_call_specialization(call.span) {
        substitutions.extend(specialization.substitutions.clone());
    }

    if let Some(member_span) = call_member_span(call)
        && let Some(specialization) = file.typed_hir.method_call_specialization(member_span)
    {
        if let CallableDeclaration::Method { owner, .. } = declaration
            && let Some(owner_substitutions) =
                method_owner_substitutions_for_self_ty(owner, &specialization.self_ty)
        {
            substitutions.extend(owner_substitutions);
        }
        substitutions.extend(specialization.substitutions.clone());
        substitutions.insert("Self".to_string(), specialization.self_ty.clone());
    }

    match &declaration {
        CallableDeclaration::Function(function) => {
            if let Some(owner) = &function.owner {
                let self_ty = construction_owner
                    .map(|target| {
                        crate::analysis::presentation::owner_type_expr(target, owner.name_span)
                    })
                    .map(|ty| substitute_type_expr_parameters(&ty, &substitutions))
                    .unwrap_or_else(|| {
                        TypeExpr::Reference(crate::ast::TypeReference {
                            span: owner.name_span,
                            name: owner.name.clone(),
                        })
                    });
                substitutions.entry("Self".to_string()).or_insert(self_ty);
            }
        }
        CallableDeclaration::Method { owner, .. } => {
            substitutions
                .entry("Self".to_string())
                .or_insert_with(|| owner.target_ty().clone());
        }
        CallableDeclaration::Primitive(_) | CallableDeclaration::InterfaceMethod(_) => {}
    }

    let (
        kind,
        name,
        parameters,
        return_type,
        result_provenance,
        requirements,
        mut generic_parameters,
        receiver,
    ) = match declaration {
        CallableDeclaration::Function(function) => (
            "func",
            function.name.as_str(),
            function.parameters.parameters.as_slice(),
            &function.return_type,
            function.result_provenance.as_ref(),
            function.requirements.as_ref(),
            function.generics.parameters.iter().collect::<Vec<_>>(),
            None,
        ),
        CallableDeclaration::Primitive(primitive) => (
            "primitive",
            primitive.name.as_str(),
            primitive.parameters.parameters.as_slice(),
            &primitive.return_type,
            primitive.result_provenance.as_ref(),
            primitive.requirements.as_ref(),
            primitive.generics.parameters.iter().collect::<Vec<_>>(),
            None,
        ),
        CallableDeclaration::Method { method, .. } => (
            "method",
            method.name.as_str(),
            method.parameters.parameters.as_slice(),
            &method.return_type,
            method.result_provenance.as_ref(),
            method.requirements.as_ref(),
            method.generics.parameters.iter().collect::<Vec<_>>(),
            Some(&method.receiver),
        ),
        CallableDeclaration::InterfaceMethod(method) => (
            "method",
            method.name.as_str(),
            method.parameters.parameters.as_slice(),
            &method.return_type,
            method.result_provenance.as_ref(),
            method.requirements.as_ref(),
            method.generics.parameters.iter().collect::<Vec<_>>(),
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
    let constructed_name = construction_owner.map(|owner| {
        let owner_ty = crate::analysis::presentation::owner_type_expr(owner, call.span);
        let owner_ty = substitute_type_expr_parameters(&owner_ty, &substitutions);
        format!(
            "{}.{}",
            type_expr_presentation_label(&owner_ty, &file.resolved),
            name.rsplit('.').next().unwrap_or(name)
        )
    });
    if let Some(owner) = construction_owner {
        generic_parameters.drain(..owner.generic_parameters.len().min(generic_parameters.len()));
    }
    let display_name =
        constructed_name
            .as_deref()
            .unwrap_or_else(|| match call.callee.without_groups() {
                Expr::Identifier(identifier) => identifier.name.as_str(),
                _ => name,
            });
    let name = specialized_callable_name(
        display_name,
        &generic_parameters,
        &substitutions,
        &file.resolved,
    );
    let callable = match receiver {
        Some(receiver) => format!(
            "{}.{name}",
            receiver_presentation(receiver, &substitutions, &file.resolved)
        ),
        None => name,
    };
    let label = crate::analysis::presentation::CallablePresentation::new(
        kind,
        callable,
        Vec::new(),
        specialized_parameters
            .iter()
            .map(|(label, _)| label.clone())
            .collect(),
        return_label,
        crate::analysis::presentation::result_origin_labels(result_provenance),
        if substitutions.is_empty() {
            crate::analysis::presentation::where_predicate_labels(requirements, &file.resolved)
        } else {
            Vec::new()
        },
    )
    .render();

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
        result_type: return_type,
        is_specialized: !substitutions.is_empty(),
    })
}

fn callable_value_signature_info(
    file: &FileAnalysis,
    call: &CallExpr,
    fact: &crate::typecheck::CallableCallFact,
    offset: usize,
) -> Option<SignatureHelpInfo> {
    let signature = &fact.signature;
    let prefix = fact.specialization.capability.source_prefix();
    let callable_name = match call.callee.without_groups() {
        Expr::Identifier(identifier) => identifier.name.clone(),
        Expr::Member(member) => member.member.clone(),
        _ => "callback".to_string(),
    };
    let parameters = signature
        .parameters
        .iter()
        .map(|parameter| {
            let ty = parameter.ty.clone();
            let type_label = type_expr_presentation_label(&ty, &file.resolved);
            let label = if parameter.name.starts_with("argument") {
                type_label
            } else {
                format!("{}: {type_label}", parameter.name)
            };
            SignatureParameterInfo {
                label,
                documentation: None,
                ty,
            }
        })
        .collect::<Vec<_>>();
    let return_label = type_expr_presentation_label(&signature.return_type, &file.resolved);
    let label = crate::analysis::presentation::CallablePresentation::new(
        format!("{prefix}func"),
        callable_name,
        Vec::new(),
        parameters
            .iter()
            .map(|parameter| parameter.label.clone())
            .collect(),
        return_label,
        crate::analysis::presentation::result_origin_labels(signature.result_provenance.as_ref()),
        crate::analysis::presentation::where_predicate_labels(
            signature.where_clause.as_ref(),
            &file.resolved,
        ),
    )
    .render();
    Some(SignatureHelpInfo {
        label,
        parameters,
        active_parameter: active_parameter(call, offset, signature.parameters.len()),
        documentation: None,
        result_type: signature.return_type.clone(),
        is_specialized: false,
    })
}

fn call_target(file: &FileAnalysis, call: &CallExpr) -> Option<ByteSpan> {
    match call.callee.without_groups() {
        Expr::Identifier(identifier) => file.typed_hir.function_call_target(identifier.span),
        Expr::Member(member) => file
            .typed_hir
            .function_call_target(member.member_span)
            .or_else(|| file.typed_hir.method_call_target(member.member_span))
            .or_else(|| {
                file.typed_hir
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
        owner: &'a dyn MethodOwnerDecl,
        method: &'a MethodDecl,
    },
    InterfaceMethod(&'a MethodDecl),
}

impl CallableDeclaration<'_> {
    fn span(&self) -> ByteSpan {
        match self {
            Self::Function(function) => function.span,
            Self::Primitive(primitive) => primitive.span,
            Self::Method { method, .. } => method.span,
            Self::InterfaceMethod(method) => method.span,
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
        Item::Instance(_) | Item::Conformance(_) => item.method_owner().and_then(|owner| {
            owner.methods().find_map(|method| {
                (method.name_span == target)
                    .then_some(CallableDeclaration::Method { owner, method })
            })
        }),
        Item::Interface(interface) => interface
            .methods
            .iter()
            .find(|method| method.name_span == target)
            .map(CallableDeclaration::InterfaceMethod),
        Item::Construct(construct) => construct.members.iter().find_map(|member| {
            let ConstructMemberDecl::Function(function) = &member.declaration else {
                return None;
            };
            (function.name_span == target || function.member_name_span == target)
                .then_some(CallableDeclaration::Function(function))
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
    generic_parameters: &[&crate::ast::GenericParam],
    substitutions: &HashMap<String, TypeExpr>,
    resolved: &crate::resolve::ResolveOutput,
) -> String {
    if generic_parameters.is_empty() {
        return name.to_string();
    }
    let arguments = generic_parameters
        .iter()
        .map(|parameter| substitutions.get(&parameter.name))
        .collect::<Option<Vec<_>>>();
    let Some(arguments) = arguments else {
        let parameters = generic_parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        return format!("{name}<{parameters}>");
    };
    let arguments = arguments
        .into_iter()
        .map(|argument| type_expr_presentation_label(argument, resolved))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}<{arguments}>")
}

fn receiver_presentation(
    receiver: &MethodReceiver,
    substitutions: &HashMap<String, TypeExpr>,
    resolved: &crate::resolve::ResolveOutput,
) -> String {
    let owner = substitutions
        .get("Self")
        .map(|ty| type_expr_presentation_label(ty, resolved))
        .unwrap_or_else(|| "Self".to_string());
    format!("{}{owner}", receiver.mode.source_prefix())
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

construct Bucket<T> {
    pub default literal [](...items: T): Self {
        return Bucket<T> { length: items.len() }
    }
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
pub func identity<T>(value: T): T from value {
    return value
}
"#;
        let (sources, analysis) = analyze_namespace_import_text(root, module);
        let file = analysis.root_file().expect("expected root file");
        let offset = root.find("42").expect("expected argument");

        let signature = signature_help_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected signature help");

        assert_eq!(
            signature.label,
            "func identity<i32>(value: i32): i32 from value"
        );
        assert_eq!(signature.active_parameter, 0);
        assert_eq!(
            signature.documentation.as_deref(),
            Some("Returns its input.")
        );
    }

    #[test]
    fn presents_opaque_result_contract_without_witness() {
        let text = r#"interface Source {
    pub type Item
    pub method &self.get(): Self.Item
}
struct Number { value: i32 }
conform Source for Number {
    type Item = i32
    method &self.get(): i32 { return self.value }
}
func make(value: i32): some Source<Item = i32> {
    return Number { value: value }
}
func read(): i32 {
    let source = make(42)
    return source.get()
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("42").expect("expected call argument");
        let signature = signature_help_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected signature help");

        assert_eq!(
            signature.label,
            "func make(value: i32): some Source<Item = i32>"
        );
        assert!(!signature.label.contains("Number"));
    }

    #[test]
    fn presents_construct_function_generics_on_the_owner() {
        let text = r#"struct Bucket<T> { value: T }

construct Bucket<T> {
    pub default func new(value: T): Self {
        return Bucket<T> { value: value }
    }
}

func main(): i32 {
    let bucket = Bucket.new(42)
    return 0
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("42").expect("expected construct argument");

        let signature = signature_help_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected signature help");

        assert_eq!(
            signature.label,
            "func Bucket<i32>.new(value: i32): Bucket<i32>"
        );
        assert_eq!(signature.parameters[0].label, "value: i32");
    }

    #[test]
    fn presents_direct_builtin_callable_signature() {
        let text = r#"func invoke<F>(callback: F, input: i32): i32 where F: &+func(value: i32): i32 {
    var callable = move callback
    return callable(input)
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.rfind("input)").expect("expected callable argument");

        let signature = signature_help_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected callable signature help");

        assert_eq!(signature.label, "&+func callable(value: i32): i32");
        assert_eq!(signature.parameters[0].label, "value: i32");
    }

    #[test]
    fn presents_method_receiver_as_mode_and_specialized_owner() {
        let text = r#"struct Box<T> { value: T }

instance Box<T> {
    method &self.replace(value: T): T {
        return value
    }
}

func main(box: &Box<i32>): i32 {
    return box.replace(42)
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("42").expect("expected method argument");

        let signature = signature_help_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected signature help");

        assert_eq!(signature.label, "method &Box<i32>.replace(value: i32): i32");
        assert_eq!(signature.parameters[0].label, "value: i32");
    }

    #[test]
    fn presents_specialized_interface_default_method_generics() {
        let text = r#"interface Identity {
    pub method &self.keep<T>(value: T): T from value {
        return value
    }
}

copy struct Unit { marker: i32 }

conform Identity for Unit {}

func main(): i32 {
    let unit = Unit { marker: 0 }
    return unit.keep(42)
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("42").expect("expected default method argument");

        let signature = signature_help_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected default method signature help");

        assert_eq!(
            signature.label,
            "method &Unit.keep<i32>(value: i32): i32 from value"
        );
        assert_eq!(signature.parameters[0].label, "value: i32");
    }

    #[test]
    fn presents_bound_interface_method_with_specialized_contract() {
        let text = r#"interface Lookup<V> {
    pub method &self.get(fallback: &V): &V from self | fallback
}

func read<M>(map: &M, fallback: &i32): &i32 from map | fallback where M: Lookup<i32> {
    return map.get(fallback)
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.rfind("fallback)").expect("expected call argument");

        let signature = signature_help_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected bound method signature help");

        assert_eq!(
            signature.label,
            "method &M.get(fallback: &i32): &i32 from self | fallback"
        );
        assert_eq!(signature.parameters[0].label, "fallback: &i32");
    }

    #[test]
    fn presents_every_bound_when_generic_arguments_are_not_inferred() {
        let text = r#"interface Readable {}
interface Measurable {}

func inspect<T>(value: i32): i32 where T: Readable + Measurable {
    return value
}

func main(): i32 {
    return inspect(42)
}
"#;
        let (sources, analysis) = analyze_text(text);
        let file = analysis.root_file().expect("expected root file");
        let offset = text.find("42").expect("expected call argument");

        let signature = signature_help_for_file_analysis(&sources, &analysis, file, offset)
            .expect("expected signature help");

        assert_eq!(
            signature.label,
            "func inspect<T>(value: i32): i32 where T: Readable + Measurable"
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
