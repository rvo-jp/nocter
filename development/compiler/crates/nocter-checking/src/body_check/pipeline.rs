use std::collections::HashMap;

use nocter_compile_input::CompileUnitInput;
use nocter_model::{Arena, ArenaBuilder, BodyId, TypeId, TypeStore};
use nocter_source_index::{SourceIndex, SourceRole};
use nocter_syntax::{NodeId, NodeKind, SyntaxElement};

use super::checker::{BodyChecker, BodyUnitInput, CheckedBodyOutput, NodeProjection};
use super::context::BodyProgramFacts;
use super::error::{BodyCheckError, BodyCheckInternalError};
use super::ownership::{OwnershipBodyInput, analyze_body_ownership};
use crate::checked::{
    CheckedProgram, CheckedProgramAuthorities, CheckedProgramOutput, ClosureTableBuilder,
};
use crate::copyability::CopyabilityTable;
use crate::loans::{LoanBodyInput, analyze_program_loans};
use crate::preparation::PreparedCheckingParts;
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
    let mut prepared = prepared.into_parts();
    let mut closures = ClosureTableBuilder::new();
    let mut checked_types = std::mem::take(&mut prepared.types);
    let mut checked_copyabilities = std::mem::take(&mut prepared.copyabilities);
    let facts = BodyProgramFacts::from_prepared(&prepared);
    match check_declared_bodies(
        input,
        facts,
        &mut checked_types,
        &mut checked_copyabilities,
        &mut closures,
        BodyConstructionInput {
            sources: &prepared.body_sources,
            names: &prepared.body_names,
            retain_recovery: true,
        },
    ) {
        Ok(output) => build_body_analysis_recovery(
            prepared,
            checked_types,
            checked_copyabilities,
            Vec::new(),
            output.bodies,
            output.projections,
        )
        .map_err(|error| crate::BodyCheckFailure::new(error.into(), None)),
        Err(failure) => Err(recover_body_construction_failure(
            failure,
            true,
            prepared,
            checked_types,
            checked_copyabilities,
        )),
    }
}

