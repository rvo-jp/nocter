//! Closed front-end acceptance of every definition in one private first-fragment unit.
use super::{Failure as BodyFailure, Query as TypedQuery, flow};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::Program;
use nocter_diagnostics::SourceDiagnostic;
use nocter_model::AccessCallableId;
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
    Integrity(flow::IntegrityError),
}

/// Complete safety/cleanup evidence only for the closed private grammar, not native validity.
/// There is no parts constructor: bodies cannot be omitted or paired with another program.
#[derive(Clone, Debug)]
pub struct Unit {
    program: Arc<Program>,
    bodies: BTreeMap<AccessCallableId, flow::Query>,
    result_evidence: Arc<super::results::Evidence>,
}
impl Unit {
    /// Checks all definitions, including unreachable and mutually recursive callees.
    /// # Errors
    /// Returns source-backed typing/flow rejection, or an inconsistent compiler product.
    pub fn prepare(context: &DeclarationContext<'_>) -> Result<Self, Failure> {
        let mut controls = BTreeMap::new();
        for (callable, _) in context.program().callables().iter() {
            let body = TypedQuery::prepare(context, callable)
                .map_err(|failure| Failure::Body { callable, failure })?;
            let control = flow::Control::prepare(Arc::new(body)).map_err(Failure::Integrity)?;
            controls.insert(callable, control);
        }
        let result_evidence = super::results::close(context, &mut controls)?;
        let verified = result_evidence
            .verify(context)
            .map_err(|_| Failure::Integrity(flow::IntegrityError::InconsistentTypedBody))?;
        let mut bodies = BTreeMap::new();
        for (callable, control) in controls {
            let query = control.finish().map_err(Failure::Integrity)?;
            let bound = query
                .bind_verified(&verified)
                .map_err(|failure| Failure::Body { callable, failure })?;
            let mut diagnostics = bound
                .diagnostics()
                .map_err(|failure| Failure::Body { callable, failure })?
                .into_iter()
                .map(|diagnostic| diagnostic.source().clone())
                .collect::<Vec<_>>();
            diagnostics.extend(
                bound
                    .access_diagnostics()
                    .map_err(|failure| Failure::Body { callable, failure })?
                    .into_iter()
                    .map(|diagnostic| diagnostic.source().clone()),
            );
            if !diagnostics.is_empty() {
                return Err(Failure::Flow {
                    callable,
                    diagnostics: diagnostics.into_boxed_slice(),
                });
            }
            if query.access().status() != flow::access::Status::Analyzed {
                return Err(Failure::Integrity(
                    flow::IntegrityError::InconsistentTypedBody,
                ));
            }
            bodies.insert(callable, query);
        }
        Ok(Self {
            program: context.program_handle(),
            bodies,
            result_evidence,
        })
    }
    /// The named producer established for a callable result in this bounded unit.
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
    /// The exact checked body in this accepted unit's sole body catalog.
    #[must_use]
    pub fn body(&self, callable: AccessCallableId) -> Option<&flow::Query> {
        self.bodies.get(&callable)
    }
    #[must_use]
    pub fn bodies(&self) -> impl ExactSizeIterator<Item = (AccessCallableId, &flow::Query)> {
        self.bodies.iter().map(|(&id, query)| (id, query))
    }
    /// Rebinds every accepted body before exposing any current-source projection.
    /// # Errors
    /// Rejects changed authority or any changed body; never rechecks a sibling behind the caller.
    pub fn bind<'query, 'source>(
        &'query self,
        context: &DeclarationContext<'source>,
    ) -> Result<Context<'query, 'source>, BodyFailure> {
        if !Arc::ptr_eq(&self.program, &context.program_handle()) {
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
