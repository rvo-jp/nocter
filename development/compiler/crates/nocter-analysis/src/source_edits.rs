use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use nocter_filesystem::{OpenDocument, SourceOverlay, SourceOverlayError, SourceOverride};
use nocter_source::{SourceId, TextRange};

use crate::{AnalysisSnapshot, EvidenceIntegrityError};

/// One protocol-independent source replacement selected by compiler analysis.
///
/// The source identity and replacement text travel together so mutation features cannot silently
/// project an edit onto the document that happened to receive the request.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticSourceEdit {
    source: SourceId,
    range: TextRange,
    new_text: Box<str>,
}

impl SemanticSourceEdit {
    #[must_use]
    pub fn new(source: SourceId, range: TextRange, new_text: impl Into<Box<str>>) -> Self {
        Self {
            source,
            range,
            new_text: new_text.into(),
        }
    }

    #[must_use]
    pub const fn source(&self) -> SourceId {
        self.source
    }

    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }

    #[must_use]
    pub fn new_text(&self) -> &str {
        &self.new_text
    }
}

/// One exact speculative compiler input derived from an immutable source generation and its edits.
///
/// The overlay is private and can reach compilation only through the workspace mutation boundary.
/// A caller cannot substitute another overlay after this value has been constructed.
#[derive(Debug)]
pub struct SemanticMutationCandidate<'source> {
    source: &'source AnalysisSnapshot,
    edit_groups: Box<[SemanticSourceEditGroup<'source>]>,
    overlay: SourceOverlay,
    expectation: SemanticMutationExpectation,
}

#[derive(Debug)]
pub(crate) enum SemanticMutationExpectation {
    Checked,
    Rename(crate::SemanticRenamePlan),
}

impl<'source> SemanticMutationCandidate<'source> {
    pub(crate) fn checked(
        source: &'source AnalysisSnapshot,
        edits: impl IntoIterator<Item = SemanticSourceEdit>,
    ) -> Result<Self, SemanticMutationBuildError> {
        Self::new(source, edits, SemanticMutationExpectation::Checked)
    }

    pub(crate) fn rename(
        source: &'source AnalysisSnapshot,
        plan: crate::SemanticRenamePlan,
    ) -> Result<Self, SemanticMutationBuildError> {
        let edits = plan
            .edits()
            .iter()
            .map(|edit| SemanticSourceEdit::new(edit.source(), edit.range(), plan.replacement()))
            .collect::<Vec<_>>();
        Self::new(source, edits, SemanticMutationExpectation::Rename(plan))
    }

    fn new(
        source: &'source AnalysisSnapshot,
        edits: impl IntoIterator<Item = SemanticSourceEdit>,
        expectation: SemanticMutationExpectation,
    ) -> Result<Self, SemanticMutationBuildError> {
        let edit_groups = canonical_edit_groups(source, edits)?;
        let overlay = candidate_overlay(source, &edit_groups)?;
        Ok(Self {
            source,
            edit_groups,
            overlay,
            expectation,
        })
    }

    #[must_use]
    pub const fn source(&self) -> &AnalysisSnapshot {
        self.source
    }

    #[must_use]
    pub const fn source_overlay(&self) -> &SourceOverlay {
        &self.overlay
    }

    /// Seals the exact compiler result produced from this candidate's private overlay.
    ///
    /// # Errors
    ///
    /// Returns the candidate generation's complete semantic-evidence inconsistency.
    pub fn validate(
        self,
        candidate: Box<AnalysisSnapshot>,
    ) -> Result<Option<ValidatedSemanticMutation<'source>>, EvidenceIntegrityError> {
        if !same_overlay(&self.overlay, candidate.source_overlay()) {
            return Ok(None);
        }
        if !candidate.seals_semantic_mutation()? {
            return Ok(None);
        }
        if let SemanticMutationExpectation::Rename(plan) = &self.expectation
            && !self
                .source
                .rename_candidate_preserves_identity(plan, &candidate)?
        {
            return Ok(None);
        }
        Ok(Some(ValidatedSemanticMutation {
            edit_groups: self.edit_groups,
            _candidate: candidate,
        }))
    }
}

