use std::collections::HashMap;

use nocter_compile_input::CompileUnitInput;
use nocter_model::{Arena, ArenaBuilder, BodyId, TypeId, TypeStore};
use nocter_source_index::{SourceIndex, SourceRole};
use nocter_syntax::{NodeId, NodeKind, SyntaxElement};

use super::checker::{BodyChecker, BodyUnitInput, CheckedBodyOutput, NodeProjection};
use super::context::BodyProgramFacts;
use super::error::{BodyCheckError, BodyCheckInternalError};
use super::ownership::{OwnershipBodyInput, analyze_body_ownership};
use super::semantic_transaction::{BodySemanticAccess, BodySemanticAuthority};
use crate::checked::{
    CheckedProgram, CheckedProgramAuthorities, CheckedProgramOutput, ClosureAuthority,
    ClosureTransaction,
};
use crate::loans::{LoanBodyInput, analyze_program_loans};
use crate::preparation::BodyCheckingParts;
use crate::provenance::{ProvenanceBodyInput, analyze_program_provenance};
use crate::{
    BodySource, BodySourceCatalog, CheckedBody, DropTable, PreparedChecking, ResolvedBodyNames,
};

/// Checks one prepared program without retaining a tooling recovery value.
///
/// # Errors
///
/// Returns one source-backed body rule or an internal consistency error.
pub fn check_prepared_program<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    prepared: PreparedChecking<'syntax>,
) -> Result<CheckedProgramOutput, BodyCheckError> {
    check_prepared_program_internal(input, prepared, false)
        .map_err(|failure| failure.into_parts().0)
}

/// Checks typed bodies while retaining current-generation analysis recovery on authored failure.
///
/// # Errors
///
/// Returns the first canonical body error and, when typed-body construction was the rejecting
/// boundary, the current-generation [`crate::BodyAnalysisRecovery`]. Recovery always contains the
/// completed preparation stage and may additionally contain independent phase-owned typed
/// interruptions from every authored body failure.
pub fn check_prepared_program_recovering<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    prepared: PreparedChecking<'syntax>,
) -> Result<CheckedProgramOutput, crate::BodyCheckFailure> {
    check_prepared_program_internal(input, prepared, true)
}

/// Types bodies from a rejected declaration graph without opening a checked-program transition.
///
/// Every body is checked transactionally. Independently successful bodies become sparse editor
/// evidence whether or not another body fails; ownership, provenance, executable dispatch, and
/// target closure are never attempted by this endpoint.
///
/// # Errors
///
/// Returns the first canonical body error together with any independently retained body evidence.
pub fn analyze_prepared_program_bodies<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    prepared: crate::PreparedBodyAnalysis<'syntax>,
) -> Result<crate::BodyAnalysisRecovery, crate::BodyCheckFailure> {
    let (accepted_semantics, prepared) = prepared.into_parts().into_body_parts();
    let mut semantics = BodySemanticAuthority::new(accepted_semantics, ClosureAuthority::new());
    let facts = BodyProgramFacts::from_prepared(&prepared);
    match check_declared_bodies(
        input,
        facts,
        &mut semantics,
        BodyConstructionInput {
            sources: &prepared.body_sources,
            names: &prepared.body_names,
            retain_recovery: true,
        },
    ) {
        Ok(output) => {
            let checked_semantics = semantics.finish_recovery();
            build_body_analysis_recovery(
                prepared,
                checked_semantics,
                Vec::new(),
                output.bodies,
                output.projections,
            )
            .map_err(|error| crate::BodyCheckFailure::new(error.into(), None))
        }
        Err(failure) => {
            let checked_semantics = semantics.finish_recovery();
            Err(recover_body_construction_failure(
                failure,
                true,
                prepared,
                checked_semantics,
            ))
        }
    }
}

