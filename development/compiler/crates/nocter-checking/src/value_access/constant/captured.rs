//! Definition-owned anonymous invocation availability, separate from environment transport.
use super::{Diagnostic, Failure, INVALID, Rule, Site, Violation, flow};
use nocter_diagnostics::DiagnosticNote;
use nocter_model::{ValueClosure, ValueFunctionOrigin};
use std::collections::{BTreeMap, VecDeque};

pub(in crate::value_access) struct Availability {
    states: BTreeMap<ValueClosure, Option<Violation>>,
}
impl Availability {
    pub(in crate::value_access) fn prepare(
        closures: &BTreeMap<ValueClosure, flow::Query>,
    ) -> Result<Self, flow::IntegrityError> {
        let mut states = BTreeMap::new();
        let mut dependents =
            BTreeMap::<ValueClosure, Vec<(ValueClosure, nocter_model::AccessExpressionId)>>::new();
        for (&code, query) in closures {
            let mut violation = super::check(query, &|_| true, None)?;
            for (id, expression) in query.typed().body().expressions().iter() {
                if let crate::value_access::ExpressionKind::Call {
                    target: ValueFunctionOrigin::Closure(target),
                    ..
                } = expression.kind
                {
                    dependents.entry(target).or_default().push((code, id));
                    if !closures.contains_key(&target) && violation.is_none() {
                        violation = Some(Violation {
                            site: Site::Expression(id),
                            rule: Rule::CapturedCall(target),
                        });
                    }
                }
            }
            states.insert(code, violation);
        }
        let mut pending: VecDeque<_> = states
            .iter()
            .filter_map(|(&code, cause)| cause.is_some().then_some(code))
            .collect();
        while let Some(target) = pending.pop_front() {
            for &(code, expression) in dependents.get(&target).into_iter().flatten() {
                let state = states.get_mut(&code).ok_or(INVALID)?;
                if state.is_none() {
                    *state = Some(Violation {
                        site: Site::Expression(expression),
                        rule: Rule::CapturedCall(target),
                    });
                    pending.push_back(code);
                }
            }
        }
        Ok(Self { states })
    }
    pub(in crate::value_access) fn permitted_template(&self, code: ValueClosure) -> bool {
        self.states.get(&code).is_some_and(Option::is_none)
    }
    pub(in crate::value_access) fn annotate(
        &self,
        mut rule: Rule,
        diagnostic: &mut Diagnostic,
        closures: &BTreeMap<ValueClosure, flow::Query>,
        verified: &crate::value_access::results::Verified<'_, '_>,
    ) -> Result<(), Failure> {
        // Every propagated cause points to an earlier unavailable fact, so cycles cannot choose
        // themselves as evidence or create an infinite diagnostic path.
        while let Rule::CapturedCall(code) = rule {
            let Some(Some(cause)) = self.states.get(&code) else {
                break;
            };
            let bound = closures
                .get(&code)
                .ok_or(Failure::InconsistentSyntax)?
                .bind_verified(verified)?;
            let projected = (*cause).project(&bound)?;
            diagnostic.source.push_note(DiagnosticNote::new(
                format!(
                    "captured invocation depends on this operation: {}",
                    projected.source.message()
                ),
                projected.source.primary(),
            ));
            rule = cause.rule;
        }
        Ok(())
    }
}
