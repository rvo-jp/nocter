//! Derived condition references belong to one immutable ordinary checked body.
use super::{INVALID, Rule, Site, Statement, Violation, flow};
use crate::value_access::{CallTemplate, ExpressionKind, evaluation, instances};
use nocter_declarations::value_access::{CodeInstanceKey, CodeTemplateKey};
use nocter_model::{AccessExpressionId, ValueClosure, ValueCode};
use std::sync::Arc;

#[derive(Clone, Debug)]
enum Clause {
    Invocation {
        expression: AccessExpressionId,
        // An interrupted call has no callee edge. Unresolved producer/view metadata is retained
        // as unavailable edge metadata, never permission to strengthen an ordinary runtime body.
        template: Option<Result<CallTemplate, flow::IntegrityError>>,
        // An input schema may permit this witness's payload to be absent. This attribution
        // grants no tag, initialization, source-value lineage or const eligibility proof.
        input_resource: Option<guards::InputResource>,
        receiver: Option<receivers::Condition>,
    },
    Inputs(AccessExpressionId),
    Data(expressions::Condition),
    Drop(drops::Condition),
    Cleanup(cleanup::Condition),
}
impl Clause {
    fn data_condition(&self) -> Option<Violation> {
        match self {
            Self::Data(condition) => Some(condition.violation()),
            Self::Drop(condition) => Some(condition.violation()),
            Self::Cleanup(condition) => Some(condition.violation()),
            Self::Invocation { .. } | Self::Inputs(_) => None,
        }
    }
}

/// One original source operation and its definition-owned resource attribution.
pub(super) struct CallExpression<'a> {
    pub expression: AccessExpressionId,
    pub kind: &'a ExpressionKind,
    pub input_resource: Option<nocter_model::AccessLocalId>,
}

/// One completed edge borrowed from its matched definition-owned body product.
pub(in crate::value_access) struct CallEdge<'a> {
    expression: AccessExpressionId,
    template: &'a Result<CallTemplate, flow::IntegrityError>,
    input_resource: Option<nocter_model::AccessLocalId>,
}
impl CallEdge<'_> {
    pub(super) fn template(&self) -> Result<&CallTemplate, flow::IntegrityError> {
        self.template.as_ref().map_err(|error| *error)
    }
    pub(super) fn input_resource(&self) -> Option<nocter_model::AccessLocalId> {
        self.input_resource
    }
    pub(in crate::value_access) fn expression(&self) -> AccessExpressionId {
        self.expression
    }
}

mod bound;
mod cleanup;
mod drops;
mod expressions;
mod guards;
mod receivers;
use bound::BoundContext;
pub(in crate::value_access) use bound::{BoundCheck, BoundCheckFailure, InvocationCondition};

