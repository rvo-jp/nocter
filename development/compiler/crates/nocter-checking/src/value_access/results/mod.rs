//! One closure of callable result producers, followed by identity-only body projection.
mod admission;
mod applications;
mod carriers;
mod channels;
pub(super) use carriers::instantiate as instantiate_referent_carrier_data;
pub use carriers::{ReferentCarrierContracts, ReferentCarrierData};
pub(super) use channels::completed_input as instantiate_completed_input_data;
pub(super) use channels::instantiate_prepared_data;
pub use channels::{PreparedData, PreparedDataFailure};
pub(super) use channels::{input as instantiate_input_data, result as instantiate_result_contract};
mod instantiation;
pub use instantiation::InstantiationFailure;
pub(super) use instantiation::{
    call as instantiate_invocation, completed_input as instantiate_source_completed_input_data,
    data as instantiate_value_data, result as instantiate_result_data,
    stored as instantiate_stored_data, value as instantiate_value,
};
mod producers;
pub use producers::{CreationRecipe, ProducerShape};
mod projection;
mod relations;
mod resources;
pub use relations::{InvocationRecipe, Relation as ProducerRelation, Term as ProducerTerm};
pub use resources::{
    PartialRootContracts, ResourceDataFailure, ResourceDataProjection, ResourceDataStep,
};
mod solve;
mod typing;
pub(super) use typing::deferred_equalities;
mod value_sources;
pub use value_sources::{
    CallValueSources, ValueSource, ValueSourceBoundary, ValueSourceCarrier, ValueSourceFlow,
    ValueSourceId, ValueSourceInput, ValueSourceNode, ValueSourceRead, ValueSourceSelection,
    ValueSourceSlot, ValueSources,
};
mod unproduced;
mod values;
pub(super) use values::root as instantiate_referent_root_data;
pub(super) use values::root_after as instantiate_referent_root_data_after;
pub(super) use values::values as instantiate_value_data_map;
pub use values::{ValueData, ValueDataFailure};

