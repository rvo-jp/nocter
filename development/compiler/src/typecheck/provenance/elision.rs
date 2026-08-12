//! Shared classification for omitted result-origin contracts.
//!
//! Surface syntax records only an author's explicit upper bound. Every
//! semantic consumer expands an omitted clause through this module so
//! validation, abstract calls, conformance, and tooling cannot invent
//! different elision rules.

use super::{
    InputId, StorageOrigin, ValueProvenance, type_may_carry_result_provenance,
    type_may_retain_fresh_result_storage,
};
use crate::resolve::{ParameterSignature, ResolveOutput};
use crate::typecheck::allocation::allocator_capability_kind;
use crate::typecheck::model::Type;
use crate::typecheck::provenance::ResultProvenanceInputs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::typecheck) enum ElidedResultContract {
    /// The result exposes no caller-managed input origin.
    Independent,
    /// One input is the only origin that an omitted clause may retain.
    Unique {
        label: String,
        contract: ValueProvenance,
    },
    /// Source must distinguish the retained subset before callers can use it.
    Ambiguous {
        labels: Vec<String>,
        conservative: ValueProvenance,
    },
}

impl ElidedResultContract {
    pub(in crate::typecheck) fn allowed_contract(&self) -> Option<&ValueProvenance> {
        match self {
            Self::Independent => None,
            Self::Unique { contract, .. } => Some(contract),
            Self::Ambiguous { .. } => None,
        }
    }

    pub(in crate::typecheck) fn abstract_summary(&self) -> Option<ValueProvenance> {
        match self {
            Self::Independent => None,
            Self::Unique { contract, .. } => Some(contract.clone()),
            Self::Ambiguous { conservative, .. } => Some(conservative.clone()),
        }
    }

    pub(in crate::typecheck) fn unique_input(&self) -> Option<InputId> {
        let Self::Unique {
            contract: ValueProvenance::Origins(origins),
            ..
        } = self
        else {
            return None;
        };
        match origins.as_slice() {
            [StorageOrigin::Input(input)] => Some(*input),
            _ => None,
        }
    }
}

/// Expands a success-result contract into the type-shaped summary needed by
/// lifetime analysis. Recoverable errors are not part of `from`, but their
/// owned storage still belongs to the caller's current allocation context and
/// must remain visible to escape checking.
pub(in crate::typecheck) fn result_provenance_summary(
    success_contract: Option<ValueProvenance>,
    return_type: &Type,
    resolved: &ResolveOutput,
) -> Option<ValueProvenance> {
    if let Type::Fallible { success, error } = return_type {
        // Preserve both outcome layers even when a branch is
        // storage-independent. `None` means "no information" to the summary
        // engine and would make it reconstruct provenance from every call
        // argument, accidentally tying an owned Error to a scratch buffer.
        let success = abstract_value_summary(success_contract, success, resolved)
            .unwrap_or(ValueProvenance::Independent);
        let error =
            abstract_value_summary(None, error, resolved).unwrap_or(ValueProvenance::Independent);
        return Some(ValueProvenance::Fallible {
            success: Some(Box::new(success)),
            error: Some(Box::new(error)),
        });
    }
    abstract_value_summary(success_contract, return_type, resolved)
}

fn abstract_value_summary(
    contract: Option<ValueProvenance>,
    ty: &Type,
    resolved: &ResolveOutput,
) -> Option<ValueProvenance> {
    contract.or_else(|| {
        type_may_retain_fresh_result_storage(ty, resolved).then(|| {
            ValueProvenance::Independent
                .with_returned_allocation_from(ValueProvenance::current_allocation_context())
        })
    })
}

pub(in crate::typecheck) fn elided_declaration_result_contract(
    method: Option<&crate::ast::CallableDecl>,
    inputs: ResultProvenanceInputs<'_>,
    return_type: &Type,
    resolved: &ResolveOutput,
) -> ElidedResultContract {
    let mut candidates = Vec::new();
    if let Some(method) = method {
        let receiver = method.receiver.implicit_parameter();
        let receiver_type = crate::typecheck::type_expr::type_expr_to_type_with_substitutions(
            &receiver.ty,
            resolved,
            None,
            &std::collections::HashMap::new(),
        );
        candidates.push((
            "self".to_string(),
            InputId::declared_at(method.receiver.name_span),
            receiver_type,
        ));
    }
    for input in inputs.elision_inputs() {
        let input_type = crate::typecheck::type_expr::type_expr_to_type_with_substitutions(
            input.ty,
            resolved,
            None,
            &std::collections::HashMap::new(),
        );
        candidates.push((
            input.label.to_string(),
            InputId::declared_at(input.name_span),
            input_type,
        ));
    }
    elided_typed_result_contract(candidates, return_type, resolved)
}

