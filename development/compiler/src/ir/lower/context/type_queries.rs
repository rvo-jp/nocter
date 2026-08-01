use crate::abi::{AbiValue, abi_value_from_type_expr_with_resolver};
use crate::ast::{TypeExpr, substitute_type_expr_parameters};

use super::LoweringContext;

impl LoweringContext<'_> {
    /// Resolves an AST type in the active generic specialization and source graph.
    ///
    /// Lowering code must not ask the root resolver to lay out an unsubstituted
    /// generic type. Keeping that rule here makes aggregate initialization,
    /// primitive layout queries, and future collection lowering agree.
    pub(in crate::ir::lower) fn abi_value_for_type_expr(&self, ty: &TypeExpr) -> Option<AbiValue> {
        let (_root_source, resolved) = self.resolved_calls()?;
        let ty = substitute_type_expr_parameters(ty, &self.generic_substitutions);
        abi_value_from_type_expr_with_resolver(&ty, resolved, |source| self.resolved_source(source))
            .ok()
    }
}