fn check_prepared_program_internal<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    prepared: PreparedChecking<'syntax>,
    retain_prepared: bool,
) -> Result<CheckedProgramOutput, crate::BodyCheckFailure> {
    let (accepted_semantics, prepared) = prepared.into_parts().into_body_parts();
    let mut semantics = BodySemanticAuthority::new(accepted_semantics, ClosureAuthority::new());
    let facts = BodyProgramFacts::from_prepared(&prepared);

    let CheckedBodiesOutput {
        bodies: checked_bodies,
        projections,
        opaque_witnesses,
        associated_type_completion_contexts,
    } = match check_declared_bodies(
        input,
        facts,
        &mut semantics,
        BodyConstructionInput {
            sources: &prepared.body_sources,
            names: &prepared.body_names,
            retain_recovery: retain_prepared,
        },
    ) {
        Ok(checked) => checked,
        Err(failure) => {
            let checked_semantics = semantics.finish_recovery();
            return Err(recover_body_construction_failure(
                failure,
                retain_prepared,
                prepared,
                checked_semantics,
            ));
        }
    };

    complete_checked_program(
        prepared,
        semantics,
        CheckedBodiesOutput {
            bodies: checked_bodies,
            projections,
            opaque_witnesses,
            associated_type_completion_contexts,
        },
        retain_prepared,
    )
}

fn complete_checked_program(
    prepared: BodyCheckingParts<'_>,
    checked_semantics: BodySemanticAuthority,
    output: CheckedBodiesOutput,
    retain_prepared: bool,
) -> Result<CheckedProgramOutput, crate::BodyCheckFailure> {
    let CheckedBodiesOutput {
        bodies: mut checked_bodies,
        projections,
        opaque_witnesses,
        associated_type_completion_contexts,
    } = output;
    let facts = BodyProgramFacts::from_prepared(&prepared);

    let mut checked_semantics = checked_semantics
        .finish_checked()
        .map_err(|error| crate::BodyCheckFailure::new(error.into(), None))?;
    let opaque_witnesses =
        crate::OpaqueWitnessTable::build(prepared.environment.graph(), opaque_witnesses)
            .map_err(|_| BodyCheckInternalError::OpaqueWitnessPlanning)
            .map_err(|error| crate::BodyCheckFailure::new(error.into(), None))?;
    let mut cleanup = checked_semantics.transaction();
    let cleanup_access = cleanup.access();
    if let Err(error) = attach_body_cleanups(
        facts,
        cleanup_access.types,
        cleanup_access.copyabilities,
        checked_semantics.closures(),
        &prepared.body_sources,
        &mut checked_bodies,
    ) {
        checked_semantics.retain_recovery_branch(cleanup);
        let recovery = if retain_prepared {
            build_body_analysis_recovery(
                prepared,
                checked_semantics.semantics().clone(),
                Vec::new(),
                checked_bodies,
                projections,
            )
            .map(Some)
        } else {
            Ok(None)
        };
        return Err(crate::BodyCheckFailure::from_recovery_result(
            error, recovery,
        ));
    }
    checked_semantics.accept(cleanup);

    let (provenance, loans) = match analyze_checked_body_relations(
        prepared.environment.graph(),
        checked_semantics.semantics().types(),
        prepared.environment.drops(),
        prepared.environment.interface_implementations(),
        checked_semantics.closures(),
        &prepared.body_sources,
        &checked_bodies,
    ) {
        Ok(relations) => relations,
        Err(error) => {
            let recovery = if retain_prepared {
                build_body_analysis_recovery(
                    prepared,
                    checked_semantics.semantics().clone(),
                    Vec::new(),
                    checked_bodies,
                    projections,
                )
                .map(Some)
            } else {
                Ok(None)
            };
            return Err(crate::BodyCheckFailure::from_recovery_result(
                error, recovery,
            ));
        }
    };

    finish_checked_program(
        prepared,
        CheckedProgramCompletion {
            semantics: checked_semantics,
            bodies: checked_bodies,
            projections,
            opaque_witnesses,
            associated_type_completion_contexts: associated_type_completion_contexts
                .into_boxed_slice(),
            provenance,
            loans,
        },
    )
}

