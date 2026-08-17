use std::collections::{BTreeMap, BTreeSet};

use nocter_checking::{
    ConcreteDestructionPlan, DropSelection, SequenceElement, StaticSelection, TypeSubstitution,
};
use nocter_model::{BodyId, BodyNodeId, ExecutableItemId, TypeId};

use super::{
    DraftDispatchEdge, DraftDispatchPlan, DraftDispatchStep, ExecutableClosureBuilder,
    collect_drops, item_id,
};
use crate::executable::ExecutableSequenceIteration;
use crate::{
    CallableInstanceKey, ExecutableItemKey, ExecutablePackInput, ExecutableProgramError,
    ExecutableSequencePlan, ExecutableSequenceSegment, ExecutableSequenceSpread,
};

pub(super) struct DraftSequencePlan {
    source: BodyNodeId,
    constructor: ExecutableItemKey,
    input: ExecutablePackInput,
    result: TypeId,
    segments: Vec<ExecutableSequenceSegment>,
    allocation: nocter_checking::AllocationSelection,
}

struct SegmentSpecialization<'a> {
    owner: BodyNodeId,
    input: ExecutablePackInput,
    substitution: &'a TypeSubstitution,
    dispatches: &'a [DraftDispatchEdge],
    node_types: &'a BTreeMap<BodyNodeId, TypeId>,
}

impl DraftSequencePlan {
    pub(super) fn freeze(
        self,
        item_ids: &BTreeMap<ExecutableItemKey, ExecutableItemId>,
    ) -> Result<ExecutableSequencePlan, ExecutableProgramError> {
        Ok(ExecutableSequencePlan::new(
            self.source,
            item_id(item_ids, &self.constructor)?,
            self.input,
            self.result,
            self.segments,
            self.allocation,
        ))
    }
}

