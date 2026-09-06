use std::collections::HashMap;
use std::sync::Arc;

use nocter_model::{Arena, ArenaBuilder, BodyId, TypeStore};
use nocter_source_index::SourceIndex;

use super::error::{BodyCheckFailure, BodyCheckInternalError};
use super::semantic_transaction::CheckedSemanticAuthority;
use crate::body_relations::{BodyRelationCatalog, BodyRelationProjection};
use crate::checked::{CheckedProgram, CheckedProgramAuthorities, CheckedProgramOutput};
use crate::effects::analyze_program_effects;
use crate::loans::analyze_program_loans;
use crate::provenance::analyze_program_provenance;
use crate::{CheckedBody, ResolvedBodyNames};

/// Exact-current canonical body materialization before program-wide relation analysis.
///
/// The value owns every semantic and source-projection component required by final assembly. It
/// contains no borrowed syntax, so the computation graph can retain it without extending a parser
/// generation's lifetime.
#[derive(Clone, Debug)]
pub struct QueriedProgramMaterialization {
    pub(super) environment: Arc<crate::program_environment::ProgramEnvironment>,
    pub(super) source_access: Arc<nocter_frontend_bindings::SourceAccessTable>,
    pub(super) body_names: Arc<Arena<BodyId, ResolvedBodyNames>>,
    pub(super) source_index: Arc<SourceIndex>,
    pub(super) semantics: Arc<CheckedSemanticAuthority>,
    pub(super) bodies: Arc<Arena<BodyId, CheckedBody>>,
    pub(super) node_origins:
        Arc<Arena<BodyId, HashMap<nocter_model::BodyNodeId, nocter_source_index::SourceOrigin>>>,
    pub(super) opaque_witnesses: Arc<crate::OpaqueWitnessTable>,
    pub(super) associated_type_completion_contexts: Arc<[crate::AssociatedTypeCompletionContext]>,
}

#[derive(Clone, Debug)]
pub struct ReusableProgramRelations {
    provenance: Arc<crate::ProvenanceTable>,
    effects: Arc<crate::EffectTable>,
    loans: Arc<crate::LoanTable>,
}

#[derive(Clone, Debug)]
pub struct ReusableProgramRelationFailure(crate::BodyRelationError);

#[derive(Clone, Debug)]
pub enum ReusableProgramRelationOutcome {
    Analyzed(Box<ReusableProgramRelations>),
    Failed(Box<ReusableProgramRelationFailure>),
}

/// Immutable exact-current result of canonical body replay before relation analysis.
#[derive(Debug)]
pub enum QueriedProgramMaterializationOutcome {
    Materialized(Box<QueriedProgramMaterialization>),
    Failed(Box<BodyCheckFailure>),
}

/// Immutable exact-current result of canonical body replay and program finalization.
#[derive(Debug)]
pub enum QueriedProgramFinalizationOutcome {
    Checked(Box<CheckedProgramOutput>),
    Failed(Box<BodyCheckFailure>),
}

/// Computes source-neutral whole-program relations from one canonical materialization.
#[must_use]
pub fn analyze_queried_program_relations(
    materialized: &QueriedProgramMaterialization,
) -> ReusableProgramRelationOutcome {
    analyze_materialized_program_relations(materialized)
}

/// Joins exact-current materialization with source-neutral relation analysis.
#[must_use]
pub fn finalize_queried_program_materialization(
    materialized: QueriedProgramMaterialization,
    relations: &ReusableProgramRelationOutcome,
) -> QueriedProgramFinalizationOutcome {
    match finalize_materialized_program(materialized, relations, true) {
        Ok(checked) => QueriedProgramFinalizationOutcome::Checked(Box::new(checked)),
        Err(failure) => QueriedProgramFinalizationOutcome::Failed(Box::new(failure)),
    }
}

pub(super) fn analyze_materialized_program_relations(
    materialized: &QueriedProgramMaterialization,
) -> ReusableProgramRelationOutcome {
    match analyze_checked_body_relations(
        &materialized.environment,
        materialized.semantics.semantics().types(),
        materialized.semantics.closures(),
        &materialized.bodies,
    ) {
        Ok(relations) => ReusableProgramRelationOutcome::Analyzed(Box::new(relations)),
        Err(error) => {
            ReusableProgramRelationOutcome::Failed(Box::new(ReusableProgramRelationFailure(error)))
        }
    }
}