fn recover_body_construction_failure(
    failure: RecoveringBodyConstructionFailure,
    retain_prepared: bool,
    prepared: BodyCheckingParts<'_>,
    checked_semantics: crate::semantic_authority::SemanticAuthority,
) -> crate::BodyCheckFailure {
    let RecoveringBodyConstructionFailure {
        error,
        interruptions,
        checked_bodies,
        projections,
    } = failure;
    let recovery = if retain_prepared {
        build_body_analysis_recovery(
            prepared,
            checked_semantics,
            interruptions,
            checked_bodies,
            projections,
        )
        .map(Some)
    } else {
        Ok(None)
    };
    crate::BodyCheckFailure::from_recovery_result(*error, recovery)
}

fn build_body_analysis_recovery(
    mut prepared: BodyCheckingParts<'_>,
    checked_semantics: crate::semantic_authority::SemanticAuthority,
    interruptions: Vec<(
        crate::TypedBodyInterruption,
        crate::recovery::TypedInterruptionEvidence,
    )>,
    checked_bodies: Vec<(BodyId, CheckedBodyOutput)>,
    projections: Vec<NodeProjection>,
) -> Result<crate::BodyAnalysisRecovery, BodyCheckInternalError> {
    prepared.source_index = extend_source_index(prepared.source_index, projections)?;
    let mut checked_bodies = checked_bodies.into_iter().peekable();
    let mut recovered = ArenaBuilder::<BodyId, Option<CheckedBody>>::new();
    for (body, _) in prepared.environment.graph().declarations().bodies().iter() {
        let value = if checked_bodies
            .peek()
            .is_some_and(|(candidate, _)| *candidate == body)
        {
            checked_bodies.next().map(|(_, output)| output.body)
        } else {
            None
        };
        let actual = recovered.insert(value);
        if actual != body {
            return Err(BodyCheckInternalError::NonCanonicalBody(body));
        }
    }
    if let Some((body, _)) = checked_bodies.next() {
        return Err(BodyCheckInternalError::NonCanonicalBody(body));
    }
    let (prepared, body_names, source_index) = prepared.into_semantic_parts(checked_semantics);
    Ok(crate::BodyAnalysisRecovery::new(
        prepared,
        body_names,
        source_index,
        interruptions,
        recovered.finish(),
    ))
}

struct CheckedProgramCompletion {
    semantics: crate::semantic_authority::CheckedSemanticAuthority,
    bodies: Vec<(BodyId, CheckedBodyOutput)>,
    projections: Vec<NodeProjection>,
    opaque_witnesses: crate::OpaqueWitnessTable,
    associated_type_completion_contexts: Box<[crate::AssociatedTypeCompletionContext]>,
    provenance: crate::ProvenanceTable,
    loans: crate::LoanTable,
}

fn finish_checked_program(
    prepared: BodyCheckingParts<'_>,
    completion: CheckedProgramCompletion,
) -> Result<CheckedProgramOutput, crate::BodyCheckFailure> {
    let CheckedProgramCompletion {
        mut semantics,
        bodies: checked_bodies,
        projections,
        opaque_witnesses,
        associated_type_completion_contexts,
        provenance,
        loans,
    } = completion;
    let mut bodies = ArenaBuilder::<BodyId, CheckedBody>::new();
    for (body, checked) in checked_bodies {
        let actual = bodies.insert(checked.body);
        if actual != body {
            return Err(crate::BodyCheckFailure::new(
                BodyCheckInternalError::NonCanonicalBody(body).into(),
                None,
            ));
        }
    }

    let BodyCheckingParts {
        environment,
        body_sources: _,
        body_names: _,
        source_namespaces: _,
        source_index,
    } = prepared;
    let graph = environment.graph();
    let source_index = extend_source_index(source_index, projections)
        .map_err(|error| crate::BodyCheckFailure::new(error.into(), None))?;
    let mut semantic_completion = semantics.transaction();
    let completion_access = semantic_completion.access();
    completion_access
        .copyabilities
        .complete(graph, completion_access.types)
        .map_err(BodyCheckInternalError::Copyability)
        .map_err(|error| crate::BodyCheckFailure::new(error.into(), None))?;
    semantics.accept(semantic_completion);
    Ok(CheckedProgramOutput::new(
        CheckedProgram::new(
            environment,
            semantics,
            CheckedProgramAuthorities {
                provenance,
                loans,
                opaque_witnesses,
                associated_type_completion_contexts,
            },
            bodies.finish(),
        ),
        source_index,
    ))
}

