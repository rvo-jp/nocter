//! Validated ordinary standard-library declarations used by interpolation lowering.

use crate::ast::{
    AstFile, FunctionDecl, InterfaceDecl, Item, MethodDecl, MethodReceiverMode, StructDecl,
    Visibility, canonical_type_expr,
};
use crate::semantics::{InterpolationRuntime, RuntimeCallable, TrustedDeclarationFacts};
use std::collections::HashMap;

pub(crate) fn attach_interpolation_runtime(
    modules: &HashMap<String, &AstFile>,
    facts: &mut TrustedDeclarationFacts,
) {
    let Some(runtime) = interpolation_runtime(modules) else {
        return;
    };
    facts.set_interpolation_runtime(runtime);
}

fn interpolation_runtime(modules: &HashMap<String, &AstFile>) -> Option<InterpolationRuntime> {
    let string_module = modules.get("std/string")?;
    let fmt_module = modules.get("std/fmt")?;
    let string = string_module.items.iter().find_map(|item| match item {
        Item::Struct(declaration) if owned_string_shape_matches(declaration) => Some(declaration),
        _ => None,
    })?;
    let constructor = find_function(
        string_module,
        "String.with_capacity",
        Some("String"),
        &[("requested_capacity", "usize")],
        "String",
    )?;
    let format_interface = find_format_interface(fmt_module)?;
    let format_method = format_interface
        .methods
        .iter()
        .find(|method| format_method_shape_matches(method))?;

    Some(InterpolationRuntime::new(
        string.span,
        runtime_callable(constructor),
        format_interface.span,
        "std/fmt.Format".to_string(),
        format_method.name_span,
        format_method.name.clone(),
    ))
}

fn find_format_interface(ast: &AstFile) -> Option<&InterfaceDecl> {
    ast.items.iter().find_map(|item| match item {
        Item::Interface(interface)
            if interface.name == "Format"
                && interface.visibility == Visibility::Public
                && interface.target.is_none()
                && interface.generics.parameters.is_empty()
                && interface.requirements.is_none()
                && interface.associated_types.is_empty()
                && interface.methods.len() == 1 =>
        {
            Some(interface)
        }
        _ => None,
    })
}

fn format_method_shape_matches(method: &MethodDecl) -> bool {
    method.name == "format_into"
        && method.visibility == Visibility::Public
        && method.receiver.mode == MethodReceiverMode::ReadonlyBorrow
        && method.generics.parameters.is_empty()
        && method.requirements.is_none()
        && method.parameters.parameters.len() == 1
        && method.parameters.parameters[0].name == "output"
        && canonical_type_expr(&method.parameters.parameters[0].ty) == "&+String"
        && canonical_type_expr(&method.return_type) == "void"
        && method.result_provenance.is_none()
        && method.body.is_none()
}

fn owned_string_shape_matches(declaration: &StructDecl) -> bool {
    declaration.name == "String"
        && !declaration.is_copy
        && declaration.generics.parameters.is_empty()
}

fn find_function<'a>(
    ast: &'a AstFile,
    target_name: &str,
    owner: Option<&str>,
    parameters: &[(&str, &str)],
    return_type: &str,
) -> Option<&'a FunctionDecl> {
    ast.items.iter().find_map(|item| match item {
        Item::Function(function) => {
            function_shape_matches(function, target_name, owner, parameters, return_type)
                .then_some(function)
        }
        Item::Construct(construct) => construct.functions().find_map(|(_, function)| {
            function_shape_matches(function, target_name, owner, parameters, return_type)
                .then_some(function)
        }),
        _ => None,
    })
}

fn function_shape_matches(
    function: &FunctionDecl,
    target_name: &str,
    owner: Option<&str>,
    parameters: &[(&str, &str)],
    return_type: &str,
) -> bool {
    function.name == target_name
        && function.owner.as_ref().map(|owner| owner.name.as_str()) == owner
        && function.generics.parameters.is_empty()
        && function.parameters.parameters.len() == parameters.len()
        && function
            .parameters
            .parameters
            .iter()
            .zip(parameters)
            .all(|(actual, (name, ty))| {
                actual.name == *name && canonical_type_expr(&actual.ty) == *ty
            })
        && function_return_type_matches(function, owner, return_type)
}

fn function_return_type_matches(
    function: &FunctionDecl,
    owner: Option<&str>,
    return_type: &str,
) -> bool {
    let actual = canonical_type_expr(&function.return_type);
    actual == return_type || actual == "Self" && owner == Some(return_type)
}

fn runtime_callable(function: &FunctionDecl) -> RuntimeCallable {
    RuntimeCallable {
        declaration: function.name_span,
        target_name: function.name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::source::SourceMap;

    fn parse_text(sources: &mut SourceMap, name: &str, text: &str) -> AstFile {
        let source = sources.add_source(name, None, text);
        let lexed = lex(sources, source);
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        parse(sources, source, &lexed.tokens)
            .ast
            .expect("expected AST")
    }

    #[test]
    fn validates_interpolation_runtime_contract_by_shape() {
        let mut sources = SourceMap::new();
        let string = parse_text(
            &mut sources,
            "string.nct",
            "struct String {}\nfunc String.with_capacity(requested_capacity: usize): String {}\n",
        );
        let fmt = parse_text(
            &mut sources,
            "fmt.nct",
            r#"pub interface Format {
    pub method &self.format_into(output: &+String): void
}
"#,
        );
        let modules = HashMap::from([
            ("std/string".to_string(), &string),
            ("std/fmt".to_string(), &fmt),
        ]);

        let runtime = interpolation_runtime(&modules).expect("expected runtime");
        let format_span = fmt
            .items
            .iter()
            .find_map(|item| match item {
                Item::Interface(interface) => Some(interface.span),
                _ => None,
            })
            .expect("expected format interface");

        assert_eq!(runtime.constructor.target_name, "String.with_capacity");
        assert_eq!(runtime.format_interface_declaration, format_span);
        assert_eq!(runtime.format_interface_canonical_name, "std/fmt.Format");
        assert_eq!(runtime.format_method_name, "format_into");
    }

    #[test]
    fn rejects_name_only_runtime_declarations() {
        let mut sources = SourceMap::new();
        let string = parse_text(&mut sources, "string.nct", "struct String {}\n");
        let fmt = parse_text(
            &mut sources,
            "fmt.nct",
            "pub interface Format { pub method self.format_into(output: &+String): void }\n",
        );
        let modules = HashMap::from([
            ("std/string".to_string(), &string),
            ("std/fmt".to_string(), &fmt),
        ]);

        assert!(interpolation_runtime(&modules).is_none());
    }
}
