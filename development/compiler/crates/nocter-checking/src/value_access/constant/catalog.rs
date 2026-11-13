//! Definition-owned anonymous invocation availability, separate from environment transport.
use super::{Diagnostic, Failure, INVALID, Requirements, Rule, Site, Violation, flow};
use nocter_diagnostics::DiagnosticNote;
use nocter_model::{ValueClosure, ValueCode, ValueFunctionOrigin};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

#[derive(Debug)]
pub(in crate::value_access) struct Catalog {
    requirements: BTreeMap<ValueCode, Requirements>,
    states: BTreeMap<ValueClosure, Option<Violation>>,
}
impl Catalog {
    pub(in crate::value_access) fn prepare(
        bodies: impl IntoIterator<Item = Arc<flow::Query>>,
    ) -> Result<Self, flow::IntegrityError> {
        let mut requirements = BTreeMap::new();
        for body in bodies {
            let item = Requirements::prepare(body)?;
            if requirements.insert(item.code(), item).is_some() {
                return Err(INVALID);
            }
        }
        let mut states = BTreeMap::new();
        let mut dependents =
            BTreeMap::<ValueClosure, Vec<(ValueClosure, nocter_model::AccessExpressionId)>>::new();
        for (&source, item) in &requirements {
            let ValueCode::Closure(code) = source else {
                continue;
            };
            let mut violation = item.check(&|_| true, None)?;
            for call in item.call_expressions() {
                let (id, expression) = call?;
                if let crate::value_access::ExpressionKind::Call {
                    target: ValueFunctionOrigin::Closure(target),
                    ..
                } = *expression
                {
                    dependents.entry(target).or_default().push((code, id));
                    if !requirements.contains_key(&ValueCode::Closure(target))
                        && violation.is_none()
                    {
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
        Ok(Self {
            requirements,
            states,
        })
    }
    pub(in crate::value_access) fn traversal(&self) -> super::Traversal {
        let scope = self
            .requirements
            .iter()
            .map(|(&code, item)| (code, item.body().typed()))
            .collect();
        super::Traversal::new(&scope)
    }
    pub(in crate::value_access) fn get(&self, code: ValueCode) -> Option<&Requirements> {
        self.requirements.get(&code)
    }
    pub(in crate::value_access) fn permitted_template(&self, code: ValueClosure) -> bool {
        self.states.get(&code).is_some_and(Option::is_none)
    }
    pub(in crate::value_access) fn annotate(
        &self,
        mut rule: Rule,
        diagnostic: &mut Diagnostic,
        verified: &crate::value_access::results::Verified<'_, '_>,
    ) -> Result<(), Failure> {
        // Every propagated cause points to an earlier unavailable fact, so cycles cannot choose
        // themselves as evidence or create an infinite diagnostic path.
        while let Rule::CapturedCall(code) = rule {
            let Some(Some(cause)) = self.states.get(&code) else {
                break;
            };
            let bound = self
                .requirements
                .get(&ValueCode::Closure(code))
                .ok_or(Failure::InconsistentSyntax)?
                .body()
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
