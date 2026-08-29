use std::collections::HashMap;

use nocter_model::BodyNodeId;
use nocter_source_index::{SemanticEntity, SourceAccess, SourceOrigin};
use nocter_syntax::{BodySyntaxLocator, BodySyntaxProjection, SyntaxOrigin};

use super::checker::NodeProjection;
use crate::{AssociatedTypeCompletionContext, BodySource};

#[derive(Clone, Debug)]
struct ProjectionRecipe {
    entity: SemanticEntity,
    origin: BodySyntaxLocator,
    access: Option<SourceAccess>,
}

#[derive(Clone, Debug)]
struct AssociatedTypeCompletionRecipe {
    origin: BodySyntaxLocator,
    candidates: Box<[nocter_model::AssociatedTypeId]>,
}

/// Sole source-neutral representation of successful body-checking source evidence.
///
/// The recipe is captured before a checked body may enter a reusable query result. Materialization
/// is the only path back to current-generation syntax and source coordinates.
#[derive(Clone, Debug)]
pub(super) struct BodySourceRecipe {
    body: nocter_model::BodyId,
    projections: Box<[ProjectionRecipe]>,
    node_origins: HashMap<BodyNodeId, BodySyntaxLocator>,
    associated_type_completion_contexts: Box<[AssociatedTypeCompletionRecipe]>,
}

pub(super) struct CurrentBodySourceEvidence {
    pub(super) projections: Vec<NodeProjection>,
    pub(super) node_origins: HashMap<BodyNodeId, SourceOrigin>,
    pub(super) associated_type_completion_contexts: Vec<AssociatedTypeCompletionContext>,
}

impl BodySourceRecipe {
    pub(super) fn capture(
        source: BodySource<'_>,
        projections: Vec<NodeProjection>,
        node_origins: HashMap<BodyNodeId, SourceOrigin>,
        associated_type_completion_contexts: Vec<AssociatedTypeCompletionContext>,
    ) -> Result<Self, BodySourceRecipeError> {
        let syntax = projection(source)?;
        let projections = projections
            .into_iter()
            .map(|projection| {
                Ok(ProjectionRecipe {
                    entity: projection.entity,
                    origin: locate(&syntax, projection.origin.syntax())?,
                    access: projection.access,
                })
            })
            .collect::<Result<Vec<_>, BodySourceRecipeError>>()?;
        let node_origins = node_origins
            .into_iter()
            .map(|(node, origin)| Ok((node, locate(&syntax, origin.syntax())?)))
            .collect::<Result<_, BodySourceRecipeError>>()?;
        let associated_type_completion_contexts = associated_type_completion_contexts
            .into_iter()
            .map(|context| {
                Ok(AssociatedTypeCompletionRecipe {
                    origin: locate(&syntax, context.origin().syntax())?,
                    candidates: context.candidates().into(),
                })
            })
            .collect::<Result<Vec<_>, BodySourceRecipeError>>()?;
        Ok(Self {
            body: source.body(),
            projections: projections.into_boxed_slice(),
            node_origins,
            associated_type_completion_contexts: associated_type_completion_contexts
                .into_boxed_slice(),
        })
    }

    pub(super) fn materialize(
        &self,
        source: BodySource<'_>,
    ) -> Result<CurrentBodySourceEvidence, BodySourceRecipeError> {
        if source.body() != self.body {
            return Err(BodySourceRecipeError::BodyMismatch);
        }
        let syntax = projection(source)?;
        let projections = self
            .projections
            .iter()
            .map(|recipe| {
                Ok(NodeProjection {
                    entity: recipe.entity,
                    origin: source_origin(source, resolve(&syntax, recipe.origin)?)?,
                    access: recipe.access,
                })
            })
            .collect::<Result<Vec<_>, BodySourceRecipeError>>()?;
        let node_origins = self
            .node_origins
            .iter()
            .map(|(node, locator)| Ok((*node, source_origin(source, resolve(&syntax, *locator)?)?)))
            .collect::<Result<_, BodySourceRecipeError>>()?;
        let associated_type_completion_contexts = self
            .associated_type_completion_contexts
            .iter()
            .map(|recipe| {
                Ok(AssociatedTypeCompletionContext::new(
                    source_origin(source, resolve(&syntax, recipe.origin)?)?,
                    recipe.candidates.clone(),
                ))
            })
            .collect::<Result<Vec<_>, BodySourceRecipeError>>()?;
        Ok(CurrentBodySourceEvidence {
            projections,
            node_origins,
            associated_type_completion_contexts,
        })
    }
}

fn projection(source: BodySource<'_>) -> Result<BodySyntaxProjection, BodySourceRecipeError> {
    BodySyntaxProjection::for_body(source.syntax(), source.block())
        .ok_or(BodySourceRecipeError::InvalidBody)
}

fn locate(
    projection: &BodySyntaxProjection,
    origin: SyntaxOrigin,
) -> Result<BodySyntaxLocator, BodySourceRecipeError> {
    projection
        .locator(origin)
        .ok_or(BodySourceRecipeError::MissingOrigin)
}

fn resolve(
    projection: &BodySyntaxProjection,
    locator: BodySyntaxLocator,
) -> Result<SyntaxOrigin, BodySourceRecipeError> {
    projection
        .resolve(locator)
        .ok_or(BodySourceRecipeError::MissingOrigin)
}

fn source_origin(
    source: BodySource<'_>,
    origin: SyntaxOrigin,
) -> Result<SourceOrigin, BodySourceRecipeError> {
    match origin {
        SyntaxOrigin::Node(node) => SourceOrigin::from_node(source.syntax(), node)
            .map_err(|_| BodySourceRecipeError::MissingOrigin),
        SyntaxOrigin::Token(token) => SourceOrigin::from_token(source.syntax(), token)
            .map_err(|_| BodySourceRecipeError::MissingOrigin),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodySourceRecipeError {
    InvalidBody,
    BodyMismatch,
    MissingOrigin,
}
