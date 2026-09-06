use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;

use nocter_model::{CompilationTarget, ConstantId, ConstantValue, FrozenValue};
use nocter_syntax::Punctuation;
use nocter_syntax::SyntaxOrigin;

use crate::ConstantExpressionRule;
use crate::model::{
    ConstantExpressionPlan, ConstantOperation, ConstantScalarType, FrozenExpressionPlan, PlanNode,
    PlanNodeId,
};
use crate::support::{integer_spec, shift};
use crate::{
    FloatBinaryOperation, FloatBits, FloatComparisonOperation, FloatFormat, TargetFloatEvaluator,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstantEvaluationRule {
    ArithmeticFailure,
    DependencyCycle,
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
            ConstantEvaluationRule::DependencyCycle => {
                Some(ConstantExpressionRule::DependencyCycle)
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
        lookup: |id| Ok(lookup(id)),
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
        FrozenExpressionPlan::FixedArray { elements, .. } => elements
            .iter()
            .map(|element| evaluate_frozen_expression_plan(element, lookup))
            .collect::<Result<Vec<_>, _>>()
            .map(|values| FrozenValue::FixedArray(values.into_boxed_slice())),
    }
}

/// Evaluates a complete constant dependency graph after rejecting every authored cycle.
///
/// # Errors
///
/// Returns a dependency-cycle, arithmetic, missing-dependency, or invalid-plan failure. The latter
/// two indicate a caller contract violation rather than an authored language error.
pub fn evaluate_constant_plans<S: BuildHasher>(
    plans: &HashMap<ConstantId, ConstantExpressionPlan, S>,
) -> Result<HashMap<ConstantId, ConstantValue>, ConstantEvaluationError> {
    let order = dependency_order(plans)?;
    let mut values = HashMap::with_capacity(plans.len());
    for id in order {
        let plan = plans.get(&id).ok_or(ConstantEvaluationError {
            rule: ConstantEvaluationRule::MissingConstant,
            origin: fallback_origin(plans),
        })?;
        let mut evaluator = Evaluator {
            plan,
            lookup: |dependency| Ok(values.get(&dependency).cloned()),
        };
        let value = evaluator.evaluate(plan.root)?.value;
        values.insert(id, value);
    }
    Ok(values)
}

impl<L> Evaluator<'_, L>
where
    L: FnMut(ConstantId) -> Result<Option<ConstantValue>, ConstantEvaluationError>,
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
                let value = (self.lookup)(id)?.ok_or(ConstantEvaluationError {
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
        evaluate_binary_values(entry, operator, left_value, right_value, self.plan.target)
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
    match operator {
        Punctuation::Bang => {
            let operand_value = operand_value.ok_or_else(|| invalid(entry.origin))?;
            let ConstantValue::Bool(value) = operand_value.value else {
                return Err(invalid(entry.origin));
            };
            Ok(TypedValue {
                value: ConstantValue::Bool(!value),
            })
        }
        Punctuation::Minus => {
            if let ConstantScalarType::Float(format) = entry.ty {
                let operand_value = operand_value.ok_or_else(|| invalid(entry.origin))?;
                let value = constant_float(&operand_value.value, format, entry.origin)?;
                return Ok(TypedValue {
                    value: float_value(TargetFloatEvaluator::new(plan.target).negate(value)),
                });
            }
            let ConstantScalarType::Integer(builtin) = entry.ty else {
                return Err(invalid(entry.origin));
            };
            let Some(spec) = integer_spec(builtin).filter(|spec| spec.signed) else {
                return Err(invalid(entry.origin));
            };
            let value = if let ConstantOperation::IntegerLiteral(magnitude) =
                plan.nodes[operand.0].operation
            {
                if i128::from(magnitude) > spec.maximum + 1 {
                    return Err(arithmetic(entry.origin));
                }
                -i128::from(magnitude)
            } else {
                let operand_value = operand_value.ok_or_else(|| invalid(entry.origin))?;
                let ConstantValue::Integer(value) = operand_value.value else {
                    return Err(invalid(entry.origin));
                };
                value
                    .checked_neg()
                    .ok_or_else(|| arithmetic(entry.origin))?
            };
            if !spec.contains(value) {
                return Err(arithmetic(entry.origin));
            }
            Ok(TypedValue {
                value: ConstantValue::Integer(value),
            })
        }
        _ => Err(invalid(entry.origin)),
    }
}

fn evaluate_binary_values(
    entry: &PlanNode,
    operator: Punctuation,
    left: TypedValue,
    right: TypedValue,
    target: CompilationTarget,
) -> Result<TypedValue, ConstantEvaluationError> {
    match operator {
        Punctuation::EqualEqual | Punctuation::BangEqual => {
            let equal = match (&left.value, &right.value) {
                (ConstantValue::Float32(left), ConstantValue::Float32(right)) => {
                    TargetFloatEvaluator::new(target)
                        .compare(
                            FloatComparisonOperation::Equal,
                            FloatBits::Binary32(*left),
                            FloatBits::Binary32(*right),
                        )
                        .ok_or_else(|| invalid(entry.origin))?
                }
                (ConstantValue::Float64(left), ConstantValue::Float64(right)) => {
                    TargetFloatEvaluator::new(target)
                        .compare(
                            FloatComparisonOperation::Equal,
                            FloatBits::Binary64(*left),
                            FloatBits::Binary64(*right),
                        )
                        .ok_or_else(|| invalid(entry.origin))?
                }
                _ => left.value == right.value,
            };
            Ok(TypedValue {
                value: ConstantValue::Bool(if operator == Punctuation::EqualEqual {
                    equal
                } else {
                    !equal
                }),
            })
        }
        Punctuation::Less
        | Punctuation::LessEqual
        | Punctuation::Greater
        | Punctuation::GreaterEqual => {
            let ordering = match (left.value, right.value) {
                (ConstantValue::Integer(left), ConstantValue::Integer(right)) => left.cmp(&right),
                (ConstantValue::Character(left), ConstantValue::Character(right)) => {
                    left.cmp(&right)
                }
                (ConstantValue::Float32(left), ConstantValue::Float32(right)) => {
                    return evaluate_float_comparison(
                        entry,
                        operator,
                        target,
                        FloatBits::Binary32(left),
                        FloatBits::Binary32(right),
                    );
                }
                (ConstantValue::Float64(left), ConstantValue::Float64(right)) => {
                    return evaluate_float_comparison(
                        entry,
                        operator,
                        target,
                        FloatBits::Binary64(left),
                        FloatBits::Binary64(right),
                    );
                }
                _ => return Err(invalid(entry.origin)),
            };
            let value = match operator {
                Punctuation::Less => ordering.is_lt(),
                Punctuation::LessEqual => !ordering.is_gt(),
                Punctuation::Greater => ordering.is_gt(),
                Punctuation::GreaterEqual => !ordering.is_lt(),
                _ => unreachable!(),
            };
            Ok(TypedValue {
                value: ConstantValue::Bool(value),
            })
        }
        _ if matches!(entry.ty, ConstantScalarType::Float(_)) => {
            evaluate_float_binary(entry, operator, &left, &right, target)
        }
        _ => evaluate_integer_binary(entry, operator, left, right),
    }
}

fn evaluate_float_comparison(
    entry: &PlanNode,
    operator: Punctuation,
    target: CompilationTarget,
    left: FloatBits,
    right: FloatBits,
) -> Result<TypedValue, ConstantEvaluationError> {
    let evaluator = TargetFloatEvaluator::new(target);
    let strict = |left, right| {
        evaluator
            .compare(FloatComparisonOperation::Less, left, right)
            .ok_or_else(|| invalid(entry.origin))
    };
    let equal = || {
        evaluator
            .compare(FloatComparisonOperation::Equal, left, right)
            .ok_or_else(|| invalid(entry.origin))
    };
    let value = match operator {
        Punctuation::Less => strict(left, right)?,
        Punctuation::LessEqual => strict(left, right)? || equal()?,
        Punctuation::Greater => strict(right, left)?,
        Punctuation::GreaterEqual => strict(right, left)? || equal()?,
        _ => return Err(invalid(entry.origin)),
    };
    Ok(TypedValue {
        value: ConstantValue::Bool(value),
    })
}

fn evaluate_float_binary(
    entry: &PlanNode,
    operator: Punctuation,
    left: &TypedValue,
    right: &TypedValue,
    target: CompilationTarget,
) -> Result<TypedValue, ConstantEvaluationError> {
    let ConstantScalarType::Float(format) = entry.ty else {
        return Err(invalid(entry.origin));
    };
    let operation = match operator {
        Punctuation::Plus => FloatBinaryOperation::Add,
        Punctuation::Minus => FloatBinaryOperation::Subtract,
        Punctuation::Star => FloatBinaryOperation::Multiply,
        Punctuation::Slash => FloatBinaryOperation::Divide,
        Punctuation::Percent => FloatBinaryOperation::Remainder,
        _ => return Err(invalid(entry.origin)),
    };
    let left = constant_float(&left.value, format, entry.origin)?;
    let right = constant_float(&right.value, format, entry.origin)?;
    let value = TargetFloatEvaluator::new(target)
        .binary(operation, left, right)
        .ok_or_else(|| invalid(entry.origin))?;
    Ok(TypedValue {
        value: float_value(value),
    })
}

fn evaluate_integer_binary(
    entry: &PlanNode,
    operator: Punctuation,
    left: TypedValue,
    right: TypedValue,
) -> Result<TypedValue, ConstantEvaluationError> {
    let ConstantScalarType::Integer(builtin) = entry.ty else {
        return Err(invalid(entry.origin));
    };
    let Some(spec) = integer_spec(builtin) else {
        return Err(invalid(entry.origin));
    };
    let (ConstantValue::Integer(left), ConstantValue::Integer(right)) = (left.value, right.value)
    else {
        return Err(invalid(entry.origin));
    };
    let result = match operator {
        Punctuation::Plus => left.checked_add(right),
        Punctuation::Minus => left.checked_sub(right),
        Punctuation::Star => left.checked_mul(right),
        Punctuation::Slash => left.checked_div(right),
        Punctuation::Percent => left.checked_rem(right),
        Punctuation::ShiftLeft | Punctuation::ShiftRight => shift(left, right, operator, spec),
        _ => None,
    }
    .filter(|value| spec.contains(*value))
    .ok_or_else(|| arithmetic(entry.origin))?;
    Ok(TypedValue {
        value: ConstantValue::Integer(result),
    })
}

fn evaluate_conversion(
    entry: &PlanNode,
    operand: &TypedValue,
    target: CompilationTarget,
) -> Result<TypedValue, ConstantEvaluationError> {
    match (entry.ty, &operand.value) {
        (ConstantScalarType::Integer(target), ConstantValue::Integer(value)) => {
            if !integer_spec(target).is_some_and(|spec| spec.contains(*value)) {
                return Err(arithmetic(entry.origin));
            }
            Ok(TypedValue {
                value: ConstantValue::Integer(*value),
            })
        }
        (ConstantScalarType::Float(format), ConstantValue::Integer(value)) => Ok(TypedValue {
            value: float_value(TargetFloatEvaluator::new(target).integer(*value, format)),
        }),
        (ConstantScalarType::Float(FloatFormat::Binary64), ConstantValue::Float32(value)) => {
            Ok(TypedValue {
                value: float_value(TargetFloatEvaluator::new(target).widen_binary32(*value)),
            })
        }
        _ => Err(invalid(entry.origin)),
    }
}

fn constant_float(
    value: &ConstantValue,
    format: FloatFormat,
    origin: SyntaxOrigin,
) -> Result<FloatBits, ConstantEvaluationError> {
    match (format, value) {
        (FloatFormat::Binary32, ConstantValue::Float32(bits)) => Ok(FloatBits::Binary32(*bits)),
        (FloatFormat::Binary64, ConstantValue::Float64(bits)) => Ok(FloatBits::Binary64(*bits)),
        _ => Err(invalid(origin)),
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

fn dependency_order<S: BuildHasher>(
    plans: &HashMap<ConstantId, ConstantExpressionPlan, S>,
) -> Result<Vec<ConstantId>, ConstantEvaluationError> {
    let mut active = HashSet::new();
    let mut complete = HashSet::new();
    let mut order = Vec::with_capacity(plans.len());
    let mut ids = plans.keys().copied().collect::<Vec<_>>();
    ids.sort_unstable();
    for id in ids {
        visit_dependencies(id, plans, &mut active, &mut complete, &mut order)?;
    }
    Ok(order)
}

fn visit_dependencies<S: BuildHasher>(
    id: ConstantId,
    plans: &HashMap<ConstantId, ConstantExpressionPlan, S>,
    active: &mut HashSet<ConstantId>,
    complete: &mut HashSet<ConstantId>,
    order: &mut Vec<ConstantId>,
) -> Result<(), ConstantEvaluationError> {
    if complete.contains(&id) {
        return Ok(());
    }
    let Some(plan) = plans.get(&id) else {
        return Err(ConstantEvaluationError {
            rule: ConstantEvaluationRule::MissingConstant,
            origin: fallback_origin(plans),
        });
    };
    active.insert(id);
    for (dependency, origin) in plan.references() {
        if !plans.contains_key(&dependency) {
            return Err(ConstantEvaluationError {
                rule: ConstantEvaluationRule::MissingConstant,
                origin,
            });
        }
        if active.contains(&dependency) {
            return Err(ConstantEvaluationError {
                rule: ConstantEvaluationRule::DependencyCycle,
                origin,
            });
        }
        visit_dependencies(dependency, plans, active, complete, order)?;
    }
    active.remove(&id);
    complete.insert(id);
    order.push(id);
    Ok(())
}

fn fallback_origin<S: BuildHasher>(
    plans: &HashMap<ConstantId, ConstantExpressionPlan, S>,
) -> SyntaxOrigin {
    plans
        .values()
        .next()
        .map(|plan| plan.nodes[plan.root.0].origin)
        .expect("constant plan set must not request an absent constant when empty")
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
