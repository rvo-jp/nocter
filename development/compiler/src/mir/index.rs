//! Retained MIR products shared by buildability and machine-IR lowering.

use super::{Body, BuildError};
use crate::semantic::BodyId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type CachedBody = Option<Result<Body, BuildError>>;

#[derive(Debug, Clone, Default)]
pub(crate) struct BodyCache {
    entries: Arc<Mutex<HashMap<BodyId, CachedBody>>>,
}

impl BodyCache {
    pub(crate) fn get_or_build(
        &self,
        id: BodyId,
        build: impl FnOnce() -> CachedBody,
    ) -> CachedBody {
        let mut entries = self
            .entries
            .lock()
            .expect("MIR body cache lock must not be poisoned");
        if let Some(cached) = entries.get(&id) {
            return cached.clone();
        }
        let body = build();
        entries.insert(id, body.clone());
        body
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries
            .lock()
            .expect("MIR body cache lock must not be poisoned")
            .len()
    }
}
