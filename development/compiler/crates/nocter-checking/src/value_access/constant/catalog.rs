//! Definition-owned anonymous invocation availability, separate from environment transport.
use super::{Diagnostic, Failure, INVALID, Requirements, Rule, Violation, flow};
use nocter_diagnostics::DiagnosticNote;
use nocter_model::{ValueClosure, ValueCode};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

#[derive(Clone, Copy, Debug)]
enum Cause {
    Local(Violation),
    Conditional(super::conditions::ViolationProof),
}
#[derive(Debug)]
pub(in crate::value_access) struct Catalog {
    requirements: BTreeMap<ValueCode, Requirements>,
    states: BTreeMap<ValueClosure, Option<Cause>>,
    conditions: super::conditions::Conditions,
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
        let conditions = super::conditions::Conditions::prepare(&requirements)?;
        let mut states = BTreeMap::new();
        let dependents = conditions.captured_dependents();
        for (&source, item) in &requirements {
            let ValueCode::Closure(code) = source else {
                continue;
            };
            let mut violation = item.check(&|_| true, None)?.map(Cause::Local);
            if violation.is_none() {
                violation = conditions
                    .violation(item, &|target| {
                        requirements.contains_key(&ValueCode::Closure(target))
                    })?
                    .map(Cause::Conditional);
            }
            states.insert(code, violation);
        }
        let mut pending: VecDeque<_> = states
            .iter()
            .filter_map(|(&code, cause)| cause.is_some().then_some(code))
            .collect();
        while let Some(target) = pending.pop_front() {
            for &(code, proof) in dependents.get(&target).into_iter().flatten() {
                let state = states.get_mut(&code).ok_or(INVALID)?;
                if state.is_none() {
                    *state = Some(Cause::Conditional(proof));
                    pending.push_back(code);
                }
            }
        }
        Ok(Self {
            requirements,
            states,
            conditions,
        })
    }
    pub(in crate::value_access) fn closed_violation(
        &self,
        code: ValueCode,
        verified: &crate::value_access::results::Verified<'_, '_>,
    ) -> Result<
        Option<(nocter_model::AccessCodeId, Diagnostic)>,
        crate::value_access::accepted::Failure,
    > {
        let item = self
            .requirements
            .get(&code)
            .ok_or(crate::value_access::accepted::Failure::Integrity(INVALID))?;
        let Some(proof) = self
            .conditions
            .violation(item, &|code| self.permitted_template(code))
            .map_err(crate::value_access::accepted::Failure::Integrity)?
        else {
            return Ok(None);
        };
        let (callable, mut diagnostic) = self
            .conditions
            .project(proof, &self.requirements, verified)
            .map_err(|failure| crate::value_access::accepted::Failure::Body {
                callable: code.source_owner(),
                failure,
            })?;
        self.annotate(diagnostic.rule(), &mut diagnostic, verified)
            .map_err(|failure| crate::value_access::accepted::Failure::Body {
                callable,
                failure,
            })?;
        Ok(Some((callable, diagnostic)))
    }
    pub(in crate::value_access) fn prove_fixed_root(
        &self,
        root: ValueCode,
        verified: &crate::value_access::results::Verified<'_, '_>,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<bool, crate::value_access::accepted::Failure> {
        self.conditions
            .prove_fixed_root(root, &self.requirements, verified, work)
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
            let projected = match *cause {
                Cause::Local(violation) => {
                    let bound = self
                        .requirements
                        .get(&ValueCode::Closure(code))
                        .ok_or(Failure::InconsistentSyntax)?
                        .body()
                        .bind_verified(verified)?;
                    violation.project(&bound)?
                }
                Cause::Conditional(proof) => {
                    self.conditions
                        .project(proof, &self.requirements, verified)?
                        .1
                }
            };
            diagnostic.source.push_note(DiagnosticNote::new(
                format!(
                    "captured invocation depends on this operation: {}",
                    projected.source.message()
                ),
                projected.source.primary(),
            ));
            for note in projected.source.notes() {
                diagnostic.source.push_note(note.clone());
            }
            rule = projected.rule();
        }
        Ok(())
    }
}
