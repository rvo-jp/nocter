//! Finite symbolic invocation obligations; closed false facts never become positive capability.
use super::{Diagnostic, Failure, INVALID, Requirements, Rule, Site, Violation, flow};
use crate::value_access::{CallTemplate, ExpressionKind, results, source_origin};
use nocter_declarations::value_access::{CodeTemplateKey, FunctionWitness};
use nocter_model::{AccessExpressionId, ValueCode, ValueFunctionOrigin};
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug)]
struct Cause {
    code: ValueCode,
    expression: AccessExpressionId,
    parent: Option<usize>,
    parameter: Option<nocter_model::ValueParameter>,
}
#[derive(Debug)]
pub(super) struct Invocations {
    terms: BTreeMap<ValueCode, BTreeMap<FunctionWitness, usize>>,
    causes: Vec<Cause>,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct ViolationProof {
    pub(super) rule: Rule,
    cause: usize,
}
impl Invocations {
    pub(super) fn prepare(
        requirements: &BTreeMap<ValueCode, Requirements>,
    ) -> Result<Self, flow::IntegrityError> {
        let mut graph = Self {
            terms: BTreeMap::new(),
            causes: Vec::new(),
        };
        let mut callers =
            BTreeMap::<ValueCode, Vec<(ValueCode, AccessExpressionId, CodeTemplateKey)>>::new();
        let mut pending = VecDeque::new();
        for (&code, item) in requirements {
            graph.seed(item, &mut pending)?;
            for call in item.completed_calls() {
                // Missing producer/view metadata supplies no positive or negative callee fact.
                if let Ok(CallTemplate::Code(callee)) = call.template() {
                    callers.entry(callee.code()).or_default().push((
                        code,
                        call.expression(),
                        callee.clone(),
                    ));
                }
            }
        }
        while let Some((callee, witness)) = pending.pop_front() {
            let cause = *graph
                .terms
                .get(&callee)
                .and_then(|terms| terms.get(&witness))
                .ok_or(INVALID)?;
            for (code, expression, frame) in callers.get(&callee).into_iter().flatten() {
                let parameter = match witness.origin {
                    ValueFunctionOrigin::Parameter(parameter) => Some(parameter),
                    _ => None,
                };
                if parameter.is_some_and(|parameter| !frame.witnesses().contains_key(&parameter)) {
                    // An outside capture is not a callee formal. Its unqualified substitution
                    // supplies no closed failure and cannot strengthen the ordinary body.
                    continue;
                }
                let selected = if parameter.is_some() {
                    frame
                        .resolve(
                            requirements
                                .get(code)
                                .ok_or(INVALID)?
                                .body()
                                .typed()
                                .program(),
                            witness,
                        )
                        .map_err(|_| INVALID)?
                } else {
                    witness
                };
                if graph.insert(
                    requirements
                        .get(code)
                        .ok_or(INVALID)?
                        .body()
                        .typed()
                        .program(),
                    *code,
                    selected,
                    Cause {
                        code: *code,
                        expression: *expression,
                        parent: Some(cause),
                        parameter,
                    },
                )? {
                    pending.push_back((*code, selected));
                }
            }
        }
        Ok(graph)
    }
    fn seed(
        &mut self,
        item: &Requirements,
        pending: &mut VecDeque<(ValueCode, FunctionWitness)>,
    ) -> Result<(), flow::IntegrityError> {
        let code = item.code();
        for entry in item.call_expressions() {
            let (id, expression) = entry?;
            let ExpressionKind::Call {
                target,
                kind,
                arguments,
            } = expression
            else {
                return Err(INVALID);
            };
            if super::uncommitted_parameter_call(*kind, *target, arguments) {
                continue;
            }
            if !matches!(
                target,
                ValueFunctionOrigin::Declaration(_)
                    | ValueFunctionOrigin::Closure(_)
                    | ValueFunctionOrigin::Parameter(_)
            ) {
                continue;
            }
            let witness = FunctionWitness {
                origin: *target,
                invocation: kind.invocation(arguments).ok_or(INVALID)?,
            };
            if self.insert(
                item.body().typed().program(),
                code,
                witness,
                Cause {
                    code,
                    expression: id,
                    parent: None,
                    parameter: None,
                },
            )? {
                pending.push_back((code, witness));
            }
        }
        Ok(())
    }
    fn insert(
        &mut self,
        program: &nocter_declarations::value_access::Program,
        code: ValueCode,
        witness: FunctionWitness,
        cause: Cause,
    ) -> Result<bool, flow::IntegrityError> {
        // This is only the atomic authored-capability fact, never the callee's full eligibility.
        // Its own symbolic requirements still propagate independently through the stored edges.
        if matches!(witness.origin, ValueFunctionOrigin::Declaration(_))
            && super::invocation_rule(
                program,
                crate::value_access::CallKind::Value,
                witness.origin,
                &|_| true,
            )?
            .is_none()
        {
            return Ok(false);
        }
        let terms = self.terms.entry(code).or_default();
        if terms.contains_key(&witness) {
            return Ok(false);
        }
        terms.insert(witness, self.causes.len());
        self.causes.push(cause);
        Ok(true)
    }
    pub(super) fn violation(
        &self,
        item: &Requirements,
        available: &dyn Fn(nocter_model::ValueClosure) -> bool,
    ) -> Result<Option<ViolationProof>, flow::IntegrityError> {
        let program = item.body().typed().program();
        for (witness, &cause) in self.terms.get(&item.code()).into_iter().flatten() {
            let rule = super::invocation_rule(
                program,
                crate::value_access::CallKind::Value,
                witness.origin,
                available,
            )?;
            if let Some(rule) = rule {
                return Ok(Some(ViolationProof { rule, cause }));
            }
        }
        Ok(None)
    }
    pub(super) fn captured_dependents(
        &self,
    ) -> BTreeMap<nocter_model::ValueClosure, Vec<(nocter_model::ValueClosure, ViolationProof)>>
    {
        let mut dependents = BTreeMap::<_, Vec<_>>::new();
        for (&code, terms) in &self.terms {
            let ValueCode::Closure(source) = code else {
                continue;
            };
            for (witness, &cause) in terms {
                if let ValueFunctionOrigin::Closure(target) = witness.origin {
                    dependents.entry(target).or_default().push((
                        source,
                        ViolationProof {
                            rule: Rule::CapturedCall(target),
                            cause,
                        },
                    ));
                }
            }
        }
        dependents
    }
    pub(super) fn project(
        &self,
        proof: ViolationProof,
        requirements: &BTreeMap<ValueCode, Requirements>,
        verified: &results::Verified<'_, '_>,
    ) -> Result<(nocter_model::AccessCodeId, Diagnostic), Failure> {
        let mut chain = Vec::new();
        let mut current = proof.cause;
        loop {
            let cause = self
                .causes
                .get(current)
                .ok_or(Failure::InconsistentSyntax)?;
            chain.push(cause);
            let Some(parent) = cause.parent else {
                break;
            };
            if parent >= current {
                return Err(Failure::InconsistentSyntax);
            }
            current = parent;
        }
        let leaf = chain.last().ok_or(Failure::InconsistentSyntax)?;
        let item = requirements
            .get(&leaf.code)
            .ok_or(Failure::InconsistentSyntax)?;
        let bound = item.body().bind_verified(verified)?;
        let mut diagnostic = Violation {
            site: Site::Expression(leaf.expression),
            rule: proof.rule,
        }
        .project(&bound)?;
        for cause in chain.iter().rev().skip(1) {
            if let Some(parameter) = cause.parameter {
                let item = requirements
                    .get(&cause.code)
                    .ok_or(Failure::InconsistentSyntax)?;
                let bound = item.body().bind_verified(verified)?;
                let ExpressionKind::Call { arguments, .. } = &item
                    .body()
                    .typed()
                    .body()
                    .expressions()
                    .get(cause.expression)
                    .ok_or(Failure::InconsistentSyntax)?
                    .kind
                else {
                    return Err(Failure::InconsistentSyntax);
                };
                let argument = arguments
                    .iter()
                    .find(|argument| {
                        argument
                            .binding
                            .as_ref()
                            .is_some_and(|binding| binding.parameter == parameter)
                    })
                    .ok_or(Failure::InconsistentSyntax)?;
                let origin = bound
                    .typed()
                    .expression_origin(argument.value)
                    .ok_or(Failure::InconsistentSyntax)?;
                diagnostic.note_witness(source_origin(bound.typed().syntax(), origin)?);
            }
        }
        for cause in chain.iter().take(chain.len() - 1) {
            let item = requirements
                .get(&cause.code)
                .ok_or(Failure::InconsistentSyntax)?;
            let bound = item.body().bind_verified(verified)?;
            let origin = bound
                .typed()
                .expression_origin(cause.expression)
                .ok_or(Failure::InconsistentSyntax)?;
            diagnostic.note_call(source_origin(bound.typed().syntax(), origin)?);
        }
        Ok((leaf.code.source_owner(), diagnostic))
    }
}
