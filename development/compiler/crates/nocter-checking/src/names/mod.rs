mod diagnostic;
mod model;
mod resolver;
mod reusable;

#[cfg(test)]
mod tests;

use std::fmt;

use nocter_compile_input::{CompileUnitInput, ModuleIdentity};
use nocter_declarations::DeclarationGraph;
use nocter_diagnostics::SourceDiagnostic;
use nocter_frontend_bindings::FrontendBindings;
use nocter_model::{Arena, ArenaBuilder, BodyId, ModuleId};
use nocter_source::SourceId;
use nocter_source_index::{SemanticEntity, SourceIndex, SourceRole};
use nocter_syntax::NodeId;
use nocter_syntax::SyntaxOrigin;

#[cfg(test)]
use crate::catalog_body_sources;
use crate::{
    BodyNameEvidence, BodySourceCatalog, BodySourceError, NameRejection, QueriedBodyNameRejection,
    ReusableBodyNameQueryOutcome,
};

pub use diagnostic::NameRule;
pub use model::{
    BodyScope, Capture, CaptureMode, LocalBinding, LocalBindingKind, NameTarget, ResolvedBodyNames,
    ResolvedNameUse, ScopeBinding,
};
use resolver::BodyNameResolver;
pub use reusable::{ReusableBodyNames, ReusableBodyNamesError, ReusableBodyResolutionError};

pub(crate) struct PartialNameResolution {
    pub(crate) bodies: Arena<BodyId, BodyNameEvidence>,
    pub(crate) source_index: SourceIndex,
}

pub(crate) struct RecoveringNameResolutionError {
    pub(crate) error: Box<NameResolutionError>,
    pub(crate) recovery: Option<Box<PartialNameResolution>>,
}

/// Complete temporary name-resolution product plus the extended source projection.
#[derive(Debug)]
pub struct NameResolution<'syntax> {
    body_sources: BodySourceCatalog<'syntax>,
    bodies: Arena<BodyId, ResolvedBodyNames>,
    source_index: SourceIndex,
}

impl<'syntax> NameResolution<'syntax> {
    #[must_use]
    pub const fn body_sources(&self) -> &BodySourceCatalog<'syntax> {
        &self.body_sources
    }

    #[must_use]
    pub const fn bodies(&self) -> &Arena<BodyId, ResolvedBodyNames> {
        &self.bodies
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        BodySourceCatalog<'syntax>,
        Arena<BodyId, ResolvedBodyNames>,
        SourceIndex,
    ) {
        (self.body_sources, self.bodies, self.source_index)
    }
}

/// Authored body-name failure or an inconsistent compiler boundary.
#[derive(Clone, Debug)]
pub enum NameResolutionError {
    Rule(SourceDiagnostic),
    Internal(NameResolutionInternalError),
}

impl NameResolutionError {
    #[must_use]
    pub const fn source_diagnostic(&self) -> Option<&SourceDiagnostic> {
        match self {
            Self::Rule(diagnostic) => Some(diagnostic),
            Self::Internal(_) => None,
        }
    }
}

