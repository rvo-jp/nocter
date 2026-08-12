//! Validated standard-library interface roles used by collection iteration.

use crate::ast::{
    AstFile, InterfaceDecl, Item, MethodReceiverMode, ResultProvenanceOriginKind, Visibility,
};
use crate::semantics::{
    IterationAssociatedTypeInput, IterationProtocolInput, IterationRuntimeInput,
    TrustedDeclarationInputs,
};
use std::collections::HashMap;

pub(crate) fn attach_iteration_runtime(
    modules: &HashMap<String, &AstFile>,
    facts: &mut TrustedDeclarationInputs,
) {
    let Some(runtime) = iteration_runtime(modules) else {
        return;
    };
    facts.set_iteration_runtime(runtime);
}

fn iteration_runtime(modules: &HashMap<String, &AstFile>) -> Option<IterationRuntimeInput> {
    let module = modules.get("std/iter")?;
    Some(IterationRuntimeInput {
        iterator: find_interface(
            module,
            "std/iter",
            "Iterator",
            &[],
            Some(AssociatedTypeShape {
                name: "Item",
                bounds: &[],
            }),
            "next",
            MethodReceiverMode::ReadwriteBorrow,
            "Self.Item?",
            IterationResultContract {
                from_receiver: true,
            },
        )?,
        exact_size: find_interface(
            module,
            "std/iter",
            "ExactSizeIterator",
            &[],
            None,
            "remaining_len",
            MethodReceiverMode::ReadonlyBorrow,
            "usize",
            IterationResultContract::independent(),
        )?,
    })
}

fn find_interface(
    module: &AstFile,
    module_name: &str,
    name: &str,
    generic_parameters: &[&str],
    associated_type: Option<AssociatedTypeShape<'_>>,
    method_name: &str,
    receiver_mode: MethodReceiverMode,
    return_type: &str,
    result_contract: IterationResultContract,
) -> Option<IterationProtocolInput> {
    let declaration = module.items.iter().find_map(|item| match item {
        Item::Interface(declaration) if declaration.name == name => Some(declaration),
        _ => None,
    })?;
    let method = declaration
        .methods
        .iter()
        .find(|method| method.name == method_name)?;
    interface_shape_matches(
        declaration,
        method,
        generic_parameters,
        associated_type,
        receiver_mode,
        return_type,
        result_contract,
    )
    .then(|| IterationProtocolInput {
        interface_declaration: declaration.name_span,
        interface_canonical_name: format!("{module_name}.{name}"),
        method_declaration: method.name_span,
        method_name: method.name.clone(),
        associated_type: associated_type.map(|expected| {
            let actual = declaration
                .associated_types
                .iter()
                .find(|actual| actual.name == expected.name)
                .expect("validated associated type");
            IterationAssociatedTypeInput {
                declaration: actual.name_span,
                name: actual.name.clone(),
            }
        }),
    })
}

fn interface_shape_matches(
    declaration: &InterfaceDecl,
    method: &crate::ast::MethodDecl,
    generic_parameters: &[&str],
    associated_type: Option<AssociatedTypeShape<'_>>,
    receiver_mode: MethodReceiverMode,
    return_type: &str,
    result_contract: IterationResultContract,
) -> bool {
    declaration.visibility == Visibility::Public
        && declaration.target.is_none()
        && declaration
            .generics
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .eq(generic_parameters.iter().copied())
        && declaration.requirements.is_none()
        && associated_type_shape_matches(declaration, associated_type)
        && method.visibility == Visibility::Public
        && method.receiver.mode == receiver_mode
        && method.parameters.parameters.is_empty()
        && crate::ast::canonical_type_expr(&method.return_type) == return_type
        && result_provenance_matches(method, result_contract.from_receiver)
        && method.body.is_none()
}

#[derive(Debug, Clone, Copy)]
struct AssociatedTypeShape<'a> {
    name: &'a str,
    bounds: &'a [&'a str],
}

fn associated_type_shape_matches(
    declaration: &InterfaceDecl,
    expected: Option<AssociatedTypeShape<'_>>,
) -> bool {
    match (declaration.associated_types.as_slice(), expected) {
        ([], None) => true,
        ([actual], Some(expected)) => {
            actual.name == expected.name
                && actual
                    .bounds
                    .iter()
                    .map(crate::ast::canonical_type_expr)
                    .eq(expected.bounds.iter().copied())
        }
        _ => false,
    }
}

#[derive(Debug, Clone, Copy)]
struct IterationResultContract {
    from_receiver: bool,
}

impl IterationResultContract {
    const fn independent() -> Self {
        Self {
            from_receiver: false,
        }
    }
}

fn result_provenance_matches(method: &crate::ast::MethodDecl, from_receiver: bool) -> bool {
    match (&method.result_provenance, from_receiver) {
        (None, _) => true,
        (Some(clause), true) => {
            matches!(
                clause.origins.as_slice(),
                [origin] if origin.kind == ResultProvenanceOriginKind::Receiver
            )
        }
        (Some(_), false) => false,
    }
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
            r#"pub interface Iterator {
    pub type Item
    pub method &+self.next(): Self.Item?
}
pub interface ExactSizeIterator {
    pub method &self.remaining_len(): usize
}
"#,
        );
        let modules = HashMap::from([("std/iter".to_string(), &iter)]);

        let runtime = iteration_runtime(&modules).expect("expected runtime");

        assert_eq!(runtime.iterator.method_declaration.source, iter.span.source);
        assert_eq!(runtime.exact_size.method_name, "remaining_len");
    }

    #[test]
    fn rejects_name_only_iteration_interfaces() {
        let mut sources = SourceMap::new();
        let iter = parse_text(
            &mut sources,
            "pub interface Iterator { pub type Item pub method self.next(): usize }\n",
        );
        let modules = HashMap::from([("std/iter".to_string(), &iter)]);

        assert!(iteration_runtime(&modules).is_none());
    }
}