impl ExecutableClosureBuilder<'_> {
    pub(super) fn specialize_sequence_plans(
        &mut self,
        body: BodyId,
        dependencies: &crate::CheckedBodyDependencies,
        substitution: &TypeSubstitution,
        dispatches: &[DraftDispatchEdge],
        drops: &mut BTreeMap<DropSelection, ExecutableItemKey>,
    ) -> Result<Vec<DraftSequencePlan>, ExecutableProgramError> {
        let (node_types, sequences) = {
            let checked = self
                .target
                .checked()
                .bodies()
                .get(body)
                .ok_or(ExecutableProgramError::UnknownBody(body))?;
            let node_types = dependencies
                .nodes()
                .iter()
                .copied()
                .map(|node| {
                    checked
                        .nodes()
                        .get(node)
                        .map(|checked| (node, checked.ty()))
                        .ok_or(ExecutableProgramError::MissingRoot(node))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            let sequences = dependencies
                .nodes()
                .iter()
                .copied()
                .filter_map(|source| {
                    let node = checked.nodes().get(source)?;
                    let nocter_checking::CheckedOperation::Sequence(sequence) = node.operation()
                    else {
                        return None;
                    };
                    Some((source, node.ty(), sequence.clone()))
                })
                .collect::<Vec<_>>();
            (node_types, sequences)
        };
        let mut plans = Vec::new();
        for (source, source_type, sequence) in sequences {
            let constructor = direct_callable_key(dispatches, sequence.constructor())
                .ok_or(ExecutableProgramError::InvalidSequencePlan(source))?;
            let signature = super::callable_signature(
                self.target,
                &mut self.resolver,
                &constructor,
                &constructor.substitution(),
            )?;
            let Some(input) = signature.pack() else {
                return Err(ExecutableProgramError::InvalidSequencePlan(source));
            };
            let result = self.resolver.specialize_type(source_type, substitution)?;
            if !signature.inputs().is_empty() || signature.result() != result {
                return Err(ExecutableProgramError::InvalidSequencePlan(source));
            }
            let segments = sequence
                .elements()
                .iter()
                .map(|element| {
                    let context = SegmentSpecialization {
                        owner: source,
                        input,
                        substitution,
                        dispatches,
                        node_types: &node_types,
                    };
                    self.specialize_sequence_segment(element, &context, drops)
                })
                .collect::<Result<Vec<_>, _>>()?;
            plans.push(DraftSequencePlan {
                source,
                constructor: ExecutableItemKey::Callable(constructor),
                input,
                result,
                segments,
                allocation: sequence.allocation(),
            });
        }
        Ok(plans)
    }

    fn specialize_sequence_segment(
        &mut self,
        element: &SequenceElement,
        context: &SegmentSpecialization<'_>,
        drops: &mut BTreeMap<DropSelection, ExecutableItemKey>,
    ) -> Result<ExecutableSequenceSegment, ExecutableProgramError> {
        match element {
            SequenceElement::Value(source) => {
                let source_type = context
                    .node_types
                    .get(source)
                    .copied()
                    .ok_or(ExecutableProgramError::InvalidSequencePlan(context.owner))?;
                let ty = self
                    .resolver
                    .specialize_type(source_type, context.substitution)?;
                if ty != context.input.element() {
                    return Err(ExecutableProgramError::InvalidSequencePlan(context.owner));
                }
                let destruction = self
                    .resolver
                    .resolve_destruction(source_type, context.substitution)?;
                self.record_sequence_destruction(destruction.as_ref(), drops)?;
                Ok(ExecutableSequenceSegment::Value {
                    source: *source,
                    ty,
                    destruction,
                })
            }
            SequenceElement::Spread {
                mode,
                iteration,
                exact_size,
            } => {
                if !is_invocation_dispatch(context.dispatches, iteration.next())
                    || !is_invocation_dispatch(context.dispatches, exact_size)
                {
                    return Err(ExecutableProgramError::InvalidSequencePlan(context.owner));
                }
                let iterator_type = context
                    .node_types
                    .get(&iteration.iterator())
                    .copied()
                    .ok_or(ExecutableProgramError::InvalidSequencePlan(context.owner))?;
                let destruction = self
                    .resolver
                    .resolve_destruction(iterator_type, context.substitution)?;
                let iterator_type = self
                    .resolver
                    .specialize_type(iterator_type, context.substitution)?;
                let item = self
                    .resolver
                    .specialize_type(iteration.item(), context.substitution)?;
                let contribution = mode
                    .contribution_type(self.target.checked().types(), iteration.item())
                    .ok_or(ExecutableProgramError::InvalidSequencePlan(context.owner))?;
                let contribution = self
                    .resolver
                    .specialize_type(contribution, context.substitution)?;
                if contribution != context.input.element() {
                    return Err(ExecutableProgramError::InvalidSequencePlan(context.owner));
                }
                self.record_sequence_destruction(destruction.as_ref(), drops)?;
                Ok(ExecutableSequenceSegment::Spread(
                    ExecutableSequenceSpread::new(
                        *mode,
                        ExecutableSequenceIteration::new(
                            iteration.iterator(),
                            iterator_type,
                            item,
                            iteration.next().clone(),
                            exact_size.clone(),
                        ),
                        contribution,
                        destruction,
                    ),
                ))
            }
        }
    }

    fn record_sequence_destruction(
        &mut self,
        plan: Option<&ConcreteDestructionPlan>,
        drops: &mut BTreeMap<DropSelection, ExecutableItemKey>,
    ) -> Result<(), ExecutableProgramError> {
        let Some(plan) = plan else {
            return Ok(());
        };
        let mut selections = BTreeSet::new();
        collect_drops(plan, &mut selections);
        for selection in selections {
            self.record_drop(selection, drops)?;
        }
        Ok(())
    }
}

fn direct_callable_key(
    dispatches: &[DraftDispatchEdge],
    source: &StaticSelection,
) -> Option<CallableInstanceKey> {
    dispatches
        .iter()
        .find(|edge| edge.source == *source)
        .and_then(|edge| match &edge.plan {
            DraftDispatchPlan::Invocation(DraftDispatchStep::Direct(
                ExecutableItemKey::Callable(key),
            )) => Some(key.clone()),
            _ => None,
        })
}

fn is_invocation_dispatch(dispatches: &[DraftDispatchEdge], source: &StaticSelection) -> bool {
    dispatches
        .iter()
        .find(|edge| edge.source == *source)
        .is_some_and(|edge| {
            matches!(
                &edge.plan,
                DraftDispatchPlan::Invocation(_) | DraftDispatchPlan::OpaqueInvocation { .. }
            )
        })
}
