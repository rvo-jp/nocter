use std::collections::BTreeMap;

use nocter_checking::{SequenceElement, StaticSelection, TypeSubstitution};
use nocter_model::{BodyId, BodyNodeId, ExecutableItemId, TypeId};

use super::{
    DraftDispatchEdge, DraftDispatchPlan, DraftDispatchStep, ExecutableClosureBuilder, item_id,
};
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
                    self.specialize_sequence_segment(
                        source,
                        element,
                        input,
                        substitution,
                        dispatches,
                        &node_types,
                    )
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
        owner: BodyNodeId,
        element: &SequenceElement,
        input: ExecutablePackInput,
        substitution: &TypeSubstitution,
        dispatches: &[DraftDispatchEdge],
        node_types: &BTreeMap<BodyNodeId, TypeId>,
    ) -> Result<ExecutableSequenceSegment, ExecutableProgramError> {
        match element {
            SequenceElement::Value(source) => {
                let ty = node_types
                    .get(source)
                    .copied()
                    .ok_or(ExecutableProgramError::InvalidSequencePlan(owner))?;
                let ty = self.resolver.specialize_type(ty, substitution)?;
                if ty != input.element() {
                    return Err(ExecutableProgramError::InvalidSequencePlan(owner));
                }
                Ok(ExecutableSequenceSegment::Value {
                    source: *source,
                    ty,
                })
            }
            SequenceElement::Spread {
                mode,
                iteration,
                exact_size,
            } => {
                if !is_invocation_dispatch(dispatches, iteration.next())
                    || !is_invocation_dispatch(dispatches, exact_size)
                {
                    return Err(ExecutableProgramError::InvalidSequencePlan(owner));
                }
                let iterator_type = node_types
                    .get(&iteration.iterator())
                    .copied()
                    .ok_or(ExecutableProgramError::InvalidSequencePlan(owner))?;
                let iterator_type = self.resolver.specialize_type(iterator_type, substitution)?;
                let item = self
                    .resolver
                    .specialize_type(iteration.item(), substitution)?;
                let contribution = mode
                    .contribution_type(self.target.checked().types(), iteration.item())
                    .ok_or(ExecutableProgramError::InvalidSequencePlan(owner))?;
                let contribution = self.resolver.specialize_type(contribution, substitution)?;
                if contribution != input.element() {
                    return Err(ExecutableProgramError::InvalidSequencePlan(owner));
                }
                Ok(ExecutableSequenceSegment::Spread(
                    ExecutableSequenceSpread::new(
                        *mode,
                        iteration.iterator(),
                        iterator_type,
                        item,
                        contribution,
                        iteration.next().clone(),
                        exact_size.clone(),
                    ),
                ))
            }
        }
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
