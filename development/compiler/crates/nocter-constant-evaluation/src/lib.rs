//! Shared planning and evaluation of typed Nocter constant expressions.
//!
//! Name and type lookup remain owned by the caller's semantic phase. This crate receives those
//! decisions through [`ConstantResolver`], freezes one typed plan, and is the sole implementation
//! of constant arithmetic, short-circuiting, conversions, and dependency-cycle detection.

mod evaluate;
mod floating;
mod model;
mod plan;
mod support;
#[cfg(test)]
mod tests;

use nocter_language::DiagnosticCode;

pub use evaluate::{
    ConstantEvaluationError, ConstantEvaluationRule, evaluate_constant_plans,
    evaluate_expression_plan, evaluate_frozen_expression_plan,
};
pub use floating::{
    FloatBinaryOperation, FloatBits, FloatComparisonOperation, FloatFormat, FloatLiteralError,
    TargetFloatEvaluator,
};
pub use model::{
    ConstantExpressionPlan, ConstantPlanError, ConstantPlanRule, ConstantReference,
    ConstantResolver, ConstantScalarType, FrozenExpressionPlan, FrozenType,
};
pub use plan::{plan_expression, plan_frozen_expression};

/// Public constant-expression diagnostic family shared by header and body semantic adapters.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ConstantExpressionRule {
    NonConstantExpression,
    TypeMismatch,
    DependencyCycle,
    ArithmeticFailure,
}

impl ConstantExpressionRule {
    #[must_use]
    pub const fn code(self) -> DiagnosticCode {
        match self {
            Self::NonConstantExpression => DiagnosticCode::E0322,
            Self::TypeMismatch => DiagnosticCode::E0323,
            Self::DependencyCycle => DiagnosticCode::E0324,
            Self::ArithmeticFailure => DiagnosticCode::E0325,
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::NonConstantExpression => {
                "compile-time expression contains an unavailable operation"
            }
            Self::TypeMismatch => "compile-time expression does not produce its required type",
            Self::DependencyCycle => "compile-time dependency graph contains a cycle",
            Self::ArithmeticFailure => "compile-time arithmetic has no valid typed value",
        }
    }

    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::NonConstantExpression => {
                "use literals, constants, grouping, built-in operators, or a permitted lossless numeric conversion"
            }
            Self::TypeMismatch => "make the expression and its required type agree",
            Self::DependencyCycle => "remove one reference in the compile-time dependency cycle",
            Self::ArithmeticFailure => {
                "use representable literals and avoid invalid integer arithmetic or shifts"
            }
        }
    }
}
