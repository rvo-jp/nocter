//! Backend-only symbol presentation for semantically identified coercion instances.

use crate::ast::canonical_type_expr;
use crate::typecheck::TypecheckCoercionPlan;

pub(crate) fn coercion_symbol_name(plan: &TypecheckCoercionPlan) -> String {
    debug_assert!(plan.def_id.is_some());
    format!(
        "{}.__nocter$coerce${}",
        canonical_type_expr(&plan.self_ty),
        plan.focus_span.start
    )
}
