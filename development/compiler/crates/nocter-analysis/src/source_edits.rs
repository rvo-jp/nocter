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
    edits: Box<[SemanticSourceEdit]>,
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
        let edits = edits.into_iter().collect::<Vec<_>>().into_boxed_slice();
        let overlay = candidate_overlay(source, &edits)?;
        Ok(Self {
            source,
            edits,
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
            source: self.source,
            edits: self.edits,
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
    source: &'source AnalysisSnapshot,
    edits: Box<[SemanticSourceEdit]>,
    _candidate: Box<AnalysisSnapshot>,
}

impl<'source> ValidatedSemanticMutation<'source> {
    /// Consumes publication authority and returns its inseparable source generation and edits.
    #[must_use]
    pub fn into_source_edits(self) -> (&'source AnalysisSnapshot, Box<[SemanticSourceEdit]>) {
        (self.source, self.edits)
    }
}

fn candidate_overlay(
    snapshot: &AnalysisSnapshot,
    edits: &[SemanticSourceEdit],
) -> Result<SourceOverlay, SemanticMutationBuildError> {
    let grouped = grouped_edits(edits)?;
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
        if let Some(source_edits) = grouped.get(&source.id()) {
            for edit in source_edits.iter().rev() {
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
    for source in grouped.keys() {
        if snapshot.sources().get(*source).is_none() {
            return Err(SemanticMutationBuildError::MissingSource(*source));
        }
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

fn grouped_edits(
    edits: &[SemanticSourceEdit],
) -> Result<BTreeMap<SourceId, Vec<&SemanticSourceEdit>>, SemanticMutationBuildError> {
    let mut grouped = BTreeMap::<_, Vec<_>>::new();
    for edit in edits {
        grouped.entry(edit.source()).or_default().push(edit);
    }
    for (source, edits) in &mut grouped {
        edits.sort_by_key(|edit| edit.range());
        if edits
            .windows(2)
            .any(|pair| pair[0].range().end() > pair[1].range().start())
        {
            return Err(SemanticMutationBuildError::OverlappingEdits(*source));
        }
    }
    Ok(grouped)
}

/// Failure to derive one exact candidate overlay from compiler-owned source edits.
#[derive(Debug)]
pub enum SemanticMutationBuildError {
    MissingSource(SourceId),
    InvalidEdit(SourceId),
    OverlappingEdits(SourceId),
    Overlay(SourceOverlayError),
}

impl fmt::Display for SemanticMutationBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::MissingSource(_) | Self::InvalidEdit(_) | Self::OverlappingEdits(_) => None,
        }
    }
}
