use super::*;

impl<'a> LoweringContext<'a> {
    pub(in crate::ir::lower) fn drop_glue_for_type_expr(&self, ty: &TypeExpr) -> Option<DropGlue> {
        let (root_source, resolved) = self.resolved_calls()?;
        let ty = substitute_type_expr_parameters(ty, &self.generic_substitutions);
        drop_glue_for_type_expr_with_resolver(&ty, root_source, resolved, |source| {
            self.resolved_source(source)
        })
    }

    pub(in crate::ir::lower) fn aggregate_drop_for_type_expr(
        &self,
        ty: &TypeExpr,
    ) -> Option<AggregateDrop> {
        let (root_source, resolved) = self.resolved_calls()?;
        let ty = substitute_type_expr_parameters(ty, &self.generic_substitutions);
        aggregate_drop_for_type_expr_with_resolver(&ty, root_source, resolved, |source| {
            self.resolved_source(source)
        })
    }
}
