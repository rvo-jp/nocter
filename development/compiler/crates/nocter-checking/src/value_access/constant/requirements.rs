//! Derived condition references belong to one immutable ordinary checked body.
use super::{INVALID, Rule, Site, Statement, Violation, flow, representation};
use crate::value_access::{ExpressionKind, evaluation, instances};
use nocter_declarations::value_access::CodeInstanceKey;
use nocter_model::{AccessExpressionId, ValueClosure, ValueCode};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
enum Clause {
    Invocation(AccessExpressionId),
    Inputs(AccessExpressionId),
    Data(Violation),
}

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
        // Preserve the diagnostic ordering and fixed forbidden-operation checks in dormant source.
        for (id, expression) in typed.expressions().iter() {
            if matches!(expression.kind, ExpressionKind::Call { .. }) {
                clauses.push(Clause::Invocation(id));
            }
        }
        for (id, statement) in typed.statements().iter() {
            if let Statement::Drop(local) = statement {
                let data = typed.locals().get(*local).ok_or(INVALID)?.data;
                if !domain.supports(program, data)? {
                    clauses.push(Clause::Data(Violation {
                        site: Site::Statement(id),
                        rule: Rule::UnsupportedCleanup(data),
                    }));
                }
            }
        }
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
        // Only actual sparse cleanup operations contribute conditions; read inputs own nothing.
        for (point, block) in body.graph().points().iter() {
            for action in 0..block.actions.len() {
                let location = flow::Location { point, action };
                for transition in body.access().resources().at(location) {
                    for step in transition.steps() {
                        let payload = match step {
                            flow::resources::Step::Dispose { payload, .. }
                            | flow::resources::Step::DisposeArrayRemainder { payload, .. }
                            | flow::resources::Step::DisposeArrayElements { payload, .. }
                            | flow::resources::Step::ReplaceReferent { payload, .. } => *payload,
                            flow::resources::Step::Capture { .. }
                            | flow::resources::Step::AdaptFunction { .. }
                            | flow::resources::Step::Assemble { .. }
                            | flow::resources::Step::Construct { .. }
                            | flow::resources::Step::Receive { .. }
                            | flow::resources::Step::Move { .. }
                            | flow::resources::Step::MoveArrayElement { .. }
                            | flow::resources::Step::ConsolidateArrayPrefix { .. }
                            | flow::resources::Step::ReleaseContainer { .. }
                            | flow::resources::Step::Invoke { .. } => continue,
                        };
                        if !domain.supports(program, payload.data())? {
                            clauses.push(Clause::Data(Violation {
                                site: Site::Cleanup(location),
                                rule: Rule::UnsupportedCleanup(payload.data()),
                            }));
                        }
                    }
                }
            }
        }
        Ok(Self {
            body,
            clauses: clauses.into(),
        })
    }
    pub(in crate::value_access) fn check(
        &self,
        available: &dyn Fn(ValueClosure) -> bool,
        instance: Option<&CodeInstanceKey>,
    ) -> Result<Option<Violation>, flow::IntegrityError> {
        if instance.is_some_and(|key| key.code() != self.code()) {
            return Err(INVALID);
        }
        let program = self.body.typed().program();
        let expressions = self.body.typed().body().expressions();
        let mut domain = evaluation::Types::default();
        for clause in &self.clauses {
            let violation = match *clause {
                Clause::Invocation(id) => super::operation_rule(
                    program,
                    &expressions.get(id).ok_or(INVALID)?.kind,
                    available,
                    instance,
                )?
                .map(|rule| Violation {
                    site: Site::Expression(id),
                    rule,
                }),
                Clause::Inputs(id) => super::call_inputs(
                    program,
                    &expressions.get(id).ok_or(INVALID)?.kind,
                    &mut domain,
                    instance,
                )?,
                Clause::Data(violation) => {
                    let (Rule::UnsupportedData(data) | Rule::UnsupportedCleanup(data)) =
                        violation.rule()
                    else {
                        return Err(INVALID);
                    };
                    (!super::supports(program, data, &mut domain, instance)?).then_some(violation)
                }
            };
            if violation.is_some() {
                return Ok(violation);
            }
        }
        Ok(None)
    }
    pub(super) fn call_expressions(
        &self,
    ) -> impl Iterator<Item = Result<(AccessExpressionId, &ExpressionKind), flow::IntegrityError>>
    {
        self.clauses
            .iter()
            .filter_map(|clause| match clause {
                Clause::Invocation(id) => Some(*id),
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
    pub(in crate::value_access) fn bound_calls(
        &self,
        key: &CodeInstanceKey,
    ) -> Result<Vec<(AccessExpressionId, CodeInstanceKey)>, flow::IntegrityError> {
        if key.code() != self.code() {
            return Err(INVALID);
        }
        let program = self.body.typed().program();
        let mut calls = Vec::new();
        for item in self.call_expressions() {
            let (id, expression) = item?;
            if let Some(key) = instances::select_call(program, key, expression)? {
                calls.push((id, key));
            }
        }
        Ok(calls)
    }
}
