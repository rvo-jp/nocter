//! Original invocation bundles keep positive callable choices separate from executable selection.
use super::{
    CodeInstanceKey, ConcreteWitness, Fact, InstantiationFailure, Key, Query, ValueApplication,
    ValuePath,
};
use crate::value_access::{CallValueSources, results::Evidence};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::{InstanceSchema, Program};
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct CaseKey {
    bundle: usize,
    callee: ConcreteWitness,
    frame: CodeInstanceKey,
}
/// One original prepared bundle and its independently validated candidate frame.
/// Neither metadata nor a case's position grants a runtime selector, value or invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvocationCase {
    source: Arc<CallValueSources>,
    callee: ConcreteWitness,
    frame: CodeInstanceKey,
}
impl InvocationCase {
    #[must_use]
    pub fn source(&self) -> &CallValueSources {
        &self.source
    }
    #[must_use]
    pub const fn callee(&self) -> &ConcreteWitness {
        &self.callee
    }
    #[must_use]
    pub const fn frame(&self) -> &CodeInstanceKey {
        &self.frame
    }
}
/// A closed metadata domain, not an executable call or active-case receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvocationDomain {
    caller: CodeInstanceKey,
    site: ValueApplication,
    cases: Box<[InvocationCase]>,
}
impl InvocationDomain {
    #[must_use]
    pub const fn caller(&self) -> &CodeInstanceKey {
        &self.caller
    }
    #[must_use]
    pub const fn site(&self) -> ValueApplication {
        self.site
    }
    #[must_use]
    pub fn cases(&self) -> &[InvocationCase] {
        &self.cases
    }
}

pub(in crate::value_access) fn controlled_domain(
    program: &Program,
    evidence: &Evidence,
    caller: &CodeInstanceKey,
    site: ValueApplication,
    control: &crate::value_access::InstanceControl,
    work: &mut WorkBudget,
) -> Result<InvocationDomain, InstantiationFailure> {
    if caller.code() != site.owner {
        return Err(InstantiationFailure::WrongOwner);
    }
    if evidence.invocation(site).is_none() {
        return Err(InstantiationFailure::MissingCall);
    }
    if control.frame() != caller {
        return Err(InstantiationFailure::WrongOwner);
    }
    let mut query = Query::with_control(program, evidence, work, control);
    let Fact::Invocations(keys) = query.solve(&Key::Invocations(caller.clone(), site))? else {
        return Err(InstantiationFailure::Unresolved);
    };
    let original = evidence
        .value_sources(caller.code())
        .and_then(|graph| graph.call(site.call))
        .ok_or(InstantiationFailure::Unresolved)?;
    let mut sources = Vec::new();
    for source in original {
        query.step()?;
        for _ in &source.arguments {
            query.step()?;
        }
        for _ in source.parameters.keys() {
            query.step()?;
        }
        sources.push(Arc::new(source.clone()));
    }
    let cases = keys
        .into_iter()
        .map(|key| {
            query.step()?;
            Ok(InvocationCase {
                source: sources
                    .get(key.bundle)
                    .ok_or(InstantiationFailure::Unresolved)?
                    .clone(),
                callee: key.callee,
                frame: key.frame,
            })
        })
        .collect::<Result<Box<[_]>, InstantiationFailure>>()?;
    if cases.is_empty() {
        return Err(InstantiationFailure::Unresolved);
    }
    Ok(InvocationDomain {
        caller: caller.clone(),
        site,
        cases,
    })
}

