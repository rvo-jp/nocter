//! Typed identities for checked syntax sites that are not expressions or statements.

use crate::semantic::SemanticSiteId;
use crate::source::ByteSpan;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub(super) struct SemanticSiteArena {
    spans: Vec<ByteSpan>,
    ids: HashMap<ByteSpan, SemanticSiteId>,
}

impl SemanticSiteArena {
    pub(super) fn intern(&mut self, span: ByteSpan) -> SemanticSiteId {
        if let Some(id) = self.ids.get(&span) {
            return *id;
        }
        let id = SemanticSiteId::from_index(self.spans.len());
        self.spans.push(span);
        self.ids.insert(span, id);
        id
    }

    pub(super) fn id(&self, span: ByteSpan) -> Option<SemanticSiteId> {
        self.ids.get(&span).copied()
    }

    pub(super) fn span(&self, id: SemanticSiteId) -> Option<ByteSpan> {
        self.spans.get(id.index()).copied()
    }
}