impl fmt::Display for NameResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule(diagnostic) => {
                write!(formatter, "{}: {}", diagnostic.code(), diagnostic.message())
            }
            Self::Internal(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for NameResolutionError {}

impl From<SourceDiagnostic> for NameResolutionError {
    fn from(diagnostic: SourceDiagnostic) -> Self {
        Self::Rule(diagnostic)
    }
}

impl From<NameResolutionInternalError> for NameResolutionError {
    fn from(error: NameResolutionInternalError) -> Self {
        Self::Internal(error)
    }
}

/// Internal inconsistency at the lexical-name boundary.
#[derive(Clone, Debug)]
pub enum NameResolutionInternalError {
    BodySource(BodySourceError),
    DuplicateModuleSource(SourceId),
    MissingModuleSource(ModuleId),
    UnknownInputModule(ModuleIdentity),
    MissingUseResolution(NodeId),
    DuplicateUseResolution(NodeId),
    InvalidBlockImportTarget(NodeId),
    InvalidSyntaxNode(NodeId),
    InvalidSyntaxOrigin(SyntaxOrigin),
    MissingSymbol(Box<str>),
    MissingParameterProjection(nocter_model::ParameterId),
    InvalidBodyOwner(BodyId),
    ReusableBodyNames(ReusableBodyNameCatalogError),
}

impl fmt::Display for NameResolutionInternalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BodySource(error) => error.fmt(formatter),
            Self::DuplicateModuleSource(source) => {
                write!(formatter, "source {source} is projected by two modules")
            }
            Self::MissingModuleSource(module) => {
                write!(formatter, "module {module:?} has no declaration source")
            }
            Self::UnknownInputModule(module) => {
                write!(
                    formatter,
                    "input module {module:?} has no semantic identity"
                )
            }
            Self::MissingUseResolution(node) => {
                write!(
                    formatter,
                    "block import {node:?} has no discovery resolution"
                )
            }
            Self::DuplicateUseResolution(node) => {
                write!(formatter, "block import {node:?} has duplicate resolutions")
            }
            Self::InvalidBlockImportTarget(node) => {
                write!(formatter, "block import {node:?} does not target a module")
            }
            Self::InvalidSyntaxNode(node) => {
                write!(formatter, "body syntax node {node:?} has an invalid shape")
            }
            Self::InvalidSyntaxOrigin(origin) => {
                write!(
                    formatter,
                    "body syntax origin {origin:?} is outside its source"
                )
            }
            Self::MissingSymbol(spelling) => {
                write!(
                    formatter,
                    "body spelling `{spelling}` is absent from symbols"
                )
            }
            Self::MissingParameterProjection(parameter) => {
                write!(
                    formatter,
                    "parameter {parameter:?} has no declaration projection"
                )
            }
            Self::InvalidBodyOwner(body) => {
                write!(formatter, "body {body:?} has no valid parameter owner")
            }
            Self::ReusableBodyNames(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for NameResolutionInternalError {}

impl From<BodySourceError> for NameResolutionInternalError {
    fn from(error: BodySourceError) -> Self {
        Self::BodySource(error)
    }
}

/// Resolves every body-owned name and extends the source projection in canonical `BodyId` order.
///
/// This stage does not produce a partial checked program. Its syntax-backed uses are consumed by
/// typed-node construction, while local and capture identities enter the final body arenas.
///
/// # Errors
///
/// Returns an authored [`SourceDiagnostic`] for a body-name rule or an internal failure when the
/// Phase 2 program, discovery input, syntax catalog, and source projection disagree.
#[cfg(test)]
pub(crate) fn resolve_body_names<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    graph: &DeclarationGraph,
    bindings: &FrontendBindings,
    source_index: SourceIndex,
) -> Result<NameResolution<'syntax>, NameResolutionError> {
    let catalog =
        catalog_body_sources(input, graph, bindings).map_err(NameResolutionInternalError::from)?;
    resolve_cataloged_body_names(input, graph, bindings, source_index, catalog)
}

/// Resolves one exact body and removes all current syntax and symbol identities from the result.
///
/// # Errors
///
/// Returns the body's authored or internal name-resolution failure, or an integrity failure while
/// converting the accepted result into stable body-local locators and spellings.
#[cfg(test)]
pub(crate) fn resolve_reusable_body_names(
    input: &CompileUnitInput<'_>,
    graph: &DeclarationGraph,
    bindings: &FrontendBindings,
    source: crate::BodySource<'_>,
) -> Result<ReusableBodyNames, ReusableBodyResolutionError> {
    let resolved = BodyNameResolver::new(input, graph, bindings, source)
        .resolve_recovering()
        .map_err(|failure| ReusableBodyResolutionError::Resolution(*failure.error))?;
    ReusableBodyNames::capture(graph, source, &resolved.body, resolved.projections)
        .map_err(ReusableBodyResolutionError::Projection)
}

