//! Validated ordinary standard-library declarations used by interpolation lowering.

use crate::ast::{AstFile, FunctionDecl, Item, StructDecl, type_expr_display_lossy};
use crate::semantics::{
    InterpolationInputKind, InterpolationRuntime, RuntimeCallable, TrustedDeclarationFacts,
};
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

    let expected = [
        (InterpolationInputKind::Str, "append_str", "&str"),
        (InterpolationInputKind::String, "append_string", "&String"),
        (InterpolationInputKind::I32, "append_i32", "i32"),
        (InterpolationInputKind::U8, "append_u8", "u8"),
        (InterpolationInputKind::Usize, "append_usize", "usize"),
        (InterpolationInputKind::Bool, "append_bool", "bool"),
    ];
    let mut formatters = HashMap::new();
    for (kind, name, input) in expected {
        let function = find_function(
            fmt_module,
            name,
            None,
            &[("out", "&+String"), ("value", input)],
            "void",
        )?;
        formatters.insert(kind, runtime_callable(function));
    }

    Some(InterpolationRuntime::new(
        string.span,
        runtime_callable(constructor),
        formatters,
    ))
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
                actual.name == *name && type_expr_display_lossy(&actual.ty) == *ty
            })
        && function_return_type_matches(function, owner, return_type)
}

fn function_return_type_matches(
    function: &FunctionDecl,
    owner: Option<&str>,
    return_type: &str,
) -> bool {
    let actual = type_expr_display_lossy(&function.return_type);
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
    fn validates_complete_interpolation_runtime_by_shape() {
        let mut sources = SourceMap::new();
        let string = parse_text(
            &mut sources,
            "string.nct",
            "struct String {}\nfunc String.with_capacity(requested_capacity: usize): String {}\n",
        );
        let fmt = parse_text(
            &mut sources,
            "fmt.nct",
            r#"func append_str(out: &+String, value: &str): void {}
func append_string(out: &+String, value: &String): void {}
func append_i32(out: &+String, value: i32): void {}
func append_u8(out: &+String, value: u8): void {}
func append_usize(out: &+String, value: usize): void {}
func append_bool(out: &+String, value: bool): void {}
"#,
        );
        let modules = HashMap::from([
            ("std/string".to_string(), &string),
            ("std/fmt".to_string(), &fmt),
        ]);

        let runtime = interpolation_runtime(&modules).expect("expected runtime");

        assert_eq!(runtime.constructor.target_name, "String.with_capacity");
        assert_eq!(
            runtime
                .formatter(InterpolationInputKind::U8)
                .map(|callable| callable.target_name.as_str()),
            Some("append_u8")
        );
    }

    #[test]
    fn rejects_name_only_runtime_declarations() {
        let mut sources = SourceMap::new();
        let string = parse_text(&mut sources, "string.nct", "struct String {}\n");
        let fmt = parse_text(
            &mut sources,
            "fmt.nct",
            "func append_str(value: &str): void {}\n",
        );
        let modules = HashMap::from([
            ("std/string".to_string(), &string),
            ("std/fmt".to_string(), &fmt),
        ]);

        assert!(interpolation_runtime(&modules).is_none());
    }
}
