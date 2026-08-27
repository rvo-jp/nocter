use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use nocter_declarations::DeclarationGraph;
use nocter_model::ModuleId;
use nocter_source::SourceId;
use nocter_source_index::SourceIndex;

use crate::EvidenceIntegrityError;
use crate::query::presentation::visible_spelling::VisibleSpellings;
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum SpellingContext {
    Module(ModuleId),
    Source { module: ModuleId, source: SourceId },
}

/// Mutable, generation-local query accelerators kept outside immutable compiler products.
#[derive(Debug, Default)]
pub(crate) struct AnalysisQuerySession {
    checked_members: nocter_checking::MemberCompletionQuerySession,
    interrupted_members: Mutex<HashMap<usize, Arc<nocter_checking::MemberCompletionQuerySession>>>,
    spellings: Mutex<HashMap<SpellingContext, Arc<VisibleSpellings>>>,
    semantic_integrity: OnceLock<Result<(), EvidenceIntegrityError>>,
}

impl AnalysisQuerySession {
    pub(super) const fn checked_members(&self) -> &nocter_checking::MemberCompletionQuerySession {
        &self.checked_members
    }

    pub(super) fn interrupted_members(
        &self,
        index: usize,
    ) -> Arc<nocter_checking::MemberCompletionQuerySession> {
        let mut sessions = self
            .interrupted_members
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sessions
            .entry(index)
            .or_insert_with(|| Arc::new(nocter_checking::MemberCompletionQuerySession::default()))
            .clone()
    }

    pub(super) fn validate_semantics(
        &self,
        validate: impl FnOnce() -> Result<(), EvidenceIntegrityError>,
    ) -> Result<(), EvidenceIntegrityError> {
        *self.semantic_integrity.get_or_init(validate)
    }

    pub(super) fn module_spellings(
        &self,
        graph: &DeclarationGraph,
        module: ModuleId,
    ) -> Arc<VisibleSpellings> {
        self.spellings(SpellingContext::Module(module), || {
            VisibleSpellings::new(graph, module)
        })
    }

    pub(super) fn source_spellings(
        &self,
        graph: &DeclarationGraph,
        module: ModuleId,
        index: &SourceIndex,
        source: SourceId,
    ) -> Arc<VisibleSpellings> {
        self.spellings(SpellingContext::Source { module, source }, || {
            VisibleSpellings::for_source(graph, module, index, source)
        })
    }

    fn spellings(
        &self,
        context: SpellingContext,
        build: impl FnOnce() -> VisibleSpellings,
    ) -> Arc<VisibleSpellings> {
        let mut spellings = self
            .spellings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        spellings
            .entry(context)
            .or_insert_with(|| Arc::new(build()))
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::AnalysisQuerySession;

    #[test]
    fn semantic_integrity_is_sealed_once_per_generation() {
        let session = AnalysisQuerySession::default();
        let validations = AtomicUsize::new(0);
        for _ in 0..3 {
            session
                .validate_semantics(|| {
                    validations.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
                .unwrap();
        }
        assert_eq!(validations.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn interrupted_query_caches_are_keyed_lazily() {
        let session = AnalysisQuerySession::default();
        let first = session.interrupted_members(41);
        let same = session.interrupted_members(41);
        let distinct = session.interrupted_members(99);

        assert!(std::sync::Arc::ptr_eq(&first, &same));
        assert!(!std::sync::Arc::ptr_eq(&first, &distinct));
    }
}