use super::{Failure, Query, accepted, flow};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::{FunctionWitness, PayloadSchema, Program};
use nocter_model::{
    AccessExpressionId, TypeId, ValueApplication, ValueAppliedResult, ValueCallableResult,
    ValueFunctionOrigin,
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug)]
pub(super) struct Equality {
    pub(super) left: TypeId,
    pub(super) right: TypeId,
    /// The owning typed operation used to project a source-local mismatch.
    origin: AccessExpressionId,
}
#[derive(Clone, Debug, Default)]
pub(super) struct Evidence {
    producers: BTreeMap<ValueCallableResult, FunctionWitness>,
    uninhabited_results: std::collections::BTreeSet<ValueCallableResult>,
    recipes: relations::Relations,
    invocations: BTreeMap<ValueApplication, Arc<InvocationRecipe>>,
    creations: BTreeMap<nocter_model::ValueClosure, CreationRecipe>,
    applications: BTreeMap<ValueAppliedResult, ApplicationEvidence>,
    bodies: nocter_declaration_lowering::access_declarations::BodySources,
    value_sources: BTreeMap<nocter_model::ValueCode, ValueSources>,
}
#[derive(Clone, Copy, Debug)]
enum ApplicationEvidence {
    Witness(FunctionWitness),
    /// The structural contract is known, but no reachable invocation produces this value.
    Uncommitted,
    /// The call can complete, but this leaf has no represented callable values.
    Uninhabited,
}
pub(super) struct Verified<'a, 'source> {
    pub(super) evidence: &'a Evidence,
    pub(super) context: &'a DeclarationContext<'source>,
}
impl Evidence {
    pub(super) fn value_sources(&self, owner: nocter_model::ValueCode) -> Option<&ValueSources> {
        self.value_sources.get(&owner)
    }
    pub(super) fn creation(&self, closure: nocter_model::ValueClosure) -> Option<&CreationRecipe> {
        self.creations.get(&closure)
    }
    pub(super) fn invocation(&self, site: ValueApplication) -> Option<&InvocationRecipe> {
        self.invocations.get(&site).map(Arc::as_ref)
    }
    pub(super) fn recipe(&self, term: ProducerTerm) -> Option<&ProducerRelation> {
        self.recipes.get(&term)
    }
    pub(super) fn producer(&self, result: ValueCallableResult) -> Option<FunctionWitness> {
        self.producers.get(&result).copied()
    }
    pub(super) fn verify<'a, 'source>(
        &'a self,
        context: &'a DeclarationContext<'source>,
    ) -> Result<Verified<'a, 'source>, Failure> {
        self.bodies.verify(context)?;
        Ok(Verified {
            evidence: self,
            context,
        })
    }
    fn selected(&self, origin: ValueFunctionOrigin) -> Option<FunctionWitness> {
        match origin {
            ValueFunctionOrigin::Result(result) => self.producers.get(&result).copied(),
            ValueFunctionOrigin::Application(result) => match self.applications.get(&result)? {
                ApplicationEvidence::Witness(witness) => Some(*witness),
                ApplicationEvidence::Uncommitted | ApplicationEvidence::Uninhabited => None,
            },
            _ => None,
        }
    }
    fn origin(&self, origin: ValueFunctionOrigin) -> Result<ValueFunctionOrigin, Failure> {
        if self.has_no_callable_value(origin) {
            return Ok(origin);
        }
        match origin {
            ValueFunctionOrigin::Declaration(_)
            | ValueFunctionOrigin::Closure(_)
            | ValueFunctionOrigin::Parameter(_) => Ok(origin),
            _ => self
                .selected(origin)
                .map(|w| w.origin)
                .ok_or(Failure::InconsistentSyntax),
        }
    }
    fn witness(&self, witness: FunctionWitness) -> Result<FunctionWitness, Failure> {
        if self.has_no_callable_value(witness.origin) {
            return Ok(witness);
        }
        let Some(producer) = self.selected(witness.origin) else {
            return match witness.origin {
                ValueFunctionOrigin::Declaration(_)
                | ValueFunctionOrigin::Closure(_)
                | ValueFunctionOrigin::Parameter(_) => Ok(witness),
                _ => Err(Failure::InconsistentSyntax),
            };
        };
        if !producer.invocation.can_supply(witness.invocation) {
            return Err(Failure::InconsistentSyntax);
        }
        Ok(FunctionWitness {
            origin: producer.origin,
            invocation: witness.invocation,
        })
    }
    fn data(&self, program: &Program, data: TypeId) -> Result<TypeId, Failure> {
        match program
            .data_schema(data)
            .ok_or(Failure::InconsistentSyntax)?
        {
            PayloadSchema::Function(witness)
                if matches!(
                    witness.origin,
                    ValueFunctionOrigin::Result(_) | ValueFunctionOrigin::Application(_)
                ) =>
            {
                program
                    .function_data(self.witness(witness)?)
                    .ok_or(Failure::InconsistentSyntax)
            }
            _ => Ok(data),
        }
    }
    fn has_no_callable_value(&self, origin: ValueFunctionOrigin) -> bool {
        self.uninhabited_origin(origin)
            || matches!(origin, ValueFunctionOrigin::Application(application)
            if matches!(self.applications.get(&application), Some(ApplicationEvidence::Uncommitted)))
    }
    fn uninhabited_origin(&self, origin: ValueFunctionOrigin) -> bool {
        match origin {
            ValueFunctionOrigin::Result(result) => self.uninhabited_results.contains(&result),
            ValueFunctionOrigin::Application(result) => matches!(
                self.applications.get(&result),
                Some(ApplicationEvidence::Uninhabited)
            ),
            _ => false,
        }
    }
    fn uncommitted_application(&self, site: ValueApplication) -> bool {
        matches!(
            self.applications.get(&site.into()),
            Some(ApplicationEvidence::Uncommitted)
        )
    }
    pub(in crate::value_access) fn excludes_primary(
        &self,
        program: &Program,
        data: TypeId,
    ) -> bool {
        let Some(PayloadSchema::Optional(primary) | PayloadSchema::Fallible(primary)) =
            program.data_schema(data)
        else {
            return false;
        };
        self.empty_domain(program, primary)
    }
    pub(in crate::value_access) fn empty_domain(&self, program: &Program, data: TypeId) -> bool {
        program
            .instantiate_callable_domains(data, |_, witness| {
                if self.uninhabited_origin(witness.origin) {
                    Ok(None)
                } else {
                    program.concrete_witness(witness).map(Some)
                }
            })
            .is_ok_and(|key| key.is_uninhabited())
    }
    pub(in crate::value_access) fn matches_payload_metadata(
        &self,
        program: &Program,
        actual: TypeId,
        expected: TypeId,
    ) -> bool {
        program.matches_uninhabited_metadata_with(actual, expected, |witness| {
            self.uninhabited_origin(witness.origin)
        })
    }
    fn recognizes_empty_application(
        &self,
        program: &Program,
        application: ValueAppliedResult,
    ) -> bool {
        let mut origin = ValueFunctionOrigin::Application(application);
        let mut seen = std::collections::BTreeSet::new();
        loop {
            if !seen.insert(origin) {
                return false;
            }
            match origin {
                ValueFunctionOrigin::Application(application) => {
                    let Some(template) = program.application_template(application) else {
                        return false;
                    };
                    origin = template.origin;
                }
                ValueFunctionOrigin::Result(result) => {
                    return self.uninhabited_results.contains(&result);
                }
                _ => return false,
            }
        }
    }
    pub(super) fn unproduced(&self, program: &Program, data: TypeId) -> bool {
        let mut pending = vec![data];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(data) = pending.pop() {
            if !seen.insert(data) {
                continue;
            }
            match program.data_schema(data) {
                Some(PayloadSchema::Function(witness))
                    if self.has_no_callable_value(witness.origin) =>
                {
                    return true;
                }
                Some(PayloadSchema::Function(FunctionWitness {
                    origin: ValueFunctionOrigin::Closure(id),
                    ..
                })) => {
                    if let Some(schema) = program.closure(id) {
                        pending.push(schema.environment());
                    }
                }
                Some(PayloadSchema::Aggregate(schema)) => {
                    pending.extend(schema.components().map(|component| component.data));
                }
                Some(PayloadSchema::Array(array)) if !array.is_empty() => {
                    pending.push(array.element());
                }
                _ => {}
            }
        }
        false
    }
}
pub(super) use unproduced::validate;
pub(super) fn is_result(program: &Program, data: TypeId) -> bool {
    matches!(
        program.data_schema(data),
        Some(PayloadSchema::Function(FunctionWitness {
            origin: ValueFunctionOrigin::Result(_) | ValueFunctionOrigin::Application(_),
            ..
        }))
    )
}

