use std::fmt;

use nocter_checking::{ConformanceRule, NameRule, PreparationError};
use nocter_session::CompileSessionError;
use nocter_source::{SourceId, TextRange};

use crate::{AnalysisSnapshot, SemanticCompletionError, SemanticSourceEdit};

mod conformance;

pub use conformance::ConformanceActionError;

/// One compiler-owned source repair independent of editor protocol and workspace mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticCodeAction {
    title: Box<str>,
    diagnostic_code: Box<str>,
    diagnostic_range: TextRange,
    edits: Box<[SemanticSourceEdit]>,
}

impl SemanticCodeAction {
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn diagnostic_code(&self) -> &str {
        &self.diagnostic_code
    }

    #[must_use]
    pub const fn diagnostic_range(&self) -> TextRange {
        self.diagnostic_range
    }

    #[must_use]
    pub const fn edits(&self) -> &[SemanticSourceEdit] {
        &self.edits
    }
}

/// An inconsistency while planning a source repair from retained compiler state.
#[derive(Debug)]
pub enum SemanticCodeActionError {
    MissingSource(SourceId),
    InvalidDiagnosticRange { source: SourceId, range: TextRange },
    Completion(SemanticCompletionError),
    Conformance(ConformanceActionError),
}

impl fmt::Display for SemanticCodeActionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSource(source) => {
                write!(formatter, "code-action source {source} is absent")
            }
            Self::InvalidDiagnosticRange { source, range } => write!(
                formatter,
                "code-action diagnostic range is invalid in {source}: {}..{}",
                range.start().get(),
                range.end().get()
            ),
            Self::Completion(error) => error.fmt(formatter),
            Self::Conformance(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SemanticCodeActionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Completion(error) => Some(error),
            Self::Conformance(error) => Some(error),
            Self::MissingSource(_) | Self::InvalidDiagnosticRange { .. } => None,
        }
    }
}

impl From<SemanticCompletionError> for SemanticCodeActionError {
    fn from(error: SemanticCompletionError) -> Self {
        Self::Completion(error)
    }
}

impl From<ConformanceActionError> for SemanticCodeActionError {
    fn from(error: ConformanceActionError) -> Self {
        Self::Conformance(error)
    }
}

impl AnalysisSnapshot {
    /// Plans exact repairs for current diagnostics intersecting `requested_range`.
    ///
    /// Diagnostic codes select a compiler-owned rule family. Authored names and edit positions are
    /// recovered from source identities and semantic completion metadata, never from rendered
    /// diagnostic messages or help text.
    ///
    /// # Errors
    ///
    /// Returns an internal query error when retained source ranges or semantic completion state
    /// disagree with the diagnostic generation.
    pub fn semantic_code_actions(
        &self,
        source: SourceId,
        requested_range: TextRange,
    ) -> Result<Box<[SemanticCodeAction]>, SemanticCodeActionError> {
        let source_file = self
            .sources()
            .get(source)
            .ok_or(SemanticCodeActionError::MissingSource(source))?;
        let mut actions = Vec::new();
        for diagnostic in self.diagnostics() {
            let primary = diagnostic.primary();
            let range = primary.span().range();
            if primary.source() != source || !ranges_intersect(range, requested_range) {
                continue;
            }
            if diagnostic.code() == ConformanceRule::MissingMethod.code() {
                let Some(missing) = self.missing_conformance_methods() else {
                    continue;
                };
                if let Some(action) = conformance::missing_method_action(
                    self,
                    source,
                    diagnostic.code(),
                    range,
                    missing,
                )? {
                    actions.push(action);
                }
                continue;
            }
            if diagnostic.code() != NameRule::UnknownName.code() {
                continue;
            }
            let name = source_file
                .text_at(range)
                .ok_or(SemanticCodeActionError::InvalidDiagnosticRange { source, range })?;
            for completion in self.semantic_completions(source, range.start())? {
                let Some(import) = completion.automatic_import() else {
                    continue;
                };
                if completion.label() != name {
                    continue;
                }
                let edits = completion
                    .additional_edits()
                    .iter()
                    .map(|edit| SemanticSourceEdit::new(source, edit.range(), edit.new_text()))
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                if edits.is_empty() {
                    continue;
                }
                actions.push(SemanticCodeAction {
                    title: format!("Import `{name}` from `{import}`").into(),
                    diagnostic_code: diagnostic.code().into(),
                    diagnostic_range: range,
                    edits,
                });
            }
        }
        actions.sort_by(|left, right| left.title.cmp(&right.title));
        actions.dedup();
        Ok(actions.into_boxed_slice())
    }

    fn missing_conformance_methods(&self) -> Option<&nocter_checking::MissingConformanceMethods> {
        let CompileSessionError::Preparation(PreparationError::Conformance(error)) =
            self.compilation_failure()?
        else {
            return None;
        };
        error.missing_methods()
    }
}

const fn ranges_intersect(left: TextRange, right: TextRange) -> bool {
    left.start().get() <= right.end().get() && right.start().get() <= left.end().get()
}
