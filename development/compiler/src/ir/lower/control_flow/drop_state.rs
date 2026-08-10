use super::*;

pub(super) fn promote_if_aggregate_state(
    statement: &IfStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    promote_matching_aggregate_state(context, |name, context| {
        expression_contains_explicit_aggregate_move_matching(
            &statement.condition,
            context,
            &|moved, _| moved == name,
        ) || block_changes_aggregate_state(&statement.then_block, name, context)
            || statement
                .else_block
                .as_ref()
                .is_some_and(|block| block_changes_aggregate_state(block, name, context))
    })
}

pub(super) fn promote_switch_aggregate_state(
    statement: &SwitchStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    promote_matching_aggregate_state(context, |name, context| {
        expression_contains_explicit_aggregate_move_matching(
            &statement.expression,
            context,
            &|moved, _| moved == name,
        ) || statement
            .arms
            .iter()
            .any(|arm| block_changes_aggregate_state(&arm.body, name, context))
            || statement
                .wildcard_arm
                .as_ref()
                .is_some_and(|arm| block_changes_aggregate_state(&arm.body, name, context))
    })
}

pub(super) fn promote_while_aggregate_state(
    statement: &WhileStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    promote_matching_aggregate_state(context, |name, context| {
        expression_contains_explicit_aggregate_move_matching(
            &statement.condition,
            context,
            &|moved, _| moved == name,
        ) || block_changes_aggregate_state(&statement.body, name, context)
    })
}

pub(super) fn promote_loop_aggregate_state(
    statement: &LoopStmt,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    promote_matching_aggregate_state(context, |name, context| {
        block_changes_aggregate_state(&statement.body, name, context)
    })
}

fn promote_matching_aggregate_state(
    context: &mut LoweringContext,
    changes_state: impl Fn(&str, &LoweringContext) -> bool,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let names = context.aggregate_runtime_drop_candidate_names();
    let mut instructions = Vec::new();
    for name in names {
        if changes_state(&name, context)
            && let Some(instruction) = context.promote_aggregate_runtime_live(&name)?
        {
            instructions.push(instruction);
        }
    }
    Ok(instructions)
}

fn block_changes_aggregate_state(block: &Block, name: &str, context: &LoweringContext) -> bool {
    for (index, statement) in block.statements.iter().enumerate() {
        if statement_directly_consumes_aggregate(statement, name, context)
            && !statement_suffix_exits_function(
                &block.statements,
                index,
                block.result.as_deref(),
                context,
            )
        {
            return true;
        }
        if nested_control_changes_aggregate_state(statement, name, context) {
            return true;
        }
    }
    block.result.as_ref().is_some_and(|result| {
        expression_contains_explicit_aggregate_move_matching(result, context, &|moved, _| {
            moved == name
        })
    })
}

fn statement_directly_consumes_aggregate(
    statement: &Stmt,
    name: &str,
    context: &LoweringContext,
) -> bool {
    match statement {
        Stmt::Drop(statement) => statement.name == name,
        Stmt::Binding(statement) => expression_contains_explicit_aggregate_move_matching(
            &statement.initializer,
            context,
            &|moved, _| moved == name,
        ),
        Stmt::Assignment(statement) => expression_contains_explicit_aggregate_move_matching(
            &statement.value,
            context,
            &|moved, _| moved == name,
        ),
        Stmt::Expression(statement) => expression_contains_explicit_aggregate_move_matching(
            &statement.expression,
            context,
            &|moved, _| moved == name,
        ),
        Stmt::Return(_)
        | Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::If(_)
        | Stmt::IfIs(_)
        | Stmt::Switch(_)
        | Stmt::ForRange(_)
        | Stmt::CollectionFor(_)
        | Stmt::LiteralPackFor(_)
        | Stmt::While(_)
        | Stmt::Loop(_)
        | Stmt::Region(_)
        | Stmt::Break(_)
        | Stmt::Continue(_) => false,
    }
}

fn nested_control_changes_aggregate_state(
    statement: &Stmt,
    name: &str,
    context: &LoweringContext,
) -> bool {
    match statement {
        Stmt::If(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.condition,
                context,
                &|moved, _| moved == name,
            ) || block_changes_aggregate_state(&statement.then_block, name, context)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(|block| block_changes_aggregate_state(block, name, context))
        }
        Stmt::IfIs(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.expression,
                context,
                &|moved, _| moved == name,
            ) || block_changes_aggregate_state(&statement.then_block, name, context)
                || statement
                    .else_block
                    .as_ref()
                    .is_some_and(|block| block_changes_aggregate_state(block, name, context))
        }
        Stmt::Switch(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.expression,
                context,
                &|moved, _| moved == name,
            ) || statement
                .arms
                .iter()
                .any(|arm| block_changes_aggregate_state(&arm.body, name, context))
                || statement
                    .wildcard_arm
                    .as_ref()
                    .is_some_and(|arm| block_changes_aggregate_state(&arm.body, name, context))
        }
        Stmt::ForRange(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.start,
                context,
                &|moved, _| moved == name,
            ) || expression_contains_explicit_aggregate_move_matching(
                &statement.end,
                context,
                &|moved, _| moved == name,
            ) || block_changes_aggregate_state(&statement.body, name, context)
        }
        Stmt::CollectionFor(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.source,
                context,
                &|moved, _| moved == name,
            ) || block_changes_aggregate_state(&statement.body, name, context)
        }
        Stmt::LiteralPackFor(statement) => {
            block_changes_aggregate_state(&statement.body, name, context)
        }
        Stmt::While(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.condition,
                context,
                &|moved, _| moved == name,
            ) || block_changes_aggregate_state(&statement.body, name, context)
        }
        Stmt::Loop(statement) => block_changes_aggregate_state(&statement.body, name, context),
        Stmt::Region(statement) => {
            expression_contains_explicit_aggregate_move_matching(
                &statement.allocator,
                context,
                &|moved, _| moved == name,
            ) || block_changes_aggregate_state(&statement.body, name, context)
        }
        Stmt::Import(_)
        | Stmt::FromImport(_)
        | Stmt::Return(_)
        | Stmt::Binding(_)
        | Stmt::Assignment(_)
        | Stmt::Expression(_)
        | Stmt::Drop(_)
        | Stmt::Break(_)
        | Stmt::Continue(_) => false,
    }
}

