//! Separation of semantic return values from ABI return storage.
//!
//! Authored lowering initially targets the distinguished return local so all
//! source forms share one destination. After cleanup elaboration, a return
//! definition whose successful path crosses a cleanup call is rewritten to
//! an ordinary owned local and an explicit `ReturnValue` operand. Backend
//! projection then writes caller-clobbered return registers only after every
//! cleanup edge has completed.

use super::locals::{LocalOrigin, LocalStorage, ValueRepresentation};
use super::{
    BasicBlockId, Body, CallContinuation, DropPlan, DropPlanId, LocalId, Operand, Place, Statement,
    Terminator,
};
use std::collections::{HashSet, VecDeque};

pub(super) fn materialize(body: &mut Body) {
    let return_local = body.return_local;
    let Some(return_contract) = body.locals.get(return_local.index()).cloned() else {
        return;
    };
    if return_contract.representation == ValueRepresentation::Unit {
        return;
    }
    if !return_definition_precedes_cleanup_call(body) {
        return;
    }

    let staging = LocalId::from_index(body.locals.len());
    let mut staging_contract = return_contract;
    staging_contract.storage = LocalStorage::Local;
    staging_contract.origin = LocalOrigin::Desugared(body.source_span);
    let move_result = staging_contract.ownership == super::OwnershipKind::Move;
    body.locals.push(staging_contract);

    rewrite_return_definitions(body, return_local, staging);
    for block in &mut body.blocks {
        if block.terminator == Terminator::Return {
            block.terminator = Terminator::ReturnValue {
                source: if move_result {
                    Operand::Move(Place::local(staging))
                } else {
                    Operand::Copy(Place::local(staging))
                },
            };
        }
    }
}

fn return_definition_precedes_cleanup_call(body: &Body) -> bool {
    let predecessors = predecessors(body);
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    for (index, block) in body.blocks.iter().enumerate() {
        if block.terminator == Terminator::Return {
            queue.push_back((BasicBlockId::from_index(index), false));
        }
    }

    while let Some((block, cleanup_seen)) = queue.pop_front() {
        if !visited.insert((block, cleanup_seen)) {
            continue;
        }
        let Some(record) = body.blocks.get(block.index()) else {
            continue;
        };
        let cleanup_seen = cleanup_seen
            || record.statements.iter().any(|statement| {
                matches!(
                    statement,
                    Statement::ExitRegion { .. } | Statement::ExitAllocationContext { .. }
                )
            })
            || matches!(
                record.terminator,
                Terminator::Drop { plan, .. } if drop_plan_may_call(body, plan)
            );
        if cleanup_seen && block_defines_local(record, body.return_local) {
            return true;
        }
        for predecessor in predecessors.get(block.index()).into_iter().flatten() {
            queue.push_back((*predecessor, cleanup_seen));
        }
    }
    false
}

fn predecessors(body: &Body) -> Vec<Vec<BasicBlockId>> {
    let mut result = vec![Vec::new(); body.blocks.len()];
    for (index, block) in body.blocks.iter().enumerate() {
        let source = BasicBlockId::from_index(index);
        for target in successors(&block.terminator) {
            if let Some(predecessors) = result.get_mut(target.index()) {
                predecessors.push(source);
            }
        }
    }
    result
}

fn successors(terminator: &Terminator) -> Vec<BasicBlockId> {
    match terminator {
        Terminator::Goto { target } | Terminator::Drop { target, .. } => vec![*target],
        Terminator::Switch {
            then_target,
            else_target,
            ..
        } => vec![*then_target, *else_target],
        Terminator::Call { continuation, .. } => match continuation {
            CallContinuation::Continue { target } | CallContinuation::Return { target, .. } => {
                vec![*target]
            }
            CallContinuation::Outcome {
                success, failure, ..
            }
            | CallContinuation::OutcomeEffect {
                success, failure, ..
            } => vec![*success, *failure],
            CallContinuation::Never => Vec::new(),
        },
        Terminator::InspectOutcome {
            success, failure, ..
        } => vec![*success, *failure],
        Terminator::Trap
        | Terminator::PropagateFailure
        | Terminator::ReturnOutcome { .. }
        | Terminator::ReturnFailure { .. }
        | Terminator::ReturnOutcomeSuccess { .. }
        | Terminator::ReturnOptionalNone
        | Terminator::ReturnValue { .. }
        | Terminator::Return => Vec::new(),
    }
}

fn block_defines_local(block: &super::model::BasicBlock, local: LocalId) -> bool {
    block.statements.iter().any(|statement| {
        matches!(
            statement,
            Statement::BeginAggregate { destination, .. }
                | Statement::FinishAggregate { destination, .. }
                | Statement::Assign { destination, .. }
                if destination.local == local
        )
    }) || match &block.terminator {
        Terminator::Call { continuation, .. } => matches!(
            continuation,
            CallContinuation::Return { destination, .. }
                | CallContinuation::Outcome { destination, .. }
                if destination.local == local
        ),
        Terminator::InspectOutcome { destination, .. } => destination.local == local,
        _ => false,
    }
}

fn drop_plan_may_call(body: &Body, plan: DropPlanId) -> bool {
    match body.drop_plans.get(plan.index()) {
        Some(DropPlan::Noop) | None => false,
        Some(DropPlan::Direct { .. }) => true,
        Some(DropPlan::Struct { destructor, fields }) => {
            destructor.is_some()
                || fields
                    .iter()
                    .any(|field| drop_plan_may_call(body, field.plan))
        }
        Some(DropPlan::Array { element, .. }) => drop_plan_may_call(body, *element),
        Some(DropPlan::Enum { variants }) => variants.iter().any(|variant| {
            variant
                .fields
                .iter()
                .any(|field| drop_plan_may_call(body, field.plan))
        }),
        Some(DropPlan::Outcome { payload, .. }) => drop_plan_may_call(body, *payload),
    }
}

fn rewrite_return_definitions(body: &mut Body, return_local: LocalId, staging: LocalId) {
    for projection in &mut body.projections {
        if projection.base == return_local {
            projection.base = staging;
        }
    }
    for block in &mut body.blocks {
        for statement in &mut block.statements {
            match statement {
                Statement::BeginAggregate { destination, .. }
                | Statement::FinishAggregate { destination, .. }
                | Statement::Assign { destination, .. }
                    if destination.local == return_local =>
                {
                    destination.local = staging;
                }
                _ => {}
            }
        }
        match &mut block.terminator {
            Terminator::Call { continuation, .. } => match continuation {
                CallContinuation::Return { destination, .. }
                | CallContinuation::Outcome { destination, .. }
                    if destination.local == return_local =>
                {
                    destination.local = staging;
                }
                _ => {}
            },
            Terminator::InspectOutcome { destination, .. } if destination.local == return_local => {
                destination.local = staging;
            }
            _ => {}
        }
    }
}
