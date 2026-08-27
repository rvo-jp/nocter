use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use nocter_declarations::DeclarationGraph;
use nocter_model::ModuleId;
use nocter_source::SourceId;
use nocter_source_index::SourceIndex;

use crate::presentation::visible_spelling::VisibleSpellings;
use crate::{AnalysisState, CurrentAnalysis, CurrentSemanticAuthority};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum SpellingContext {
    Module(ModuleId),
    Source { module: ModuleId, source: SourceId },
}

/// Mutable, generation-local query accelerators kept outside immutable compiler products.
#[derive(Debug, Default)]
pub(super) struct AnalysisQuerySession {
    pub(super) checked_members: nocter_checking::MemberCompletionQuerySession,
    interrupted_members: Box<[nocter_checking::MemberCompletionQuerySession]>,
    spellings: Mutex<HashMap<SpellingContext, Arc<VisibleSpellings>>>,
}

impl AnalysisQuerySession {
    pub(super) fn for_state(state: &AnalysisState) -> Self {
        let interruption_count = match state {
            AnalysisState::Current(CurrentAnalysis {
                authority: CurrentSemanticAuthority::Semantic(semantic),
                ..
            }) => semantic
                .bodies()
                .map_or(0, |recovery| recovery.interruptions().count()),
            AnalysisState::DiscoveryFailed(_)
            | AnalysisState::Current(CurrentAnalysis {
                authority: CurrentSemanticAuthority::None | CurrentSemanticAuthority::Target(_),
                ..
            }) => 0,
        };
        Self {
            checked_members: nocter_checking::MemberCompletionQuerySession::default(),
            interrupted_members: std::iter::repeat_with(
                nocter_checking::MemberCompletionQuerySession::default,
            )
            .take(interruption_count)
            .collect(),
            spellings: Mutex::default(),
        }
    }

    pub(super) fn interrupted_members(
        &self,
        index: usize,
    ) -> Option<&nocter_checking::MemberCompletionQuerySession> {
        self.interrupted_members.get(index)
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