pub(super) fn finalize_materialized_program(
    materialized: QueriedProgramMaterialization,
    outcome: &ReusableProgramRelationOutcome,
    retain_recovery: bool,
) -> Result<CheckedProgramOutput, BodyCheckFailure> {
    let relations = match outcome {
        ReusableProgramRelationOutcome::Analyzed(relations) => (**relations).clone(),
        ReusableProgramRelationOutcome::Failed(failure) => {
            let projection = BodyRelationProjection::new(
                materialized.environment.graph(),
                materialized.node_origins.iter(),
            )
            .map_err(|error| BodyCheckFailure::new(error.into(), None))?;
            let error = failure.0.clone().project(&projection);
            let recovery = if retain_recovery {
                build_materialized_body_recovery(materialized).map(Some)
            } else {
                Ok(None)
            };
            return Err(BodyCheckFailure::from_recovery_result(error, recovery));
        }
    };
    Ok(finish_checked_program(materialized, relations))
}

fn build_materialized_body_recovery(
    materialized: QueriedProgramMaterialization,
) -> Result<crate::BodyAnalysisRecovery, BodyCheckInternalError> {
    let QueriedProgramMaterialization {
        environment,
        source_access,
        body_names,
        source_index,
        semantics,
        bodies,
        node_origins: _,
        opaque_witnesses: _,
        associated_type_completion_contexts: _,
    } = materialized;
    let mut evidence = ArenaBuilder::new();
    for (body, checked) in bodies.iter() {
        if evidence.insert(crate::BodyEvidence::Typed(checked.clone())) != body {
            return Err(BodyCheckInternalError::NonCanonicalBody(body));
        }
    }
    let program = crate::PreparedSemanticProgram::from_checked_parts(
        Arc::unwrap_or_clone(environment),
        semantics.semantics().clone(),
        Arc::unwrap_or_clone(source_access),
    );
    Ok(crate::BodyAnalysisRecovery::new(
        program,
        Arc::unwrap_or_clone(body_names),
        Arc::unwrap_or_clone(source_index),
        evidence.finish(),
    ))
}

fn finish_checked_program(
    materialized: QueriedProgramMaterialization,
    relations: ReusableProgramRelations,
) -> CheckedProgramOutput {
    let QueriedProgramMaterialization {
        environment,
        source_access,
        body_names: _,
        source_index,
        semantics,
        bodies,
        node_origins: _,
        opaque_witnesses,
        associated_type_completion_contexts,
    } = materialized;
    let ReusableProgramRelations {
        provenance,
        effects,
        loans,
    } = relations;
    CheckedProgramOutput::new(
        CheckedProgram::new(
            environment,
            semantics,
            CheckedProgramAuthorities {
                provenance,
                effects,
                loans,
                opaque_witnesses,
                associated_type_completion_contexts,
            },
            bodies,
            source_access,
        ),
        source_index,
    )
}

fn analyze_checked_body_relations(
    environment: &crate::program_environment::ProgramEnvironment,
    types: &TypeStore,
    closures: &crate::ClosureTable,
    checked_bodies: &Arena<BodyId, CheckedBody>,
) -> Result<ReusableProgramRelations, crate::BodyRelationError> {
    let relations = BodyRelationCatalog::new(environment.graph(), checked_bodies.iter())?;
    let provenance = analyze_program_provenance(
        environment.graph(),
        types,
        environment.capability_evidence(),
        environment.interface_implementations(),
        closures,
        &relations,
    )?;
    let effects = analyze_program_effects(environment, closures, &relations)?;
    let loans = analyze_program_loans(
        environment.graph(),
        types,
        environment.capability_evidence(),
        environment.drops(),
        &provenance,
        closures,
        &relations,
    )?;
    Ok(ReusableProgramRelations {
        provenance: Arc::new(provenance),
        effects: Arc::new(effects),
        loans: Arc::new(loans),
    })
}
