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
        let mut by_name: HashMap<String, Option<CallTarget>> = HashMap::new();
        for (name, target) in targets {
            by_name
                .entry(name)
                .and_modify(|existing| {
                    if existing.as_ref() != Some(&target) {
                        *existing = None;
                    }
                })
                .or_insert(Some(target));
        }
        Self { by_name }
    }

    pub(super) fn get(&self, name: &str) -> Option<&CallTarget> {
        self.by_name.get(name)?.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceId;

    #[test]
    fn repeated_aliases_to_one_target_remain_unique() {
        let target = CallTarget::imported(SourceId::new(7), "File.read".to_string());
        let targets = UniqueCallTargets::new(vec![
            ("std/io.File.read".to_string(), target.clone()),
            ("std/io.File.read".to_string(), target.clone()),
        ]);

        assert_eq!(targets.get("std/io.File.read"), Some(&target));
    }

    #[test]
    fn one_alias_to_distinct_targets_is_ambiguous() {
        let targets = UniqueCallTargets::new(vec![
            (
                "File.read".to_string(),
                CallTarget::imported(SourceId::new(7), "File.read".to_string()),
            ),
            (
                "File.read".to_string(),
                CallTarget::imported(SourceId::new(8), "File.read".to_string()),
            ),
        ]);

        assert_eq!(targets.get("File.read"), None);
    }
}
