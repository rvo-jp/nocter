use std::collections::HashMap;

use nocter_declaration_lowering::CompileUnitInput;
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

/// Checks typed bodies while retaining the completed pre-body semantic stage on authored failure.
///
/// # Errors
///
/// Returns the body error and, when typed-body construction was the rejecting boundary, the exact
/// current-generation [`crate::PreparedSemanticProgram`].
pub fn check_prepared_program_recovering<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    prepared: PreparedChecking<'syntax>,
) -> Result<CheckedProgramOutput, crate::BodyCheckFailure> {
    check_prepared_program_internal(input, prepared, true)
}

fn check_prepared_program_internal<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    prepared: PreparedChecking<'syntax>,
    retain_prepared: bool,
) -> Result<CheckedProgramOutput, crate::BodyCheckFailure> {
    let mut prepared = prepared.into_parts();
    let mut closures = ClosureTableBuilder::new();
    let facts = BodyProgramFacts::new(
        &prepared.graph,
        &prepared.drops,
        &prepared.conformances,
        &prepared.construction_surfaces,
        &prepared.instance_operations,
        &prepared.standard_semantics,
        &prepared.source_index,
    );
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

    let CheckedBodiesOutput {
        bodies: mut checked_bodies,
        projections,
        opaque_witnesses,
    } = match check_declared_bodies(
        input,
        facts,
        &mut checked_types,
        &mut checked_copyabilities,
        &mut closures,
        &prepared.body_sources,
        &prepared.body_names,
    ) {
        Ok(checked) => checked,
        Err(error) => {
            return Err(crate::BodyCheckFailure::new(
                error,
                retain_prepared.then(|| prepared.into_semantic_program()),
            ));
        }
    };

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
        return Err(crate::BodyCheckFailure::new(
            error,
            retain_prepared.then(|| prepared.into_semantic_program()),
        ));
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
            return Err(crate::BodyCheckFailure::new(
                error,
                retain_prepared.then(|| prepared.into_semantic_program()),
            ));
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
            provenance,
            loans,
        },
    )
}

struct CheckedProgramCompletion {
    types: TypeStore,
    copyabilities: CopyabilityTable,
    bodies: Vec<(BodyId, CheckedBodyOutput)>,
    projections: Vec<NodeProjection>,
    closures: crate::ClosureTable,
    opaque_witnesses: crate::OpaqueWitnessTable,
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
        mut copyabilities,
        drops,
        standard_semantics,
        body_sources: _,
        body_names: _,
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
                copyabilities,
                drops,
                standard_semantics,
                provenance,
                loans,
                closures,
                opaque_witnesses,
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
    body_sources: &BodySourceCatalog<'syntax>,
    body_names: &'input Arena<BodyId, ResolvedBodyNames>,
) -> Result<CheckedBodiesOutput, BodyCheckError> {
    let mut checked_bodies = Vec::new();
    let mut projections = Vec::new();
    let mut opaque_witnesses = Vec::new();
    for (body, _) in facts.graph().declarations().bodies().iter() {
        let source = body_sources
            .get(body)
            .ok_or(BodyCheckInternalError::MissingBodySource(body))?;
        let names = body_names
            .get(body)
            .ok_or(BodyCheckInternalError::MissingBodyNames(body))?;
        if names.body() != body {
            return Err(BodyCheckInternalError::BodyIdentityMismatch(body).into());
        }
        let unit = BodyUnitInput {
            source,
            names,
            closure_ids: reserve_body_closures(closures, source),
        };
        let mut checked =
            BodyChecker::new(input, facts, types, copyabilities, closures, unit)?.check()?;
        projections.append(&mut checked.projections);
        if let Some(witness) = checked.opaque_witness {
            opaque_witnesses.push(witness);
        }
        checked_bodies.push((body, checked));
    }
    Ok(CheckedBodiesOutput {
        bodies: checked_bodies,
        projections,
        opaque_witnesses,
    })
}

struct CheckedBodiesOutput {
    bodies: Vec<(BodyId, CheckedBodyOutput)>,
    projections: Vec<NodeProjection>,
    opaque_witnesses: Vec<(nocter_model::OpaqueTypeId, TypeId)>,
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
