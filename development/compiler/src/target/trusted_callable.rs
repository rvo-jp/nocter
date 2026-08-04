//! Validated standard-library callable capabilities.

use crate::ast::{AstFile, InterfaceDecl, Item, MethodReceiverMode, Visibility};
use crate::semantics::{CallableProtocol, CallableRuntime, TrustedDeclarationFacts};
use std::collections::HashMap;

pub(crate) fn attach_callable_runtime(
    modules: &HashMap<String, &AstFile>,
    facts: &mut TrustedDeclarationFacts,
) {
    if let Some(runtime) = callable_runtime(modules) {
        facts.set_callable_runtime(runtime);
    }
}

fn callable_runtime(modules: &HashMap<String, &AstFile>) -> Option<CallableRuntime> {
    let module = modules.get("std/callable")?;
    Some(CallableRuntime {
        readonly: find_interface(module, "Call", "call", MethodReceiverMode::ReadonlyBorrow)?,
        repeated: find_interface(
            module,
            "CallMut",
            "call_mut",
            MethodReceiverMode::ReadwriteBorrow,
        )?,
        consuming: find_interface(module, "CallOnce", "call_once", MethodReceiverMode::Owned)?,
    })
}

fn find_interface(
    module: &AstFile,
    name: &str,
    method_name: &str,
    receiver_mode: MethodReceiverMode,
) -> Option<CallableProtocol> {
    let declaration = module.items.iter().find_map(|item| match item {
        Item::Interface(declaration) if declaration.name == name => Some(declaration),
        _ => None,
    })?;
    callable_interface_shape_matches(declaration, method_name, receiver_mode).then(|| {
        CallableProtocol {
            interface_declaration: declaration.name_span,
            interface_canonical_name: format!("std/callable.{name}"),
            method_declaration: declaration.methods[0].name_span,
            method_name: declaration.methods[0].name.clone(),
        }
    })
}

fn callable_interface_shape_matches(
    declaration: &InterfaceDecl,
    method_name: &str,
    receiver_mode: MethodReceiverMode,
) -> bool {
    declaration.visibility == Visibility::Public
        && declaration.target.is_none()
        && declaration.generics.parameters.len() == 2
        && declaration.generics.parameters[0].name == "Input"
        && declaration.generics.parameters[1].name == "Output"
        && declaration
            .generics
            .parameters
            .iter()
            .all(|parameter| parameter.bounds.is_empty())
        && declaration.methods.len() == 1
        && declaration.methods[0].visibility == Visibility::Public
        && declaration.methods[0].name == method_name
        && declaration.methods[0].receiver.mode == receiver_mode
        && declaration.methods[0].parameters.parameters.len() == 1
        && declaration.methods[0].parameters.parameters[0].name == "value"
        && crate::ast::type_expr_display_lossy(&declaration.methods[0].parameters.parameters[0].ty)
            == "Input"
        && crate::ast::type_expr_display_lossy(&declaration.methods[0].return_type) == "Output"
        && declaration.methods[0].generics.parameters.is_empty()
        && declaration.methods[0].result_provenance.is_none()
        && declaration.methods[0].body.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::source::SourceMap;

    fn parse_module(sources: &mut SourceMap, text: &str) -> AstFile {
        let source = sources.add_source("callable.nct", None, text);
        let lexed = lex(sources, source);
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        parse(sources, source, &lexed.tokens)
            .ast
            .expect("expected callable AST")
    }

    #[test]
    fn validates_the_complete_callable_capability_bundle() {
        let mut sources = SourceMap::new();
        let module = parse_module(
            &mut sources,
            r#"pub interface Call<Input, Output> {
    pub method &self.call(value: Input): Output
}
pub interface CallMut<Input, Output> {
    pub method &+self.call_mut(value: Input): Output
}
pub interface CallOnce<Input, Output> {
    pub method self.call_once(value: Input): Output
}
"#,
        );
        let modules = HashMap::from([("std/callable".to_string(), &module)]);
        let runtime = callable_runtime(&modules).expect("expected callable runtime");
        assert_eq!(runtime.repeated.method_name, "call_mut");
        assert_eq!(
            runtime.consuming.interface_declaration.source,
            module.span.source
        );
    }

    #[test]
    fn rejects_name_only_callable_interfaces() {
        let mut sources = SourceMap::new();
        let module = parse_module(
            &mut sources,
            "pub interface Call<Input, Output> { pub method self.call(value: Input): Output }\n",
        );
        let modules = HashMap::from([("std/callable".to_string(), &module)]);
        assert!(callable_runtime(&modules).is_none());
    }
}