pub(super) fn record_assignment_runtime_initialization(
    target: &Expr,
    instructions: &mut Vec<Instruction>,
    context: &LoweringContext,
) {
    let name = match target {
        Expr::Identifier(identifier) => Some(identifier.name.as_str()),
        Expr::Group(group) => {
            return record_assignment_runtime_initialization(
                &group.expression,
                instructions,
                context,
            );
        }
        _ => None,
    };
    if let Some(name) = name
        && let Some(transition) = context.aggregate_runtime_live_transition(name, true)
        && instructions.last() != Some(&transition)
    {
        instructions.push(transition);
    }
}

pub(super) fn record_runtime_aggregate_transitions(
    instructions: &mut Vec<Instruction>,
    context: &LoweringContext,
) {
    let original = std::mem::take(instructions);
    let mut rewritten = Vec::with_capacity(original.len());
    let mut iterator = original.into_iter().peekable();
    while let Some(mut instruction) = iterator.next() {
        record_nested_runtime_aggregate_transitions(&mut instruction, context);

        let moved_by_call = aggregate_call_argument_slots(&instruction)
            .into_iter()
            .filter_map(|slot| context.aggregate_runtime_live_by_slot(slot))
            .fold(Vec::new(), |mut locations, location| {
                if !locations.contains(&location) {
                    locations.push(location);
                }
                locations
            });
        for destination in moved_by_call {
            let transition = Instruction::SetBool {
                destination,
                value: BoolValue::Const(false),
            };
            if rewritten.last() != Some(&transition) {
                rewritten.push(transition);
            }
        }

        let copy_transitions = match &instruction {
            Instruction::CopyAggregate {
                destination,
                source,
                ..
            } => aggregate_copy_transitions(*destination, *source, context),
            _ => Vec::new(),
        };
        rewritten.push(instruction);
        for transition in copy_transitions {
            if iterator.peek() != Some(&transition) {
                rewritten.push(transition);
            }
        }
    }
    *instructions = rewritten;
}

fn record_nested_runtime_aggregate_transitions(
    instruction: &mut Instruction,
    context: &LoweringContext,
) {
    match instruction {
        Instruction::If {
            then_instructions,
            else_instructions,
            ..
        } => {
            record_runtime_aggregate_transitions(then_instructions, context);
            record_runtime_aggregate_transitions(else_instructions, context);
        }
        Instruction::While {
            condition_instructions,
            body_instructions,
            ..
        } => {
            record_runtime_aggregate_transitions(condition_instructions, context);
            record_runtime_aggregate_transitions(body_instructions, context);
        }
        _ => {}
    }
}

fn aggregate_call_argument_slots(instruction: &Instruction) -> HashSet<usize> {
    let arguments = match instruction {
        Instruction::CallI32 { arguments, .. }
        | Instruction::CallOutcomeI32 { arguments, .. }
        | Instruction::CallU8 { arguments, .. }
        | Instruction::CallOutcomeU8 { arguments, .. }
        | Instruction::CallUsize { arguments, .. }
        | Instruction::CallOutcomeUsize { arguments, .. }
        | Instruction::CallBool { arguments, .. }
        | Instruction::CallOutcomeBool { arguments, .. }
        | Instruction::CallStr { arguments, .. }
        | Instruction::CallOutcomeStr { arguments, .. }
        | Instruction::CallSlice { arguments, .. }
        | Instruction::CallOutcomeSlice { arguments, .. }
        | Instruction::CallVoid { arguments, .. }
        | Instruction::CallAggregate { arguments, .. }
        | Instruction::CallOutcomeAggregate { arguments, .. }
        | Instruction::CallDirectAggregate { arguments, .. }
        | Instruction::CallOutcomeDirectAggregate { arguments, .. }
        | Instruction::TailCall { arguments, .. } => arguments,
        _ => return HashSet::new(),
    };
    arguments
        .iter()
        .filter_map(|argument| match argument {
            ScalarArgument::AggregateIndirect(argument) => Some(&argument.source),
            ScalarArgument::AggregateDirect(argument) => Some(&argument.source),
            _ => None,
        })
        .map(|source| match source {
            AggregateArgumentSource::Slot(slot) => *slot,
        })
        .collect()
}

fn aggregate_copy_transitions(
    destination: AggregateLocation,
    source: AggregateLocation,
    context: &LoweringContext,
) -> Vec<Instruction> {
    let mut transitions = Vec::new();
    if let AggregateLocation::Slot(slot) = source
        && let Some(destination) = context.aggregate_runtime_live_by_slot(slot)
    {
        transitions.push(Instruction::SetBool {
            destination,
            value: BoolValue::Const(false),
        });
    }
    if let AggregateLocation::Slot(slot) = destination
        && let Some(destination) = context.aggregate_runtime_live_by_slot(slot)
    {
        transitions.push(Instruction::SetBool {
            destination,
            value: BoolValue::Const(true),
        });
    }
    transitions
}
