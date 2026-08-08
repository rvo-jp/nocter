use crate::abi::{AbiValue, abi_value_from_type_expr_with_resolver};
use crate::ast::{TypeExpr, substitute_type_expr_parameters};
use crate::typecheck::TypecheckSliceElementKind;

use super::LoweringContext;

impl LoweringContext<'_> {
    pub(in crate::ir::lower) fn expression_ir_type(
        &self,
        expression: &crate::ast::Expr,
    ) -> Option<crate::ir::Type> {
        let ty = self.expression_type_expr(expression.span())?;
        self.ir_type_for_type_expr(&ty)
    }

    /// Returns the scalar lowering kind of a slice expression from the typecheck
    /// fact for the complete expression. This deliberately does not inspect AST
    /// shapes: conversions, calls, fields, and future expression forms must all
    /// agree on the type selected by typechecking.
    pub(in crate::ir::lower) fn expression_slice_element_kind(
        &self,
        expression: &crate::ast::Expr,
    ) -> Option<TypecheckSliceElementKind> {
        let ty = self.expression_type_expr(expression.span())?;
        let (_root_source, resolved) = self.resolved_calls()?;
        let element = crate::ir::lower::types::view_element_type_from_type_expr_with_resolver(
            &ty,
            resolved,
            |source| self.resolved_source(source),
        )?;
        Some(match element {
            crate::ir::Type::I32 => TypecheckSliceElementKind::I32,
            crate::ir::Type::U8 => TypecheckSliceElementKind::U8,
            crate::ir::Type::Usize => TypecheckSliceElementKind::Usize,
            crate::ir::Type::Integer(kind) => TypecheckSliceElementKind::Integer(kind),
            crate::ir::Type::Bool => TypecheckSliceElementKind::Bool,
            crate::ir::Type::Str => TypecheckSliceElementKind::Str,
            _ => TypecheckSliceElementKind::Other,
        })
    }

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
