use crate::abi::{AbiValue, abi_value_from_type_expr_with_resolver};
use crate::ast::{TypeExpr, substitute_type_expr_parameters};

use super::LoweringContext;

impl LoweringContext<'_> {
    pub(in crate::ir::lower) fn specialize_type_expr(&self, ty: &TypeExpr) -> TypeExpr {
        substitute_type_expr_parameters(ty, &self.generic_substitutions)
    }

    /// Resolves an AST type in the active generic specialization and source graph.
    ///
    /// Lowering code must not ask the root resolver to lay out an unsubstituted
    /// generic type. Keeping that rule here makes aggregate initialization,
    /// primitive layout queries, and future collection lowering agree.
    pub(in crate::ir::lower) fn abi_value_for_type_expr(&self, ty: &TypeExpr) -> Option<AbiValue> {
        let (_root_source, resolved) = self.resolved_calls()?;
        let ty = self.specialize_type_expr(ty);
        abi_value_from_type_expr_with_resolver(&ty, resolved, |source| self.resolved_source(source))
            .ok()
    }

    pub(in crate::ir::lower) fn expression_contains_borrow(
        &self,
        expression_span: crate::source::ByteSpan,
    ) -> bool {
        self.expression_type_expr(expression_span)
            .as_ref()
            .and_then(|ty| self.abi_value_for_type_expr(ty))
            .is_some_and(|value| value.ty.contains_borrow())
    }

    pub(in crate::ir::lower) fn ir_type_for_type_expr(
        &self,
        ty: &TypeExpr,
    ) -> Option<crate::ir::Type> {
        let (_root_source, resolved) = self.resolved_calls()?;
        let ty = self.specialize_type_expr(ty);
        crate::ir::lower::types::return_type_from_type_expr_with_resolver(&ty, resolved, |source| {
            self.resolved_source(source)
        })
    }
}
