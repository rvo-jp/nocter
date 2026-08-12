//! Go-to-implementation queries for source-backed callable contracts.

use super::editor_targets::SourceTarget;
use super::occurrences::SemanticIdentity;
use super::{CompileUnitAnalysis, FileAnalysis};

pub(crate) fn implementation_target_for_file_analysis(
    analysis: &CompileUnitAnalysis,
    file: &FileAnalysis,
    offset: usize,
) -> Option<SourceTarget> {
    let occurrence = file.occurrences.at_offset(offset)?;
    let declaration = match occurrence.identity? {
        SemanticIdentity::Definition(definition) => definition,
        SemanticIdentity::Local(_) => return None,
    };
    let implementation = analysis
        .callable_bodies
        .implementation_id(declaration)
        .and_then(|implementation| analysis.semantic_db.definition_anchor(implementation))?;
    Some(SourceTarget::new(occurrence.focus_span, implementation))
}
