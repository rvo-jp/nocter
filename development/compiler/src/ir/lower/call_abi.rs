//! Machine-IR call instructions and ABI diagnostics projected from checked MIR.

use crate::abi::{ReturnPassing, ValueLayout};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateLocation, CallTarget, Instruction, OutcomeFailureMode, ScalarArgument, Type,
};

pub(super) fn aggregate_call_instruction(
    return_type: &Type,
    destination: AggregateLocation,
    target: CallTarget,
    arguments: Vec<ScalarArgument>,
    layout: ValueLayout,
) -> Instruction {
    match return_type {
        Type::Aggregate { .. } => Instruction::CallAggregate {
            destination,
            target,
            arguments,
        },
        Type::DirectAggregate { .. } => Instruction::CallDirectAggregate {
            destination,
            target,
            arguments,
            layout,
        },
        _ => unreachable!("aggregate call instruction requires aggregate return type"),
    }
}

pub(super) fn fallible_aggregate_call_instruction(
    success_type: &Type,
    destination: AggregateLocation,
    target: CallTarget,
    arguments: Vec<ScalarArgument>,
    layout: ValueLayout,
    failure_mode: OutcomeFailureMode,
) -> Instruction {
    match success_type {
        Type::Aggregate { .. } => Instruction::CallOutcomeAggregate {
            destination,
            target,
            arguments,
            failure_mode,
        },
        Type::DirectAggregate { .. } => Instruction::CallOutcomeDirectAggregate {
            destination,
            target,
            arguments,
            layout,
            failure_mode,
        },
        _ => unreachable!("fallible aggregate call instruction requires aggregate success type"),
    }
}

pub(super) fn validate_success_return_passing(
    actual: Option<ReturnPassing>,
    callee_name: &str,
    expected_success_type: &Type,
) -> Result<(), Vec<Diagnostic>> {
    let Some(actual) = actual else {
        return Ok(());
    };
    let Some(expected) = expected_success_type.success_return_passing() else {
        return Ok(());
    };
    if actual == expected {
        return Ok(());
    }
    Err(vec![Diagnostic::error(
        "E8006",
        format!(
            "native lowering call return ABI mismatch for function `{callee_name}`: expected callee success return to use `{}`, got `{}`",
            expected.description(),
            actual.description(),
        ),
    )])
}

pub(super) fn describe_type(ty: &Type) -> &'static str {
    match ty {
        Type::I32 => "i32",
        Type::U8 => "u8",
        Type::Usize => "usize",
        Type::Integer(_) => "integer",
        Type::Bool => "bool",
        Type::Str => "&str",
        Type::Slice {
            is_readwrite: false,
        } => "&[T]",
        Type::Slice { is_readwrite: true } => "&+[T]",
        Type::Aggregate { .. } | Type::DirectAggregate { .. } => "aggregate",
        Type::Error => "error",
        Type::Borrow {
            is_readwrite,
            inner,
        } => describe_borrow(*is_readwrite, inner),
        Type::Void => "void",
        Type::Never => "never",
        Type::Optional(payload) => describe_outcome(payload, "optional"),
        Type::Fallible(payload) => describe_outcome(payload, "fallible"),
        Type::ComposedOutcome { .. } => "composed outcome",
    }
}

fn describe_borrow(is_readwrite: bool, inner: &Type) -> &'static str {
    match (is_readwrite, inner) {
        (false, Type::I32) => "&i32",
        (false, Type::U8) => "&u8",
        (false, Type::Usize) => "&usize",
        (false, Type::Integer(_)) => "&integer",
        (false, Type::Bool) => "&bool",
        (false, Type::Aggregate { .. } | Type::DirectAggregate { .. }) => "&aggregate",
        (true, Type::I32) => "&+i32",
        (true, Type::U8) => "&+u8",
        (true, Type::Usize) => "&+usize",
        (true, Type::Integer(_)) => "&+integer",
        (true, Type::Bool) => "&+bool",
        (true, Type::Aggregate { .. } | Type::DirectAggregate { .. }) => "&+aggregate",
        _ => "borrow",
    }
}

fn describe_outcome(payload: &Type, kind: &'static str) -> &'static str {
    match payload {
        Type::I32 => {
            if kind == "optional" {
                "i32?"
            } else {
                "i32!"
            }
        }
        Type::U8 => {
            if kind == "optional" {
                "u8?"
            } else {
                "u8!"
            }
        }
        Type::Usize => {
            if kind == "optional" {
                "usize?"
            } else {
                "usize!"
            }
        }
        Type::Integer(_) => {
            if kind == "optional" {
                "integer?"
            } else {
                "integer!"
            }
        }
        Type::Bool => {
            if kind == "optional" {
                "bool?"
            } else {
                "bool!"
            }
        }
        Type::Str => {
            if kind == "optional" {
                "&str?"
            } else {
                "&str!"
            }
        }
        Type::Slice {
            is_readwrite: false,
        } => {
            if kind == "optional" {
                "&[T]?"
            } else {
                "&[T]!"
            }
        }
        Type::Slice { is_readwrite: true } => {
            if kind == "optional" {
                "&+[T]?"
            } else {
                "&+[T]!"
            }
        }
        Type::Aggregate { .. } | Type::DirectAggregate { .. } => {
            if kind == "optional" {
                "aggregate?"
            } else {
                "aggregate!"
            }
        }
        Type::Borrow { .. } => {
            if kind == "optional" {
                "borrow?"
            } else {
                "borrow!"
            }
        }
        Type::Void => {
            if kind == "optional" {
                "void?"
            } else {
                "void!"
            }
        }
        Type::Never => {
            if kind == "optional" {
                "never?"
            } else {
                "never!"
            }
        }
        Type::Error => {
            if kind == "optional" {
                "error?"
            } else {
                "error!"
            }
        }
        Type::Optional(_) | Type::Fallible(_) | Type::ComposedOutcome { .. } => "composed outcome",
    }
}