/// Definition-owned operation requirements, with the exact shared checked-body generation.
/// This product alone grants neither authored const capability nor executable representation.
#[derive(Debug)]
pub struct Requirements {
    body: Arc<flow::Query>,
    clauses: Box<[Clause]>,
    input_writes: Option<std::collections::BTreeSet<nocter_model::AccessLocalId>>,
}
#[derive(Clone, Copy)]
enum CheckMode {
    Exact,
    Definition,
}
impl Requirements {
    #[must_use]
    pub fn code(&self) -> ValueCode {
        self.body.typed().code()
    }
    pub(in crate::value_access) fn body(&self) -> &flow::Query {
        &self.body
    }
    pub(super) fn prepare(body: Arc<flow::Query>) -> Result<Self, flow::IntegrityError> {
        let program = body.typed().program();
        let typed = body.typed().body();
        let mut domain = evaluation::Types::default();
        let mut expression_data = expressions::collect(&body, &mut domain)?;
        let mut clauses = Vec::new();
        let mut receivers = receivers::collect(&body)?;
        let input_writes = guards::writes(&body);
        let mut excluded_calls = std::collections::BTreeSet::new();
        let identity = program
            .code_template(body.typed().code())
            .map_err(|_| INVALID);
        // Preserve the diagnostic ordering and fixed forbidden-operation checks in dormant source.
        for (id, expression) in typed.expressions().iter() {
            if let ExpressionKind::Call {
                target,
                kind,
                arguments,
            } = &expression.kind
            {
                if excluded_call(&body, id, *target, *kind, arguments) {
                    excluded_calls.insert(id);
                    continue;
                }
                let template = instances::completed(arguments).then(|| {
                    identity
                        .as_ref()
                        .map_err(|error| *error)
                        .and_then(|identity| {
                            instances::instantiate_template_call(
                                program, identity, *target, *kind, arguments,
                            )
                        })
                });
                clauses.push(Clause::Invocation {
                    expression: id,
                    template,
                    input_resource: guards::input_resource(&body, *target, input_writes.as_ref())?,
                    receiver: receivers.remove(&id),
                });
            }
        }
        clauses.extend(
            drops::collect(&body, &mut domain)?
                .into_iter()
                .map(Clause::Drop),
        );
        for (id, expression) in typed.expressions().iter() {
            if matches!(expression.kind, ExpressionKind::Call { .. })
                && !excluded_calls.contains(&id)
            {
                clauses.push(Clause::Inputs(id));
            }
            if let Some(condition) = expression_data.remove(&id) {
                clauses.push(Clause::Data(condition));
            }
        }
        clauses.extend(
            cleanup::collect(&body, &mut domain)?
                .into_iter()
                .map(Clause::Cleanup),
        );
        Ok(Self {
            body,
            clauses: clauses.into(),
            input_writes,
        })
    }
    pub(in crate::value_access) fn check(
        &self,
        available: &dyn Fn(ValueClosure) -> bool,
    ) -> Result<Option<Violation>, flow::IntegrityError> {
        self.check_clauses(available, None, None, CheckMode::Exact)
            .map_err(|error| match error {
                BoundCheckFailure::Integrity(error) => error,
                BoundCheckFailure::Invocation { .. }
                | BoundCheckFailure::Data { .. }
                | BoundCheckFailure::Control(_) => INVALID,
            })
    }
    fn check_clauses(
        &self,
        available: &dyn Fn(ValueClosure) -> bool,
        instance: Option<&CodeInstanceKey>,
        mut calls: Option<&mut BoundContext<'_, '_>>,
        mode: CheckMode,
    ) -> Result<Option<Violation>, BoundCheckFailure> {
        if instance.is_some_and(|key| key.code() != self.code()) {
            return Err(INVALID.into());
        }
        let mut domain = evaluation::Types::default();
        for clause in &self.clauses {
            if let Some(calls) = calls.as_deref()
                && !calls.active(clause)?
            {
                continue;
            }
            let violation = match self.check_clause(
                clause,
                available,
                instance,
                calls.as_deref_mut(),
                &mut domain,
            ) {
                // Definition-only queries have no observed input payload. Retain the original
                // data/cleanup clause for exact-instance admission; this proves no eligibility.
                Err(BoundCheckFailure::Data {
                    failure: crate::value_access::InstantiationFailure::Unresolved,
                    ..
                }) if matches!(mode, CheckMode::Definition) => continue,
                Err(BoundCheckFailure::Invocation {
                    expression,
                    failure: crate::value_access::InstantiationFailure::Unresolved,
                }) if matches!(mode, CheckMode::Definition) => {
                    let calls = calls.as_deref_mut().ok_or(INVALID)?;
                    if calls.retain_input_capability(
                        &self.body,
                        expression,
                        self.input_writes.as_ref(),
                    )? {
                        continue;
                    }
                    return Err(BoundCheckFailure::Invocation {
                        expression,
                        failure: crate::value_access::InstantiationFailure::Unresolved,
                    });
                }
                result => result?,
            };
            if violation.is_some() {
                return Ok(violation);
            }
        }
        Ok(None)
    }
    fn check_clause(
        &self,
        clause: &Clause,
        available: &dyn Fn(ValueClosure) -> bool,
        instance: Option<&CodeInstanceKey>,
        mut calls: Option<&mut BoundContext<'_, '_>>,
        domain: &mut evaluation::Types,
    ) -> Result<Option<Violation>, BoundCheckFailure> {
        let program = self.body.typed().program();
        let expressions = self.body.typed().body().expressions();
        Ok(match clause {
            Clause::Invocation {
                expression: id,
                input_resource,
                receiver,
                ..
            } => {
                let violation = self.invocation_clause(
                    *id,
                    receiver.as_ref(),
                    available,
                    calls.as_deref_mut(),
                )?;
                if let (Some(violation), Some(calls)) = (violation, calls.as_deref_mut()) {
                    if calls.defer_invocation(
                        &self.body,
                        *id,
                        input_resource.as_ref(),
                        violation,
                    )? {
                        None
                    } else {
                        Some(violation)
                    }
                } else {
                    violation
                }
            }
            Clause::Inputs(id) => {
                let callees = match calls.as_deref_mut() {
                    Some(calls) => calls.resolve_all(*id)?,
                    None => None,
                };
                let expression = &expressions.get(*id).ok_or(INVALID)?.kind;
                first_violation(callees, |callee| {
                    super::call_inputs(program, expression, domain, callee)
                })?
            }
            Clause::Data(condition) => {
                let supported = match calls.as_deref_mut() {
                    Some(calls) => calls.expression(&self.body, condition, domain)?,
                    None => {
                        super::supports(program, condition.data(&self.body)?, domain, instance)?
                    }
                };
                (!supported).then_some(condition.violation())
            }
            Clause::Cleanup(condition) => {
                let data = condition.data(&self.body)?;
                let supported = match calls.as_deref_mut() {
                    Some(calls) => {
                        domain.supports_instance(program, calls.cleanup(&self.body, condition)?)?
                    }
                    None => super::supports(program, data, domain, instance)?,
                };
                (!supported).then_some(condition.violation())
            }
            Clause::Drop(condition) => {
                let supported = match calls {
                    Some(calls) => calls.drop(&self.body, condition, domain)?,
                    None => super::supports(program, condition.data()?, domain, instance)?,
                };
                (!supported).then_some(condition.violation())
            }
        })
    }
    fn invocation_clause(
        &self,
        id: AccessExpressionId,
        receiver: Option<&receivers::Condition>,
        available: &dyn Fn(ValueClosure) -> bool,
        calls: Option<&mut BoundContext<'_, '_>>,
    ) -> Result<Option<Violation>, BoundCheckFailure> {
        let expression = &self
            .body
            .typed()
            .body()
            .expressions()
            .get(id)
            .ok_or(INVALID)?
            .kind;
        if let Some(receiver) = receiver {
            // Receiver eligibility is independent of an invocation edge or callee input frame.
            return match calls {
                Some(calls) => calls.interrupted_invocation(id, &receiver.selections, available),
                None => Ok(None),
            };
        }
        let callees = match calls {
            Some(calls) => calls.resolve_all(id)?,
            None => None,
        };
        first_violation(callees, |callee| {
            let origin = callee
                .map(|callee| callee.code().origin().ok_or(INVALID))
                .transpose()?;
            super::operation_rule(self.body.typed().program(), expression, available, origin).map(
                |rule| {
                    rule.map(|rule| Violation {
                        site: Site::Expression(id),
                        rule,
                    })
                },
            )
        })
        .map_err(Into::into)
    }
    pub(in crate::value_access) fn needs_value_context(&self) -> bool {
        self.body
            .typed()
            .program()
            .payload_dependencies_all(self.body.typed().body().expressions().iter().filter_map(
                |(_, expression)| expression.ty.access().map(nocter_model::ValueAccess::data),
            ))
            .is_some_and(|dependencies| !dependencies.contracts().is_empty())
    }
    /// Coverage of the current conjunctive vocabulary, not eligibility or executable layout.
    pub(super) fn clause_coverage(&self) -> Result<bool, flow::IntegrityError> {
        let program = self.body.typed().program();
        let Ok(identity) = program.code_template(self.code()) else {
            return Ok(false);
        };
        let mut domain = evaluation::Types::default();
        for clause in &self.clauses {
            match clause {
                Clause::Invocation {
                    expression,
                    template,
                    input_resource,
                    ..
                } => {
                    // A flat conjunction does not discharge a possible resource-state guard.
                    if input_resource.is_some() {
                        return Ok(false);
                    }
                    let ExpressionKind::Call {
                        target,
                        kind,
                        arguments,
                    } = &self
                        .body
                        .typed()
                        .body()
                        .expressions()
                        .get(*expression)
                        .ok_or(INVALID)?
                        .kind
                    else {
                        return Err(INVALID);
                    };
                    if super::uncommitted_parameter_call(*kind, *target, arguments) {
                        continue;
                    }
                    if target.code().is_none() {
                        let nocter_model::ValueFunctionOrigin::Parameter(parameter) = target else {
                            return Ok(false);
                        };
                        if !identity.witnesses().contains_key(parameter)
                            || !matches!(template, Some(Ok(CallTemplate::Parameter(_))))
                        {
                            return Ok(false);
                        }
                    }
                    if target.code().is_some()
                        && instances::completed(arguments)
                        && !matches!(template, Some(Ok(CallTemplate::Code(_))))
                    {
                        return Ok(false);
                    }
                }
                Clause::Inputs(id) => {
                    if !self.inputs_covered(*id, &mut domain)? {
                        return Ok(false);
                    }
                }
                Clause::Data(_) | Clause::Drop(_) | Clause::Cleanup(_) => {
                    let violation = clause.data_condition().ok_or(INVALID)?;
                    let (Rule::UnsupportedData(data) | Rule::UnsupportedCleanup(data)) =
                        violation.rule()
                    else {
                        return Err(INVALID);
                    };
                    let Some(nocter_declarations::value_access::PayloadSchema::Function(witness)) =
                        program.data_schema(data)
                    else {
                        return Ok(false);
                    };
                    let nocter_model::ValueFunctionOrigin::Parameter(parameter) = witness.origin
                    else {
                        return Ok(false);
                    };
                    if !identity.witnesses().contains_key(&parameter) {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }
    /// The receiving boundary of a parameter-selected call must be independent of a further
    /// callable binding frame. Its selected body is covered separately by `Term::Body`.
    fn inputs_covered(
        &self,
        id: AccessExpressionId,
        domain: &mut evaluation::Types,
    ) -> Result<bool, flow::IntegrityError> {
        let program = self.body.typed().program();
        let ExpressionKind::Call {
            target,
            kind,
            arguments,
        } = &self
            .body
            .typed()
            .body()
            .expressions()
            .get(id)
            .ok_or(INVALID)?
            .kind
        else {
            return Err(INVALID);
        };
        if !instances::completed(arguments) {
            return Ok(true);
        }
        let contract = if matches!(target, nocter_model::ValueFunctionOrigin::Parameter(_)) {
            // A parameter-selected body is a separate retained obligation. For now its
            // receiving boundary must be witness-independent: no callable arguments.
            if arguments.iter().any(|argument| argument.binding.is_some()) {
                return Ok(false);
            }
            let Some(contract) =
                program.function_contract(nocter_declarations::value_access::FunctionWitness {
                    origin: *target,
                    invocation: kind.invocation(arguments).ok_or(INVALID)?,
                })
            else {
                return Ok(false);
            };
            let explicit = kind.explicit_arguments(arguments).ok_or(INVALID)?;
            if explicit.len() != contract.inputs.len() {
                return Err(INVALID);
            }
            for (argument, input) in explicit.iter().zip(&contract.inputs) {
                if argument.operation != input.operation {
                    return Err(INVALID);
                }
                if matches!(
                    program.data_schema(input.data),
                    Some(nocter_declarations::value_access::PayloadSchema::Function(
                        _
                    ))
                ) || !domain.supports(program, input.data)?
                {
                    return Ok(false);
                }
            }
            return Ok(true);
        } else {
            let Some(contract) = super::input_contract(program, *target, *kind, arguments)? else {
                return Ok(false);
            };
            contract
        };
        for input in &contract.inputs {
            match program.data_schema(input.data).ok_or(INVALID)? {
                nocter_declarations::value_access::PayloadSchema::Function(witness)
                    if matches!(
                        witness.origin,
                        nocter_model::ValueFunctionOrigin::Parameter(_)
                            | nocter_model::ValueFunctionOrigin::Declaration(_)
                            | nocter_model::ValueFunctionOrigin::Closure(_)
                    ) => {}
                nocter_declarations::value_access::PayloadSchema::Function(_) => {
                    return Ok(false);
                }
                _ if domain.supports(program, input.data)? => {}
                _ => return Ok(false),
            }
        }
        Ok(true)
    }
    pub(super) fn data_conditions(&self) -> impl Iterator<Item = Violation> + '_ {
        self.clauses.iter().filter_map(Clause::data_condition)
    }
    /// Definition-owned schema attributions, not permission to defer a failed obligation.
    #[cfg(test)]
    pub(in crate::value_access) fn invocation_input_resources(
        &self,
    ) -> impl Iterator<Item = (AccessExpressionId, nocter_model::AccessLocalId)> + '_ {
        self.clauses.iter().filter_map(|clause| match clause {
            Clause::Invocation {
                expression,
                input_resource: Some(input),
                ..
            } => Some((*expression, input.local())),
            _ => None,
        })
    }
    #[cfg(test)]
    pub(in crate::value_access) fn stable_invocation_inputs(
        &self,
    ) -> impl Iterator<Item = (AccessExpressionId, nocter_model::AccessLocalId)> + '_ {
        self.clauses.iter().filter_map(|clause| match clause {
            Clause::Invocation {
                expression,
                input_resource: Some(input),
                ..
            } if input.stable => Some((*expression, input.local())),
            _ => None,
        })
    }
    pub(in crate::value_access) fn completed_calls(&self) -> impl Iterator<Item = CallEdge<'_>> {
        self.clauses.iter().filter_map(|clause| match clause {
            Clause::Invocation {
                expression,
                template: Some(template),
                input_resource,
                ..
            } => Some(CallEdge {
                expression: *expression,
                template,
                input_resource: input_resource.as_ref().map(guards::InputResource::local),
            }),
            _ => None,
        })
    }
    /// Original invocation clauses, including candidates that a flat conjunction cannot discharge.
    pub(super) fn call_expressions(
        &self,
    ) -> impl Iterator<Item = Result<CallExpression<'_>, flow::IntegrityError>> {
        self.clauses
            .iter()
            .filter_map(|clause| match clause {
                Clause::Invocation {
                    expression,
                    input_resource,
                    ..
                } => Some((
                    *expression,
                    input_resource.as_ref().map(guards::InputResource::local),
                )),
                _ => None,
            })
            .map(|(id, resource)| {
                Ok(CallExpression {
                    expression: id,
                    kind: &self
                        .body
                        .typed()
                        .body()
                        .expressions()
                        .get(id)
                        .ok_or(INVALID)?
                        .kind,
                    input_resource: resource,
                })
            })
    }
    /// Compose completed source call bindings without choosing a code for an unresolved parameter.
    /// Each edge retains its original operation ID in this exact body product.
    /// # Errors
    /// Rejects a mismatched frame, invalid frozen view or incompatible binding metadata.
    pub fn template_calls(
        &self,
        key: &CodeTemplateKey,
    ) -> Result<Vec<(AccessExpressionId, crate::value_access::CallTemplate)>, flow::IntegrityError>
    {
        if key.code() != self.code() {
            return Err(INVALID);
        }
        let program = self.body.typed().program();
        let mut calls = Vec::new();
        for clause in &self.clauses {
            let Clause::Invocation {
                expression: id,
                template: Some(template),
                ..
            } = clause
            else {
                continue;
            };
            let selected = match template.as_ref().map_err(|error| *error)? {
                CallTemplate::Code(callee) => {
                    CallTemplate::Code(callee.compose(program, key).map_err(|_| INVALID)?)
                }
                CallTemplate::Applied(callee) => CallTemplate::Applied(callee.clone()),
                CallTemplate::Parameter(_) => {
                    let ExpressionKind::Call {
                        target,
                        kind,
                        arguments,
                    } = &self
                        .body
                        .typed()
                        .body()
                        .expressions()
                        .get(*id)
                        .ok_or(INVALID)?
                        .kind
                    else {
                        return Err(INVALID);
                    };
                    instances::instantiate_template_call(program, key, *target, *kind, arguments)?
                }
            };
            calls.push((*id, selected));
        }
        Ok(calls)
    }
}

