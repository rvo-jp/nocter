//! Fixed-target closure analysis; target independence alone proves no condition or plan equivalence.
use super::super::{ExpressionKind, Query, instances};
use nocter_model::ValueCode;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub(super) fn source_recipe(
    query: &Query,
    call: nocter_model::AccessExpressionId,
) -> Option<&crate::value_access::InvocationRecipe> {
    query
        .result_evidence
        .as_ref()?
        .invocation(nocter_model::ValueApplication {
            owner: query.code(),
            call,
        })
}

/// Only code whose entire completed-call closure names fixed code can ignore witness bindings
/// while discovering bodies. Unknown bodies/targets conservatively prevent this projection.
/// Recursive fixed-code components require no finite unrolling or arbitrary iteration cutoff.
pub(in crate::value_access) fn witness_independent(
    scope: &BTreeMap<ValueCode, &Query>,
) -> BTreeSet<ValueCode> {
    let mut dependent = BTreeSet::new();
    let mut callers = BTreeMap::<ValueCode, BTreeSet<ValueCode>>::new();
    for (&code, query) in scope {
        for (id, expression) in query.body().expressions().iter() {
            if !instances::is_completed_call(&expression.kind) {
                continue;
            }
            let ExpressionKind::Call { target, .. } = expression.kind else {
                continue;
            };
            let target = if let Some(recipe) = source_recipe(query, id) {
                recipe
                    .callees
                    .first()
                    .filter(|_| recipe.callees.len() == 1)
                    .and_then(|witness| query.program().concrete_witness(*witness).ok())
                    .map(|witness| witness.code())
            } else if query.result_evidence.is_some() {
                target.code()
            } else {
                None
            };
            if let Some(target) = target {
                if scope.contains_key(&target) {
                    callers.entry(target).or_default().insert(code);
                } else {
                    dependent.insert(code);
                }
            } else {
                dependent.insert(code);
            }
        }
    }
    let mut pending: VecDeque<_> = dependent.iter().copied().collect();
    while let Some(code) = pending.pop_front() {
        for &caller in callers.get(&code).into_iter().flatten() {
            if dependent.insert(caller) {
                pending.push_back(caller);
            }
        }
    }
    scope
        .keys()
        .filter(|code| !dependent.contains(code))
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_access::{
        accepted::SourceUnit,
        tests::{bound, input},
    };

    #[test]
    fn fixed_cycles_are_independent_but_dynamic_transitive_and_missing_targets_are_not() {
        let source = "const func leaf(): void {}\nconst func fixed(action: func(): void): void { fixed(action) }\nconst func direct(action: func(): void): void { leaf() }\nconst func invoke(action: func(): void): void { action() }\nconst func wrapper(action: func(): void): void { invoke(action) }";
        let (sources, tree) = input(source);
        let unit = SourceUnit::prepare(bound(&sources, &tree))
            .unwrap()
            .into_unit();
        let mut scope = unit
            .bodies()
            .map(|(_, query)| (query.typed().code(), query.typed()))
            .collect::<BTreeMap<_, _>>();
        let code = |name| {
            ValueCode::Declaration(
                unit.program()
                    .resolve_code(unit.program().symbols().get(name).unwrap())
                    .unwrap(),
            )
        };
        let independent = witness_independent(&scope);
        for name in ["leaf", "fixed", "direct"] {
            assert!(independent.contains(&code(name)), "{name}");
        }
        for name in ["invoke", "wrapper"] {
            assert!(!independent.contains(&code(name)), "{name}");
        }
        scope.remove(&code("leaf"));
        let independent = witness_independent(&scope);
        assert!(!independent.contains(&code("direct")));
        assert!(independent.contains(&code("fixed")));
    }
}
