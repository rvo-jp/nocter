//! Closed front-end acceptance of every definition in one private first-fragment unit.
pub use super::staged::{ModuleSourceUnit, SourceFailure, SourceUnit};
mod const_admission;
mod deferred;
mod instance_metadata;
use super::{Failure as BodyFailure, Query as TypedQuery, flow};
pub(super) use const_admission::{ConstAdmission, ConstConditions, check_required_const};
pub use instance_metadata::InstanceMetadata;
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
    deferred_drives: BTreeSet<AccessCodeId>,
    future_elisions: deferred::Elisions,
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
        let mut definition_work =
            nocter_constant_evaluation::state::WorkBudget::new(evaluation.condition_limit);
        let Checked {
            program,
            bodies,
            closures,
            result_evidence,
            const_templates,
            deferred_drives,
            future_elisions,
        } = check_queries(context, queries, &mut definition_work)?;
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
            deferred_drives,
            future_elisions,
        })
    }
    /// The empty deferred activation is proven non-waiting when driven to completion.
    /// This grants no source future construction, transport or suspension permission.
    #[must_use]
    pub fn can_drive_without_suspension(&self, id: AccessCodeId) -> bool {
        self.deferred_drives.contains(&id)
    }
    /// Whether this body's checked unstarted futures of this data have no represented storage.
    #[must_use]
    pub fn elides_future_data(&self, code: nocter_model::ValueCode, data: TypeId) -> bool {
        self.future_elisions.contains(code, data)
    }
    /// Whether an original local future transition has no physical payload work.
    #[must_use]
    pub fn elides_future_step(
        &self,
        code: nocter_model::ValueCode,
        step: &flow::resources::Step,
    ) -> bool {
        self.future_elisions.step(code, step)
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
    /// The named/captured-code or own-input witness established for one exact result leaf.
    #[must_use]
    pub fn result_witness(
        &self,
        result: nocter_model::ValueCallableResult,
    ) -> Option<nocter_declarations::value_access::FunctionWitness> {
        self.result_evidence.producer(result)
    }
    /// Select original body-input metadata, preserving the complete capture input.
    /// This selects no input order, operation, runtime value or ownership permission.
    /// # Errors
    /// Rejects absent bodies, non-input locals, unresolved contexts and exhausted metadata work.
    pub fn instantiate_input_data(
        &self,
        frame: &nocter_declarations::value_access::CodeInstanceKey,
        local: nocter_model::AccessLocalId,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<nocter_declarations::value_access::DataInstanceKey, super::InstantiationFailure>
    {
        let body = self
            .code_body(frame.code())
            .ok_or(super::InstantiationFailure::Unresolved)?;
        super::results::instantiate_input_data(
            &self.program,
            &self.result_evidence,
            body,
            frame,
            local,
            work,
        )
    }
    /// Select the original body's result metadata, including independently closed contracts.
    /// Unlike `instantiate_result_data`, this need not prove that a diverging body returns a value.
    /// # Errors
    /// Rejects absent bodies, unresolved contexts and exhausted metadata work.
    pub fn instantiate_result_contract(
        &self,
        frame: &nocter_declarations::value_access::CodeInstanceKey,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<nocter_declarations::value_access::DataInstanceKey, super::InstantiationFailure>
    {
        let body = self
            .code_body(frame.code())
            .ok_or(super::InstantiationFailure::Unresolved)?;
        super::results::instantiate_result_contract(
            &self.program,
            &self.result_evidence,
            body,
            frame,
            work,
        )
    }
    /// Freeze ordinary operand and stored-definition metadata at original source actions.
    /// This supplies no runtime value, source access or executable-body receipt.
    /// # Errors
    /// Rejects missing bodies, unavailable/ambiguous data and exhausted metadata work.
    pub fn instantiate_value_data_map(
        &self,
        frame: &nocter_declarations::value_access::CodeInstanceKey,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<super::ValueData, super::ValueDataFailure> {
        super::results::instantiate_value_data_map(
            &self.program,
            &self.result_evidence,
            self.code_body(frame.code()),
            frame,
            self.future_elisions.body(frame.code()),
            work,
        )
    }
    /// Select original continuations using this frame's represented outcome domains.
    /// This does not erase tag evaluation or grant initialized storage, access or execution.
    /// # Errors
    /// Rejects missing bodies, inconsistent original points and exhausted metadata work.
    pub fn instantiate_control(
        &self,
        frame: &nocter_declarations::value_access::CodeInstanceKey,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<super::InstanceControl, super::InstantiationFailure> {
        super::results::instantiate_control(
            &self.program,
            &self.result_evidence,
            self.code_body(frame.code()),
            frame,
            work,
        )
    }
    /// Select reference-root contract metadata at one original source occurrence.
    /// This grants no initialized value, source access, physical allocation or stable layout.
    /// # Errors
    /// Rejects missing/foreign occurrences, unresolved context and exhausted metadata work.
    pub fn instantiate_referent_root_data(
        &self,
        frame: &nocter_declarations::value_access::CodeInstanceKey,
        at: flow::Location,
        referent: super::Referent,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<nocter_declarations::value_access::DataInstanceKey, super::InstantiationFailure>
    {
        let body = self
            .code_body(frame.code())
            .ok_or(super::InstantiationFailure::Unresolved)?;
        super::results::instantiate_referent_root_data(
            &self.program,
            &self.result_evidence,
            body,
            frame,
            at,
            referent,
            work,
        )
    }
    /// Select a recorded root contract on normal completion of an original source operation.
    /// This does not prove completion, mutation, initialized storage or a resource-phase result.
    /// # Errors
    /// Rejects missing post-operation occurrences, unresolved context and exhausted metadata work.
    pub fn instantiate_referent_root_data_after(
        &self,
        frame: &nocter_declarations::value_access::CodeInstanceKey,
        at: flow::Location,
        referent: super::Referent,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<nocter_declarations::value_access::DataInstanceKey, super::InstantiationFailure>
    {
        let body = self
            .code_body(frame.code())
            .ok_or(super::InstantiationFailure::Unresolved)?;
        super::results::instantiate_referent_root_data_after(
            &self.program,
            &self.result_evidence,
            body,
            frame,
            at,
            referent,
            work,
        )
    }
    /// Resolve one original input's context across all retained normal-completion boundaries.
    /// Retired source identities remain metadata: this proves no surviving input, caller write,
    /// resource phase or actual return. Preparation-time snapshots are not a completion fallback.
    /// # Errors
    /// Rejects non-input locals, absent completions, ambiguous contexts and exhausted metadata work.
    pub fn instantiate_completed_input_data(
        &self,
        frame: &nocter_declarations::value_access::CodeInstanceKey,
        local: nocter_model::AccessLocalId,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<nocter_declarations::value_access::DataInstanceKey, super::InstantiationFailure>
    {
        let body = self
            .code_body(frame.code())
            .ok_or(super::InstantiationFailure::Unresolved)?;
        super::results::instantiate_completed_input_data(
            &self.program,
            &self.result_evidence,
            body,
            frame,
            local,
            work,
        )
    }
    /// Resolve containing contracts only when requested, preserving each original source/path pair.
    /// Metadata grants no initialized owner, physical backing, geometry or resource-phase receipt.
    /// # Errors
    /// Rejects absent occurrences, unresolved containing contexts and exhausted metadata work.
    /// No partial before/after result is published; an independent request can retry.
    pub fn instantiate_referent_carrier_data(
        &self,
        frame: &nocter_declarations::value_access::CodeInstanceKey,
        at: flow::Location,
        referent: super::Referent,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<super::ReferentCarrierContracts, super::InstantiationFailure> {
        let body = self
            .code_body(frame.code())
            .ok_or(super::InstantiationFailure::Unresolved)?;
        super::results::instantiate_referent_carrier_data(
            &self.program,
            &self.result_evidence,
            body,
            frame,
            at,
            referent,
            work,
        )
    }
    /// Freeze actual data at original argument preparations, including interrupted prefixes.
    /// This creates no executable callee, invocation effect, owning value or materialization permission.
    /// # Errors
    /// Rejects missing bodies, unavailable/ambiguous preparation data and exhausted metadata work.
    pub fn instantiate_prepared_data(
        &self,
        frame: &nocter_declarations::value_access::CodeInstanceKey,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<super::PreparedData, super::PreparedDataFailure> {
        super::results::instantiate_prepared_data(
            &self.program,
            &self.result_evidence,
            self.code_body(frame.code()),
            frame,
            work,
        )
    }
    /// Project ordered resource payload metadata against this body's original source values.
    /// The projection grants no ownership, source access, layout or executable transition receipt.
    /// # Errors
    /// Rejects a missing body, invalid original action, incompatible data or exhausted metadata work.
    pub fn project_resource_data<'a, 'work>(
        &'a self,
        frame: &'a nocter_declarations::value_access::CodeInstanceKey,
        at: flow::Location,
        work: &'work mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<super::ResourceDataProjection<'a, 'work>, super::ResourceDataFailure> {
        super::ResourceDataProjection::new(
            &self.program,
            &self.result_evidence,
            self.code_body(frame.code()),
            frame,
            at,
            None,
            work,
        )
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
        if caller.code() != site.owner {
            return Err(super::InstantiationFailure::WrongOwner);
        }
        let control = self.instantiate_control(caller, work)?;
        super::results::instantiate_controlled_invocation(
            &self.program,
            &self.result_evidence,
            caller,
            site,
            &control,
            work,
        )
    }
    /// Retain original prepared call bundles and all validated positive candidate frames.
    /// This grants no active selector, initialized input, const permission or executable call.
    /// # Errors
    /// Rejects wrong/missing calls, unresolved candidate inputs and exhausted metadata work.
    pub fn instantiate_invocation_domain(
        &self,
        caller: &nocter_declarations::value_access::CodeInstanceKey,
        site: nocter_model::ValueApplication,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<super::InvocationDomain, super::InstantiationFailure> {
        if caller.code() != site.owner {
            return Err(super::InstantiationFailure::WrongOwner);
        }
        let control = self.instantiate_control(caller, work)?;
        super::results::instantiate_controlled_invocation_domain(
            &self.program,
            &self.result_evidence,
            caller,
            site,
            &control,
            work,
        )
    }
    /// Resolve a callable source value in this exact frame without granting a runtime value.
    /// # Errors
    /// Rejects absent/ambiguous provenance, unsupported effects and exhausted metadata work.
    pub fn instantiate_value(
        &self,
        caller: &nocter_declarations::value_access::CodeInstanceKey,
        value: super::ValueSourceId,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<nocter_declarations::value_access::ConcreteWitness, super::InstantiationFailure>
    {
        super::results::InstantiationQuery::controlled(
            &self.program,
            &self.result_evidence,
            self.code_body(caller.code()),
            caller,
            work,
        )?
        .value(caller, value)
    }
    /// Normalize one source value's contextual data without granting storage or a source receipt.
    /// The source schema remains semantic; the returned key retains its actual callable contexts.
    /// # Errors
    /// Rejects absent/ambiguous provenance, incompatible schemas, unsupported effects and work exhaustion.
    pub fn instantiate_value_data(
        &self,
        caller: &nocter_declarations::value_access::CodeInstanceKey,
        value: super::ValueSourceId,
        source: TypeId,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<nocter_declarations::value_access::DataInstanceKey, super::InstantiationFailure>
    {
        super::results::InstantiationQuery::controlled(
            &self.program,
            &self.result_evidence,
            self.code_body(caller.code()),
            caller,
            work,
        )?
        .data(caller, value, source)
    }
    /// Resolve contextual data at an original source-definition boundary.
    /// This does not choose a resource phase, validate alias access or grant a stored value.
    /// # Errors
    /// Rejects absent/ambiguous provenance, invalid boundaries and exhausted metadata work.
    pub fn instantiate_stored_data(
        &self,
        caller: &nocter_declarations::value_access::CodeInstanceKey,
        selected: &super::ValueSourceSelection,
        source: TypeId,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<nocter_declarations::value_access::DataInstanceKey, super::InstantiationFailure>
    {
        super::results::InstantiationQuery::controlled(
            &self.program,
            &self.result_evidence,
            self.code_body(caller.code()),
            caller,
            work,
        )?
        .stored(caller, selected, source)
    }
    /// Resolve the contextual data of original returned values in this exact frame.
    /// This supplies no return value, initialized owner or executable ABI.
    /// # Errors
    /// Rejects absent/ambiguous return producers and exhausted metadata work.
    pub fn instantiate_result_data(
        &self,
        frame: &nocter_declarations::value_access::CodeInstanceKey,
        source: TypeId,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<nocter_declarations::value_access::DataInstanceKey, super::InstantiationFailure>
    {
        super::results::instantiate_result_data(
            &self.program,
            &self.result_evidence,
            frame,
            source,
            work,
        )
    }
    /// Original sparse value definitions, scoped to this complete matched Unit.
    /// These metadata edges grant no initialization, numeric or executable proof.
    #[must_use]
    pub fn value_sources(
        &self,
        owner: nocter_model::ValueCode,
    ) -> Option<&super::results::ValueSources> {
        self.result_evidence.value_sources(owner)
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
    /// Original environment schema and actual producer snapshot of a reached closure formation.
    /// This exact-Unit metadata supplies no runtime environment, permission or executable proof.
    #[must_use]
    pub fn creation_recipe(
        &self,
        closure: nocter_model::ValueClosure,
    ) -> Option<&super::CreationRecipe> {
        self.result_evidence.creation(closure)
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
    deferred_drives: BTreeSet<AccessCodeId>,
    future_elisions: deferred::Elisions,
}
impl Checked {
    pub(super) fn project_const_violation(
        &self,
        context: &DeclarationContext<'_>,
        frame: &nocter_declarations::value_access::CodeInstanceKey,
        violation: super::constant::Violation,
    ) -> Result<super::constant::Diagnostic, super::Failure> {
        let query = self
            .code(frame.code())
            .ok_or(super::Failure::InconsistentSyntax)?;
        let bound = query.bind(context)?;
        let mut diagnostic = violation.project(&bound)?;
        let verified = self
            .result_evidence
            .verify(context)
            .map_err(|_| super::Failure::InconsistentSyntax)?;
        self.const_templates
            .annotate(diagnostic.rule(), &mut diagnostic, &verified)?;
        Ok(diagnostic)
    }
    pub(super) fn instantiate_call_boundary(
        &self,
        frame: &nocter_declarations::value_access::CodeInstanceKey,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<super::results::CallBoundary, super::InstantiationFailure> {
        super::results::instantiate_call_boundary(
            &self.program,
            &self.result_evidence,
            self.code(frame.code()),
            frame,
            work,
        )
    }
    pub(super) fn instantiate_completed_input_data(
        &self,
        frame: &nocter_declarations::value_access::CodeInstanceKey,
        local: nocter_model::AccessLocalId,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<nocter_declarations::value_access::DataInstanceKey, super::InstantiationFailure>
    {
        super::results::instantiate_completed_input_data(
            &self.program,
            &self.result_evidence,
            self.code(frame.code())
                .ok_or(super::InstantiationFailure::Unresolved)?,
            frame,
            local,
            work,
        )
    }
    pub(super) fn instantiate_control(
        &self,
        frame: &nocter_declarations::value_access::CodeInstanceKey,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<super::InstanceControl, super::InstantiationFailure> {
        super::results::instantiate_control(
            &self.program,
            &self.result_evidence,
            self.code(frame.code()),
            frame,
            work,
        )
    }
    pub(super) fn instantiate_value_data_map(
        &self,
        frame: &nocter_declarations::value_access::CodeInstanceKey,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<super::ValueData, super::ValueDataFailure> {
        super::results::instantiate_value_data_map(
            &self.program,
            &self.result_evidence,
            self.code(frame.code()),
            frame,
            None,
            work,
        )
    }
    pub(super) fn project_resource_data<'a, 'work>(
        &'a self,
        frame: &'a nocter_declarations::value_access::CodeInstanceKey,
        at: flow::Location,
        control: &'a super::InstanceControl,
        work: &'work mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<super::ResourceDataProjection<'a, 'work>, super::ResourceDataFailure> {
        super::ResourceDataProjection::new(
            &self.program,
            &self.result_evidence,
            self.code(frame.code()),
            frame,
            at,
            Some(control),
            work,
        )
    }

    pub(super) fn instantiate_prepared_data(
        &self,
        frame: &nocter_declarations::value_access::CodeInstanceKey,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<super::PreparedData, super::PreparedDataFailure> {
        super::results::instantiate_prepared_data(
            &self.program,
            &self.result_evidence,
            self.code(frame.code()),
            frame,
            work,
        )
    }

    pub(super) fn instantiate_input_data(
        &self,
        frame: &nocter_declarations::value_access::CodeInstanceKey,
        local: nocter_model::AccessLocalId,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<nocter_declarations::value_access::DataInstanceKey, super::InstantiationFailure>
    {
        let body = self
            .code(frame.code())
            .ok_or(super::InstantiationFailure::Unresolved)?;
        super::results::instantiate_input_data(
            &self.program,
            &self.result_evidence,
            body,
            frame,
            local,
            work,
        )
    }
    pub(super) fn instantiate_invocation(
        &self,
        caller: &nocter_declarations::value_access::CodeInstanceKey,
        site: nocter_model::ValueApplication,
        control: &super::InstanceControl,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<nocter_declarations::value_access::CodeInstanceKey, super::InstantiationFailure>
    {
        if caller.code() != site.owner {
            return Err(super::InstantiationFailure::WrongOwner);
        }
        super::results::instantiate_controlled_invocation(
            &self.program,
            &self.result_evidence,
            caller,
            site,
            control,
            work,
        )
    }
    pub(super) fn instantiate_invocation_domain(
        &self,
        caller: &nocter_declarations::value_access::CodeInstanceKey,
        site: nocter_model::ValueApplication,
        control: &super::InstanceControl,
        work: &mut nocter_constant_evaluation::state::WorkBudget,
    ) -> Result<super::InvocationDomain, super::InstantiationFailure> {
        super::results::instantiate_controlled_invocation_domain(
            &self.program,
            &self.result_evidence,
            caller,
            site,
            control,
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
    pub(super) fn controls(
        &self,
    ) -> Result<BTreeMap<nocter_model::ValueCode, &flow::Control>, flow::IntegrityError> {
        flow::scope(self.controls.values())
    }
    pub(super) fn program(&self) -> &Program {
        &self.program
    }
    pub(super) fn scope(
        &self,
    ) -> Result<BTreeMap<nocter_model::ValueCode, &TypedQuery>, flow::IntegrityError> {
        Ok(self
            .controls()?
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
            deferred::require_await_execution(context, &body)?;
            deferred::require_local_activation(context, id)?;
            let control = flow::Control::prepare(Arc::new(body)).map_err(Failure::Integrity)?;
            Ok((id, control))
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
    definition_work: &mut nocter_constant_evaluation::state::WorkBudget,
) -> Result<Checked, Failure> {
    admit_resolved(
        context,
        resolve_queries(context, queries)?,
        None,
        definition_work,
    )
}
pub(super) fn admit_resolved(
    context: &DeclarationContext<'_>,
    resolved: Resolved,
    invoked: Option<&BTreeSet<nocter_model::ValueCode>>,
    definition_work: &mut nocter_constant_evaluation::state::WorkBudget,
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
            let nocter_model::ValueCode::Closure(id) = code else {
                continue;
            };
            if invoked.is_some_and(|set| !set.contains(&code)) {
                let query = &nested.typed;
                let roots = std::iter::once(query.result_data())
                    .chain(query.body().locals().iter().map(|(_, local)| local.data))
                    .chain(
                        query
                            .body()
                            .expressions()
                            .iter()
                            .filter_map(|(_, expression)| {
                                expression.ty.access().map(nocter_model::ValueAccess::data)
                            }),
                    );
                if !super::checker::payload_requirements::ready(&program, roots).map_err(
                    |failure| Failure::Body {
                        callable: code.source_owner(),
                        failure,
                    },
                )? {
                    continue;
                }
            }
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
    let future_elisions = deferred::Elisions::prepare(
        bodies.values().chain(closures.values()).map(Arc::as_ref),
        &verified,
    )?;
    let deferred_drives = deferred::qualify(&program, &bodies, &verified, &future_elisions)?;
    let const_templates = const_admission::check_const_bodies(
        &program,
        &bodies,
        &closures,
        &verified,
        definition_work,
    )?;
    // The catalog retains the shared static body/condition product. That is not permission
    // for the plan source to realize a body outside the invocation selection.
    if let Some(invoked) = invoked {
        closures.retain(|&id, _| invoked.contains(&nocter_model::ValueCode::Closure(id)));
    }
    Ok(Checked {
        program,
        bodies,
        closures,
        result_evidence,
        const_templates,
        deferred_drives,
        future_elisions,
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