fn same_overlay(expected: &SourceOverlay, actual: &SourceOverlay) -> bool {
    if expected.len() != actual.len() {
        return false;
    }
    expected.sources().all(|(path, source)| {
        actual
            .source(path)
            .is_some_and(|actual| actual.bytes() == source.bytes())
            && actual.document(path).map(OpenDocument::version)
                == expected.document(path).map(OpenDocument::version)
    })
}

/// A source generation, exact edit set, and compiler-accepted candidate retained as one value.
///
/// This value is deliberately neither `Clone` nor `Copy`. Protocol publication consumes it, so no
/// unrelated snapshot or edit set can borrow its validation authority.
#[derive(Debug)]
pub struct ValidatedSemanticMutation<'source> {
    edit_groups: Box<[SemanticSourceEditGroup<'source>]>,
    _candidate: Box<AnalysisSnapshot>,
}

impl<'source> ValidatedSemanticMutation<'source> {
    /// Consumes publication authority and returns canonical source-grouped edits.
    #[must_use]
    pub fn into_source_edit_groups(self) -> Box<[SemanticSourceEditGroup<'source>]> {
        self.edit_groups
    }
}

/// Canonical non-overlapping edits for one source in ascending byte-range order.
///
/// Construction remains private to the mutation boundary. Protocol projections can translate
/// coordinates but cannot regroup or reinterpret edit validity.
#[derive(Debug)]
pub struct SemanticSourceEditGroup<'source> {
    source: &'source nocter_source::SourceFile,
    document_version: Option<nocter_filesystem::DocumentVersion>,
    edits: Box<[SemanticSourceEdit]>,
}

impl SemanticSourceEditGroup<'_> {
    #[must_use]
    pub const fn source(&self) -> &nocter_source::SourceFile {
        self.source
    }

    #[must_use]
    pub const fn document_version(&self) -> Option<nocter_filesystem::DocumentVersion> {
        self.document_version
    }

    #[must_use]
    pub const fn edits(&self) -> &[SemanticSourceEdit] {
        &self.edits
    }
}

