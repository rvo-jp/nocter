//! Derived condition references belong to one immutable ordinary checked body.
use super::{INVALID, Rule, Site, Statement, Violation, flow, representation};
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
    },
    Inputs(AccessExpressionId),
    Data(Violation),
    Drop(drops::Condition),
    Cleanup(cleanup::Condition),
}
impl Clause {
    fn data_condition(&self) -> Option<Violation> {
        match self {
            Self::Data(violation) => Some(*violation),
            Self::Drop(condition) => Some(condition.violation()),
            Self::Cleanup(condition) => Some(condition.violation()),
            Self::Invocation { .. } | Self::Inputs(_) => None,
        }
    }
}

/// One completed edge borrowed from its matched definition-owned body product.
pub(in crate::value_access) struct CallEdge<'a> {
    expression: AccessExpressionId,
    template: &'a Result<CallTemplate, flow::IntegrityError>,
}
impl CallEdge<'_> {
    pub(super) fn template(&self) -> Result<&CallTemplate, flow::IntegrityError> {
        self.template.as_ref().map_err(|error| *error)
    }
    pub(in crate::value_access) fn expression(&self) -> AccessExpressionId {
        self.expression
    }
}

mod bound;
mod cleanup;
mod drops;
use bound::BoundCalls;
pub(in crate::value_access) use bound::{BoundCheck, BoundCheckFailure};

/// Definition-owned operation requirements, with the exact shared checked-body generation.
/// This product alone grants neither authored const capability nor executable representation.
#[derive(Debug)]
pub struct Requirements {
    body: Arc<flow::Query>,
    clauses: Box<[Clause]>,
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
        let mut projections = representation::Projections::default();
        let mut clauses = Vec::new();
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
                });
            }
        }
        clauses.extend(
            drops::collect(&body, &mut domain)?
                .into_iter()
                .map(Clause::Drop),
        );
        for (id, expression) in typed.expressions().iter() {
            if matches!(expression.kind, ExpressionKind::Call { .. }) {
                clauses.push(Clause::Inputs(id));
            }
            if let Some(value) = expression.ty.normal()
                && !domain.supports(program, value.data())?
                && !projections.uninhabited(&body, id, &mut domain)?
            {
                clauses.push(Clause::Data(Violation {
                    site: Site::Expression(id),
                    rule: Rule::UnsupportedData(value.data()),
                }));
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
        })
    }
    pub(in crate::value_access) fn check(
        &self,
        available: &dyn Fn(ValueClosure) -> bool,
    ) -> Result<Option<Violation>, flow::IntegrityError> {
        self.check_clauses(available, None, None)
            .map_err(|error| match error {
                BoundCheckFailure::Integrity(error) => error,
                BoundCheckFailure::Invocation { .. } | BoundCheckFailure::Resource { .. } => {
                    INVALID
                }
            })
    }
    fn check_clauses(
        &self,
        available: &dyn Fn(ValueClosure) -> bool,
        instance: Option<&CodeInstanceKey>,
        mut calls: Option<&mut BoundCalls<'_, '_>>,
    ) -> Result<Option<Violation>, BoundCheckFailure> {
        if instance.is_some_and(|key| key.code() != self.code()) {
            return Err(INVALID.into());
        }
        let program = self.body.typed().program();
        let expressions = self.body.typed().body().expressions();
        let mut domain = evaluation::Types::default();
        for clause in &self.clauses {
            let violation = match clause {
                Clause::Invocation { expression: id, .. } => {
                    let callee = match calls.as_deref_mut() {
                        Some(calls) => calls.resolve(*id)?,
                        None => None,
                    };
                    super::operation_rule(
                        program,
                        &expressions.get(*id).ok_or(INVALID)?.kind,
                        available,
                        callee,
                    )?
                    .map(|rule| Violation {
                        site: Site::Expression(*id),
                        rule,
                    })
                }
                Clause::Inputs(id) => {
                    let callee = match calls.as_deref_mut() {
                        Some(calls) => calls.resolve(*id)?,
                        None => None,
                    };
                    super::call_inputs(
                        program,
                        &expressions.get(*id).ok_or(INVALID)?.kind,
                        &mut domain,
                        callee,
                    )?
                }
                Clause::Data(violation) => {
                    let (Rule::UnsupportedData(data) | Rule::UnsupportedCleanup(data)) =
                        violation.rule()
                    else {
                        return Err(INVALID.into());
                    };
                    (!super::supports(program, data, &mut domain, instance)?).then_some(*violation)
                }
                Clause::Cleanup(condition) => {
                    let data = condition.data(&self.body)?;
                    let supported = match calls.as_deref_mut() {
                        Some(calls) => domain
                            .supports_instance(program, calls.cleanup(&self.body, condition)?)?,
                        None => super::supports(program, data, &mut domain, instance)?,
                    };
                    (!supported).then_some(condition.violation())
                }
                Clause::Drop(condition) => {
                    let supported = match calls.as_deref_mut() {
                        Some(calls) => calls.drop(&self.body, condition, &mut domain)?,
                        None => super::supports(program, condition.data()?, &mut domain, instance)?,
                    };
                    (!supported).then_some(condition.violation())
                }
            };
            if violation.is_some() {
                return Ok(violation);
            }
        }
        Ok(None)
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
                } => {
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
    pub(in crate::value_access) fn completed_calls(&self) -> impl Iterator<Item = CallEdge<'_>> {
        self.clauses.iter().filter_map(|clause| match clause {
            Clause::Invocation {
                expression,
                template: Some(template),
            } => Some(CallEdge {
                expression: *expression,
                template,
            }),
            _ => None,
        })
    }
    pub(in crate::value_access) fn call_expressions(
        &self,
    ) -> impl Iterator<Item = Result<(AccessExpressionId, &ExpressionKind), flow::IntegrityError>>
    {
        self.clauses
            .iter()
            .filter_map(|clause| match clause {
                Clause::Invocation { expression, .. } => Some(*expression),
                _ => None,
            })
            .map(|id| {
                Ok((
                    id,
                    &self
                        .body
                        .typed()
                        .body()
                        .expressions()
                        .get(id)
                        .ok_or(INVALID)?
                        .kind,
                ))
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
            } = clause
            else {
                continue;
            };
            let selected = match template.as_ref().map_err(|error| *error)? {
                CallTemplate::Code(callee) => {
                    CallTemplate::Code(callee.compose(program, key).map_err(|_| INVALID)?)
                }
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
