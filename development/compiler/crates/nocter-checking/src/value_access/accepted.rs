//! Closed front-end acceptance of every definition in one private first-fragment unit.
pub use super::staged::{ModuleSourceUnit, SourceFailure, SourceUnit};
mod const_admission;
use super::{Failure as BodyFailure, Query as TypedQuery, flow};
pub(super) use const_admission::check_required_const;
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::Program;
use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{AccessCodeId, TypeId};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Debug)]
pub enum Failure {
    Body {
        callable: AccessCodeId,
        failure: BodyFailure,
    },
    Flow {
        callable: AccessCodeId,
        diagnostics: Box<[SourceDiagnostic]>,
    },
    Evaluation {
        callable: AccessCodeId,
        diagnostic: Box<super::evaluation::Diagnostic>,
    },
    Const {
        callable: AccessCodeId,
        diagnostic: Box<super::constant::Diagnostic>,
    },
    Integrity(flow::IntegrityError),
}

/// Known data either has a logical payload contract or is a proved-unproduced shape.
/// The latter grants no storage, cleanup or executable environment obligation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataEvidence<P = flow::resources::Payload> {
    Payload(P),
    /// The program remains the sole shape/contract authority; no payload is supplied here.
    Unproduced,
}
impl<P: Clone> DataEvidence<P> {
    #[must_use]
    pub fn payload(&self) -> Option<P> {
        match self {
            Self::Payload(payload) => Some(payload.clone()),
            Self::Unproduced => None,
        }
    }
}

