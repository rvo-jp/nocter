//! Finite symbolic invocation and direct-data obligations over the matched checked bodies.
use super::{Diagnostic, Failure, INVALID, Requirements, Rule, Site, Violation, flow};
use crate::value_access::{CallTemplate, ExpressionKind, results, source_origin};
use nocter_declarations::value_access::{CodeTemplateKey, FunctionWitness, PayloadSchema};
use nocter_model::{AccessExpressionId, ValueCode, ValueFunctionOrigin};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Term {
    Invocation(FunctionWitness),
    Representation(FunctionWitness),
    Cleanup(FunctionWitness),
    Input {
        operation: nocter_model::ValueInputOperation,
        witness: FunctionWitness,
    },
}
impl Term {
    fn witness(self) -> FunctionWitness {
        match self {
            Self::Invocation(witness)
            | Self::Representation(witness)
            | Self::Cleanup(witness)
            | Self::Input { witness, .. } => witness,
        }
    }
    fn substitute(self, witness: FunctionWitness) -> Self {
        match self {
            Self::Invocation(_) => Self::Invocation(witness),
            Self::Representation(_) => Self::Representation(witness),
            Self::Cleanup(_) => Self::Cleanup(witness),
            Self::Input { operation, .. } => Self::Input { operation, witness },
        }
    }
}
#[derive(Debug)]
struct Cause {
    code: ValueCode,
    site: Site,
    rule: Option<Rule>,
    parent: Option<usize>,
    parameter: Option<nocter_model::ValueParameter>,
}
impl Cause {
    fn expression(&self) -> Result<AccessExpressionId, Failure> {
        match self.site {
            Site::Expression(id) => Ok(id),
            _ => Err(Failure::InconsistentSyntax),
        }
    }
}
#[derive(Debug)]
pub(super) struct Conditions {
    terms: BTreeMap<ValueCode, BTreeMap<Term, usize>>,
    causes: Vec<Cause>,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct ViolationProof {
    pub(super) rule: Rule,
    cause: usize,
}
impl Conditions {
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
                    graph.seed_inputs(item, &call, callee, &mut pending)?;
                    callers.entry(callee.code()).or_default().push((
                        code,
                        call.expression(),
                        callee.clone(),
                    ));
                }
            }
        }
        while let Some((callee, term)) = pending.pop_front() {
            let cause = *graph
                .terms
                .get(&callee)
                .and_then(|terms| terms.get(&term))
                .ok_or(INVALID)?;
            let witness = term.witness();
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
                    term.substitute(selected),
                    Cause {
                        code: *code,
                        site: Site::Expression(*expression),
                        rule: None,
                        parent: Some(cause),
                        parameter,
                    },
                )? {
                    pending.push_back((*code, term.substitute(selected)));
                }
            }
        }
        Ok(graph)
    }
    fn seed(
        &mut self,
        item: &Requirements,
        pending: &mut VecDeque<(ValueCode, Term)>,
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
                Term::Invocation(witness),
                Cause {
                    code,
                    site: Site::Expression(id),
                    rule: None,
                    parent: None,
                    parameter: None,
                },
            )? {
                pending.push_back((code, Term::Invocation(witness)));
            }
        }
        for violation in item.data_conditions() {
            let (Rule::UnsupportedData(data) | Rule::UnsupportedCleanup(data)) = violation.rule()
            else {
                return Err(INVALID);
            };
            let Some(PayloadSchema::Function(witness)) =
                item.body().typed().program().data_schema(data)
            else {
                continue;
            };
            if !matches!(
                witness.origin,
                ValueFunctionOrigin::Parameter(_)
                    | ValueFunctionOrigin::Declaration(_)
                    | ValueFunctionOrigin::Closure(_)
            ) {
                continue;
            }
            let term = match violation.rule() {
                Rule::UnsupportedData(_) => Term::Representation(witness),
                Rule::UnsupportedCleanup(_) => Term::Cleanup(witness),
                _ => return Err(INVALID),
            };
            if self.insert(
                item.body().typed().program(),
                code,
                term,
                Cause {
                    code,
                    site: violation.site,
                    rule: Some(violation.rule()),
                    parent: None,
                    parameter: None,
                },
            )? {
                pending.push_back((code, term));
            }
        }
        Ok(())
    }
    fn seed_inputs(
        &mut self,
        item: &Requirements,
        call: &super::requirements::CallEdge<'_>,
        frame: &CodeTemplateKey,
        pending: &mut VecDeque<(ValueCode, Term)>,
    ) -> Result<(), flow::IntegrityError> {
        let program = item.body().typed().program();
        let ExpressionKind::Call {
            kind, arguments, ..
        } = &item
            .body()
            .typed()
            .body()
            .expressions()
            .get(call.expression())
            .ok_or(INVALID)?
            .kind
        else {
            return Err(INVALID);
        };
        let contract = super::input_contract(
            program,
            frame.code().origin().ok_or(INVALID)?,
            *kind,
            arguments,
        )?
        .ok_or(INVALID)?;
        let arguments = kind.explicit_arguments(arguments).ok_or(INVALID)?;
        if contract.inputs.len() != arguments.len() {
            return Err(INVALID);
        }
        for (input, argument) in contract.inputs.iter().zip(arguments) {
            if input.operation != argument.operation {
                return Err(INVALID);
            }
            let Some(PayloadSchema::Function(witness)) = program.data_schema(input.data) else {
                continue;
            };
            let selected = match witness.origin {
                ValueFunctionOrigin::Parameter(parameter)
                    if frame.witnesses().contains_key(&parameter) =>
                {
                    frame.resolve(program, witness).map_err(|_| INVALID)?
                }
                ValueFunctionOrigin::Declaration(_) | ValueFunctionOrigin::Closure(_) => witness,
                _ => continue,
            };
            let term = Term::Input {
                operation: input.operation,
                witness: selected,
            };
            if self.insert(
                program,
                item.code(),
                term,
                Cause {
                    code: item.code(),
                    site: Site::Expression(argument.value),
                    rule: Some(Rule::UnsupportedInput(input.data)),
                    parent: None,
                    parameter: None,
                },
            )? {
                pending.push_back((item.code(), term));
            }
        }
        Ok(())
    }
    fn insert(
        &mut self,
        program: &nocter_declarations::value_access::Program,
        code: ValueCode,
        term: Term,
        cause: Cause,
    ) -> Result<bool, flow::IntegrityError> {
        // This is only the atomic authored-capability fact, never the callee's full eligibility.
        // Its own symbolic requirements still propagate independently through the stored edges.
        let witness = term.witness();
        if matches!(term, Term::Invocation(_))
            && matches!(witness.origin, ValueFunctionOrigin::Declaration(_))
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
        if terms.contains_key(&term) {
            return Ok(false);
        }
        terms.insert(term, self.causes.len());
        self.causes.push(cause);
        Ok(true)
    }
    pub(super) fn violation(
        &self,
        item: &Requirements,
        available: &dyn Fn(nocter_model::ValueClosure) -> bool,
    ) -> Result<Option<ViolationProof>, flow::IntegrityError> {
        let program = item.body().typed().program();
        let mut domain = crate::value_access::evaluation::Types::default();
        for (&term, &cause) in self.terms.get(&item.code()).into_iter().flatten() {
            let witness = term.witness();
            let rule = match term {
                Term::Invocation(_) => super::invocation_rule(
                    program,
                    crate::value_access::CallKind::Value,
                    witness.origin,
                    available,
                )?,
                Term::Representation(_) | Term::Cleanup(_) | Term::Input { .. }
                    if matches!(witness.origin, ValueFunctionOrigin::Parameter(_)) =>
                {
                    None
                }
                Term::Representation(_) | Term::Cleanup(_) | Term::Input { .. } => {
                    let data = program.function_data(witness).ok_or(INVALID)?;
                    if domain.supports(program, data)? {
                        None
                    } else {
                        let mut current = cause;
                        loop {
                            let node = self.causes.get(current).ok_or(INVALID)?;
                            if let Some(rule) = node.rule {
                                break Some(rule);
                            }
                            current = node.parent.ok_or(INVALID)?;
                        }
                    }
                }
            };
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
            for (&term, &cause) in terms {
                if let Term::Invocation(FunctionWitness {
                    origin: ValueFunctionOrigin::Closure(target),
                    ..
                }) = term
                {
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
            site: leaf.site,
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
                    .get(cause.expression()?)
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
                .expression_origin(cause.expression()?)
                .ok_or(Failure::InconsistentSyntax)?;
            diagnostic.note_call(source_origin(bound.typed().syntax(), origin)?);
        }
        Ok((leaf.code.source_owner(), diagnostic))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_access::{
        accepted::SourceUnit,
        tests::{bound, input},
    };
    use std::sync::Arc;

    #[test]
    fn unused_owned_callable_cleanup_composes_without_becoming_an_invocation_condition() {
        let source = "const func discard(take action: once(i32): i32): void {}\nconst func inspect(action: func(i32): i32): void {}\nconst func update(edit action: mut(i32): i32): void {}\nfunc run(take input: error, take editing: error): void { let action = (input^; value: i32): i32 { value }\nvar update_action: mut(i32): i32 = (editing^; value: i32): i32 { value }\ninspect(action)\nupdate(update_action)\ndiscard(action^) }";
        let (sources, tree) = input(source);
        let unit = SourceUnit::prepare(bound(&sources, &tree))
            .unwrap()
            .into_unit();
        let mut requirements = BTreeMap::new();
        for (_, body) in unit.bodies() {
            let item = Requirements::prepare(Arc::new(body.clone())).unwrap();
            requirements.insert(item.code(), item);
        }
        for body in unit.closures().values() {
            let item = Requirements::prepare(body.clone()).unwrap();
            requirements.insert(item.code(), item);
        }
        let graph = Conditions::prepare(&requirements).unwrap();
        let program = unit.program();
        let code = |name| {
            ValueCode::Declaration(
                program
                    .resolve_code(program.symbols().get(name).unwrap())
                    .unwrap(),
            )
        };
        let discard = code("discard");
        let ValueCode::Declaration(id) = discard else {
            unreachable!();
        };
        let parameter = program.codes().get(id).unwrap().parameters()[0];
        let data = program.parameters().get(parameter).unwrap().data();
        let discard_terms = &graph.terms[&discard];
        assert!(discard_terms.keys().any(|term| matches!(term, Term::Cleanup(FunctionWitness { origin: ValueFunctionOrigin::Parameter(found), .. }) if *found == parameter.into())));
        assert!(
            discard_terms
                .keys()
                .all(|term| matches!(term, Term::Cleanup(_)))
        );
        assert!(!graph.terms.contains_key(&code("inspect")));
        assert!(!graph.terms.contains_key(&code("update")));
        let terms = &graph.terms[&code("run")];
        for operation in [
            nocter_model::ValueInputOperation::Read,
            nocter_model::ValueInputOperation::Edit,
            nocter_model::ValueInputOperation::Take,
        ] {
            assert!(terms.keys().any(|term| matches!(term, Term::Input { operation: found, witness: FunctionWitness { origin: ValueFunctionOrigin::Closure(_), .. } } if *found == operation)));
        }
        assert!(
            terms
                .keys()
                .all(|term| !matches!(term, Term::Invocation(_)))
        );

        let (&term, &cause) = terms
            .iter()
            .find(|(term, _)| {
                matches!(
                    term,
                    Term::Cleanup(FunctionWitness {
                        origin: ValueFunctionOrigin::Closure(_),
                        invocation: nocter_model::ValueInvocation::Take
                    })
                )
            })
            .unwrap();
        assert!(matches!(term, Term::Cleanup(_)));
        let mut current = cause;
        loop {
            let node = &graph.causes[current];
            if let Some(rule) = node.rule {
                assert_eq!(rule, Rule::UnsupportedCleanup(data));
                assert_eq!(node.code, discard);
                break;
            }
            current = node.parent.unwrap();
        }
    }
}
