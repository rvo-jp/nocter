use super::super::documents::{OpenDocument, WorkspaceRoot};
use super::build::build_snapshot;
use super::invalidation::SnapshotChange;
use super::model::LspSnapshot;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default)]
pub(in crate::driver::lsp) struct SnapshotStore {
    generation: Cell<u64>,
    current: RefCell<Option<Arc<LspSnapshot>>>,
}

impl SnapshotStore {
    pub(in crate::driver::lsp) fn current(
        &self,
        documents: &HashMap<String, OpenDocument>,
        workspace_roots: &[WorkspaceRoot],
    ) -> Arc<LspSnapshot> {
        if let Some(snapshot) = self.current.borrow().as_ref()
            && snapshot.matches_inputs(documents, workspace_roots)
        {
            return Arc::clone(snapshot);
        }
        self.rebuild(documents, workspace_roots, SnapshotChange::Full)
    }

    pub(in crate::driver::lsp) fn rebuild(
        &self,
        documents: &HashMap<String, OpenDocument>,
        workspace_roots: &[WorkspaceRoot],
        change: SnapshotChange,
    ) -> Arc<LspSnapshot> {
        let generation = self.generation.get().saturating_add(1);
        let previous = self.current.borrow().as_ref().cloned();
        let snapshot = Arc::new(build_snapshot(
            generation,
            documents,
            workspace_roots,
            previous.as_deref(),
            &change,
        ));
        self.generation.set(generation);
        self.current.replace(Some(Arc::clone(&snapshot)));
        snapshot
    }
}