/// A proved empty target has no invocation obligation outside the reached call channels.
fn excluded_call(
    body: &flow::Query,
    id: AccessExpressionId,
    target: nocter_model::ValueFunctionOrigin,
    kind: crate::value_access::CallKind,
    arguments: &[crate::value_access::Argument],
) -> bool {
    let query = body.typed();
    let Some(evidence) = &query.result_evidence else {
        return false;
    };
    let Some(sources) = evidence.value_sources(query.code()) else {
        return false;
    };
    let Some(invocation) = kind.invocation(arguments) else {
        return false;
    };
    let Some(data) =
        query
            .program()
            .function_data(nocter_declarations::value_access::FunctionWitness {
                origin: target,
                invocation,
            })
    else {
        return false;
    };
    sources.call(id).is_none() && evidence.excludes_value(query.program(), data)
}

/// A bound clause must hold for every original complete frame, never a representative.
fn first_violation<'a>(
    candidates: Option<&'a [CodeInstanceKey]>,
    mut check: impl FnMut(
        Option<&'a CodeInstanceKey>,
    ) -> Result<Option<Violation>, flow::IntegrityError>,
) -> Result<Option<Violation>, flow::IntegrityError> {
    let Some(candidates) = candidates else {
        return check(None);
    };
    if candidates.is_empty() {
        return Err(INVALID);
    }
    for candidate in candidates {
        let violation = check(Some(candidate))?;
        if violation.is_some() {
            return Ok(violation);
        }
    }
    Ok(None)
}
