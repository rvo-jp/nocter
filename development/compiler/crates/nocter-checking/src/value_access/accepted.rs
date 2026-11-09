//! Closed front-end acceptance of every definition in one private first-fragment unit.
use super::{Failure as BodyFailure, Query as TypedQuery, flow};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::Program;
use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{AccessCallableId, TypeId};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug)]
pub enum Failure {
    Body {
        callable: AccessCallableId,
        failure: BodyFailure,
    },
    Flow {
        callable: AccessCallableId,
        diagnostics: Box<[SourceDiagnostic]>,
    },
    Const {
        callable: AccessCallableId,
        diagnostic: Box<super::constant::Diagnostic>,
    },
    Integrity(flow::IntegrityError),
}

/// Known data either has a logical payload contract or is a proved-unproduced callable shape.
/// The latter grants no storage, cleanup or executable environment obligation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataEvidence {
    Payload(flow::resources::Payload),
    Unproduced(nocter_declarations::value_access::FunctionConstraint),
}
impl DataEvidence {
    #[must_use]
    pub const fn payload(&self) -> Option<flow::resources::Payload> {
        match self {
            Self::Payload(payload) => Some(*payload),
            Self::Unproduced(_) => None,
        }
    }
}

/// Complete safety/cleanup evidence only for the closed private grammar, not native validity.
/// There is no parts constructor: bodies cannot be omitted or paired with another program.
#[derive(Clone, Debug)]
pub struct Unit {
    closures: BTreeMap<nocter_model::ValueClosure, flow::Query>,
    program: Arc<Program>,
    header: Arc<Program>,
    bodies: BTreeMap<AccessCallableId, flow::Query>,
    result_evidence: Arc<super::results::Evidence>,
}
impl Unit {
    /// Checks all definitions, including unreachable and mutually recursive callees.
    /// # Errors
    /// Returns source-backed typing/flow rejection, or an inconsistent compiler product.
    pub fn prepare(context: &DeclarationContext<'_>) -> Result<Self, Failure> {
        let mut queries = BTreeMap::new();
        for (callable, _) in context.program().callables().iter() {
            let body = TypedQuery::prepare(context, callable)
                .map_err(|failure| Failure::Body { callable, failure })?;
            queries.insert(callable, body);
        }
        let program = super::types::canonicalize(context, &mut queries)?;
        let mut controls = queries
            .into_iter()
            .map(|(id, body)| {
                Ok((
                    id,
                    flow::Control::prepare(Arc::new(body)).map_err(Failure::Integrity)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, Failure>>()?;
        let result_evidence = super::results::close(context, &mut controls)?;
        let verified = result_evidence
            .verify(context)
            .map_err(|_| Failure::Integrity(flow::IntegrityError::InconsistentTypedBody))?;
        let mut bodies = BTreeMap::new();
        let mut closures = BTreeMap::new();
        for (callable, control) in controls {
            if control.typed.code() != nocter_model::ValueCode::Declaration(callable) {
                return Err(Failure::Integrity(
                    flow::IntegrityError::InconsistentTypedBody,
                ));
            }
            for (&id, nested) in &control.typed.closures {
                if nested.typed.code() != nocter_model::ValueCode::Closure(id) {
                    return Err(Failure::Integrity(
                        flow::IntegrityError::InconsistentTypedBody,
                    ));
                }
                let checked = nested.clone().finish().map_err(Failure::Integrity)?;
                let checked = admit(checked, &verified)?;
                closures.insert(id, checked);
            }
            let query = control.finish().map_err(Failure::Integrity)?;
            let query = admit(query, &verified)?;
            bodies.insert(callable, query);
        }
        for (&callable, query) in &bodies {
            if program
                .callables()
                .get(callable)
                .ok_or(Failure::Integrity(
                    flow::IntegrityError::InconsistentTypedBody,
                ))?
                .compile_time()
                == nocter_model::CompileTimeGuarantee::Evaluatable
                && let Some(violation) =
                    super::constant::check(query).map_err(Failure::Integrity)?
            {
                let bound = query
                    .bind_verified(&verified)
                    .map_err(|failure| Failure::Body { callable, failure })?;
                let diagnostic = violation
                    .project(&bound)
                    .map_err(|failure| Failure::Body { callable, failure })?;
                return Err(Failure::Const {
                    callable,
                    diagnostic: Box::new(diagnostic),
                });
            }
        }
        Ok(Self {
            closures,
            program,
            header: context.program_handle(),
            bodies,
            result_evidence,
        })
    }
    /// The named/captured-code or own-input witness established for a callable result in this bounded unit.
    #[must_use]
    pub fn result_witness(
        &self,
        callable: AccessCallableId,
    ) -> Option<nocter_declarations::value_access::FunctionWitness> {
        self.result_evidence.producer(callable)
    }
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }
    /// Project data through this complete unit's closed result and payload evidence.
    /// # Errors
    /// Rejects an unknown type or a result identity without closed evidence.
    pub fn data_evidence(&self, data: TypeId) -> Result<DataEvidence, flow::IntegrityError> {
        if self.result_evidence.unproduced(self.program(), data) {
            let Some(nocter_declarations::value_access::PayloadSchema::Function(witness)) =
                self.program().data_schema(data)
            else {
                return Err(flow::IntegrityError::InconsistentTypedBody);
            };
            return self
                .program()
                .function_contract(witness)
                .map(DataEvidence::Unproduced)
                .ok_or(flow::IntegrityError::InconsistentTypedBody);
        }
        flow::resources::payload(self.program(), data).map(DataEvidence::Payload)
    }
    /// The exact checked body in this accepted unit's sole body catalog.
    #[must_use]
    pub fn body(&self, callable: AccessCallableId) -> Option<&flow::Query> {
        self.bodies.get(&callable)
    }
    /// Select the exact accepted named or captured body. Source ownership is not code identity.
    #[must_use]
    pub fn code_body(&self, code: nocter_model::ValueCode) -> Option<&flow::Query> {
        match code {
            nocter_model::ValueCode::Declaration(id) => self.bodies.get(&id),
            nocter_model::ValueCode::Closure(id) => self.closures.get(&id),
        }
    }
    #[must_use]
    pub fn bodies(&self) -> impl ExactSizeIterator<Item = (AccessCallableId, &flow::Query)> {
        self.bodies.iter().map(|(&id, query)| (id, query))
    }
    #[must_use]
    pub fn closures(&self) -> &BTreeMap<nocter_model::ValueClosure, flow::Query> {
        &self.closures
    }
    /// Rebinds every accepted body before exposing any current-source projection.
    /// # Errors
    /// Rejects changed authority or any changed body; never rechecks a sibling behind the caller.
    pub fn bind<'query, 'source>(
        &'query self,
        context: &DeclarationContext<'source>,
    ) -> Result<Context<'query, 'source>, BodyFailure> {
        if !Arc::ptr_eq(&self.header, &context.program_handle()) {
            return Err(BodyFailure::ChangedAuthority);
        }
        let verified = self.result_evidence.verify(context)?;
        let bodies = self
            .bodies
            .iter()
            .map(|(&id, query)| Ok((id, query.bind_verified(&verified)?)))
            .collect::<Result<_, BodyFailure>>()?;
        Ok(Context { unit: self, bodies })
    }
}
fn admit(
    query: flow::Query,
    verified: &super::results::Verified<'_, '_>,
) -> Result<flow::Query, Failure> {
    let callable = query.typed().source_owner();
    let bound = query
        .bind_verified(verified)
        .map_err(|failure| Failure::Body { callable, failure })?;
    let mut diagnostics = bound
        .diagnostics()
        .map_err(|failure| Failure::Body { callable, failure })?
        .into_iter()
        .map(|d| d.source().clone())
        .collect::<Vec<_>>();
    diagnostics.extend(
        bound
            .access_diagnostics()
            .map_err(|failure| Failure::Body { callable, failure })?
            .into_iter()
            .map(|d| d.source().clone()),
    );
    if !diagnostics.is_empty() {
        return Err(Failure::Flow {
            callable,
            diagnostics: diagnostics.into(),
        });
    }
    if query.access().status() != flow::access::Status::Analyzed {
        return Err(Failure::Integrity(
            flow::IntegrityError::InconsistentTypedBody,
        ));
    }
    Ok(query)
}
#[derive(Debug)]
pub struct Context<'query, 'source> {
    unit: &'query Unit,
    bodies: BTreeMap<AccessCallableId, flow::Context<'query, 'source>>,
}
impl Context<'_, '_> {
    #[must_use]
    pub const fn unit(&self) -> &Unit {
        self.unit
    }
    #[must_use]
    pub fn body(&self, id: AccessCallableId) -> Option<&flow::Context<'_, '_>> {
        self.bodies.get(&id)
    }
}

#[cfg(test)]
mod tests;
