use std::collections::HashMap;
use std::hash::Hash;

use nocter_source_index::SyntaxOrigin;

use super::{TypeBindingError, TypeBindingRule};

/// Source-aware uniqueness authority for one authored declaration-header collection.
///
/// The caller supplies the semantic key and the exact authored name. This component owns the
/// first-occurrence policy so requirement families do not independently choose whether a repeated
/// identity is a source rule or a malformed declaration graph.
pub(super) struct AuthoredUniqueness<K> {
    origins: HashMap<K, SyntaxOrigin>,
}

impl<K> Default for AuthoredUniqueness<K> {
    fn default() -> Self {
        Self {
            origins: HashMap::new(),
        }
    }
}

impl<K: Eq + Hash> AuthoredUniqueness<K> {
    pub(super) fn record(
        &mut self,
        key: K,
        origin: SyntaxOrigin,
        rule: TypeBindingRule,
    ) -> Result<(), TypeBindingError> {
        if let Some(first) = self.origins.insert(key, origin) {
            return Err(TypeBindingError::duplicate_rule(rule, first, origin));
        }
        Ok(())
    }
}