pub(super) fn close(
    context: &DeclarationContext<'_>,
    program: Arc<Program>,
    controls: &mut BTreeMap<nocter_model::ValueCodeRoot, flow::Control>,
) -> Result<(Arc<Program>, Arc<Evidence>), accepted::Failure> {
    if controls
        .values()
        .any(|control| !Arc::ptr_eq(&program, &control.typed.program))
    {
        return Err(accepted::Failure::Integrity(
            flow::IntegrityError::InconsistentTypedBody,
        ));
    }
    let evidence = Arc::new(solve::solve(context, controls)?);
    let scope = flow::scope(controls.values()).map_err(accepted::Failure::Integrity)?;
    solve::prove_equalities(context, &scope, &evidence)?;
    let mut witnesses = BTreeMap::new();
    for origin in evidence
        .producers
        .keys()
        .copied()
        .map(ValueFunctionOrigin::Result)
        .chain(
            evidence
                .applications
                .keys()
                .copied()
                .map(ValueFunctionOrigin::Application),
        )
    {
        for invocation in [
            nocter_model::ValueInvocation::Read,
            nocter_model::ValueInvocation::Edit,
            nocter_model::ValueInvocation::Take,
        ] {
            let source = FunctionWitness { origin, invocation };
            if program.function_data(source).is_some() && !evidence.has_no_callable_value(origin) {
                witnesses.insert(
                    source,
                    evidence.witness(source).map_err(|_| {
                        accepted::Failure::Integrity(flow::IntegrityError::InconsistentTypedBody)
                    })?,
                );
            }
        }
    }
    // An empty unit still retains its canonical construction role, rather than a header fallback.
    if controls.is_empty() {
        return Ok((program, evidence));
    }
    let projection = program
        .resolve_body_types(&witnesses)
        .map_err(|error| match error {
            nocter_declarations::value_access::BodyTypeError::RecursiveEnvironment(id) => {
                projection::recursive_environment(context, &scope, id)
            }
            _ => accepted::Failure::Integrity(flow::IntegrityError::InconsistentTypedBody),
        })?;
    let program = Arc::new(projection.program().clone());
    for (&root, control) in controls.iter_mut() {
        let query = Arc::make_mut(&mut control.typed);
        attach(query, &evidence);
        crate::value_access::types::bind_program(query, &program);
        projection::project(context, query, &evidence, &projection).map_err(|failure| {
            accepted::Failure::Body {
                callable: root.source_owner(),
                failure,
            }
        })?;
        seal(query);
        control
            .refine_outcome_domains()
            .map_err(accepted::Failure::Integrity)?;
    }
    // Source definitions depend on resolved receiving actions, not provisional PendingValue
    // plans. The original control/action identities and value relationships stay unchanged.
    let mut completed = (*evidence).clone();
    let scope = flow::scope(controls.values()).map_err(accepted::Failure::Integrity)?;
    for (&owner, control) in &scope {
        admission::validate(context, control, &evidence).map_err(|failure| {
            accepted::Failure::Body {
                callable: control.typed.source_owner(),
                failure,
            }
        })?;
        let aliases =
            flow::Aliases::prepare(&control.typed).map_err(accepted::Failure::Integrity)?;
        let graph =
            value_sources::analyze(control, &aliases).map_err(accepted::Failure::Integrity)?;
        completed.value_sources.insert(owner, graph);
    }
    let evidence = Arc::new(completed);
    for control in controls.values_mut() {
        attach(Arc::make_mut(&mut control.typed), &evidence);
    }
    Ok((program, evidence))
}
fn attach(query: &mut Query, evidence: &Arc<Evidence>) {
    query.result_evidence = Some(evidence.clone());
    for nested in query.closures.values_mut() {
        attach(Arc::make_mut(&mut nested.typed), evidence);
    }
}
fn seal(query: &mut Query) {
    query.results_closed = true;
    query.equalities.clear();
    for nested in query.closures.values_mut() {
        seal(Arc::make_mut(&mut nested.typed));
    }
}

fn reject(
    context: &DeclarationContext<'_>,
    query: &Query,
    origin: Option<AccessExpressionId>,
    message: &str,
) -> accepted::Failure {
    accepted::Failure::Body {
        callable: query.source_owner(),
        failure: super::diagnostics::query_rejection(
            context,
            query,
            origin,
            super::Rule::TypeMismatch,
            message,
        ),
    }
}
