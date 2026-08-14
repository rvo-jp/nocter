//! Retained MIR products shared by buildability and machine-IR lowering.

use super::{Body, BuildError};
use crate::semantic::BodyId;
use crate::source::SourceId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type CachedBody = Option<Result<Body, BuildError>>;

#[derive(Debug, Clone, Default)]
pub(crate) struct BodyCache {
    entries: Arc<Mutex<HashMap<BodyInstanceKey, CachedBody>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BodyInstanceKey {
    source: SourceId,
    body: BodyId,
    substitutions: Vec<(String, String)>,
}

impl BodyInstanceKey {
    fn new(
        source: SourceId,
        body: BodyId,
        substitutions: &HashMap<String, crate::ast::TypeExpr>,
    ) -> Self {
        let mut substitutions = substitutions
            .iter()
            .map(|(parameter, ty)| (parameter.clone(), crate::ast::canonical_type_expr(ty)))
            .collect::<Vec<_>>();
        substitutions.sort_unstable();
        Self {
            source,
            body,
            substitutions,
        }
    }
}

impl BodyCache {
    pub(crate) fn get_or_build_specialized(
        &self,
        source: SourceId,
        id: BodyId,
        substitutions: &HashMap<String, crate::ast::TypeExpr>,
        build: impl FnOnce() -> CachedBody,
    ) -> CachedBody {
        let mut entries = self
            .entries
            .lock()
            .expect("MIR body cache lock must not be poisoned");
        let key = BodyInstanceKey::new(source, id, substitutions);
        if let Some(cached) = entries.get(&key) {
            return cached.clone();
        }
        let body = build();
        entries.insert(key, body.clone());
        body
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries
            .lock()
            .expect("MIR body cache lock must not be poisoned")
            .len()
    }

    #[cfg(test)]
    pub(crate) fn cached_specialized(
        &self,
        source: SourceId,
        id: BodyId,
        substitutions: &HashMap<String, crate::ast::TypeExpr>,
    ) -> Option<Result<Body, BuildError>> {
        self.entries
            .lock()
            .expect("MIR body cache lock must not be poisoned")
            .get(&BodyInstanceKey::new(source, id, substitutions))
            .cloned()
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{TypeExpr, TypeReference};
    use crate::source::{ByteSpan, SourceId};

    fn named_type(name: &str) -> TypeExpr {
        TypeExpr::Reference(TypeReference {
            span: ByteSpan::new(SourceId::new(0), 0, 1),
            name: name.to_string(),
        })
    }

    #[test]
    fn body_instance_key_is_order_independent_and_type_sensitive() {
        let body = BodyId::from_index(7);
        let source = SourceId::new(2);
        let first = HashMap::from([
            ("T".to_string(), named_type("i32")),
            ("U".to_string(), named_type("bool")),
        ]);
        let reordered = HashMap::from([
            ("U".to_string(), named_type("bool")),
            ("T".to_string(), named_type("i32")),
        ]);
        let different = HashMap::from([
            ("T".to_string(), named_type("usize")),
            ("U".to_string(), named_type("bool")),
        ]);

        assert_eq!(
            BodyInstanceKey::new(source, body, &first),
            BodyInstanceKey::new(source, body, &reordered)
        );
        assert_ne!(
            BodyInstanceKey::new(source, body, &first),
            BodyInstanceKey::new(source, body, &different)
        );
        assert_ne!(
            BodyInstanceKey::new(SourceId::new(3), body, &first),
            BodyInstanceKey::new(source, body, &first)
        );
    }
}