/// Resolves one exact body while retaining authored rejection as a query result.
///
/// Internal resolution and source-neutral projection failures remain errors because no safe
/// semantic evidence can be published for them.
pub(crate) fn resolve_reusable_body_names_for_query(
    input: &CompileUnitInput<'_>,
    graph: &DeclarationGraph,
    bindings: &FrontendBindings,
    source: crate::BodySource<'_>,
) -> Result<ReusableBodyNameQueryOutcome, ReusableBodyResolutionError> {
    match BodyNameResolver::new(input, graph, bindings, source).resolve_recovering() {
        Ok(resolved) => {
            ReusableBodyNames::capture(graph, source, &resolved.body, resolved.projections)
                .map(ReusableBodyNameQueryOutcome::Resolved)
                .map_err(ReusableBodyResolutionError::Projection)
        }
        Err(failure) => {
            let resolver::BodyResolutionFailure { error, partial } = failure;
            let NameResolutionError::Rule(diagnostic) = *error else {
                return Err(ReusableBodyResolutionError::Resolution(*error));
            };
            let partial = partial
                .map(|partial| {
                    ReusableBodyNames::capture(graph, source, &partial.body, partial.projections)
                })
                .transpose()
                .map_err(ReusableBodyResolutionError::Projection)?;
            Ok(ReusableBodyNameQueryOutcome::Rejected(
                QueriedBodyNameRejection::new(source.body(), diagnostic, partial),
            ))
        }
    }
}

/// Rebinds one source-neutral lexical result into an exact current body and extends its source
/// projection in the same operation.
///
/// # Errors
///
/// Returns an integrity failure when the current body shape, semantic identity, or symbol suffix
/// differs from the reusable result's declared input domain.
#[cfg(test)]
pub(crate) fn materialize_reusable_body_names(
    reusable: &ReusableBodyNames,
    graph: &DeclarationGraph,
    source: crate::BodySource<'_>,
    source_index: SourceIndex,
) -> Result<(ResolvedBodyNames, SourceIndex), ReusableBodyNamesError> {
    let (names, projections) = reusable.materialize(graph, source)?;
    Ok((names, extend_name_source_index(source_index, projections)))
}

/// Materializes one complete canonical body-name arena and extends source projection once.
///
/// # Errors
///
/// Returns an integrity failure for missing, duplicate, noncanonical, or unbindable body results.
fn materialize_reusable_body_name_catalog(
    graph: &DeclarationGraph,
    sources: &BodySourceCatalog<'_>,
    recipes: &[&ReusableBodyNames],
    source_index: SourceIndex,
) -> Result<(Arena<BodyId, ResolvedBodyNames>, SourceIndex), ReusableBodyNameCatalogError> {
    let mut by_body = std::collections::HashMap::new();
    for recipe in recipes {
        let body = recipe.body();
        if by_body.insert(body, *recipe).is_some() {
            return Err(ReusableBodyNameCatalogError::Duplicate(body));
        }
    }
    let mut bodies = ArenaBuilder::new();
    let mut projections = Vec::new();
    for source in sources.iter() {
        let body = source.body();
        let recipe = by_body
            .remove(&body)
            .ok_or(ReusableBodyNameCatalogError::Missing(body))?;
        let (names, mut body_projections) = recipe
            .materialize(graph, source)
            .map_err(ReusableBodyNameCatalogError::Projection)?;
        let actual = bodies.insert(names);
        if actual != body {
            return Err(ReusableBodyNameCatalogError::NonCanonical(body));
        }
        projections.append(&mut body_projections);
    }
    if let Some((body, _)) = by_body.into_iter().next() {
        return Err(ReusableBodyNameCatalogError::Unknown(body));
    }
    Ok((
        bodies.finish(),
        extend_name_source_index(source_index, projections),
    ))
}

pub(crate) enum QueriedBodyNameCatalog {
    Resolved {
        bodies: Arena<BodyId, ResolvedBodyNames>,
        source_index: SourceIndex,
    },
    Rejected {
        bodies: Arena<BodyId, BodyNameEvidence>,
        source_index: SourceIndex,
        diagnostic: SourceDiagnostic,
    },
}

