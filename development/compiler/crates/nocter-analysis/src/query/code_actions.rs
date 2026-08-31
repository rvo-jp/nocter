use std::fmt;

use nocter_diagnostics::DiagnosticRepair;
use nocter_source::{SourceId, TextRange};

use crate::{AnalysisSnapshot, SemanticSourceEdit};

use super::completion::SemanticCompletionError;

mod interface_implementation;
mod outcomes;

pub use interface_implementation::InterfaceImplementationActionError;
pub use outcomes::OutcomeActionError;

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

    /// Binds this compiler-selected repair to one immutable source generation.
    ///
    /// # Errors
    ///
    /// Returns an exact source-edit construction failure.
    pub fn candidate<'source>(
        &self,
        source: &'source AnalysisSnapshot,
    ) -> Result<crate::SemanticMutationCandidate<'source>, crate::SemanticMutationBuildError> {
        crate::SemanticMutationCandidate::checked(source, self.edits.iter().cloned())
    }
}

/// An inconsistency while planning a source repair from retained compiler state.
#[derive(Debug)]
pub enum SemanticCodeActionError {
    Completion(SemanticCompletionError),
    InterfaceImplementation(InterfaceImplementationActionError),
    Outcome(OutcomeActionError),
}

impl fmt::Display for SemanticCodeActionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Completion(error) => error.fmt(formatter),
            Self::InterfaceImplementation(error) => error.fmt(formatter),
            Self::Outcome(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SemanticCodeActionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Completion(error) => Some(error),
            Self::InterfaceImplementation(error) => Some(error),
            Self::Outcome(error) => Some(error),
        }
    }
}

impl From<SemanticCompletionError> for SemanticCodeActionError {
    fn from(error: SemanticCompletionError) -> Self {
        Self::Completion(error)
    }
}

impl From<InterfaceImplementationActionError> for SemanticCodeActionError {
    fn from(error: InterfaceImplementationActionError) -> Self {
        Self::InterfaceImplementation(error)
    }
}

impl From<OutcomeActionError> for SemanticCodeActionError {
    fn from(error: OutcomeActionError) -> Self {
        Self::Outcome(error)
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
        let mut actions = Vec::new();
        for diagnostic in self.diagnostics() {
            let primary = diagnostic.primary();
            let range = primary.span().range();
            if primary.source() != source || !diagnostic_matches_request(range, requested_range) {
                continue;
            }
            let Some(repair) = diagnostic.repair() else {
                continue;
            };
            if repair == &DiagnosticRepair::ImplementMissingInterfaceMethod {
                let Some(query) = self
                    .semantic_query()
                    .map_err(InterfaceImplementationActionError::from)?
                    .and_then(crate::query::evidence::SemanticQueryContext::interface_implementation_mutation)
                else {
                    continue;
                };
                if let Some(action) = interface_implementation::missing_method_action(
                    self,
                    source,
                    diagnostic.code(),
                    range,
                    query,
                )? {
                    actions.push(action);
                }
                continue;
            }
            if repair == &DiagnosticRepair::AddCallableOutcomeContract {
                if let Some(action) =
                    outcomes::callable_contract_action(self, source, diagnostic.code(), range)?
                {
                    actions.push(action);
                }
                continue;
            }
            let DiagnosticRepair::ImportUnknownName { name } = repair else {
                continue;
            };
            for completion in self.semantic_completions(source, range.start())? {
                let Some(import) = completion.automatic_import() else {
                    continue;
                };
                if import.unresolved_name() != name.as_ref() {
                    continue;
                }
                let mut edits = completion
                    .additional_edits()
                    .iter()
                    .map(|edit| SemanticSourceEdit::new(source, edit.range(), edit.new_text()))
                    .collect::<Vec<_>>();
                if let Some(replacement) = import.replacement() {
                    edits.push(SemanticSourceEdit::new(source, range, replacement));
                }
                if edits.is_empty() {
                    continue;
                }
                actions.push(SemanticCodeAction {
                    title: if let Some(replacement) = import.replacement() {
                        format!("Use `{replacement}` from `{}`", import.route()).into()
                    } else {
                        format!("Import `{name}` from `{}`", import.route()).into()
                    },
                    diagnostic_code: diagnostic.code().into(),
                    diagnostic_range: range,
                    edits: edits.into_boxed_slice(),
                });
            }
        }
        actions.sort_by(|left, right| left.title.cmp(&right.title));
        actions.dedup();
        Ok(actions.into_boxed_slice())
    }
}

const fn diagnostic_matches_request(diagnostic: TextRange, requested: TextRange) -> bool {
    if requested.is_empty() {
        return if diagnostic.is_empty() {
            diagnostic.start().get() == requested.start().get()
        } else {
            diagnostic.contains_offset(requested.start())
        };
    }
    if diagnostic.is_empty() {
        return requested.contains_offset(diagnostic.start());
    }
    diagnostic.overlaps(requested)
}

#[cfg(test)]
mod tests {
    use nocter_source::{ByteOffset, TextRange};

    use super::diagnostic_matches_request;

    const fn offset(value: u32) -> ByteOffset {
        ByteOffset::new(value)
    }

    #[test]
    fn code_action_ranges_distinguish_overlap_adjacency_and_cursor_queries() {
        let diagnostic = TextRange::new(offset(10), offset(20));
        assert!(diagnostic_matches_request(
            diagnostic,
            TextRange::new(offset(15), offset(25))
        ));
        assert!(!diagnostic_matches_request(
            diagnostic,
            TextRange::new(offset(0), offset(10))
        ));
        assert!(!diagnostic_matches_request(
            diagnostic,
            TextRange::new(offset(20), offset(25))
        ));
        assert!(diagnostic_matches_request(
            diagnostic,
            TextRange::empty(offset(10))
        ));
        assert!(!diagnostic_matches_request(
            diagnostic,
            TextRange::empty(offset(20))
        ));

        let empty_diagnostic = TextRange::empty(offset(10));
        assert!(diagnostic_matches_request(
            empty_diagnostic,
            TextRange::new(offset(10), offset(11))
        ));
        assert!(diagnostic_matches_request(
            empty_diagnostic,
            TextRange::empty(offset(10))
        ));
    }
}