pub(in crate::typecheck) fn elided_signature_result_contract(
    parameters: &[ParameterSignature],
    return_type: &Type,
    resolved: &ResolveOutput,
) -> ElidedResultContract {
    elided_typed_result_contract(
        parameters.iter().map(|parameter| {
            (
                parameter.name.clone(),
                InputId::declared_at(parameter.name_span),
                crate::typecheck::type_expr::type_expr_to_type_with_substitutions(
                    &parameter.ty,
                    resolved,
                    None,
                    &std::collections::HashMap::new(),
                ),
            )
        }),
        return_type,
        resolved,
    )
}

fn elided_success_type(ty: &Type) -> &Type {
    match ty {
        Type::Fallible { success, .. } => success,
        _ => ty,
    }
}

pub(in crate::typecheck) fn elided_typed_result_contract(
    inputs: impl IntoIterator<Item = (String, InputId, Type)>,
    return_type: &Type,
    resolved: &ResolveOutput,
) -> ElidedResultContract {
    if !type_may_carry_result_provenance(elided_success_type(return_type), resolved) {
        return ElidedResultContract::Independent;
    }

    classify(
        inputs
            .into_iter()
            .filter_map(|(label, input, ty)| {
                (type_may_carry_result_provenance(&ty, resolved)
                    || allocator_capability_kind(&ty, resolved).is_some()
                    || matches!(ty, Type::Parameter(_) | Type::Unresolved(_) | Type::Unknown))
                .then_some((label, StorageOrigin::Input(input)))
            })
            .collect(),
    )
}

fn classify(candidates: Vec<(String, StorageOrigin)>) -> ElidedResultContract {
    match candidates.as_slice() {
        [] => ElidedResultContract::Independent,
        [(label, origin)] => ElidedResultContract::Unique {
            label: label.clone(),
            contract: ValueProvenance::Origins(vec![origin.clone()]),
        },
        _ => ElidedResultContract::Ambiguous {
            labels: candidates.iter().map(|(label, _)| label.clone()).collect(),
            conservative: ValueProvenance::Origins(
                candidates.into_iter().map(|(_, origin)| origin).collect(),
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::resolve::resolve;
    use crate::source::SourceMap;
    use crate::typecheck::environments::environment_for_function;
    use crate::typecheck::type_expr::type_expr_to_type_in_environment;

    #[test]
    fn classifies_zero_one_and_multiple_storage_inputs() {
        let text = r#"func fresh(size: usize): &str { return "" }
func view(value: &str, size: usize): &str { return value }
func choose(left: &str, right: &str): &str { return left }
func main(): i32 { return 0 }
"#;
        let mut sources = SourceMap::new();
        let source = sources.add_source("test.nct", None, text);
        let tokens = lex(&sources, source);
        let ast = parse(&sources, source, &tokens.tokens).ast.unwrap();
        let resolved = resolve(&sources, &ast);
        let functions = ast
            .items
            .iter()
            .filter_map(|item| match item {
                crate::ast::Item::Function(function) if function.name != "main" => Some(function),
                _ => None,
            })
            .collect::<Vec<_>>();

        let classify = |function: &crate::ast::FunctionDecl| {
            let environment = environment_for_function(function, &resolved);
            let return_type =
                type_expr_to_type_in_environment(&function.return_type, &resolved, &environment);
            elided_declaration_result_contract(
                None,
                ResultProvenanceInputs::parameters(&function.parameters.parameters),
                &return_type,
                &resolved,
            )
        };

        assert_eq!(classify(functions[0]), ElidedResultContract::Independent);
        assert!(matches!(
            classify(functions[1]),
            ElidedResultContract::Unique { ref label, .. } if label == "value"
        ));
        assert!(matches!(
            classify(functions[2]),
            ElidedResultContract::Ambiguous { ref labels, .. }
                if labels == &["left".to_string(), "right".to_string()]
        ));
    }
}