enum QueriedBodyNameRecipe<'a> {
    Resolved(&'a ReusableBodyNames),
    Rejected(&'a QueriedBodyNameRejection),
}

/// Materializes one complete canonical body-name query catalog exactly once.
pub(crate) fn materialize_queried_body_name_catalog(
    graph: &DeclarationGraph,
    sources: &BodySourceCatalog<'_>,
    recipes: &[&ReusableBodyNames],
    rejections: &[&QueriedBodyNameRejection],
    source_index: SourceIndex,
) -> Result<QueriedBodyNameCatalog, ReusableBodyNameCatalogError> {
    let Some(first_rejection) = rejections.first() else {
        let (bodies, source_index) =
            materialize_reusable_body_name_catalog(graph, sources, recipes, source_index)?;
        return Ok(QueriedBodyNameCatalog::Resolved {
            bodies,
            source_index,
        });
    };
    let first_diagnostic = first_rejection.diagnostic().clone();
    let mut by_body = std::collections::HashMap::new();
    for recipe in recipes {
        let body = recipe.body();
        if by_body
            .insert(body, QueriedBodyNameRecipe::Resolved(recipe))
            .is_some()
        {
            return Err(ReusableBodyNameCatalogError::Duplicate(body));
        }
    }
    for rejection in rejections {
        let body = rejection.body();
        if by_body
            .insert(body, QueriedBodyNameRecipe::Rejected(rejection))
            .is_some()
        {
            return Err(ReusableBodyNameCatalogError::Duplicate(body));
        }
    }

    let mut bodies = ArenaBuilder::new();
    let mut projections = Vec::new();
    for source in sources.iter() {
        let body = source.body();
        let recipe = by_body
            .remove(&body)
            .ok_or(ReusableBodyNameCatalogError::Missing(body))?;
        let evidence = match recipe {
            QueriedBodyNameRecipe::Resolved(recipe) => {
                let (names, mut body_projections) = recipe
                    .materialize(graph, source)
                    .map_err(ReusableBodyNameCatalogError::Projection)?;
                projections.append(&mut body_projections);
                BodyNameEvidence::Resolved(names)
            }
            QueriedBodyNameRecipe::Rejected(rejection) => {
                let partial = rejection
                    .partial_names()
                    .map(|names| names.materialize(graph, source))
                    .transpose()
                    .map_err(ReusableBodyNameCatalogError::Projection)?
                    .map(|(names, mut body_projections)| {
                        projections.append(&mut body_projections);
                        names
                    });
                BodyNameEvidence::Rejected(NameRejection::new(
                    rejection.diagnostic().clone(),
                    partial,
                ))
            }
        };
        let actual = bodies.insert(evidence);
        if actual != body {
            return Err(ReusableBodyNameCatalogError::NonCanonical(body));
        }
    }
    if let Some((body, _)) = by_body.into_iter().next() {
        return Err(ReusableBodyNameCatalogError::Unknown(body));
    }

    let source_index = extend_name_source_index(source_index, projections);
    Ok(QueriedBodyNameCatalog::Rejected {
        bodies: bodies.finish(),
        source_index,
        diagnostic: first_diagnostic,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReusableBodyNameCatalogError {
    Missing(BodyId),
    Duplicate(BodyId),
    Unknown(BodyId),
    NonCanonical(BodyId),
    Projection(ReusableBodyNamesError),
}

impl std::fmt::Display for ReusableBodyNameCatalogError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid reusable body-name catalog: {self:?}")
    }
}

impl std::error::Error for ReusableBodyNameCatalogError {}

#[cfg(test)]
pub(crate) fn resolve_cataloged_body_names<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    graph: &DeclarationGraph,
    bindings: &FrontendBindings,
    source_index: SourceIndex,
    catalog: BodySourceCatalog<'syntax>,
) -> Result<NameResolution<'syntax>, NameResolutionError> {
    resolve_cataloged_body_names_recovering(input, graph, bindings, source_index, catalog)
        .map_err(|failure| *failure.error)
}

