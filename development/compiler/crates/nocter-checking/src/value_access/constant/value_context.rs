//! A definition-check filter, never callable eligibility, a value or an instance proof.
use super::{Requirements, Rule, Site, Violation};
use crate::value_access::ExpressionKind;
use nocter_model::{ValueCode, ValueFunctionOrigin};
use std::collections::{BTreeMap, VecDeque};
pub(super) fn prepare(
    requirements: &BTreeMap<ValueCode, Requirements>,
) -> BTreeMap<ValueCode, Violation> {
    let mut contextual = BTreeMap::new();
    let mut callers =
        BTreeMap::<ValueCode, Vec<(ValueCode, nocter_model::AccessExpressionId)>>::new();
    for (&code, item) in requirements {
        for (id, expression) in item.body().typed().body().expressions().iter() {
            let ExpressionKind::Call {
                target, arguments, ..
            } = &expression.kind
            else {
                continue;
            };
            if matches!(target, ValueFunctionOrigin::Contract(_))
                || arguments
                    .iter()
                    .filter_map(|argument| argument.binding.as_ref())
                    .any(|binding| {
                        matches!(binding.witness.origin, ValueFunctionOrigin::Contract(_))
                    })
            {
                contextual.entry(code).or_insert(Violation {
                    site: Site::Expression(id),
                    rule: Rule::ValueInvocation,
                });
            }
            if let Some(callee) = target.code() {
                callers.entry(callee).or_default().push((code, id));
            }
        }
    }
    let mut pending = contextual.keys().copied().collect::<VecDeque<_>>();
    while let Some(callee) = pending.pop_front() {
        for &(caller, id) in callers.get(&callee).into_iter().flatten() {
            if let std::collections::btree_map::Entry::Vacant(entry) = contextual.entry(caller) {
                entry.insert(Violation {
                    site: Site::Expression(id),
                    rule: Rule::ValueInvocation,
                });
                pending.push_back(caller);
            }
        }
    }
    contextual
}