/// Complete safety/cleanup evidence only for the closed private grammar, not native validity.
/// There is no parts constructor: bodies cannot be omitted or paired with another program.
#[derive(Clone, Debug)]
pub struct Unit {
    initializers: BTreeMap<AccessCodeId, Arc<nocter_model::FrozenData>>,
    closures: BTreeMap<nocter_model::ValueClosure, Arc<flow::Query>>,
    program: Arc<Program>,
    header: Arc<Program>,
    primitives: nocter_declaration_lowering::access_declarations::PrimitiveBindings,
    bodies: BTreeMap<nocter_model::ValueCodeRoot, Arc<flow::Query>>,
    result_evidence: Arc<super::results::Evidence>,
    const_templates: Arc<super::constant::Catalog>,
}
impl Unit {
    /// Checks all definitions, including unreachable and mutually recursive callees.
    /// # Errors
    /// Returns source-backed typing/flow rejection, or an inconsistent compiler product.
    pub fn prepare(context: &DeclarationContext<'_>) -> Result<Self, Failure> {
        Self::prepare_with_limits(
            context,
            nocter_constant_evaluation::CompileTimeEvaluationLimits::default(),
        )
    }
    /// Checks and evaluates required initializers with explicit deterministic limits.
    /// # Errors
    /// Rejects an invalid body, unsupported domain, failed required evaluation or compiler integrity.
    pub fn prepare_with_limits(
        context: &DeclarationContext<'_>,
        limits: impl Into<nocter_constant_evaluation::state::Limits>,
    ) -> Result<Self, Failure> {
        if !context.program().headers_complete() {
            return Err(Failure::Integrity(
                flow::IntegrityError::InconsistentTypedBody,
            ));
        }
        let evaluation = super::evaluation::prepare(context, limits.into())?;
        Self::close_evaluation(context, evaluation)
    }
    pub(super) fn close_evaluation(
        context: &DeclarationContext<'_>,
        evaluation: super::evaluation::Output,
    ) -> Result<Self, Failure> {
        let declarations = context.program().codes();
        let complete_bodies = declarations.iter().all(|(id, code)| {
            let checked_role = context.primitives().role(id).is_some();
            let source_body = evaluation.queries.contains_key(&id.into());
            if code.kind() == nocter_declarations::value_access::CodeKind::Primitive {
                checked_role && !source_body
            } else {
                !checked_role && source_body
            }
        }) && evaluation.queries.keys().all(|root| match root {
            nocter_model::ValueCodeRoot::Declaration(id) => declarations.get(*id).is_some(),
            nocter_model::ValueCodeRoot::Expression(_) => true,
        });
        let requires_value = |root| match root {
            nocter_model::ValueCodeRoot::Declaration(id) => declarations
                .get(id)
                .is_some_and(|code| code.kind().is_initializer()),
            nocter_model::ValueCodeRoot::Expression(_) => true,
        };
        let complete_values = evaluation
            .queries
            .keys()
            .all(|&root| !requires_value(root) || evaluation.values.contains_key(&root))
            && evaluation
                .values
                .keys()
                .all(|&root| requires_value(root) && evaluation.queries.contains_key(&root));
        if !context.program().headers_complete() || !complete_bodies || !complete_values {
            return Err(Failure::Integrity(
                flow::IntegrityError::InconsistentTypedBody,
            ));
        }
        context
            .verify_header_values(|id| evaluation.values.get(&id.into()).map(AsRef::as_ref))
            .map_err(|_| Failure::Integrity(flow::IntegrityError::InconsistentTypedBody))?;
        let queries = evaluation.queries;
        let Checked {
            program,
            bodies,
            closures,
            result_evidence,
            const_templates,
        } = check_queries(context, queries)?;
        Ok(Self {
            initializers: evaluation
                .values
                .into_iter()
                .filter_map(|(root, value)| match root {
                    nocter_model::ValueCodeRoot::Declaration(id) => Some((id, value)),
                    nocter_model::ValueCodeRoot::Expression(_) => None,
                })
                .collect(),
            closures,
            program,
            header: context.program_handle(),
            primitives: context.primitives().clone(),
            bodies,
            result_evidence,
            const_templates,
        })
    }
    #[must_use]
    pub fn constant_value(&self, id: AccessCodeId) -> Option<&nocter_model::FrozenData> {
        (self.program.codes().get(id)?.kind()
            == nocter_declarations::value_access::CodeKind::ConstantInitializer)
            .then_some(())?;
        self.initializers.get(&id).map(Arc::as_ref)
    }
    #[must_use]
    pub fn static_value(&self, id: nocter_model::ValueStatic) -> Option<&nocter_model::FrozenData> {
        self.program.static_declaration(id)?;
        self.initializers.get(&id.initializer).map(Arc::as_ref)
    }
    /// The named/captured-code or own-input witness established for a callable result in this bounded unit.
    #[must_use]
    pub fn result_witness(
        &self,
        callable: AccessCodeId,
    ) -> Option<nocter_declarations::value_access::FunctionWitness> {
        self.result_evidence.producer(callable)
    }
    /// Resolve one original call's concrete metadata frame without choosing a value alternative.
    /// # Errors
    /// Rejects a foreign owner, missing call, unresolved/ambiguous context or exhausted work.
    /// No partial query state, executable target, const permission or ownership is published.
    pub fn instantiate_invocation(
        &self,
        caller: &nocter_declarations::value_access::CodeInstanceKey,
        site: nocter_model::ValueApplication,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<nocter_declarations::value_access::CodeInstanceKey, super::InstantiationFailure>
    {
        super::results::instantiate_invocation(
            &self.program,
            &self.result_evidence,
            caller,
            site,
            work,
        )
    }
    /// Original bindings of a reached committed call, independently of its result data.
    /// The recipe belongs to this exact accepted Unit and grants no executable call/frame proof.
    #[must_use]
    pub fn invocation_recipe(
        &self,
        site: nocter_model::ValueApplication,
    ) -> Option<&super::InvocationRecipe> {
        self.result_evidence.invocation(site)
    }
    /// Original source-flow producer equations, retained before code/type result projection.
    /// This is metadata in this exact accepted Unit; it grants no concrete creation frame,
    /// runtime value, representation, callable target or independently reusable source receipt.
    #[must_use]
    pub fn producer_relation(&self, term: super::ProducerTerm) -> Option<&super::ProducerRelation> {
        self.result_evidence.recipe(term)
    }
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }
    /// Exact checked bodyless declarations retained from this unit's header authority.
    #[must_use]
    pub const fn primitives(
        &self,
    ) -> &nocter_declaration_lowering::access_declarations::PrimitiveBindings {
        &self.primitives
    }
    /// Project data through this complete unit's closed result and payload evidence.
    /// # Errors
    /// Rejects an unknown type or a result identity without closed evidence.
    pub fn data_evidence(&self, data: TypeId) -> Result<DataEvidence, flow::IntegrityError> {
        if self.result_evidence.unproduced(self.program(), data) {
            return Ok(DataEvidence::Unproduced);
        }
        flow::resources::payload(self.program(), data).map(DataEvidence::Payload)
    }
    /// Exact definition-owned conditions. This alone does not promise named const capability.
    #[must_use]
    pub fn const_requirements(
        &self,
        code: nocter_model::ValueCode,
    ) -> Option<&super::constant::Requirements> {
        self.const_templates.get(code)
    }
    /// The exact checked body in this accepted unit's sole body catalog.
    #[must_use]
    pub fn body(&self, callable: AccessCodeId) -> Option<&flow::Query> {
        self.bodies.get(&callable.into()).map(Arc::as_ref)
    }
    /// Select the exact accepted named or captured body. Source ownership is not code identity.
    #[must_use]
    pub fn code_body(&self, code: nocter_model::ValueCode) -> Option<&flow::Query> {
        match code {
            nocter_model::ValueCode::Declaration(id) => {
                self.bodies.get(&id.into()).map(Arc::as_ref)
            }
            nocter_model::ValueCode::Expression(_) => None,
            nocter_model::ValueCode::Closure(id) => self.closures.get(&id).map(Arc::as_ref),
        }
    }
    pub fn bodies(&self) -> impl Iterator<Item = (AccessCodeId, &flow::Query)> {
        self.bodies.iter().filter_map(|(&root, query)| match root {
            nocter_model::ValueCodeRoot::Declaration(id) => Some((id, query.as_ref())),
            nocter_model::ValueCodeRoot::Expression(_) => None,
        })
    }
    #[must_use]
    pub fn closures(&self) -> &BTreeMap<nocter_model::ValueClosure, Arc<flow::Query>> {
        &self.closures
    }
    /// Rebinds every accepted body before exposing any current-source projection.
    /// # Errors
    /// Rejects changed authority or any changed body; never rechecks a sibling behind the caller.
    pub fn bind<'query, 'source>(
        &'query self,
        context: &DeclarationContext<'source>,
    ) -> Result<Context<'query, 'source>, BodyFailure> {
        if !context.program().headers_complete()
            || !context.program().preserves_headers(&self.header)
        {
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
pub(super) struct Checked {
    pub(super) program: Arc<Program>,
    pub(super) bodies: BTreeMap<nocter_model::ValueCodeRoot, Arc<flow::Query>>,
    closures: BTreeMap<nocter_model::ValueClosure, Arc<flow::Query>>,
    result_evidence: Arc<super::results::Evidence>,
    const_templates: Arc<super::constant::Catalog>,
}
impl Checked {
    pub(super) fn instantiate_invocation(
        &self,
        caller: &nocter_declarations::value_access::CodeInstanceKey,
        site: nocter_model::ValueApplication,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<nocter_declarations::value_access::CodeInstanceKey, super::InstantiationFailure>
    {
        super::results::instantiate_invocation(
            &self.program,
            &self.result_evidence,
            caller,
            site,
            work,
        )
    }
    /// Internal plan/diagnostic lookup includes required-expression bodies as well as callables.
    pub(super) fn code(&self, code: nocter_model::ValueCode) -> Option<&flow::Query> {
        match code {
            nocter_model::ValueCode::Closure(id) => self.closures.get(&id).map(Arc::as_ref),
            nocter_model::ValueCode::Declaration(_) | nocter_model::ValueCode::Expression(_) => {
                self.bodies.get(&code.root()).map(Arc::as_ref)
            }
        }
    }
}
/// Canonical typing/result/control closure before effect-sensitive admission. This is an
/// intermediate product, not executable or whole-unit evidence.
pub(super) struct Resolved {
    program: Arc<Program>,
    controls: BTreeMap<nocter_model::ValueCodeRoot, flow::Control>,
    result_evidence: Arc<super::results::Evidence>,
}
impl Resolved {
    pub(super) fn program(&self) -> &Program {
        &self.program
    }
    pub(super) fn scope(
        &self,
    ) -> Result<BTreeMap<nocter_model::ValueCode, &TypedQuery>, flow::IntegrityError> {
        Ok(flow::scope(self.controls.values())?
            .into_iter()
            .map(|(code, control)| (code, control.typed.as_ref()))
            .collect())
    }
}
pub(super) fn resolve_queries(
    context: &DeclarationContext<'_>,
    mut queries: BTreeMap<nocter_model::ValueCodeRoot, TypedQuery>,
) -> Result<Resolved, Failure> {
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
    let (program, result_evidence) = super::results::close(context, program, &mut controls)?;
    Ok(Resolved {
        program,
        controls,
        result_evidence,
    })
}
/// The same ordinary acceptance authority serves required const closures and final whole-unit closure.
pub(super) fn check_queries(
    context: &DeclarationContext<'_>,
    queries: BTreeMap<nocter_model::ValueCodeRoot, TypedQuery>,
) -> Result<Checked, Failure> {
    admit_resolved(context, resolve_queries(context, queries)?, None)
}
pub(super) fn admit_resolved(
    context: &DeclarationContext<'_>,
    resolved: Resolved,
    invoked: Option<&BTreeSet<nocter_model::ValueCode>>,
) -> Result<Checked, Failure> {
    let Resolved {
        program,
        controls,
        result_evidence,
    } = resolved;
    let writes = flow::call_writes::Catalog::prepare_with_context(controls.values(), context)
        .map_err(Failure::Integrity)?;
    let verified = result_evidence
        .verify(context)
        .map_err(|_| Failure::Integrity(flow::IntegrityError::InconsistentTypedBody))?;
    let mut bodies = BTreeMap::new();
    let mut closures = BTreeMap::new();
    for (root, control) in controls {
        for (code, nested) in flow::scope([&control]).map_err(Failure::Integrity)? {
            if invoked.is_some_and(|set| !set.contains(&code)) {
                continue;
            }
            let nocter_model::ValueCode::Closure(id) = code else {
                continue;
            };
            let checked = nested.clone().finish(&writes).map_err(Failure::Integrity)?;
            let checked = admit(checked, &verified)?;
            if closures.insert(id, Arc::new(checked)).is_some() {
                return Err(Failure::Integrity(
                    flow::IntegrityError::InconsistentTypedBody,
                ));
            }
        }
        if invoked.is_none_or(|set| set.contains(&root.code())) {
            let query = control.finish(&writes).map_err(Failure::Integrity)?;
            let query = admit(query, &verified)?;
            bodies.insert(root, Arc::new(query));
        }
    }
    let const_templates =
        const_admission::check_const_bodies(&program, &bodies, &closures, &verified)?;
    Ok(Checked {
        program,
        bodies,
        closures,
        result_evidence,
        const_templates,
    })
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
    bodies: BTreeMap<nocter_model::ValueCodeRoot, flow::Context<'query, 'source>>,
}
impl Context<'_, '_> {
    #[must_use]
    pub const fn unit(&self) -> &Unit {
        self.unit
    }
    #[must_use]
    pub fn body(&self, id: AccessCodeId) -> Option<&flow::Context<'_, '_>> {
        self.bodies.get(&id.into())
    }
}

#[cfg(test)]
mod tests;
