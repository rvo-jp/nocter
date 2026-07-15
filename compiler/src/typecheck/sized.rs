mod value;
mod walk;

use super::environments::function_self_type;
use super::model::Type;
use super::type_expr::type_expr_to_type_with_self_type;
use crate::ast::{
    AstFile, FunctionDecl, ImplDecl, ImplMember, Item, MethodDecl, Parameter, PrimitiveDecl,
    TraitDecl,
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
            Item::Primitive(primitive) => {
                check_primitive(sources, primitive, resolved, diagnostics);
            }
            Item::Struct(struct_) => {
                for field in &struct_.fields {
                    let subject = format!("struct field `{}.{}`", struct_.name, field.name);
                    check_value_type(sources, &field.ty, &subject, resolved, None, diagnostics);
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
            Item::Trait(trait_) => {
                check_trait(sources, trait_, resolved, diagnostics);
            }
            Item::Impl(impl_) => {
                check_impl(sources, impl_, resolved, diagnostics);
            }
            Item::Use(_) | Item::Import(_) | Item::FromImport(_) | Item::TypeAlias(_) => {}
        }
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
    check_block(sources, &function.body, resolved, self_type, diagnostics);
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

fn check_trait(
    sources: &SourceMap,
    trait_: &TraitDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for method in &trait_.methods {
        let prefix = format!("trait method `{}.{}`", trait_.name, method.name);
        check_method_with_prefix(sources, method, &prefix, resolved, None, diagnostics);
    }
}

fn check_impl(
    sources: &SourceMap,
    impl_: &ImplDecl,
    resolved: &ResolveOutput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let self_type = type_expr_to_type_with_self_type(&impl_.target_ty, resolved, None);
    for member in &impl_.members {
        match member {
            ImplMember::Function(function) => {
                check_function(sources, function, resolved, Some(&self_type), diagnostics);
            }
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
    check_parameter_type(
        sources,
        &method.receiver,
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
