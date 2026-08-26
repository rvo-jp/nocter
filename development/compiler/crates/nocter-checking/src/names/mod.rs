mod diagnostic;
mod model;
mod resolver;

#[cfg(test)]
mod tests;

use std::fmt;

use nocter_compile_input::{CompileUnitInput, ModuleIdentity};
use nocter_declarations::DeclarationGraph;
use nocter_diagnostics::SourceDiagnostic;
use nocter_frontend_bindings::FrontendBindings;
use nocter_model::{Arena, ArenaBuilder, BodyId, ModuleId};
use nocter_source::SourceId;
use nocter_source_index::{
    DuplicateDocumentation, DuplicateSourceBinding, SemanticEntity, SourceIndex, SourceRole,
};
use nocter_syntax::NodeId;
use nocter_syntax::SyntaxOrigin;

use crate::{BodySourceCatalog, BodySourceError, catalog_body_sources};

pub use diagnostic::NameRule;
pub use model::{
    BodyScope, Capture, CaptureMode, LocalBinding, LocalBindingKind, NameTarget, ResolvedBodyNames,
    ResolvedNameUse, ScopeBinding,
};
use resolver::BodyNameResolver;

pub(crate) struct PartialNameResolution {
    pub(crate) bodies: Arena<BodyId, Option<ResolvedBodyNames>>,
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
#[derive(Debug)]
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
#[derive(Debug)]
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
    DuplicateSourceBinding(DuplicateSourceBinding),
    DuplicateDocumentation(DuplicateDocumentation),
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
            Self::DuplicateSourceBinding(error) => error.fmt(formatter),
            Self::DuplicateDocumentation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for NameResolutionInternalError {}

impl From<BodySourceError> for NameResolutionInternalError {
    fn from(error: BodySourceError) -> Self {
        Self::BodySource(error)
    }
}

impl From<DuplicateSourceBinding> for NameResolutionInternalError {
    fn from(error: DuplicateSourceBinding) -> Self {
        Self::DuplicateSourceBinding(error)
    }
}

impl From<DuplicateDocumentation> for NameResolutionInternalError {
    fn from(error: DuplicateDocumentation) -> Self {
        Self::DuplicateDocumentation(error)
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
pub fn resolve_body_names<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    graph: &DeclarationGraph,
    bindings: &FrontendBindings,
    source_index: SourceIndex,
) -> Result<NameResolution<'syntax>, NameResolutionError> {
    let catalog =
        catalog_body_sources(input, graph, bindings).map_err(NameResolutionInternalError::from)?;
    resolve_cataloged_body_names(input, graph, bindings, source_index, catalog)
}

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
                    let actual = bodies.insert(partial);
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
        let actual = bodies.insert(Some(resolved.body));
        if actual != expected {
            return Err(RecoveringNameResolutionError {
                error: Box::new(NameResolutionInternalError::InvalidBodyOwner(expected).into()),
                recovery: None,
            });
        }
        projections.extend(resolved.projections);
    }

    let source_index = extend_name_source_index(source_index, projections).map_err(|error| {
        RecoveringNameResolutionError {
            error: Box::new(error.into()),
            recovery: None,
        }
    })?;

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
        .try_finish_with(|body, names| {
            names.ok_or(NameResolutionInternalError::InvalidBodyOwner(body))
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
) -> Result<SourceIndex, NameResolutionInternalError> {
    let mut source_index = source_index.into_builder();
    for projection in projections {
        source_index
            .insert(projection.entity, projection.role, projection.origin)
            .map_err(NameResolutionInternalError::from)?;
        if let Some(markdown) = projection.documentation {
            source_index
                .insert_documentation(projection.entity, markdown)
                .map_err(NameResolutionInternalError::from)?;
        }
    }
    Ok(source_index.finish())
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