fn extend_source_index(
    source_index: SourceIndex,
    projections: Vec<NodeProjection>,
) -> Result<SourceIndex, BodyCheckInternalError> {
    let mut source_index = source_index.into_builder();
    for projection in projections {
        match projection.access {
            Some(access) => source_index.insert_with_access(
                projection.entity,
                SourceRole::Reference,
                projection.origin,
                access,
            ),
            None => {
                source_index.insert(projection.entity, SourceRole::Reference, projection.origin)
            }
        }?;
    }
    Ok(source_index.finish())
}

fn attach_body_cleanups(
    facts: BodyProgramFacts<'_>,
    types: &mut nocter_model::TypeTransaction,
    copyabilities: &mut crate::copyability::CopyabilityTransaction,
    closures: &crate::ClosureTable,
    body_sources: &BodySourceCatalog<'_>,
    checked_bodies: &mut [(BodyId, CheckedBodyOutput)],
) -> Result<(), BodyCheckError> {
    for (body, checked) in checked_bodies {
        let source = body_sources
            .get(*body)
            .ok_or(BodyCheckInternalError::MissingBodySource(*body))?;
        let cleanups = analyze_body_ownership(
            facts.graph(),
            types,
            copyabilities,
            facts.drops(),
            closures,
            OwnershipBodyInput::new(
                source,
                &checked.body,
                &checked.node_origins,
                &checked.copy_proofs,
            ),
        )?;
        checked.body.attach_cleanups(cleanups)?;
    }
    Ok(())
}

fn check_declared_bodies<'input, 'syntax>(
    input: &'input CompileUnitInput<'syntax>,
    facts: BodyProgramFacts<'input>,
    semantics: &'input mut BodySemanticAuthority,
    construction: BodyConstructionInput<'input, 'syntax>,
) -> Result<CheckedBodiesOutput, RecoveringBodyConstructionFailure> {
    let BodyConstructionInput {
        sources: body_sources,
        names: body_names,
        retain_recovery,
    } = construction;
    let mut checked_bodies = Vec::new();
    let mut projections = Vec::new();
    let mut opaque_witnesses = Vec::new();
    let mut associated_type_completion_contexts = Vec::new();
    let mut first_error = None;
    let mut interruptions = Vec::new();
    for (body, _) in facts.graph().declarations().bodies().iter() {
        let source = body_sources
            .get(body)
            .ok_or(BodyCheckInternalError::MissingBodySource(body))
            .map_err(|error| RecoveringBodyConstructionFailure::single(error.into()))?;
        let names = body_names
            .get(body)
            .ok_or(BodyCheckInternalError::MissingBodyNames(body))
            .map_err(|error| RecoveringBodyConstructionFailure::single(error.into()))?;
        if names.body() != body {
            return Err(RecoveringBodyConstructionFailure::single(
                BodyCheckInternalError::BodyIdentityMismatch(body).into(),
            ));
        }
        let attempt = attempt_body(input, facts, source, names, retain_recovery, semantics);
        let mut body_output = match attempt {
            Ok(output) => output,
            Err(BodyAttemptFailure::Direct(failure)) => {
                return Err(RecoveringBodyConstructionFailure {
                    error: Box::new(failure.into_parts().0),
                    interruptions: Vec::new(),
                    checked_bodies: Vec::new(),
                    projections: Vec::new(),
                });
            }
            Err(BodyAttemptFailure::Recovering {
                failure,
                interruption_state,
            }) => {
                let (error, interruption) = failure.into_parts();
                let recoverable = interruption.is_some() || error.source_diagnostic().is_some();
                if let Some(interruption) = interruption {
                    let evidence = interruption_state
                        .expect("typed interruption must retain its semantic state");
                    interruptions.push((interruption, evidence));
                }
                if !recoverable {
                    return Err(RecoveringBodyConstructionFailure {
                        error: Box::new(error),
                        interruptions,
                        checked_bodies,
                        projections,
                    });
                }
                if first_error.is_none() {
                    first_error = Some(error);
                }
                continue;
            }
        };
        projections.append(&mut body_output.projections);
        associated_type_completion_contexts
            .append(&mut body_output.associated_type_completion_contexts);
        if let Some(witness) = body_output.opaque_witness {
            opaque_witnesses.push(witness);
        }
        checked_bodies.push((body, body_output));
    }
    if let Some(error) = first_error {
        return Err(RecoveringBodyConstructionFailure {
            error: Box::new(error),
            interruptions,
            checked_bodies,
            projections,
        });
    }
    Ok(CheckedBodiesOutput {
        bodies: checked_bodies,
        projections,
        opaque_witnesses,
        associated_type_completion_contexts,
    })
}