fn check_prepared_program_internal<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    prepared: PreparedChecking<'syntax>,
    retain_prepared: bool,
) -> Result<CheckedProgramOutput, crate::BodyCheckFailure> {
    let mut prepared = prepared.into_parts();
    let mut closures = ClosureTableBuilder::new();
    let mut checked_types = if retain_prepared {
        prepared.types.clone()
    } else {
        std::mem::take(&mut prepared.types)
    };
    let mut checked_copyabilities = if retain_prepared {
        prepared.copyabilities.clone()
    } else {
        std::mem::take(&mut prepared.copyabilities)
    };
    let facts = BodyProgramFacts::from_prepared(&prepared);

    let CheckedBodiesOutput {
        bodies: checked_bodies,
        projections,
        opaque_witnesses,
        associated_type_completion_contexts,
    } = match check_declared_bodies(
        input,
        facts,
        &mut checked_types,
        &mut checked_copyabilities,
        &mut closures,
        BodyConstructionInput {
            sources: &prepared.body_sources,
            names: &prepared.body_names,
            retain_recovery: retain_prepared,
        },
    ) {
        Ok(checked) => checked,
        Err(failure) => {
            return Err(recover_body_construction_failure(
                failure,
                retain_prepared,
                prepared,
                checked_types,
                checked_copyabilities,
            ));
        }
    };

    complete_checked_program(
        prepared,
        checked_types,
        checked_copyabilities,
        closures,
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
    prepared: PreparedCheckingParts<'_>,
    mut checked_types: TypeStore,
    mut checked_copyabilities: CopyabilityTable,
    closures: ClosureTableBuilder,
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

    let closures = closures
        .finish()
        .map_err(|error| crate::BodyCheckFailure::new(error.into(), None))?;
    let opaque_witnesses = crate::OpaqueWitnessTable::build(&prepared.graph, opaque_witnesses)
        .map_err(|_| BodyCheckInternalError::OpaqueWitnessPlanning)
        .map_err(|error| crate::BodyCheckFailure::new(error.into(), None))?;
    if let Err(error) = attach_body_cleanups(
        facts,
        &mut checked_types,
        &mut checked_copyabilities,
        &closures,
        &prepared.body_sources,
        &mut checked_bodies,
    ) {
        let recovery = retain_prepared
            .then(|| {
                build_body_analysis_recovery(
                    prepared,
                    checked_types,
                    checked_copyabilities,
                    Vec::new(),
                    checked_bodies,
                    projections,
                )
            })
            .and_then(Result::ok);
        return Err(crate::BodyCheckFailure::new(error, recovery));
    }

    let (provenance, loans) = match analyze_checked_body_relations(
        &prepared.graph,
        &checked_types,
        &prepared.drops,
        &prepared.conformances,
        &closures,
        &prepared.body_sources,
        &checked_bodies,
    ) {
        Ok(relations) => relations,
        Err(error) => {
            let recovery = retain_prepared
                .then(|| {
                    build_body_analysis_recovery(
                        prepared,
                        checked_types,
                        checked_copyabilities,
                        Vec::new(),
                        checked_bodies,
                        projections,
                    )
                })
                .and_then(Result::ok);
            return Err(crate::BodyCheckFailure::new(error, recovery));
        }
    };

    finish_checked_program(
        prepared,
        CheckedProgramCompletion {
            types: checked_types,
            copyabilities: checked_copyabilities,
            bodies: checked_bodies,
            projections,
            closures,
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
    prepared: PreparedCheckingParts<'_>,
    checked_types: TypeStore,
    checked_copyabilities: CopyabilityTable,
) -> crate::BodyCheckFailure {
    let RecoveringBodyConstructionFailure {
        error,
        interruptions,
        checked_bodies,
        projections,
    } = failure;
    let recovery = retain_prepared
        .then(|| {
            build_body_analysis_recovery(
                prepared,
                checked_types,
                checked_copyabilities,
                interruptions,
                checked_bodies,
                projections,
            )
        })
        .and_then(Result::ok);
    crate::BodyCheckFailure::new(*error, recovery)
}

fn build_body_analysis_recovery(
    mut prepared: PreparedCheckingParts<'_>,
    checked_types: TypeStore,
    checked_copyabilities: CopyabilityTable,
    interruptions: Vec<(crate::TypedBodyInterruption, TypeStore, CopyabilityTable)>,
    checked_bodies: Vec<(BodyId, CheckedBodyOutput)>,
    projections: Vec<NodeProjection>,
) -> Result<crate::BodyAnalysisRecovery, BodyCheckInternalError> {
    prepared.types = checked_types;
    prepared.copyabilities = checked_copyabilities;
    prepared.source_index = extend_source_index(prepared.source_index, projections)?;
    let mut checked_bodies = checked_bodies.into_iter().peekable();
    let mut recovered = ArenaBuilder::<BodyId, Option<CheckedBody>>::new();
    for (body, _) in prepared.graph.declarations().bodies().iter() {
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
    let (prepared, source_index) = prepared.into_semantic_parts();
    Ok(crate::BodyAnalysisRecovery::new(
        prepared,
        source_index,
        interruptions,
        recovered.finish(),
    ))
}

struct CheckedProgramCompletion {
    types: TypeStore,
    copyabilities: CopyabilityTable,
    bodies: Vec<(BodyId, CheckedBodyOutput)>,
    projections: Vec<NodeProjection>,
    closures: crate::ClosureTable,
    opaque_witnesses: crate::OpaqueWitnessTable,
    associated_type_completion_contexts: Box<[crate::AssociatedTypeCompletionContext]>,
    provenance: crate::ProvenanceTable,
    loans: crate::LoanTable,
}

fn finish_checked_program(
    mut prepared: PreparedCheckingParts<'_>,
    completion: CheckedProgramCompletion,
) -> Result<CheckedProgramOutput, crate::BodyCheckFailure> {
    let CheckedProgramCompletion {
        types,
        copyabilities,
        bodies: checked_bodies,
        projections,
        closures,
        opaque_witnesses,
        associated_type_completion_contexts,
        provenance,
        loans,
    } = completion;
    prepared.types = types;
    prepared.copyabilities = copyabilities;

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

    let PreparedCheckingParts {
        graph,
        mut types,
        conformances,
        construction_surfaces,
        instance_operations,
        declaration_patterns,
        mut copyabilities,
        drops,
        standard_semantics,
        body_sources: _,
        body_names: _,
        source_namespaces: _,
        source_access,
        source_index,
    } = prepared;
    let source_index = extend_source_index(source_index, projections)
        .map_err(|error| crate::BodyCheckFailure::new(error.into(), None))?;
    copyabilities
        .complete(&graph, &mut types)
        .map_err(BodyCheckInternalError::Copyability)
        .map_err(|error| crate::BodyCheckFailure::new(error.into(), None))?;
    Ok(CheckedProgramOutput::new(
        CheckedProgram::new(
            graph,
            types,
            CheckedProgramAuthorities {
                conformances,
                construction_surfaces,
                instance_operations,
                declaration_patterns,
                copyabilities,
                drops,
                standard_semantics,
                provenance,
                loans,
                closures,
                opaque_witnesses,
                associated_type_completion_contexts,
                source_access,
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
    types: &mut TypeStore,
    copyabilities: &mut CopyabilityTable,
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
    types: &'input mut TypeStore,
    copyabilities: &'input mut CopyabilityTable,
    closures: &'input mut ClosureTableBuilder,
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
        let checkpoint =
            retain_recovery.then(|| (types.clone(), copyabilities.clone(), closures.clone()));
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
        let unit = BodyUnitInput {
            source,
            names,
            closure_ids: reserve_body_closures(closures, source),
        };
        let body_checker = BodyChecker::new(input, facts, types, copyabilities, closures, unit)
            .map_err(RecoveringBodyConstructionFailure::single)?;
        let mut body_output = match body_checker.check() {
            Ok(output) => output,
            Err(failure) => {
                let (error, interruption) = failure.into_parts();
                if !retain_recovery {
                    return Err(RecoveringBodyConstructionFailure {
                        error: Box::new(error),
                        interruptions: Vec::new(),
                        checked_bodies: Vec::new(),
                        projections: Vec::new(),
                    });
                }
                let recoverable = interruption.is_some() || error.source_diagnostic().is_some();
                if let Some(interruption) = interruption {
                    interruptions.push((interruption, types.clone(), copyabilities.clone()));
                }
                if !recoverable {
                    return Err(RecoveringBodyConstructionFailure {
                        error: Box::new(error),
                        interruptions,
                        checked_bodies,
                        projections,
                    });
                }
                let Some((type_checkpoint, copyability_checkpoint, closure_checkpoint)) =
                    checkpoint
                else {
                    return Err(RecoveringBodyConstructionFailure::single(error));
                };
                *types = type_checkpoint;
                *copyabilities = copyability_checkpoint;
                *closures = closure_checkpoint;
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

#[derive(Clone, Copy)]
struct BodyConstructionInput<'input, 'syntax> {
    sources: &'input BodySourceCatalog<'syntax>,
    names: &'input Arena<BodyId, ResolvedBodyNames>,
    retain_recovery: bool,
}

struct RecoveringBodyConstructionFailure {
    error: Box<BodyCheckError>,
    interruptions: Vec<(crate::TypedBodyInterruption, TypeStore, CopyabilityTable)>,
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
    closures: &mut ClosureTableBuilder,
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
    conformances: &crate::ConformanceTable,
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
    let provenance =
        analyze_program_provenance(graph, types, conformances, closures, &provenance_inputs)?;
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
