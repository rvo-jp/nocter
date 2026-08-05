//! Cached total lengths for compiler-owned literal element packs.
//!
//! Exact-size protocol calls run once at hidden literal entry, after every
//! segment has been prepared and before the literal body can consume a pack.

use super::collection_for::{receiver_expression, receiver_parameter_type, synthetic_call};
use super::context::{LiteralPackLoweringSegment, LoweringContext};
use super::expressions::{TemporaryAllocator, lower_call_arguments_with_explicit_types};
use crate::diagnostics::Diagnostic;
use crate::ir::{Instruction, UsizeValue};

pub(super) fn runtime_length_name(capture_name: &str) -> String {
    format!("<literal-pack-length:{capture_name}>")
}

pub(super) fn lower_runtime_length_initialization(
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(pack) = context.active_literal_pack().cloned() else {
        return Ok(Vec::new());
    };
    let Some(length_name) = pack.runtime_length_name else {
        return Ok(Vec::new());
    };
    let destination = context.next_usize_local_location()?;
    context.define_usize_local(length_name);
    let fixed_count = pack
        .segments
        .iter()
        .filter(|segment| matches!(segment, LiteralPackLoweringSegment::Value { .. }))
        .count() as u64;
    let mut value = UsizeValue::Const(fixed_count);
    let mut instructions = Vec::new();
    let mut temporaries = TemporaryAllocator::new(context)?;

    for segment in &pack.segments {
        let LiteralPackLoweringSegment::Spread {
            iterator_parameter_name,
            plan,
        } = segment
        else {
            continue;
        };
        let target = context
            .iteration_method_target(&plan.exact_size)
            .ok_or_else(|| length_diagnostic("the exact-size target is unavailable"))?;
        let receiver =
            receiver_expression(&plan.exact_size, iterator_parameter_name, plan.spread_span);
        let call = synthetic_call(
            plan.spread_span,
            &plan.exact_size.target_name,
            vec![receiver],
        );
        let parameter_types = vec![receiver_parameter_type(&plan.exact_size, plan.spread_span)];
        let (argument_instructions, arguments) = lower_call_arguments_with_explicit_types(
            &call,
            &target,
            &plan.exact_size.target_name,
            context,
            &mut temporaries,
            Some(&parameter_types),
        )?;
        instructions.extend(argument_instructions);
        let segment_length = temporaries.next_usize()?;
        instructions.push(Instruction::CallUsize {
            destination: segment_length,
            target,
            arguments,
        });
        let sum = temporaries.next_usize()?;
        instructions.push(Instruction::AddUsize {
            destination: sum,
            left: value,
            right: UsizeValue::Location(segment_length),
        });
        value = UsizeValue::Location(sum);
    }
    instructions.push(Instruction::SetUsize { destination, value });
    Ok(instructions)
}

fn length_diagnostic(detail: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8014",
        format!("literal pack length initialization failed: {detail}"),
    )]
}