fn attempt_body<'input, 'syntax>(
    input: &'input CompileUnitInput<'syntax>,
    facts: BodyProgramFacts<'input>,
    source: BodySource<'syntax>,
    names: &'input ResolvedBodyNames,
    retain_recovery: bool,
    state: &'input mut BodySemanticAuthority,
) -> Result<CheckedBodyOutput, BodyAttemptFailure> {
    let mut transaction = state.transaction();
    if !retain_recovery {
        let output = construct_body(input, facts, source, names, transaction.access())
            .map_err(BodyAttemptFailure::Direct)?;
        let committed = transaction
            .commit(state)
            .expect("body transaction must commit to its exact accepted authorities");
        *state = committed;
        return Ok(output);
    }
    match construct_body(input, facts, source, names, transaction.access()) {
        Ok(output) => {
            let committed = transaction
                .commit(state)
                .expect("body transaction must commit to its exact accepted authorities");
            *state = committed;
            Ok(output)
        }
        Err(failure) => {
            let recovery_semantics = transaction.freeze_recovery();
            let interruption_state =
                match retain_interruption_evidence(&failure, recovery_semantics) {
                    Ok(state) => state,
                    Err(error) => {
                        return Err(BodyAttemptFailure::Recovering {
                            failure: super::error::BodyConstructionFailure::new(error.into(), None),
                            interruption_state: None,
                        });
                    }
                };
            Err(BodyAttemptFailure::Recovering {
                failure,
                interruption_state,
            })
        }
    }
}

enum BodyAttemptFailure {
    Direct(super::error::BodyConstructionFailure),
    Recovering {
        failure: super::error::BodyConstructionFailure,
        interruption_state: Option<crate::recovery::TypedInterruptionEvidence>,
    },
}

fn retain_interruption_evidence(
    failure: &super::error::BodyConstructionFailure,
    semantics: crate::semantic_authority::SemanticAuthority,
) -> Result<Option<crate::recovery::TypedInterruptionEvidence>, BodyCheckInternalError> {
    let Some(interruption) = failure.interruption() else {
        return Ok(None);
    };
    let evidence = match interruption.kind() {
        crate::TypedBodyInterruptionKind::MemberSelection { .. } => {
            crate::recovery::TypedInterruptionEvidence::MemberSelection(Box::new(
                crate::recovery::MemberInterruptionEvidence { semantics },
            ))
        }
        crate::TypedBodyInterruptionKind::OutcomeContract {
            proposed_result, ..
        } => crate::recovery::TypedInterruptionEvidence::Outcome(Box::new(
            semantics.types().project(*proposed_result)?,
        )),
        crate::TypedBodyInterruptionKind::ConstructionSelection { .. }
        | crate::TypedBodyInterruptionKind::StructuralConstruction { .. }
        | crate::TypedBodyInterruptionKind::EnumPattern { .. }
        | crate::TypedBodyInterruptionKind::AssociatedTypeProjection { .. } => {
            crate::recovery::TypedInterruptionEvidence::None
        }
    };
    Ok(Some(evidence))
}