pub(crate) fn resolve_cataloged_body_names_recovering<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    graph: &DeclarationGraph,
    bindings: &FrontendBindings,
    source_index: SourceIndex,
    catalog: BodySourceCatalog<'syntax>,
) -> Result<NameResolution<'syntax>, RecoveringNameResolutionError> {
    resolve_cataloged_body_names_active(input, graph, bindings, source_index, catalog)
}

fn resolve_cataloged_body_names_active<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    graph: &DeclarationGraph,
    bindings: &FrontendBindings,
    source_index: SourceIndex,
    catalog: BodySourceCatalog<'syntax>,
) -> Result<NameResolution<'syntax>, RecoveringNameResolutionError> {
    let mut bodies = ArenaBuilder::new();
    let mut projections = Vec::new();
    let mut first_error = None;

    for source in catalog.iter() {
        let expected = source.body();
        let resolved =
            match BodyNameResolver::new(input, graph, bindings, source).resolve_recovering() {
                Ok(resolved) => resolved,
                Err(failure) => {
                    if failure.error.source_diagnostic().is_none() {
                        return Err(RecoveringNameResolutionError {
                            error: failure.error,
                            recovery: None,
                        });
                    }
                    let partial = failure.partial.map(|partial| {
                        projections.extend(partial.projections);
                        partial.body
                    });
                    let diagnostic = failure
                        .error
                        .source_diagnostic()
                        .expect("source-backed name rejection")
                        .clone();
                    let actual = bodies.insert(BodyNameEvidence::Rejected(NameRejection::new(
                        diagnostic, partial,
                    )));
                    if actual != expected {
                        return Err(RecoveringNameResolutionError {
                            error: Box::new(
                                NameResolutionInternalError::InvalidBodyOwner(expected).into(),
                            ),
                            recovery: None,
                        });
                    }
                    if first_error.is_none() {
                        first_error = Some(failure.error);
                    }
                    continue;
                }
            };
        let actual = bodies.insert(BodyNameEvidence::Resolved(resolved.body));
        if actual != expected {
            return Err(RecoveringNameResolutionError {
                error: Box::new(NameResolutionInternalError::InvalidBodyOwner(expected).into()),
                recovery: None,
            });
        }
        projections.extend(resolved.projections);
    }

    let source_index = extend_name_source_index(source_index, projections);

    if let Some(error) = first_error {
        return Err(RecoveringNameResolutionError {
            error,
            recovery: Some(Box::new(PartialNameResolution {
                bodies: bodies.finish(),
                source_index,
            })),
        });
    }

    let bodies = bodies
        .try_finish_with(|body, evidence| {
            let BodyNameEvidence::Resolved(names) = evidence else {
                return Err(NameResolutionInternalError::InvalidBodyOwner(body));
            };
            Ok(names)
        })
        .map_err(|error| RecoveringNameResolutionError {
            error: Box::new(error.into()),
            recovery: None,
        })?;

    Ok(NameResolution {
        body_sources: catalog,
        bodies,
        source_index,
    })
}

fn extend_name_source_index(
    source_index: SourceIndex,
    projections: Vec<Projection>,
) -> SourceIndex {
    let mut source_index = source_index.into_builder();
    for projection in projections {
        source_index.insert(projection.entity, projection.role, projection.origin);
        if let Some(markdown) = projection.documentation {
            source_index.insert_documentation(projection.entity, markdown);
        }
    }
    source_index.finish()
}

pub(super) struct Projection {
    entity: SemanticEntity,
    role: SourceRole,
    origin: nocter_source_index::SourceOrigin,
    documentation: Option<Box<str>>,
}

impl Projection {
    pub(super) const fn new(
        entity: SemanticEntity,
        role: SourceRole,
        origin: nocter_source_index::SourceOrigin,
    ) -> Self {
        Self {
            entity,
            role,
            origin,
            documentation: None,
        }
    }

    pub(super) fn with_documentation(mut self, markdown: impl Into<Box<str>>) -> Self {
        self.documentation = Some(markdown.into());
        self
    }
}