fn candidate_overlay(
    snapshot: &AnalysisSnapshot,
    edit_groups: &[SemanticSourceEditGroup<'_>],
) -> Result<SourceOverlay, SemanticMutationBuildError> {
    let mut sources = BTreeMap::new();
    for (path, source) in snapshot.source_overlay().sources() {
        sources.insert(
            path.to_path_buf(),
            (
                snapshot.document_version(path),
                SourceOverride::new(source.bytes()),
            ),
        );
    }
    for source in snapshot.sources().iter() {
        let path = PathBuf::from(source.name().as_str());
        let version = snapshot.document_version(&path);
        let mut text = source.text().to_owned();
        if let Some(group) = edit_groups
            .iter()
            .find(|group| group.source.id() == source.id())
        {
            for edit in group.edits.iter().rev() {
                let start = usize::try_from(edit.range().start().get())
                    .map_err(|_| SemanticMutationBuildError::InvalidEdit(source.id()))?;
                let end = usize::try_from(edit.range().end().get())
                    .map_err(|_| SemanticMutationBuildError::InvalidEdit(source.id()))?;
                if !text.is_char_boundary(start) || !text.is_char_boundary(end) || start > end {
                    return Err(SemanticMutationBuildError::InvalidEdit(source.id()));
                }
                text.replace_range(start..end, edit.new_text());
            }
        }
        sources.insert(path, (version, SourceOverride::new(text.into_bytes())));
    }
    let mut builder = SourceOverlay::builder();
    for (path, (version, source)) in sources {
        match version {
            Some(version) => {
                builder.insert_document(path, OpenDocument::new(version, source.bytes()))
            }
            None => builder.insert_source(path, source),
        }
        .map_err(SemanticMutationBuildError::Overlay)?;
    }
    Ok(builder.finish())
}

fn canonical_edit_groups(
    snapshot: &AnalysisSnapshot,
    edits: impl IntoIterator<Item = SemanticSourceEdit>,
) -> Result<Box<[SemanticSourceEditGroup<'_>]>, SemanticMutationBuildError> {
    let mut grouped = BTreeMap::<_, Vec<_>>::new();
    for edit in edits {
        grouped.entry(edit.source()).or_default().push(edit);
    }
    if grouped.is_empty() {
        return Err(SemanticMutationBuildError::EmptyMutation);
    }
    for (source, edits) in &mut grouped {
        edits.sort_by_key(SemanticSourceEdit::range);
        if edits
            .windows(2)
            .any(|pair| pair[0].range().end() > pair[1].range().start())
        {
            return Err(SemanticMutationBuildError::OverlappingEdits(*source));
        }
    }
    grouped
        .into_iter()
        .map(|(source, edits)| {
            let source_file = snapshot
                .sources()
                .get(source)
                .ok_or(SemanticMutationBuildError::MissingSource(source))?;
            let path = std::path::Path::new(source_file.name().as_str());
            Ok(SemanticSourceEditGroup {
                source: source_file,
                document_version: snapshot.document_version(path),
                edits: edits.into_boxed_slice(),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

/// Failure to derive one exact candidate overlay from compiler-owned source edits.
#[derive(Debug)]
pub enum SemanticMutationBuildError {
    EmptyMutation,
    MissingSource(SourceId),
    InvalidEdit(SourceId),
    OverlappingEdits(SourceId),
    Overlay(SourceOverlayError),
}

impl fmt::Display for SemanticMutationBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMutation => formatter.write_str("semantic mutation has no source edits"),
            Self::MissingSource(source) => {
                write!(formatter, "mutation source is missing: {source}")
            }
            Self::InvalidEdit(source) => write!(formatter, "mutation edit is invalid in {source}"),
            Self::OverlappingEdits(source) => {
                write!(formatter, "mutation edits overlap in {source}")
            }
            Self::Overlay(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SemanticMutationBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Overlay(error) => Some(error),
            Self::EmptyMutation
            | Self::MissingSource(_)
            | Self::InvalidEdit(_)
            | Self::OverlappingEdits(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use nocter_source::{ByteOffset, TextRange};
    use nocter_workspace_revision::GenerationId;

    use super::*;
    use crate::tests::{TempTree, bundled_snapshot};

    #[test]
    fn canonical_edit_groups_are_non_empty_sorted_and_non_overlapping() {
        let tree = TempTree::new();
        let (_, snapshot) = bundled_snapshot(
            &tree,
            "func main(): void { return }\n",
            GenerationId::new(80),
        );
        let source = snapshot.sources().iter().next().unwrap().id();
        let range = |start, end| TextRange::new(ByteOffset::new(start), ByteOffset::new(end));

        let groups = canonical_edit_groups(
            &snapshot,
            [
                SemanticSourceEdit::new(source, range(8, 8), "later"),
                SemanticSourceEdit::new(source, range(2, 2), "earlier"),
            ],
        )
        .unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].edits()[0].range(), range(2, 2));
        assert_eq!(groups[0].edits()[1].range(), range(8, 8));

        assert!(matches!(
            canonical_edit_groups(&snapshot, []),
            Err(SemanticMutationBuildError::EmptyMutation)
        ));
        assert!(matches!(
            canonical_edit_groups(
                &snapshot,
                [
                    SemanticSourceEdit::new(source, range(1, 5), "left"),
                    SemanticSourceEdit::new(source, range(4, 7), "right"),
                ],
            ),
            Err(SemanticMutationBuildError::OverlappingEdits(overlap)) if overlap == source
        ));
    }
}