impl Query<'_, '_> {
    /// Common result metadata from every original candidate, never a selected executable frame.
    pub(super) fn source_invocation_result(
        &mut self,
        owner: &Key,
        caller: &CodeInstanceKey,
        site: ValueApplication,
        path: &ValuePath<()>,
        data: Option<nocter_model::TypeId>,
    ) -> Result<Fact, InstantiationFailure> {
        let result = |frame| match data {
            Some(data) => Key::DataResult(frame, path.clone(), data),
            None => Key::Result(frame, path.clone()),
        };
        match self.read(owner, Key::Call(caller.clone(), site))? {
            Fact::Frame(frame) => return self.read(owner, result(frame)),
            Fact::Empty => return Ok(Fact::Empty),
            Fact::Unavailable if self.evidence.invocation(site).is_some() => {}
            _ => return Ok(Fact::Unavailable),
        }
        let candidates = self.read(owner, Key::Invocations(caller.clone(), site))?;
        let Fact::Invocations(candidates) = candidates else {
            return Ok(candidates);
        };
        let mut fact = Fact::Empty;
        for candidate in candidates {
            self.step()?;
            let incoming = self.read(owner, result(candidate.frame))?;
            fact = fact.join(incoming);
        }
        Ok(fact)
    }
    pub(super) fn source_invocation_domain(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        site: ValueApplication,
    ) -> Result<Fact, InstantiationFailure> {
        if frame.code() != site.owner {
            return Ok(Fact::Unavailable);
        }
        let Some(bundles) = self
            .evidence
            .value_sources(frame.code())
            .and_then(|graph| graph.call(site.call))
            .map(<[_]>::to_vec)
        else {
            return Ok(Fact::Unavailable);
        };
        let mut cases = BTreeSet::new();
        let mut pending = false;
        for (bundle_id, bundle) in bundles.iter().enumerate() {
            self.step()?;
            let Some(callee) = bundle.callee.witness_source() else {
                return Ok(Fact::Unavailable);
            };
            let Some(data) = self
                .source_node(frame, callee, &ValuePath::default())
                .and_then(|node| node.data)
            else {
                return Ok(Fact::Unavailable);
            };
            let fact = self.read(
                owner,
                Key::Data(frame.clone(), callee, ValuePath::default(), data),
            )?;
            let domain = match fact {
                Fact::Data(data) => data,
                Fact::Empty => {
                    pending = true;
                    continue;
                }
                _ => return Ok(Fact::Unavailable),
            };
            let alternatives = match domain.schema(self.program) {
                Ok(InstanceSchema::Function(single)) => vec![single],
                Ok(InstanceSchema::FunctionAlternatives(choices)) => choices.to_vec(),
                _ => return Ok(Fact::Unavailable),
            };
            for callee in alternatives {
                self.step()?;
                let selected = self.adapt(Fact::Witness(callee.clone()), bundle.invocation);
                let bound = self.source_bound_bundle(owner, frame, bundle, selected)?;
                match bound {
                    Fact::Frame(target) => {
                        cases.insert(CaseKey {
                            bundle: bundle_id,
                            callee,
                            frame: target,
                        });
                    }
                    Fact::Empty => pending = true,
                    _ => return Ok(Fact::Unavailable),
                }
            }
        }
        if pending {
            Ok(Fact::Empty)
        } else if cases.is_empty() {
            Ok(Fact::Unavailable)
        } else {
            Ok(Fact::Invocations(cases))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_access::{ExpressionKind, accepted::SourceUnit};
    use nocter_constant_evaluation::state::Limits;

    #[test]
    fn internal_candidate_queries_reject_foreign_sites_with_equal_call_slots() {
        let source = nocter_test_support::ValueSourceFixture::new(
            "func identity(value: i32): i32 { value }\nfunc make(): func(i32): i32 { identity }\nfunc first(): func(i32): i32 { make() }\nfunc second(): func(i32): i32 { make() }",
        );
        let unit = SourceUnit::prepare(source.syntax()).unwrap().into_unit();
        let program = unit.program();
        let code = |name| {
            program
                .resolve_code(program.symbols().get(name).unwrap())
                .unwrap()
        };
        let first = code("first");
        let second = code("second");
        let body = unit.code_body(first.into()).unwrap().typed();
        let call = body
            .body()
            .expressions()
            .iter()
            .find_map(|(id, expression)| {
                matches!(expression.kind, ExpressionKind::Call { .. }).then_some(id)
            })
            .unwrap();
        let evidence = body.result_evidence.as_ref().unwrap();
        let own = ValueApplication {
            owner: first.into(),
            call,
        };
        let foreign = ValueApplication {
            owner: second.into(),
            call,
        };
        assert!(evidence.invocation(own).is_some());
        assert!(evidence.invocation(foreign).is_some());
        let frame = program.bind_code(first.into(), []).unwrap();
        let mut work = WorkBudget::new(Limits::default().steps());
        let mut query = Query::new(program, evidence, &mut work);
        assert!(matches!(
            query.solve(&Key::Invocations(frame.clone(), own)).unwrap(),
            Fact::Invocations(_)
        ));
        assert_eq!(
            query.solve(&Key::Invocations(frame, foreign)).unwrap(),
            Fact::Unavailable
        );
    }
}
