use std::collections::{BTreeMap, BTreeSet};

use nocter_checking::{
    ArgumentPackSegment, ConcreteDestructionPlan, DropSelection, StaticSelection, TypeSubstitution,
};
use nocter_model::{ArgumentPack, BodyId, BodyNodeId, ExecutableItemId, TypeId};

use super::{
    DraftDispatchEdge, DraftDispatchPlan, DraftDispatchStep, ExecutableClosureBuilder,
    collect_drops, item_id,
};
use crate::executable::ExecutablePackIteration;
use crate::{
    CallableInstanceKey, ExecutableArgumentPackPlan, ExecutableItemKey, ExecutablePackInput,
    ExecutablePackSegment, ExecutablePackSpread, ExecutableProgramError, ExecutableSequencePlan,
};

pub(super) struct DraftSequencePlan {
    source: BodyNodeId,
    constructor: ExecutableItemKey,
    input: ExecutablePackInput,
    result: TypeId,
    segments: Vec<ExecutablePackSegment>,
    allocation: nocter_checking::AllocationSelection,
}

struct SegmentSpecialization<'a> {
    owner: BodyNodeId,
    sequence: bool,
    input: ExecutablePackInput,
    substitution: &'a TypeSubstitution,
    dispatches: &'a [DraftDispatchEdge],
    node_types: &'a BTreeMap<BodyNodeId, TypeId>,
}

