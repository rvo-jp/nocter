mod value;
mod walk;

use super::environments::{environment_for_literal, function_self_type, impl_self_type};
use super::model::Type;
use super::type_expr::type_expr_to_type_with_self_type;
use super::{copyability::type_expr_is_copy, diagnostics::copy_struct_field_not_copy_diagnostic};
use crate::ast::{
    AstFile, FunctionDecl, ImplDecl, ImplMember, InterfaceDecl, Item, MethodDecl, Parameter,
    PrimitiveDecl,
};
use crate::diagnostics::Diagnostic;
use crate::resolve::ResolveOutput;
use crate::source::SourceMap;

use value::check_value_type;
use walk::check_block;

pub(super) fn check_sized_value_types(
    sources: &SourceMap,
    ast: &AstFile,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in &ast.items {
        match item {
            Item::Function(function) => {
                let self_type = function_self_type(function, resolved);
                check_function(sources, function, resolved, self_type.as_ref(), diagnostics);
            }
            Item::Test(test) => {
                check_block(sources, &test.body, resolved, None, diagnostics);
            }
            Item::Primitive(primitive) => {
                check_primitive(sources, primitive, resolved, diagnostics);
            }
            Item::Struct(struct_) => {
                for field in &struct_.fields {
                    let subject = format!("struct field `{}.{}`", struct_.name, field.name);
                    check_value_type(sources, &field.ty, &subject, resolved, None, diagnostics);
                    if struct_.is_copy {
                        let field_type =
                            type_expr_to_type_with_self_type(&field.ty, resolved, None);
                        if !field_type.is_unknown_or_unresolved()
                            && field_type.first_unsized_part().is_none()
                            && type_expr_is_copy(&field.ty, resolved)
                                .is_some_and(|is_copy| !is_copy)
                        {
                            diagnostics.push(copy_struct_field_not_copy_diagnostic(
                                sources,
                                struct_,
                                field,
                                &field_type,
                            ));
                        }
                    }
                }
            }
            Item::Enum(enum_) => {
                for variant in &enum_.variants {
                    for payload in &variant.payload {
                        let subject =
                            format!("enum variant payload `{}.{}`", enum_.name, variant.name);
                        check_parameter_type(
                            sources,
                            payload,
                            &subject,
                            resolved,
                            None,
                            diagnostics,
                        );
                    }
                }
            }
            Item::Interface(interface) => {
                check_interface(sources, interface, resolved, diagnostics);
            }
            Item::Impl(impl_) => {
                check_impl(sources, impl_, resolved, diagnostics);
            }
            Item::Construct(construct) => {
                for (_, function) in construct.functions() {
                    let self_type = function_self_type(function, resolved);
                    check_function(sources, function, resolved, self_type.as_ref(), diagnostics);
                }
                for (_, literal) in construct.literals() {
                    check_literal(sources, literal, resolved, diagnostics);
                }
            }
            Item::Coerce(coerce) => {
                check_impl(sources, &coerce.callable_impl(), resolved, diagnostics);
            }
            Item::Import(_) | Item::FromImport(_) | Item::TypeAlias(_) => {}
        }
    }
}

fn check_literal(
    sources: &SourceMap,
    literal: &crate::ast::LiteralDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let environment = environment_for_literal(literal, resolved);
    let self_type = environment.self_type();
    for parameter in &literal.parameters.parameters {
        check_parameter_type(
            sources,
            parameter,
            "literal parameter",
            resolved,
            self_type,
            diagnostics,
        );
    }
    if let Some(capture) = &literal.capture {
        check_value_type(
            sources,
            &capture.element_type,
            "literal element capture",
            resolved,
            self_type,
            diagnostics,
        );
    }
    check_value_type(
        sources,
        &literal.return_type,
        "literal return type",
        resolved,
        self_type,
        diagnostics,
    );
    if let Some(body) = &literal.body {
        check_block(sources, body, resolved, self_type, diagnostics);
    }
}

fn check_function(
    sources: &SourceMap,
    function: &FunctionDecl,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let prefix = format!("function `{}`", function.name);
    check_parameter_list(
        sources,
        &function.parameters.parameters,
        &prefix,
        resolved,
        self_type,
        diagnostics,
    );
    check_value_type(
        sources,
        &function.return_type,
        &format!("{prefix} return type"),
        resolved,
        self_type,
        diagnostics,
    );
    if let Some(body) = &function.body {
        check_block(sources, body, resolved, self_type, diagnostics);
    }
}

fn check_primitive(
    sources: &SourceMap,
    primitive: &PrimitiveDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let prefix = format!("primitive `{}`", primitive.name);
    check_parameter_list(
        sources,
        &primitive.parameters.parameters,
        &prefix,
        resolved,
        None,
        diagnostics,
    );
    check_value_type(
        sources,
        &primitive.return_type,
        &format!("{prefix} return type"),
        resolved,
        None,
        diagnostics,
    );
}

fn check_interface(
    sources: &SourceMap,
    interface: &InterfaceDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for method in &interface.methods {
        let prefix = format!("interface method `{}.{}`", interface.name, method.name);
        check_method_with_prefix(sources, method, &prefix, resolved, None, diagnostics);
    }
}

fn check_impl(
    sources: &SourceMap,
    impl_: &ImplDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let self_type = impl_self_type(impl_, resolved);
    for member in &impl_.members {
        match member {
            ImplMember::Method(method) => {
                let prefix = format!("method `{}`", method.name);
                check_method_with_prefix(
                    sources,
                    method,
                    &prefix,
                    resolved,
                    Some(&self_type),
                    diagnostics,
                );
            }
            ImplMember::Drop(drop_) => {
                check_value_type(
                    sources,
                    &drop_.binding.ty,
                    "drop binding type",
                    resolved,
                    Some(&self_type),
                    diagnostics,
                );
                check_block(
                    sources,
                    &drop_.body,
                    resolved,
                    Some(&self_type),
                    diagnostics,
                );
            }
        }
    }
}

fn check_method_with_prefix(
    sources: &SourceMap,
    method: &MethodDecl,
    prefix: &str,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let receiver = method.receiver.implicit_parameter();
    check_parameter_type(
        sources,
        &receiver,
        &format!("{prefix} receiver"),
        resolved,
        self_type,
        diagnostics,
    );
    check_parameter_list(
        sources,
        &method.parameters.parameters,
        prefix,
        resolved,
        self_type,
        diagnostics,
    );
    check_value_type(
        sources,
        &method.return_type,
        &format!("{prefix} return type"),
        resolved,
        self_type,
        diagnostics,
    );
    if let Some(body) = &method.body {
        check_block(sources, body, resolved, self_type, diagnostics);
    }
}

fn check_parameter_list(
    sources: &SourceMap,
    parameters: &[Parameter],
    prefix: &str,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for parameter in parameters {
        check_parameter_type(sources, parameter, prefix, resolved, self_type, diagnostics);
    }
}

fn check_parameter_type(
    sources: &SourceMap,
    parameter: &Parameter,
    prefix: &str,
    resolved: &ResolveOutput,
    self_type: Option<&Type>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_value_type(
        sources,
        &parameter.ty,
        &format!("{prefix} parameter `{}`", parameter.name),
        resolved,
        self_type,
        diagnostics,
    );
}
