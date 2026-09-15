use nocter_model::{CompilationTarget, ConstantId, ConstantValue, FrozenValue};
use nocter_syntax::Punctuation;
use nocter_syntax::SyntaxOrigin;

use crate::ConstantExpressionRule;
use crate::model::{
    ConstantExpressionPlan, ConstantOperation, ConstantScalarType, FrozenExpressionPlan, PlanNode,
    PlanNodeId,
};
use crate::scalar::{self, ScalarEvaluationFailure};
use crate::support::integer_spec;
use crate::{
    CompileTimeBinaryOperation, CompileTimeComparisonOperation, CompileTimeUnaryOperation,
    FloatBits, TargetFloatEvaluator,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstantEvaluationRule {
    ArithmeticFailure,
    MissingConstant,
    InvalidPlan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConstantEvaluationError {
    rule: ConstantEvaluationRule,
    origin: SyntaxOrigin,
}

impl ConstantEvaluationError {
    #[must_use]
    pub const fn expression_rule(self) -> Option<ConstantExpressionRule> {
        match self.rule {
            ConstantEvaluationRule::ArithmeticFailure => {
                Some(ConstantExpressionRule::ArithmeticFailure)
            }
            ConstantEvaluationRule::MissingConstant | ConstantEvaluationRule::InvalidPlan => None,
        }
    }

    #[must_use]
    pub const fn rule(self) -> ConstantEvaluationRule {
        self.rule
    }

    #[must_use]
    pub const fn origin(self) -> SyntaxOrigin {
        self.origin
    }
}

#[derive(Clone, Debug)]
struct TypedValue {
    value: ConstantValue,
}

struct Evaluator<'a, L> {
    plan: &'a ConstantExpressionPlan,
    lookup: L,
}

/// Evaluates one already typed plan using frozen constant values supplied by the caller.
///
/// # Errors
///
/// Returns the exact arithmetic or plan-integrity failure selected while evaluating the plan.
pub fn evaluate_expression_plan(
    plan: &ConstantExpressionPlan,
    mut lookup: impl FnMut(ConstantId) -> Option<ConstantValue>,
) -> Result<ConstantValue, ConstantEvaluationError> {
    let mut evaluator = Evaluator {
        plan,
        lookup: |id, _| Ok(lookup(id)),
    };
    evaluator.evaluate(plan.root).map(|value| value.value)
}

/// Evaluates one closed scalar-or-array plan without recovering aggregate structure from syntax.
///
/// # Errors
///
/// Returns the first scalar leaf evaluation failure.
pub fn evaluate_frozen_expression_plan(
    plan: &FrozenExpressionPlan,
    lookup: &mut impl FnMut(ConstantId) -> Option<ConstantValue>,
) -> Result<FrozenValue, ConstantEvaluationError> {
    match plan {
        FrozenExpressionPlan::Scalar(plan) => {
            evaluate_expression_plan(plan, lookup).map(FrozenValue::Scalar)
        }
        FrozenExpressionPlan::Tuple { elements, .. } => elements
            .iter()
            .map(|element| evaluate_frozen_expression_plan(element, lookup))
            .collect::<Result<Vec<_>, _>>()
            .map(|values| FrozenValue::Tuple(values.into_boxed_slice())),
        FrozenExpressionPlan::FixedArray { elements, .. } => elements
            .iter()
            .map(|element| evaluate_frozen_expression_plan(element, lookup))
            .collect::<Result<Vec<_>, _>>()
            .map(|values| FrozenValue::FixedArray(values.into_boxed_slice())),
    }
}

impl<L> Evaluator<'_, L>
where
    L: FnMut(ConstantId, SyntaxOrigin) -> Result<Option<ConstantValue>, ConstantEvaluationError>,
{
    fn evaluate(&mut self, node: PlanNodeId) -> Result<TypedValue, ConstantEvaluationError> {
        let plan = self.plan;
        let entry = plan.nodes[node.0].clone();
        match entry.operation {
            ConstantOperation::Value(value) => Ok(TypedValue { value }),
            ConstantOperation::FloatLiteral(ref spelling) => {
                float_literal(&entry, self.plan.target, spelling)
            }
            ConstantOperation::IntegerLiteral(value) => integer_literal(&entry, value),
            ConstantOperation::Reference(id) => {
                let value = (self.lookup)(id, entry.origin)?.ok_or(ConstantEvaluationError {
                    rule: ConstantEvaluationRule::MissingConstant,
                    origin: entry.origin,
                })?;
                Ok(TypedValue { value })
            }
            ConstantOperation::Unary { operator, operand } => {
                let operand_value = if operator == Punctuation::Minus
                    && matches!(
                        plan.nodes[operand.0].operation,
                        ConstantOperation::IntegerLiteral(_)
                    ) {
                    None
                } else {
                    Some(self.evaluate(operand)?)
                };
                evaluate_unary(&entry, operator, operand, operand_value, plan)
            }
            ConstantOperation::Binary {
                operator,
                left,
                right,
            } => self.evaluate_binary(&entry, operator, left, right),
            ConstantOperation::Conversion { operand } => {
                let value = self.evaluate(operand)?;
                evaluate_conversion(&entry, &value, self.plan.target)
            }
        }
    }

    fn evaluate_binary(
        &mut self,
        entry: &PlanNode,
        operator: Punctuation,
        left: PlanNodeId,
        right: PlanNodeId,
    ) -> Result<TypedValue, ConstantEvaluationError> {
        let left_value = self.evaluate(left)?;
        if operator == Punctuation::LogicalAnd || operator == Punctuation::LogicalOr {
            let ConstantValue::Bool(left_bool) = left_value.value else {
                return Err(invalid(entry.origin));
            };
            let result = if operator == Punctuation::LogicalAnd {
                left_bool && bool_value(&self.evaluate(right)?, entry.origin)?
            } else {
                left_bool || bool_value(&self.evaluate(right)?, entry.origin)?
            };
            return Ok(TypedValue {
                value: ConstantValue::Bool(result),
            });
        }
        let right_value = self.evaluate(right)?;
        evaluate_binary_values(entry, operator, &left_value, &right_value, self.plan.target)
    }
}

fn float_literal(
    entry: &PlanNode,
    target: CompilationTarget,
    spelling: &str,
) -> Result<TypedValue, ConstantEvaluationError> {
    let ConstantScalarType::Float(format) = entry.ty else {
        return Err(invalid(entry.origin));
    };
    let value = TargetFloatEvaluator::new(target)
        .decimal_bits(spelling, format)
        .map_err(|_| arithmetic(entry.origin))?;
    Ok(TypedValue {
        value: float_value(value),
    })
}

fn integer_literal(entry: &PlanNode, value: u64) -> Result<TypedValue, ConstantEvaluationError> {
    let ConstantScalarType::Integer(builtin) = entry.ty else {
        return Err(invalid(entry.origin));
    };
    let value = i128::from(value);
    if !integer_spec(builtin).is_some_and(|spec| spec.contains(value)) {
        return Err(arithmetic(entry.origin));
    }
    Ok(TypedValue {
        value: ConstantValue::Integer(value),
    })
}

fn evaluate_unary(
    entry: &PlanNode,
    operator: Punctuation,
    operand: PlanNodeId,
    operand_value: Option<TypedValue>,
    plan: &ConstantExpressionPlan,
) -> Result<TypedValue, ConstantEvaluationError> {
    if operator == Punctuation::Minus
        && let ConstantScalarType::Integer(builtin) = entry.ty
        && let ConstantOperation::IntegerLiteral(magnitude) = plan.nodes[operand.0].operation
    {
        let spec = integer_spec(builtin)
            .filter(|spec| spec.signed)
            .ok_or_else(|| invalid(entry.origin))?;
        if i128::from(magnitude) > spec.maximum + 1 {
            return Err(arithmetic(entry.origin));
        }
        return Ok(TypedValue {
            value: ConstantValue::Integer(-i128::from(magnitude)),
        });
    }
    let operation = match operator {
        Punctuation::Bang => CompileTimeUnaryOperation::LogicalNot,
        Punctuation::Minus => CompileTimeUnaryOperation::Negate,
        _ => return Err(invalid(entry.origin)),
    };
    let operand = operand_value.ok_or_else(|| invalid(entry.origin))?;
    scalar::unary(operation, entry.ty, &operand.value, plan.target)
        .map(|value| TypedValue { value })
        .map_err(|failure| scalar_error(failure, entry.origin))
}

fn evaluate_binary_values(
    entry: &PlanNode,
    operator: Punctuation,
    left: &TypedValue,
    right: &TypedValue,
    target: CompilationTarget,
) -> Result<TypedValue, ConstantEvaluationError> {
    let comparison = match operator {
        Punctuation::EqualEqual => Some(CompileTimeComparisonOperation::Equal),
        Punctuation::BangEqual => Some(CompileTimeComparisonOperation::NotEqual),
        Punctuation::Less => Some(CompileTimeComparisonOperation::Less),
        Punctuation::LessEqual => Some(CompileTimeComparisonOperation::LessEqual),
        Punctuation::Greater => Some(CompileTimeComparisonOperation::Greater),
        Punctuation::GreaterEqual => Some(CompileTimeComparisonOperation::GreaterEqual),
        _ => None,
    };
    let result = if let Some(operation) = comparison {
        scalar::compare(operation, &left.value, &right.value, target)
    } else {
        let operation = match operator {
            Punctuation::Plus => CompileTimeBinaryOperation::Add,
            Punctuation::Minus => CompileTimeBinaryOperation::Subtract,
            Punctuation::Star => CompileTimeBinaryOperation::Multiply,
            Punctuation::Slash => CompileTimeBinaryOperation::Divide,
            Punctuation::Percent => CompileTimeBinaryOperation::Remainder,
            Punctuation::ShiftLeft => CompileTimeBinaryOperation::ShiftLeft,
            Punctuation::ShiftRight => match entry.ty {
                ConstantScalarType::Integer(builtin)
                    if integer_spec(builtin).is_some_and(|spec| spec.signed) =>
                {
                    CompileTimeBinaryOperation::ShiftRightSigned
                }
                _ => CompileTimeBinaryOperation::ShiftRightUnsigned,
            },
            _ => return Err(invalid(entry.origin)),
        };
        scalar::binary(operation, entry.ty, &left.value, &right.value, target)
    };
    result
        .map(|value| TypedValue { value })
        .map_err(|failure| scalar_error(failure, entry.origin))
}

fn evaluate_conversion(
    entry: &PlanNode,
    operand: &TypedValue,
    target: CompilationTarget,
) -> Result<TypedValue, ConstantEvaluationError> {
    scalar::convert(entry.ty, &operand.value, target)
        .map(|value| TypedValue { value })
        .map_err(|failure| scalar_error(failure, entry.origin))
}

const fn scalar_error(
    failure: ScalarEvaluationFailure,
    origin: SyntaxOrigin,
) -> ConstantEvaluationError {
    match failure {
        ScalarEvaluationFailure::Arithmetic => arithmetic(origin),
        ScalarEvaluationFailure::InvalidType => invalid(origin),
    }
}

const fn float_value(value: FloatBits) -> ConstantValue {
    match value {
        FloatBits::Binary32(bits) => ConstantValue::Float32(bits),
        FloatBits::Binary64(bits) => ConstantValue::Float64(bits),
    }
}

fn bool_value(value: &TypedValue, origin: SyntaxOrigin) -> Result<bool, ConstantEvaluationError> {
    let ConstantValue::Bool(value) = &value.value else {
        return Err(invalid(origin));
    };
    Ok(*value)
}

const fn arithmetic(origin: SyntaxOrigin) -> ConstantEvaluationError {
    ConstantEvaluationError {
        rule: ConstantEvaluationRule::ArithmeticFailure,
        origin,
    }
}

const fn invalid(origin: SyntaxOrigin) -> ConstantEvaluationError {
    ConstantEvaluationError {
        rule: ConstantEvaluationRule::InvalidPlan,
        origin,
    }
}
