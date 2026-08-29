use nocter_declarations::DeclarationGraph;
use nocter_model::{BodyId, OpaqueTypeId, TypeId};

use super::checker::CheckedBodyDraft;
use super::error::BodyCheckInternalError;
use super::semantic_transaction::BodySemanticAuthority;
use super::source_recipe::BodySourceRecipe;
use crate::checked::CheckedBodyRecipe;
use crate::{BodyClosureRecipe, BodySource, BodyTypeRecipe};

/// Complete source-neutral semantic result of checking one body in isolation.
///
/// The checked graph and opaque witness still contain identities from the private capture branch.
/// They are never exposed independently: `types` and `closures` are the sole interpretation of
/// those identities, and canonical materialization rebinds every retained reference together.
#[derive(Clone, Debug)]
pub struct ReusableCheckedBody {
    body: BodyId,
    checked: CheckedBodyRecipe,
    source: BodySourceRecipe,
    types: BodyTypeRecipe,
    closures: BodyClosureRecipe,
    opaque_witness: Option<(OpaqueTypeId, TypeId)>,
    copy_proofs: crate::copyability::CopyProofs,
}

impl ReusableCheckedBody {
    #[must_use]
    pub const fn body(&self) -> BodyId {
        self.body
    }
}

pub(super) fn capture_checked_body(
    program_semantics: &crate::semantic_authority::SemanticAuthority,
    body_semantics: &BodySemanticAuthority,
    source: BodySource<'_>,
    output: CheckedBodyDraft,
) -> Result<ReusableCheckedBody, BodyCheckInternalError> {
    let body = source.body();
    let closure_identities = body_semantics.closures().body_identities(body)?;
    let type_capture = BodyTypeRecipe::capture_authority(
        program_semantics.types(),
        body_semantics.semantics().types(),
        &closure_identities,
    )?;
    let closures =
        body_semantics
            .closures()
            .capture_body_recipe(body, &closure_identities, &type_capture)?;
    let source_recipe = BodySourceRecipe::capture(
        source,
        output.projections,
        output.node_origins,
        output.associated_type_completion_contexts,
    )?;
    Ok(ReusableCheckedBody {
        body,
        checked: output.body,
        source: source_recipe,
        types: type_capture.into_recipe(),
        closures,
        opaque_witness: output.opaque_witness,
        copy_proofs: output.copy_proofs,
    })
}

pub(super) struct MaterializedCheckedBody {
    pub(super) body: crate::CheckedBody,
    pub(super) projections: Vec<super::checker::NodeProjection>,
    pub(super) node_origins:
        std::collections::HashMap<nocter_model::BodyNodeId, nocter_source_index::SourceOrigin>,
    pub(super) opaque_witness: Option<(OpaqueTypeId, TypeId)>,
    pub(super) copy_proofs: crate::copyability::CopyProofs,
    pub(super) associated_type_completion_contexts: Vec<crate::AssociatedTypeCompletionContext>,
}

pub(super) fn materialize_checked_body(
    graph: &DeclarationGraph,
    program_semantics: &crate::semantic_authority::SemanticAuthority,
    source: BodySource<'_>,
    names: &crate::ResolvedBodyNames,
    reusable: &ReusableCheckedBody,
    accepted: &mut BodySemanticAuthority,
) -> Result<MaterializedCheckedBody, BodyCheckInternalError> {
    if source.body() != reusable.body() {
        return Err(BodyCheckInternalError::BodyIdentityMismatch(source.body()));
    }
    let mut transaction = accepted.transaction();
    let replayed_closures = reusable.closures.reserve(transaction.closures_mut());
    let replayed_types = reusable.types.replay(
        program_semantics.types(),
        transaction.types_mut(),
        replayed_closures.ids(),
    )?;
    {
        let (types, copyabilities, closures) = transaction.replay_parts();
        reusable
            .closures
            .define(closures, &replayed_closures, &replayed_types)?;
        reusable
            .closures
            .register_copyability(graph, types, copyabilities, &replayed_types)?;
    }
    let semantics = crate::checked::CheckedSemanticRebinder::new(
        &reusable.types,
        &replayed_types,
        &reusable.closures,
        &replayed_closures,
    );
    let body = crate::CheckedBody::rebind(
        reusable.checked.clone(),
        names,
        source.syntax().source(),
        &semantics,
    )?;
    let opaque_witness = reusable
        .opaque_witness
        .map(|(opaque, witness)| {
            Ok::<_, crate::CheckedSemanticRebindError>((opaque, semantics.ty(witness)?))
        })
        .transpose()?;
    let source_evidence = reusable.source.materialize(source)?;
    *accepted = transaction
        .commit(accepted)
        .map_err(|_| BodyCheckInternalError::BodySemanticCommit)?;
    Ok(MaterializedCheckedBody {
        body,
        projections: source_evidence.projections,
        node_origins: source_evidence.node_origins,
        opaque_witness,
        copy_proofs: reusable.copy_proofs.clone(),
        associated_type_completion_contexts: source_evidence.associated_type_completion_contexts,
    })
}
