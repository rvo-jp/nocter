use std::fmt;
use std::path::Path;

use nocter_analysis::{AnalysisSnapshot, EvidenceIntegrityError, SemanticLocation};
use nocter_json::Value;
use nocter_lsp::{
    DefinitionParams, DocumentUri, DocumentUriError, ImplementationParams, Location, Position,
    Range, ReferencesParams, locations_result,
};
use nocter_source::{CoordinateError, SourceId, Utf16Position};

use crate::semantic_document::{SemanticDocument, SemanticDocumentError, semantic_document};
use crate::{DocumentWorkspace, WorkspaceAnalyses};

/// Answers one definition request through exact compiler identity.
pub(crate) fn query_definition(
    documents: &DocumentWorkspace,
    analyses: &WorkspaceAnalyses,
    params: &DefinitionParams,
) -> Result<Value, NavigationQueryError> {
    let Some(document) = positioned_document(documents, analyses, params.uri())? else {
        return Ok(Value::Null);
    };
    let offset = byte_offset(&document, params.position())?;
    let locations = document
        .snapshot()
        .semantic_definition(document.source().id(), offset);
    if locations.is_empty() || !locations.coverage().is_complete() {
        return Ok(Value::Null);
    }
    project_locations(document.snapshot(), locations.values())
        .map(|locations| locations_result(&locations))
}

/// Answers one implementation request through exact compiler identity.
pub(crate) fn query_implementation(
    documents: &DocumentWorkspace,
    analyses: &WorkspaceAnalyses,
    params: &ImplementationParams,
) -> Result<Value, NavigationQueryError> {
    let Some(document) = positioned_document(documents, analyses, params.uri())? else {
        return Ok(Value::Null);
    };
    let offset = byte_offset(&document, params.position())?;
    let locations = document
        .snapshot()
        .semantic_implementation(document.source().id(), offset);
    if locations.is_empty() || !locations.coverage().is_complete() {
        return Ok(Value::Null);
    }
    project_locations(document.snapshot(), locations.values())
        .map(|locations| locations_result(&locations))
}

/// Answers one references request through exact compiler identity and reached sources only.
pub(crate) fn query_references(
    documents: &DocumentWorkspace,
    analyses: &WorkspaceAnalyses,
    params: &ReferencesParams,
) -> Result<Value, NavigationQueryError> {
    let Some(document) = positioned_document(documents, analyses, params.uri())? else {
        return Ok(Value::Array(Vec::new()));
    };
    let offset = byte_offset(&document, params.position())?;
    let locations = document
        .snapshot()
        .semantic_references(document.source().id(), offset, params.include_declaration())
        .map_err(NavigationQueryError::Evidence)?;
    if !locations.coverage().is_complete() {
        return Ok(Value::Null);
    }
    project_locations(document.snapshot(), locations.values())
        .map(|locations| locations_result(&locations))
}

fn positioned_document<'a>(
    documents: &DocumentWorkspace,
    analyses: &'a WorkspaceAnalyses,
    uri: &DocumentUri,
) -> Result<Option<SemanticDocument<'a>>, NavigationQueryError> {
    semantic_document(documents, analyses, uri).map_err(NavigationQueryError::Document)
}

fn byte_offset(
    document: &SemanticDocument<'_>,
    position: Position,
) -> Result<nocter_source::ByteOffset, NavigationQueryError> {
    document
        .source()
        .byte_offset(Utf16Position::new(position.line(), position.character()))
        .map_err(NavigationQueryError::Coordinate)
}

fn project_locations(
    snapshot: &AnalysisSnapshot,
    locations: &[SemanticLocation],
) -> Result<Vec<Location>, NavigationQueryError> {
    locations
        .iter()
        .map(|location| {
            let source = snapshot
                .sources()
                .get(location.source())
                .ok_or(NavigationQueryError::MissingSource(location.source()))?;
            let uri = DocumentUri::from_file_path(Path::new(source.name().as_str()))
                .map_err(NavigationQueryError::Uri)?;
            let range = source
                .utf16_range(location.range())
                .map_err(NavigationQueryError::Coordinate)?;
            Ok(Location::new(
                uri,
                Range::new(
                    Position::new(range.start().line(), range.start().character()),
                    Position::new(range.end().line(), range.end().character()),
                ),
            ))
        })
        .collect()
}

#[derive(Debug)]
pub enum NavigationQueryError {
    Document(SemanticDocumentError),
    Evidence(EvidenceIntegrityError),
    Coordinate(CoordinateError),
    MissingSource(SourceId),
    Uri(DocumentUriError),
}

impl fmt::Display for NavigationQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Document(error) => error.fmt(formatter),
            Self::Evidence(error) => error.fmt(formatter),
            Self::Coordinate(error) => error.fmt(formatter),
            Self::MissingSource(source) => {
                write!(formatter, "navigation source is missing: {source}")
            }
            Self::Uri(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for NavigationQueryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Document(error) => Some(error),
            Self::Evidence(error) => Some(error),
            Self::Coordinate(error) => Some(error),
            Self::Uri(error) => Some(error),
            Self::MissingSource(_) => None,
        }
    }
}
