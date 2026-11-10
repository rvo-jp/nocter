//! Shared exact-source guard for semantic products derived from sibling checked bodies.
use super::Failure;
use nocter_declaration_lowering::access_declarations::Context;
use nocter_model::AccessCallableId;
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Debug, Default)]
pub(super) struct SourceDependencies {
    bodies: BTreeMap<AccessCallableId, Arc<[u8]>>,
}
impl SourceDependencies {
    pub(super) fn insert(&mut self, id: AccessCallableId, bytes: Arc<[u8]>) {
        self.bodies.insert(id, bytes);
    }
    pub(super) fn verify(&self, context: &Context<'_>) -> Result<(), Failure> {
        let syntax = context.syntax();
        for (&id, bytes) in &self.bodies {
            let body = context
                .callable_source(id)
                .ok_or(Failure::ChangedBody)?
                .body();
            let range = syntax
                .tree()
                .node(body)
                .ok_or(Failure::InconsistentSyntax)?
                .range();
            if syntax
                .text_at(range)
                .ok_or(Failure::InconsistentSyntax)?
                .as_bytes()
                != bytes.as_ref()
            {
                return Err(Failure::ChangedBody);
            }
        }
        Ok(())
    }
}
