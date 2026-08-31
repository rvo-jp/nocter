//! Shared planning and evaluation of typed Nocter constant expressions.
//!
//! Name and type lookup remain owned by the caller's semantic phase. This crate receives those
//! decisions through [`ConstantResolver`], freezes one typed plan, and is the sole implementation
//! of constant arithmetic, short-circuiting, conversions, and dependency-cycle detection.

mod evaluate;
mod model;
mod plan;
mod support;
#[cfg(test)]
mod tests;

use nocter_language::DiagnosticCode;

pub use evaluate::{
    ConstantEvaluationError, ConstantEvaluationRule, evaluate_constant_plans,
    evaluate_expression_plan,
};
pub use model::{
    ConstantExpressionPlan, ConstantPlanError, ConstantPlanRule, ConstantReference,
    ConstantResolver, ConstantScalarType,
};
pub use plan::plan_expression;

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
                "constant expression contains an operation unavailable at compile time"
            }
            Self::TypeMismatch => "constant expression does not produce its required type",
            Self::DependencyCycle => "constant dependency graph contains a cycle",
            Self::ArithmeticFailure => "constant arithmetic has no valid typed value",
        }
    }

    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::NonConstantExpression => {
                "use literals, constants, grouping, built-in operators, or a representable integer conversion"
            }
            Self::TypeMismatch => "make the expression and its required type agree",
            Self::DependencyCycle => "remove one reference in the constant dependency cycle",
            Self::ArithmeticFailure => {
                "change the expression so it cannot overflow, divide by zero, or use an invalid shift"
            }
        }
    }
}
