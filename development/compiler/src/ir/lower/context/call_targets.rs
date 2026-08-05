use crate::ir::CallTarget;
use std::collections::HashMap;

/// Resolves a specialized callable name only when the reachable-function index
/// proves that the name identifies one concrete source target.
#[derive(Debug, Clone, Default)]
pub(super) struct UniqueCallTargets {
    by_name: HashMap<String, Option<CallTarget>>,
}

impl UniqueCallTargets {
    pub(super) fn new(targets: Vec<(String, CallTarget)>) -> Self {
        let mut by_name = HashMap::new();
        for (name, target) in targets {
            by_name
                .entry(name)
                .and_modify(|existing| *existing = None)
                .or_insert(Some(target));
        }
        Self { by_name }
    }

    pub(super) fn get(&self, name: &str) -> Option<&CallTarget> {
        self.by_name.get(name)?.as_ref()
    }
}
