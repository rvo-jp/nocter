use super::body::Scope;
use super::{LocalSymbolKind, Resolver};
use crate::ast::RegionStmt;

impl Resolver<'_> {
    pub(super) fn resolve_region_statement(
        &mut self,
        statement: &RegionStmt,
        parent_scope: &mut Scope,
    ) {
        // The allocator is selected in the parent context. The new region does
        // not exist until selection has completed.
        self.resolve_expression(&statement.allocator, parent_scope);

        let mut body_scope = parent_scope.clone();
        self.define_local_name(
            statement.name.clone(),
            statement.name_span,
            LocalSymbolKind::Region,
            &mut body_scope,
        );
        self.resolve_block(&statement.body, &mut body_scope);
    }
}
