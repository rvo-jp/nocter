//! Validated standard-library interface roles used by collection iteration.

use crate::ast::{AstFile, InterfaceDecl, Item, MethodReceiverMode, Visibility};
use crate::semantics::{IterationProtocol, IterationRuntime, TrustedDeclarationFacts};
use std::collections::HashMap;

pub(crate) fn attach_iteration_runtime(
    modules: &HashMap<String, &AstFile>,
    facts: &mut TrustedDeclarationFacts,
) {
    let Some(runtime) = iteration_runtime(modules) else {
        return;
    };
    facts.set_iteration_runtime(runtime);
}

fn iteration_runtime(modules: &HashMap<String, &AstFile>) -> Option<IterationRuntime> {
    let module = modules.get("std/iter")?;
    Some(IterationRuntime {
        iterator: find_interface(
            module,
            "std/iter",
            "Iterator",
            &["T"],
            "next",
            MethodReceiverMode::ReadwriteBorrow,
            "T?",
        )?,
        exact_size: find_interface(
            module,
            "std/iter",
            "ExactSizeIterator",
            &["T"],
            "remaining_len",
            MethodReceiverMode::ReadonlyBorrow,
            "usize",
        )?,
        readonly_conversion: find_interface(
            module,
            "std/iter",
            "Iterable",
            &["T", "I"],
            "iter",
            MethodReceiverMode::ReadonlyBorrow,
            "I",
        )?,
        owned_conversion: find_interface(
            module,
            "std/iter",
            "IntoIterator",
            &["T", "I"],
            "into_iter",
            MethodReceiverMode::Owned,
            "I",
        )?,
    })
}

fn find_interface(
    module: &AstFile,
    module_name: &str,
    name: &str,
    generic_parameters: &[&str],
    method_name: &str,
    receiver_mode: MethodReceiverMode,
    return_type: &str,
) -> Option<IterationProtocol> {
    let declaration = module.items.iter().find_map(|item| match item {
        Item::Interface(declaration) if declaration.name == name => Some(declaration),
        _ => None,
    })?;
    interface_shape_matches(
        declaration,
        generic_parameters,
        method_name,
        receiver_mode,
        return_type,
    )
    .then(|| IterationProtocol {
        interface_declaration: declaration.name_span,
        interface_canonical_name: format!("{module_name}.{name}"),
        method_declaration: declaration.methods[0].name_span,
        method_name: declaration.methods[0].name.clone(),
    })
}

fn interface_shape_matches(
    declaration: &InterfaceDecl,
    generic_parameters: &[&str],
    method_name: &str,
    receiver_mode: MethodReceiverMode,
    return_type: &str,
) -> bool {
    declaration.visibility == Visibility::Public
        && declaration.target.is_none()
        && declaration
            .generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .eq(generic_parameters.iter().copied())
        && declaration
            .generics
            .parameters
            .iter()
            .all(|parameter| parameter.bound.is_none())
        && declaration.methods.len() == 1
        && declaration.methods[0].visibility == Visibility::Public
        && declaration.methods[0].name == method_name
        && declaration.methods[0].receiver.mode == receiver_mode
        && declaration.methods[0].parameters.parameters.is_empty()
        && crate::ast::type_expr_display_lossy(&declaration.methods[0].return_type) == return_type
        && declaration.methods[0].result_provenance.is_none()
        && declaration.methods[0].body.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::source::SourceMap;

    fn parse_text(sources: &mut SourceMap, text: &str) -> AstFile {
        let source = sources.add_source("iter.nct", None, text);
        let lexed = lex(sources, source);
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        parse(sources, source, &lexed.tokens)
            .ast
            .expect("expected AST")
    }

    #[test]
    fn validates_the_complete_iteration_protocol_bundle() {
        let mut sources = SourceMap::new();
        let iter = parse_text(
            &mut sources,
            r#"pub interface Iterator<T> {
    pub method &+self.next(): T?
}
pub interface ExactSizeIterator<T> {
    pub method &self.remaining_len(): usize
}
pub interface Iterable<T, I> {
    pub method &self.iter(): I
}
pub interface IntoIterator<T, I> {
    pub method self.into_iter(): I
}
"#,
        );
        let modules = HashMap::from([("std/iter".to_string(), &iter)]);

        let runtime = iteration_runtime(&modules).expect("expected runtime");

        assert_eq!(runtime.iterator.method_declaration.source, iter.span.source);
        assert_eq!(
            runtime.readonly_conversion.interface_declaration.source,
            iter.span.source
        );
        assert_eq!(runtime.exact_size.method_name, "remaining_len");
    }

    #[test]
    fn rejects_name_only_iteration_interfaces() {
        let mut sources = SourceMap::new();
        let iter = parse_text(
            &mut sources,
            "pub interface Iterator<T> { pub method &self.next(): T? }\n",
        );
        let modules = HashMap::from([("std/iter".to_string(), &iter)]);

        assert!(iteration_runtime(&modules).is_none());
    }
}