fn construct_body<'input, 'syntax>(
    input: &'input CompileUnitInput<'syntax>,
    facts: BodyProgramFacts<'input>,
    source: BodySource<'syntax>,
    names: &'input ResolvedBodyNames,
    mut state: BodySemanticAccess<'input>,
) -> Result<CheckedBodyOutput, super::error::BodyConstructionFailure> {
    let closures = state.closures();
    let unit = BodyUnitInput {
        source,
        names,
        closure_ids: reserve_body_closures(closures, source),
    };
    BodyChecker::new(input, facts, state, unit)
        .map_err(|error| super::error::BodyConstructionFailure::new(error, None))?
        .check()
}

#[derive(Clone, Copy)]
struct BodyConstructionInput<'input, 'syntax> {
    sources: &'input BodySourceCatalog<'syntax>,
    names: &'input Arena<BodyId, ResolvedBodyNames>,
    retain_recovery: bool,
}

struct RecoveringBodyConstructionFailure {
    error: Box<BodyCheckError>,
    interruptions: Vec<(
        crate::TypedBodyInterruption,
        crate::recovery::TypedInterruptionEvidence,
    )>,
    checked_bodies: Vec<(BodyId, CheckedBodyOutput)>,
    projections: Vec<NodeProjection>,
}

impl RecoveringBodyConstructionFailure {
    fn single(error: BodyCheckError) -> Self {
        Self {
            error: Box::new(error),
            interruptions: Vec::new(),
            checked_bodies: Vec::new(),
            projections: Vec::new(),
        }
    }
}

struct CheckedBodiesOutput {
    bodies: Vec<(BodyId, CheckedBodyOutput)>,
    projections: Vec<NodeProjection>,
    opaque_witnesses: Vec<(nocter_model::OpaqueTypeId, TypeId)>,
    associated_type_completion_contexts: Vec<crate::AssociatedTypeCompletionContext>,
}

fn reserve_body_closures(
    closures: &mut ClosureTransaction,
    source: BodySource<'_>,
) -> HashMap<NodeId, nocter_model::ClosureId> {
    let mut reserved = HashMap::new();
    let mut pending = vec![source.block()];
    while let Some(node) = pending.pop() {
        if source
            .syntax()
            .node(node)
            .is_some_and(|syntax| syntax.kind() == NodeKind::ClosureExpression)
        {
            reserved.insert(node, closures.reserve(source.body()));
        }
        pending.extend(
            source
                .syntax()
                .children(node)
                .iter()
                .rev()
                .filter_map(|element| match element {
                    SyntaxElement::Node(child) => Some(*child),
                    SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
                }),
        );
    }
    reserved
}

fn analyze_checked_body_relations(
    graph: &nocter_declarations::DeclarationGraph,
    types: &TypeStore,
    drops: &DropTable,
    interface_implementations: &crate::InterfaceImplementationTable,
    closures: &crate::ClosureTable,
    body_sources: &BodySourceCatalog<'_>,
    checked_bodies: &[(BodyId, CheckedBodyOutput)],
) -> Result<(crate::ProvenanceTable, crate::LoanTable), BodyCheckError> {
    let provenance_inputs = checked_bodies
        .iter()
        .map(|(body, checked)| {
            let source = body_sources
                .get(*body)
                .ok_or(BodyCheckInternalError::MissingBodySource(*body))?;
            Ok(ProvenanceBodyInput::new(
                source,
                &checked.body,
                &checked.node_origins,
            ))
        })
        .collect::<Result<Vec<_>, BodyCheckError>>()?;
    let provenance = analyze_program_provenance(
        graph,
        types,
        interface_implementations,
        closures,
        &provenance_inputs,
    )?;
    let loan_inputs = checked_bodies
        .iter()
        .map(|(body, checked)| {
            let source = body_sources
                .get(*body)
                .ok_or(BodyCheckInternalError::MissingBodySource(*body))?;
            Ok(LoanBodyInput::new(
                source,
                &checked.body,
                &checked.node_origins,
            ))
        })
        .collect::<Result<Vec<_>, BodyCheckError>>()?;
    let loans = analyze_program_loans(graph, types, drops, &provenance, closures, &loan_inputs)?;
    Ok((provenance, loans))
}
