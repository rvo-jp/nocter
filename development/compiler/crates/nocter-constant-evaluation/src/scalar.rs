use nocter_model::{CompilationTarget, ConstantValue};
use nocter_syntax::Punctuation;

use crate::support::{integer_spec, shift};
use crate::{
    CompileTimeBinaryOperation, CompileTimeComparisonOperation, CompileTimeUnaryOperation,
    ConstantScalarType, FloatBinaryOperation, FloatBits, FloatComparisonOperation, FloatFormat,
    TargetFloatEvaluator,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScalarEvaluationFailure {
    Arithmetic,
    InvalidType,
}

pub(crate) fn unary(
    operation: CompileTimeUnaryOperation,
    ty: ConstantScalarType,
    operand: &ConstantValue,
    target: CompilationTarget,
) -> Result<ConstantValue, ScalarEvaluationFailure> {
    match (operation, ty, operand) {
        (
            CompileTimeUnaryOperation::LogicalNot,
            ConstantScalarType::Bool,
            ConstantValue::Bool(value),
        ) => Ok(ConstantValue::Bool(!value)),
        (CompileTimeUnaryOperation::Negate, ConstantScalarType::Float(format), value) => Ok(
            float_value(TargetFloatEvaluator::new(target).negate(constant_float(value, format)?)),
        ),
        (
            CompileTimeUnaryOperation::Negate,
            ConstantScalarType::Integer(builtin),
            ConstantValue::Integer(value),
        ) => {
            let spec = integer_spec(builtin)
                .filter(|spec| spec.signed)
                .ok_or(ScalarEvaluationFailure::InvalidType)?;
            let result = value
                .checked_neg()
                .filter(|result| spec.contains(*result))
                .ok_or(ScalarEvaluationFailure::Arithmetic)?;
            Ok(ConstantValue::Integer(result))
        }
        _ => Err(ScalarEvaluationFailure::InvalidType),
    }
}

pub(crate) fn binary(
    operation: CompileTimeBinaryOperation,
    ty: ConstantScalarType,
    left: &ConstantValue,
    right: &ConstantValue,
    target: CompilationTarget,
) -> Result<ConstantValue, ScalarEvaluationFailure> {
    match ty {
        ConstantScalarType::Float(format) => {
            let operation = match operation {
                CompileTimeBinaryOperation::Add => FloatBinaryOperation::Add,
                CompileTimeBinaryOperation::Subtract => FloatBinaryOperation::Subtract,
                CompileTimeBinaryOperation::Multiply => FloatBinaryOperation::Multiply,
                CompileTimeBinaryOperation::Divide => FloatBinaryOperation::Divide,
                CompileTimeBinaryOperation::Remainder => FloatBinaryOperation::Remainder,
                CompileTimeBinaryOperation::ShiftLeft
                | CompileTimeBinaryOperation::ShiftRightSigned
                | CompileTimeBinaryOperation::ShiftRightUnsigned => {
                    return Err(ScalarEvaluationFailure::InvalidType);
                }
            };
            TargetFloatEvaluator::new(target)
                .binary(
                    operation,
                    constant_float(left, format)?,
                    constant_float(right, format)?,
                )
                .map(float_value)
                .ok_or(ScalarEvaluationFailure::Arithmetic)
        }
        ConstantScalarType::Integer(builtin) => {
            let spec = integer_spec(builtin).ok_or(ScalarEvaluationFailure::InvalidType)?;
            let (ConstantValue::Integer(left), ConstantValue::Integer(right)) = (left, right)
            else {
                return Err(ScalarEvaluationFailure::InvalidType);
            };
            let result = match operation {
                CompileTimeBinaryOperation::Add => left.checked_add(*right),
                CompileTimeBinaryOperation::Subtract => left.checked_sub(*right),
                CompileTimeBinaryOperation::Multiply => left.checked_mul(*right),
                CompileTimeBinaryOperation::Divide => left.checked_div(*right),
                CompileTimeBinaryOperation::Remainder => left.checked_rem(*right),
                CompileTimeBinaryOperation::ShiftLeft => {
                    shift(*left, *right, Punctuation::ShiftLeft, spec)
                }
                CompileTimeBinaryOperation::ShiftRightSigned if spec.signed => {
                    shift(*left, *right, Punctuation::ShiftRight, spec)
                }
                CompileTimeBinaryOperation::ShiftRightUnsigned if !spec.signed => {
                    shift(*left, *right, Punctuation::ShiftRight, spec)
                }
                CompileTimeBinaryOperation::ShiftRightSigned
                | CompileTimeBinaryOperation::ShiftRightUnsigned => {
                    return Err(ScalarEvaluationFailure::InvalidType);
                }
            }
            .filter(|value| spec.contains(*value))
            .ok_or(ScalarEvaluationFailure::Arithmetic)?;
            Ok(ConstantValue::Integer(result))
        }
        ConstantScalarType::Bool | ConstantScalarType::Character | ConstantScalarType::Text => {
            Err(ScalarEvaluationFailure::InvalidType)
        }
    }
}

pub(crate) fn compare(
    operation: CompileTimeComparisonOperation,
    left: &ConstantValue,
    right: &ConstantValue,
    target: CompilationTarget,
) -> Result<ConstantValue, ScalarEvaluationFailure> {
    let equal = match (left, right) {
        (ConstantValue::Float32(left), ConstantValue::Float32(right)) => float_compare(
            target,
            FloatComparisonOperation::Equal,
            FloatBits::Binary32(*left),
            FloatBits::Binary32(*right),
        )?,
        (ConstantValue::Float64(left), ConstantValue::Float64(right)) => float_compare(
            target,
            FloatComparisonOperation::Equal,
            FloatBits::Binary64(*left),
            FloatBits::Binary64(*right),
        )?,
        (ConstantValue::Bool(left), ConstantValue::Bool(right)) => left == right,
        (ConstantValue::Character(left), ConstantValue::Character(right)) => left == right,
        (ConstantValue::Integer(left), ConstantValue::Integer(right)) => left == right,
        (ConstantValue::Text(left), ConstantValue::Text(right)) => left == right,
        _ => return Err(ScalarEvaluationFailure::InvalidType),
    };
    let result = match operation {
        CompileTimeComparisonOperation::Equal => equal,
        CompileTimeComparisonOperation::NotEqual => !equal,
        CompileTimeComparisonOperation::Less
        | CompileTimeComparisonOperation::LessEqual
        | CompileTimeComparisonOperation::Greater
        | CompileTimeComparisonOperation::GreaterEqual => ordering(operation, left, right, target)?,
    };
    Ok(ConstantValue::Bool(result))
}

pub(crate) fn convert(
    target_type: ConstantScalarType,
    operand: &ConstantValue,
    target: CompilationTarget,
) -> Result<ConstantValue, ScalarEvaluationFailure> {
    match (target_type, operand) {
        (ConstantScalarType::Integer(target_type), ConstantValue::Integer(value)) => {
            integer_spec(target_type)
                .filter(|spec| spec.contains(*value))
                .map(|_| ConstantValue::Integer(*value))
                .ok_or(ScalarEvaluationFailure::Arithmetic)
        }
        (ConstantScalarType::Float(format), ConstantValue::Integer(value)) => Ok(float_value(
            TargetFloatEvaluator::new(target).integer(*value, format),
        )),
        (ConstantScalarType::Float(FloatFormat::Binary64), ConstantValue::Float32(value)) => Ok(
            float_value(TargetFloatEvaluator::new(target).widen_binary32(*value)),
        ),
        _ => Err(ScalarEvaluationFailure::InvalidType),
    }
}

fn ordering(
    operation: CompileTimeComparisonOperation,
    left: &ConstantValue,
    right: &ConstantValue,
    target: CompilationTarget,
) -> Result<bool, ScalarEvaluationFailure> {
    let order = |ordering: std::cmp::Ordering| match operation {
        CompileTimeComparisonOperation::Less => ordering.is_lt(),
        CompileTimeComparisonOperation::LessEqual => !ordering.is_gt(),
        CompileTimeComparisonOperation::Greater => ordering.is_gt(),
        CompileTimeComparisonOperation::GreaterEqual => !ordering.is_lt(),
        CompileTimeComparisonOperation::Equal | CompileTimeComparisonOperation::NotEqual => false,
    };
    match (left, right) {
        (ConstantValue::Integer(left), ConstantValue::Integer(right)) => Ok(order(left.cmp(right))),
        (ConstantValue::Character(left), ConstantValue::Character(right)) => {
            Ok(order(left.cmp(right)))
        }
        (ConstantValue::Float32(left), ConstantValue::Float32(right)) => float_ordering(
            operation,
            target,
            FloatBits::Binary32(*left),
            FloatBits::Binary32(*right),
        ),
        (ConstantValue::Float64(left), ConstantValue::Float64(right)) => float_ordering(
            operation,
            target,
            FloatBits::Binary64(*left),
            FloatBits::Binary64(*right),
        ),
        _ => Err(ScalarEvaluationFailure::InvalidType),
    }
}

fn float_ordering(
    operation: CompileTimeComparisonOperation,
    target: CompilationTarget,
    left: FloatBits,
    right: FloatBits,
) -> Result<bool, ScalarEvaluationFailure> {
    let less = |left, right| float_compare(target, FloatComparisonOperation::Less, left, right);
    let equal = || float_compare(target, FloatComparisonOperation::Equal, left, right);
    match operation {
        CompileTimeComparisonOperation::Less => less(left, right),
        CompileTimeComparisonOperation::LessEqual => Ok(less(left, right)? || equal()?),
        CompileTimeComparisonOperation::Greater => less(right, left),
        CompileTimeComparisonOperation::GreaterEqual => Ok(less(right, left)? || equal()?),
        CompileTimeComparisonOperation::Equal | CompileTimeComparisonOperation::NotEqual => {
            Err(ScalarEvaluationFailure::InvalidType)
        }
    }
}

fn float_compare(
    target: CompilationTarget,
    operation: FloatComparisonOperation,
    left: FloatBits,
    right: FloatBits,
) -> Result<bool, ScalarEvaluationFailure> {
    TargetFloatEvaluator::new(target)
        .compare(operation, left, right)
        .ok_or(ScalarEvaluationFailure::InvalidType)
}

fn constant_float(
    value: &ConstantValue,
    format: FloatFormat,
) -> Result<FloatBits, ScalarEvaluationFailure> {
    match (format, value) {
        (FloatFormat::Binary32, ConstantValue::Float32(bits)) => Ok(FloatBits::Binary32(*bits)),
        (FloatFormat::Binary64, ConstantValue::Float64(bits)) => Ok(FloatBits::Binary64(*bits)),
        _ => Err(ScalarEvaluationFailure::InvalidType),
    }
}

const fn float_value(value: FloatBits) -> ConstantValue {
    match value {
        FloatBits::Binary32(bits) => ConstantValue::Float32(bits),
        FloatBits::Binary64(bits) => ConstantValue::Float64(bits),
    }
}