impl SegmentSpecialization<'_> {
    fn invalid(&self) -> ExecutableProgramError {
        if self.sequence {
            ExecutableProgramError::InvalidSequencePlan(self.owner)
        } else {
            ExecutableProgramError::InvalidArgumentPackPlan(self.owner)
        }
    }
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
                    let nocter_checking::CheckedOperation::PackLiteral(sequence) = node.operation()
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
                .pack()
                .segments()
                .iter()
                .map(|element| {
                    let context = SegmentSpecialization {
                        owner: source,
                        sequence: true,
                        input,
                        substitution,
                        dispatches,
                        node_types: &node_types,
                    };
                    self.specialize_pack_segment(element, &context, drops)
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

    pub(super) fn specialize_call_argument_packs(
        &mut self,
        body: BodyId,
        dependencies: &crate::CheckedBodyDependencies,
        substitution: &TypeSubstitution,
        dispatches: &[DraftDispatchEdge],
        incoming_pack: Option<ExecutablePackInput>,
        drops: &mut BTreeMap<DropSelection, ExecutableItemKey>,
    ) -> Result<Vec<ExecutableArgumentPackPlan>, ExecutableProgramError> {
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
        let mut plans = Vec::new();
        for source in dependencies.nodes().iter().copied() {
            let Some(node) = checked.nodes().get(source) else {
                return Err(ExecutableProgramError::MissingRoot(source));
            };
            let nocter_checking::CheckedOperation::Call(call) = node.operation() else {
                continue;
            };
            let Some(pack) = call.pack() else {
                continue;
            };
            let nocter_checking::CallTarget::Static(selection) = call.target() else {
                return Err(ExecutableProgramError::InvalidArgumentPackPlan(source));
            };
            let key = direct_callable_key(dispatches, selection)
                .ok_or(ExecutableProgramError::InvalidArgumentPackPlan(source))?;
            let signature = super::callable_signature(
                self.target,
                &mut self.resolver,
                &key,
                &key.substitution(),
            )?;
            let input = signature
                .pack()
                .ok_or(ExecutableProgramError::InvalidArgumentPackPlan(source))?;
            let ordinary_count = call.arguments().len() + usize::from(call.receiver().is_some());
            if signature.inputs().len() != ordinary_count
                || signature.result() != self.resolver.specialize_type(node.ty(), substitution)?
            {
                return Err(ExecutableProgramError::InvalidArgumentPackPlan(source));
            }
            if let Some(forwarded) = pack.forwarded_parameter() {
                let Some(incoming) = incoming_pack else {
                    return Err(ExecutableProgramError::InvalidArgumentPackPlan(source));
                };
                if incoming.source() != forwarded
                    || incoming.shape() != input.shape()
                    || incoming.element() != input.element()
                    || incoming.next() != input.next()
                    || !pack.segments().is_empty()
                {
                    return Err(ExecutableProgramError::InvalidArgumentPackPlan(source));
                }
                plans.push(ExecutableArgumentPackPlan::forwarded(source, input));
                continue;
            }
            let context = SegmentSpecialization {
                owner: source,
                sequence: false,
                input,
                substitution,
                dispatches,
                node_types: &node_types,
            };
            let segments = pack
                .segments()
                .iter()
                .map(|segment| self.specialize_pack_segment(segment, &context, drops))
                .collect::<Result<Vec<_>, _>>()?;
            plans.push(ExecutableArgumentPackPlan::new(source, input, segments));
        }
        Ok(plans)
    }

    fn specialize_pack_segment(
        &mut self,
        element: &ArgumentPackSegment,
        context: &SegmentSpecialization<'_>,
        drops: &mut BTreeMap<DropSelection, ExecutableItemKey>,
    ) -> Result<ExecutablePackSegment, ExecutableProgramError> {
        match element {
            ArgumentPackSegment::Value(source) => {
                let ArgumentPack::Values(expected) = context.input.shape() else {
                    return Err(context.invalid());
                };
                let source_type = context
                    .node_types
                    .get(source)
                    .copied()
                    .ok_or_else(|| context.invalid())?;
                let ty = self
                    .resolver
                    .specialize_type(source_type, context.substitution)?;
                if ty != expected {
                    return Err(context.invalid());
                }
                let destruction = self
                    .resolver
                    .resolve_destruction(source_type, context.substitution)?;
                self.record_sequence_destruction(destruction.as_ref(), drops)?;
                Ok(ExecutablePackSegment::Value {
                    source: *source,
                    ty,
                    destruction,
                })
            }
            ArgumentPackSegment::KeyedValue { key, value } => {
                let ArgumentPack::Keyed {
                    key: expected_key,
                    value: expected_value,
                } = context.input.shape()
                else {
                    return Err(context.invalid());
                };
                let key_type = context
                    .node_types
                    .get(key)
                    .copied()
                    .ok_or_else(|| context.invalid())?;
                let value_type = context
                    .node_types
                    .get(value)
                    .copied()
                    .ok_or_else(|| context.invalid())?;
                let concrete_key = self
                    .resolver
                    .specialize_type(key_type, context.substitution)?;
                let concrete_value = self
                    .resolver
                    .specialize_type(value_type, context.substitution)?;
                if concrete_key != expected_key || concrete_value != expected_value {
                    return Err(context.invalid());
                }
                let key_destruction = self
                    .resolver
                    .resolve_destruction(key_type, context.substitution)?;
                let value_destruction = self
                    .resolver
                    .resolve_destruction(value_type, context.substitution)?;
                self.record_sequence_destruction(key_destruction.as_ref(), drops)?;
                self.record_sequence_destruction(value_destruction.as_ref(), drops)?;
                Ok(ExecutablePackSegment::KeyedValue {
                    key: *key,
                    key_type: concrete_key,
                    key_destruction,
                    value: *value,
                    value_type: concrete_value,
                    value_destruction,
                })
            }
            ArgumentPackSegment::Spread {
                mode,
                iteration,
                exact_size,
            } => {
                if !is_invocation_dispatch(context.dispatches, iteration.next())
                    || !is_invocation_dispatch(context.dispatches, exact_size)
                {
                    return Err(context.invalid());
                }
                let iterator_type = context
                    .node_types
                    .get(&iteration.iterator())
                    .copied()
                    .ok_or_else(|| context.invalid())?;
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
                    .ok_or_else(|| context.invalid())?;
                let contribution = self
                    .resolver
                    .specialize_type(contribution, context.substitution)?;
                if contribution != context.input.element() {
                    return Err(context.invalid());
                }
                self.record_sequence_destruction(destruction.as_ref(), drops)?;
                Ok(ExecutablePackSegment::Spread(ExecutablePackSpread::new(
                    *mode,
                    ExecutablePackIteration::new(
                        iteration.iterator(),
                        iterator_type,
                        item,
                        iteration.next().clone(),
                        exact_size.clone(),
                    ),
                    contribution,
                    destruction,
                )))
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
