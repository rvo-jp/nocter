use nocter_declarations::{BodyOwner, CallableProvenance, ProvenanceAnnotation, ProvenanceOrigin};
use nocter_model::{BodyNodeId, CallableId, ProvenanceSet, TypeId};

use super::Analyzer;
use crate::provenance::state::ProvenanceState;
use crate::provenance::{invocation_origin_retains_place, invocation_place_can_reach_result};
use crate::{
    AggregateConstruction, AllocationSelection, AmbientStorageDependence, ArgumentPackSegment,
    BodyCheckInternalError, BodyRelationError, CallTarget, CheckedArgumentPack, CheckedCall,
    CheckedIteratorAcquisition, CheckedOperation, CheckedOutcome, CheckedPackLiteral,
    CheckedReceiver, IterationAcquisition, PlaceRoot, ProvenanceProjection, ProvenanceSource,
    ReceiverPreparation, StaticDispatch, ValueProvenance,
};

struct CallableValueProvenance {
    value: ValueProvenance,
    storage: Option<ValueProvenance>,
}

/// The two distinct storage channels available through a method receiver.
///
/// `carried` belongs to the receiver value itself (for example a view iterator's source). `place`
/// belongs to the temporary borrow used to invoke a method on an owned place. A declared direct
/// borrow result may retain the latter; an associated or generic result fixed independently of the
/// invocation may retain only the former.
struct ReceiverProvenance {
    carried: ValueProvenance,
    place: Option<ValueProvenance>,
}

impl ReceiverProvenance {
    fn carried(value: ValueProvenance) -> Self {
        Self {
            carried: value,
            place: None,
        }
    }

    fn retained(&self) -> &ValueProvenance {
        self.place.as_ref().unwrap_or(&self.carried)
    }
}

struct ArgumentProvenance {
    carried: ValueProvenance,
    place: Option<ValueProvenance>,
}

impl ArgumentProvenance {
    fn carried(value: ValueProvenance) -> Self {
        Self {
            carried: value,
            place: None,
        }
    }

    fn retained(&self, retain_invocation_place: bool) -> &ValueProvenance {
        if retain_invocation_place {
            self.place.as_ref().unwrap_or(&self.carried)
        } else {
            &self.carried
        }
    }
}

struct EvaluatedCall {
    callable: Option<CallableValueProvenance>,
    receiver: Option<ReceiverProvenance>,
    arguments: Vec<ArgumentProvenance>,
}

impl Analyzer<'_> {
    pub(super) fn evaluate_iterator_acquisition(
        &mut self,
        node: BodyNodeId,
        acquisition: &CheckedIteratorAcquisition,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyRelationError> {
        let Some(source) = self.evaluate_receiver(acquisition.source(), state)? else {
            return Ok((ValueProvenance::independent(), false));
        };
        let result = match acquisition.acquisition() {
            IterationAcquisition::Direct => source.carried,
            IterationAcquisition::Expansion(selection) => match selection.dispatch() {
                StaticDispatch::Direct(callable) => self.map_callable_summary(
                    callable,
                    Some(&source),
                    &[],
                    state.current_allocation(),
                    self.body
                        .nodes()
                        .get(node)
                        .ok_or(BodyCheckInternalError::MissingNode(node))?
                        .ty(),
                )?,
                StaticDispatch::StructuralRequirement { evidence } => {
                    if !matches!(
                        self.capability_evidence
                            .get(evidence)
                            .map(crate::body_check::CapabilityEvidence::predicate),
                        Some(crate::CheckedPredicate::Expansion { .. })
                    ) {
                        return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
                    }
                    source.carried
                }
                StaticDispatch::InterfaceMethod { .. }
                | StaticDispatch::InterfaceSelfMethod { .. }
                | StaticDispatch::InterfaceDefault { .. }
                | StaticDispatch::OpaqueMethod { .. } => {
                    return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
                }
            },
        };
        Ok((result, true))
    }

    pub(super) fn iteration_item_provenance(
        &self,
        iteration: &crate::TypedIterationStep,
        iterator: &ValueProvenance,
        current_allocation: &ValueProvenance,
        iterator_storage: ProvenanceSource,
    ) -> Result<ValueProvenance, BodyCheckInternalError> {
        Ok(self
            .iteration_result_provenance(iteration, iterator, current_allocation, iterator_storage)?
            .projected(ProvenanceProjection::OutcomeValue))
    }

    pub(super) fn async_iteration_item_provenance(
        &self,
        iteration: &crate::TypedIterationStep,
        iterator: &ValueProvenance,
        current_allocation: &ValueProvenance,
        iterator_storage: ProvenanceSource,
    ) -> Result<ValueProvenance, BodyCheckInternalError> {
        let value = self.iteration_result_provenance(
            iteration,
            iterator,
            current_allocation,
            iterator_storage,
        )?;
        Ok(value
            .projected(ProvenanceProjection::AsyncOutput)
            .projected(ProvenanceProjection::OutcomeValue)
            .projected(ProvenanceProjection::OutcomeValue))
    }

    fn iteration_result_provenance(
        &self,
        iteration: &crate::TypedIterationStep,
        iterator: &ValueProvenance,
        current_allocation: &ValueProvenance,
        iterator_storage: ProvenanceSource,
    ) -> Result<ValueProvenance, BodyCheckInternalError> {
        if iteration.item_origin() == crate::IterationItemOrigin::ReceiverLoan {
            return Ok(ValueProvenance::from_source(iterator_storage));
        }
        let acquisition = self
            .body
            .nodes()
            .get(iteration.iterator())
            .and_then(|node| match node.operation() {
                CheckedOperation::IteratorAcquisition(acquisition) => Some(acquisition),
                _ => None,
            })
            .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
        if acquisition.source().preparation() == ReceiverPreparation::Owned
            && matches!(
                self.types.get(iteration.item()),
                Some(nocter_model::TypeKind::Borrow { .. })
            )
        {
            return Ok(ValueProvenance::from_source(iterator_storage));
        }
        let callable = match iteration.next().dispatch() {
            StaticDispatch::Direct(callable)
            | StaticDispatch::InterfaceMethod {
                method: callable, ..
            }
            | StaticDispatch::InterfaceSelfMethod {
                method: callable, ..
            }
            | StaticDispatch::InterfaceDefault {
                method: callable, ..
            }
            | StaticDispatch::OpaqueMethod {
                method: callable, ..
            } => callable,
            StaticDispatch::StructuralRequirement { .. } => {
                return Err(BodyCheckInternalError::ProvenanceAnalysis);
            }
        };
        self.map_callable_summary(
            callable,
            Some(&ReceiverProvenance::carried(iterator.clone())),
            &[],
            current_allocation,
            iteration.item(),
        )
    }

    pub(super) fn evaluate_aggregate(
        &mut self,
        aggregate: &AggregateConstruction,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyRelationError> {
        let mut result = ValueProvenance::independent();
        match aggregate {
            AggregateConstruction::Struct { fields, .. } => {
                for (field, node) in fields {
                    let (value, reaches) = self.evaluate(*node, state)?;
                    if !reaches {
                        return Ok((ValueProvenance::independent(), false));
                    }
                    result.insert_projection(ProvenanceProjection::Field(*field), value);
                }
            }
            AggregateConstruction::Enum { variant, payload } => {
                let declaration = self
                    .graph
                    .declarations()
                    .variants()
                    .get(*variant)
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                for (parameter, node) in declaration.payload().iter().zip(payload) {
                    let (value, reaches) = self.evaluate(*node, state)?;
                    if !reaches {
                        return Ok((ValueProvenance::independent(), false));
                    }
                    result.insert_projection(
                        ProvenanceProjection::VariantPayload {
                            variant: *variant,
                            parameter: *parameter,
                        },
                        value,
                    );
                }
            }
            AggregateConstruction::FixedArray(elements) => {
                let mut values = ValueProvenance::independent();
                for element in elements {
                    let (value, reaches) = self.evaluate(*element, state)?;
                    if !reaches {
                        return Ok((ValueProvenance::independent(), false));
                    }
                    values.union_with(&value);
                }
                result.insert_projection(ProvenanceProjection::Element, values);
            }
            AggregateConstruction::Tuple(elements) => {
                for (index, element) in elements.iter().enumerate() {
                    let (value, reaches) = self.evaluate(*element, state)?;
                    if !reaches {
                        return Ok((ValueProvenance::independent(), false));
                    }
                    result.insert_projection(ProvenanceProjection::TupleElement(index), value);
                }
            }
        }
        Ok((result, true))
    }

    pub(super) fn evaluate_outcome(
        &mut self,
        node: BodyNodeId,
        outcome: &CheckedOutcome,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyRelationError> {
        match outcome {
            CheckedOutcome::Absent => Ok((ValueProvenance::independent(), true)),
            CheckedOutcome::Inject { payload, .. } => {
                let (value, reaches) = self.evaluate(*payload, state)?;
                Ok((
                    ValueProvenance::from_projection(ProvenanceProjection::OutcomeValue, value),
                    reaches,
                ))
            }
            CheckedOutcome::Failure(payload) => {
                let (value, reaches) = self.evaluate(*payload, state)?;
                Ok((
                    ValueProvenance::from_projection(ProvenanceProjection::OutcomeFailure, value),
                    reaches,
                ))
            }
            CheckedOutcome::Force { operand, .. } => {
                let (value, reaches) = self.evaluate(*operand, state)?;
                Ok((value.projected(ProvenanceProjection::OutcomeValue), reaches))
            }
            CheckedOutcome::Propagate { operand, .. } => {
                let (value, reaches) = self.evaluate(*operand, state)?;
                if reaches {
                    let failure = value.projected(ProvenanceProjection::OutcomeFailure);
                    if !failure.all_sources().is_empty() {
                        self.record_return(node, failure);
                    }
                }
                Ok((value.projected(ProvenanceProjection::OutcomeValue), reaches))
            }
            CheckedOutcome::Recover {
                operand,
                binding,
                fallback,
                ..
            } => {
                let (operand, reaches) = self.evaluate(*operand, state)?;
                if !reaches {
                    return Ok((ValueProvenance::independent(), false));
                }
                let entry = state.clone();
                let mut fallback_state = entry.clone();
                if let Some(binding) = binding {
                    fallback_state.set_value(
                        PlaceRoot::Local(*binding),
                        operand.projected(ProvenanceProjection::OutcomeFailure),
                    );
                }
                let (fallback_value, fallback_reaches) =
                    self.evaluate(*fallback, &mut fallback_state)?;
                let mut incoming = vec![entry];
                if fallback_reaches {
                    incoming.push(fallback_state);
                }
                state.join(&incoming);
                let mut result = operand.projected(ProvenanceProjection::OutcomeValue);
                if fallback_reaches {
                    result.union_with(&fallback_value);
                }
                Ok((result, true))
            }
        }
    }

    pub(super) fn evaluate_call(
        &mut self,
        node: BodyNodeId,
        call: &CheckedCall,
        state: &mut ProvenanceState,
        result_type: TypeId,
    ) -> Result<(ValueProvenance, bool), BodyRelationError> {
        let Some(evaluated) = self.evaluate_call_inputs(call, state)? else {
            return Ok((ValueProvenance::independent(), false));
        };
        self.validate_call_input_provenance(node, call, &evaluated)?;
        let mapped_type = call.execution().executed_result();
        let mut result = self.map_call_result(call, &evaluated, state, mapped_type)?;
        if call.execution().is_deferred() {
            let captures = Self::map_deferred_captures(&evaluated, state);
            let mut computation = ValueProvenance::independent();
            computation.insert_projection(ProvenanceProjection::AsyncCapture, captures);
            computation.insert_projection(ProvenanceProjection::AsyncOutput, result);
            result = computation;
        }
        if !self.types.may_carry_storage(result_type) {
            result = ValueProvenance::independent();
        }
        Ok((result, true))
    }

    fn validate_call_input_provenance(
        &self,
        node: BodyNodeId,
        call: &CheckedCall,
        evaluated: &EvaluatedCall,
    ) -> Result<(), BodyRelationError> {
        match call.target() {
            CallTarget::Static(selection) => {
                let callable = static_callable(selection.dispatch())
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                let declaration = self
                    .graph
                    .declarations()
                    .callables()
                    .get(callable)
                    .ok_or(BodyCheckInternalError::MissingCallable(callable))?;
                for constraint in declaration.input_provenance().constraints() {
                    let target = if declaration.receiver() == Some(constraint.target()) {
                        ProvenanceOrigin::Receiver
                    } else {
                        ProvenanceOrigin::Parameter(constraint.target())
                    };
                    let target = Self::declaration_input_value(declaration, target, evaluated)?;
                    let mut sources = ValueProvenance::independent();
                    for source in constraint.sources().origins() {
                        sources.union_with(
                            &Self::declaration_input_value(declaration, *source, evaluated)?
                                .flattened(),
                        );
                    }
                    self.require_contained_input(node, target, &sources)?;
                }
            }
            CallTarget::CallableValue { dispatch, .. } => {
                let contract = match dispatch.dispatch() {
                    StaticDispatch::StructuralRequirement { evidence } => self
                        .capability_evidence
                        .get(evidence)
                        .and_then(|evidence| match evidence.predicate() {
                            crate::CheckedPredicate::Callable { contract, .. } => Some(contract),
                            _ => None,
                        })
                        .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?,
                    _ => return Err(BodyCheckInternalError::ProvenanceAnalysis.into()),
                };
                self.validate_structural_inputs(node, contract, evaluated)?;
            }
            CallTarget::ClosureValue { closure, .. } => {
                let definition = self
                    .closures
                    .definitions()
                    .get(*closure)
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                for contract in definition.callable_requirements() {
                    self.validate_structural_inputs(node, contract, evaluated)?;
                }
            }
            CallTarget::ErasedCallableValue { value, .. } => {
                let checked = self
                    .body
                    .nodes()
                    .get(*value)
                    .ok_or(BodyCheckInternalError::MissingNode(*value))?;
                let Some(nocter_model::TypeKind::Callable(callable)) = self.types.get(checked.ty())
                else {
                    return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
                };
                self.validate_structural_inputs(node, callable.contract(), evaluated)?;
            }
        }
        Ok(())
    }

    fn declaration_input_value<'call>(
        declaration: &nocter_declarations::CallableDeclaration,
        origin: ProvenanceOrigin,
        evaluated: &'call EvaluatedCall,
    ) -> Result<&'call ValueProvenance, BodyCheckInternalError> {
        match origin {
            ProvenanceOrigin::Receiver => evaluated
                .receiver
                .as_ref()
                .map(ReceiverProvenance::retained)
                .ok_or(BodyCheckInternalError::ProvenanceAnalysis),
            ProvenanceOrigin::Parameter(parameter) => {
                let position = declaration
                    .parameters()
                    .iter()
                    .position(|candidate| *candidate == parameter)
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                evaluated
                    .arguments
                    .get(position)
                    .map(|argument| argument.retained(true))
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)
            }
        }
    }

    fn validate_structural_inputs(
        &self,
        node: BodyNodeId,
        contract: &nocter_model::CallableContract,
        evaluated: &EvaluatedCall,
    ) -> Result<(), BodyRelationError> {
        for constraint in contract.input_provenance().constraints() {
            let target = evaluated
                .arguments
                .get(constraint.target().position())
                .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?
                .retained(true);
            let mut sources = ValueProvenance::independent();
            for source in constraint.sources().origins() {
                let value = evaluated
                    .arguments
                    .get(source.position())
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?
                    .retained(true);
                sources.union_with(&value.flattened());
            }
            self.require_contained_input(node, target, &sources)?;
        }
        Ok(())
    }

    pub(super) fn require_contained_input(
        &self,
        node: BodyNodeId,
        target: &ValueProvenance,
        sources: &ValueProvenance,
    ) -> Result<(), BodyRelationError> {
        let allowed = sources.all_sources();
        let invalid = target
            .all_sources()
            .iter()
            .copied()
            .map(|origin| self.provenance_source_within(origin, &allowed))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .any(|within| !within);
        if invalid {
            return Err(BodyRelationError::rule(
                self.body_id,
                crate::BodyRule::InvalidValueProvenance,
                node,
                [],
            ));
        }
        Ok(())
    }

    fn provenance_source_within(
        &self,
        source: ProvenanceSource,
        allowed: &std::collections::BTreeSet<ProvenanceSource>,
    ) -> Result<bool, BodyRelationError> {
        if allowed.contains(&source) {
            return Ok(true);
        }
        match source {
            ProvenanceSource::Callable(origin) => {
                let BodyOwner::Callable(callable) = self.owner else {
                    return Ok(false);
                };
                let declaration = self
                    .graph
                    .declarations()
                    .callables()
                    .get(callable)
                    .ok_or(BodyCheckInternalError::MissingCallable(callable))?;
                let allowed = allowed
                    .iter()
                    .filter_map(|source| match source {
                        ProvenanceSource::Callable(origin) => Some(*origin),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                Ok(nocter_model::provenance_is_bounded_by(
                    origin,
                    &allowed,
                    |current| {
                        let target = match current {
                            ProvenanceOrigin::Receiver => declaration.receiver(),
                            ProvenanceOrigin::Parameter(parameter) => Some(parameter),
                        }?;
                        declaration
                            .input_provenance()
                            .sources(target)
                            .map(CallableProvenance::origins)
                    },
                ))
            }
            ProvenanceSource::ClosureParameter { closure, origin } => {
                let Some((current, definition)) = self.closure else {
                    return Ok(false);
                };
                if current != closure {
                    return Ok(false);
                }
                let allowed = allowed
                    .iter()
                    .filter_map(|source| match source {
                        ProvenanceSource::ClosureParameter {
                            closure: actual,
                            origin,
                        } if *actual == closure => Some(*origin),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                Ok(definition.callable_requirements().iter().all(|contract| {
                    nocter_model::provenance_is_bounded_by(origin, &allowed, |parameter| {
                        contract
                            .input_provenance()
                            .sources(parameter)
                            .map(ProvenanceSet::origins)
                    })
                }))
            }
            ProvenanceSource::CurrentAllocation
            | ProvenanceSource::Local(_)
            | ProvenanceSource::OwnedParameter(_)
            | ProvenanceSource::Region(_)
            | ProvenanceSource::StatementTemporary(_)
            | ProvenanceSource::ScopedTemporary { .. }
            | ProvenanceSource::ClosureCaptureValue { .. }
            | ProvenanceSource::ClosureEnvironment(_)
            | ProvenanceSource::Unknown => Ok(false),
        }
    }

    fn map_deferred_captures(
        evaluated: &EvaluatedCall,
        state: &ProvenanceState,
    ) -> ValueProvenance {
        let mut captures = state.current_allocation().flattened();
        if let Some(callable) = &evaluated.callable {
            captures.union_with(&callable.value.flattened());
            if let Some(storage) = &callable.storage {
                captures.union_with(&storage.flattened());
            }
        }
        if let Some(receiver) = &evaluated.receiver {
            captures.union_with(&receiver.retained().flattened());
        }
        for argument in &evaluated.arguments {
            captures.union_with(&argument.retained(true).flattened());
        }
        captures
    }

    fn evaluate_call_inputs(
        &mut self,
        call: &CheckedCall,
        state: &mut ProvenanceState,
    ) -> Result<Option<EvaluatedCall>, BodyRelationError> {
        let callable = match call.target() {
            CallTarget::CallableValue {
                value, capability, ..
            }
            | CallTarget::ClosureValue {
                value, capability, ..
            }
            | CallTarget::ErasedCallableValue {
                value, capability, ..
            } => {
                let (provenance, reaches) = self.evaluate(*value, state)?;
                if !reaches {
                    return Ok(None);
                }
                let storage = if *capability == nocter_model::CallableCapability::Owned {
                    None
                } else {
                    let checked = self
                        .body
                        .nodes()
                        .get(*value)
                        .ok_or(BodyCheckInternalError::MissingNode(*value))?;
                    let CheckedOperation::Place(place) = checked.operation() else {
                        return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
                    };
                    Some(self.place_storage(*place, state)?)
                };
                Some(CallableValueProvenance {
                    value: provenance,
                    storage,
                })
            }
            CallTarget::Static(_) => None,
        };
        let receiver = if let Some(receiver) = call.receiver() {
            let provenance = self.evaluate_receiver(receiver, state)?;
            let Some(provenance) = provenance else {
                return Ok(None);
            };
            Some(provenance)
        } else {
            None
        };
        let mut arguments = Vec::with_capacity(call.arguments().len());
        for argument in call.arguments() {
            let (value, reaches) = self.evaluate(*argument, state)?;
            if !reaches {
                return Ok(None);
            }
            let checked = self
                .body
                .nodes()
                .get(*argument)
                .ok_or(BodyCheckInternalError::MissingNode(*argument))?;
            let argument = match checked.operation() {
                CheckedOperation::Borrow { place, .. } => ArgumentProvenance {
                    carried: self.read_place(*place, state)?,
                    place: Some(value),
                },
                _ => ArgumentProvenance::carried(value),
            };
            arguments.push(argument);
        }
        if let Some(pack) = call.pack() {
            let Some(argument) = self.evaluate_argument_pack(pack, state)? else {
                return Ok(None);
            };
            arguments.push(argument);
        }
        Ok(Some(EvaluatedCall {
            callable,
            receiver,
            arguments,
        }))
    }

    fn evaluate_argument_pack(
        &mut self,
        pack: &CheckedArgumentPack,
        state: &mut ProvenanceState,
    ) -> Result<Option<ArgumentProvenance>, BodyRelationError> {
        if let Some(parameter) = pack.forwarded_parameter() {
            return Ok(Some(ArgumentProvenance::carried(
                state.value(PlaceRoot::Parameter(parameter)),
            )));
        }
        let mut elements = ValueProvenance::independent();
        for segment in pack.segments() {
            if !matches!(segment, ArgumentPackSegment::Spread { .. }) {
                for value in segment.operands() {
                    let (provenance, reaches) = self.evaluate(value, state)?;
                    if !reaches {
                        return Ok(None);
                    }
                    let checked = self
                        .body
                        .nodes()
                        .get(value)
                        .ok_or(BodyCheckInternalError::MissingNode(value))?;
                    let argument = match checked.operation() {
                        CheckedOperation::Borrow { place, .. } => ArgumentProvenance {
                            carried: self.read_place(*place, state)?,
                            place: Some(provenance),
                        },
                        _ => ArgumentProvenance::carried(provenance),
                    };
                    elements.union_with(&argument.retained(true).flattened());
                }
                continue;
            }
            if let ArgumentPackSegment::Spread {
                mode, iteration, ..
            } = segment
            {
                let (iterator, reaches) = self.evaluate(iteration.iterator(), state)?;
                if !reaches {
                    return Ok(None);
                }
                let contribution = mode
                    .contribution_type(self.types, iteration.item())
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                if self.types.may_carry_storage(contribution) {
                    elements.union_with(&self.iteration_item_provenance(
                        iteration.step(),
                        &iterator,
                        state.current_allocation(),
                        ProvenanceSource::StatementTemporary(iteration.iterator()),
                    )?);
                }
            }
        }
        Ok(Some(ArgumentProvenance::carried(elements)))
    }

    fn map_call_result(
        &self,
        call: &CheckedCall,
        evaluated: &EvaluatedCall,
        state: &ProvenanceState,
        result_type: TypeId,
    ) -> Result<ValueProvenance, BodyRelationError> {
        Ok(match call.target() {
            CallTarget::Static(selection) => {
                let callable = static_callable(selection.dispatch())
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                self.map_callable_summary(
                    callable,
                    evaluated.receiver.as_ref(),
                    &evaluated.arguments,
                    state.current_allocation(),
                    result_type,
                )?
            }
            CallTarget::CallableValue { dispatch, .. } => {
                let contract = match dispatch.dispatch() {
                    StaticDispatch::StructuralRequirement { evidence } => self
                        .capability_evidence
                        .get(evidence)
                        .and_then(|evidence| match evidence.predicate() {
                            crate::CheckedPredicate::Callable { contract, .. } => Some(contract),
                            _ => None,
                        })
                        .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?,
                    _ => return Err(BodyCheckInternalError::ProvenanceAnalysis.into()),
                };
                self.map_contract_call_result(contract, evaluated, result_type)?
            }
            CallTarget::ClosureValue { closure, .. } => {
                let summary = self
                    .closure_summaries
                    .get(closure)
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                let callable = evaluated
                    .callable
                    .as_ref()
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                let mut mapped = ValueProvenance::independent();
                let retain_place =
                    invocation_place_can_reach_result(self.graph, self.types, result_type);
                for origin in &summary.parameters {
                    let argument = evaluated
                        .arguments
                        .get(origin.position())
                        .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                    mapped.union_with(&argument.retained(retain_place).flattened());
                }
                for capture in &summary.captures {
                    let value = callable
                        .value
                        .projected(ProvenanceProjection::ClosureCaptureValue(*capture));
                    mapped.union_with(&value.flattened());
                }
                if summary.environment {
                    let environment = callable
                        .storage
                        .as_ref()
                        .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                    mapped.union_with(&environment.flattened());
                }
                match summary.ambient {
                    AmbientStorageDependence::Independent => {}
                    AmbientStorageDependence::Current => {
                        mapped.union_with(state.current_allocation());
                    }
                    AmbientStorageDependence::Unknown => {
                        mapped.union_with(&ValueProvenance::from_source(ProvenanceSource::Unknown));
                    }
                }
                mapped
            }
            CallTarget::ErasedCallableValue { value, .. } => {
                let checked = self
                    .body
                    .nodes()
                    .get(*value)
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                let Some(nocter_model::TypeKind::Callable(callable_type)) =
                    self.types.get(checked.ty())
                else {
                    return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
                };
                self.map_contract_call_result(callable_type.contract(), evaluated, result_type)?
            }
        })
    }

    fn map_contract_call_result(
        &self,
        contract: &nocter_model::CallableContract,
        evaluated: &EvaluatedCall,
        result_type: TypeId,
    ) -> Result<ValueProvenance, BodyCheckInternalError> {
        let mut mapped = ValueProvenance::independent();
        let retain_place = invocation_origin_retains_place(
            self.graph,
            self.types,
            contract.result(),
            result_type,
            !contract.provenance().origins().is_empty(),
        );
        for origin in contract.provenance().origins() {
            let argument = evaluated
                .arguments
                .get(origin.position())
                .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
            mapped.union_with(&argument.retained(retain_place).flattened());
        }
        let callable = evaluated
            .callable
            .as_ref()
            .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
        mapped.union_with(&callable.value.flattened());
        if retain_place && let Some(environment) = &callable.storage {
            mapped.union_with(&environment.flattened());
        }
        Ok(mapped)
    }

    fn evaluate_receiver(
        &mut self,
        receiver: &CheckedReceiver,
        state: &mut ProvenanceState,
    ) -> Result<Option<ReceiverProvenance>, BodyRelationError> {
        let (value, reaches) = self.evaluate(receiver.value(), state)?;
        if !reaches {
            return Ok(None);
        }
        let provenance = match receiver.preparation() {
            ReceiverPreparation::Owned
            | ReceiverPreparation::PreserveBorrow(_)
            | ReceiverPreparation::WeakenReadwriteBorrow => ReceiverProvenance::carried(value),
            ReceiverPreparation::BorrowTemporary(_) => ReceiverProvenance {
                carried: value,
                place: Some(ValueProvenance::from_source(
                    ProvenanceSource::StatementTemporary(receiver.value()),
                )),
            },
            ReceiverPreparation::BorrowPlace(_) => {
                let checked = self
                    .body
                    .nodes()
                    .get(receiver.value())
                    .ok_or(BodyCheckInternalError::MissingNode(receiver.value()))?;
                let CheckedOperation::Place(place) = checked.operation() else {
                    return Err(BodyCheckInternalError::ProvenanceAnalysis.into());
                };
                ReceiverProvenance {
                    carried: value,
                    place: Some(self.place_storage(*place, state)?),
                }
            }
        };
        Ok(Some(provenance))
    }

    fn map_callable_summary(
        &self,
        callable: CallableId,
        receiver: Option<&ReceiverProvenance>,
        arguments: &[ArgumentProvenance],
        current_allocation: &ValueProvenance,
        result_type: TypeId,
    ) -> Result<ValueProvenance, BodyCheckInternalError> {
        let declaration = self
            .graph
            .declarations()
            .callables()
            .get(callable)
            .ok_or(BodyCheckInternalError::MissingCallable(callable))?;
        let summary = self
            .summaries
            .get(&callable)
            .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
        let retain_place = invocation_origin_retains_place(
            self.graph,
            self.types,
            declaration.result(),
            result_type,
            matches!(
                declaration.provenance_annotation(),
                ProvenanceAnnotation::Explicit { .. }
            ),
        );
        let mut result = ValueProvenance::independent();
        for origin in &summary.origins {
            match origin {
                ProvenanceOrigin::Receiver => {
                    let receiver = receiver.ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                    let receiver = if retain_place {
                        receiver.place.as_ref().unwrap_or(&receiver.carried)
                    } else {
                        &receiver.carried
                    };
                    result.union_with(&receiver.flattened());
                }
                ProvenanceOrigin::Parameter(parameter) => {
                    let position = declaration
                        .parameters()
                        .iter()
                        .position(|candidate| candidate == parameter)
                        .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                    let argument = arguments
                        .get(position)
                        .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                    result.union_with(&argument.retained(retain_place).flattened());
                }
            }
        }
        match summary.ambient {
            AmbientStorageDependence::Independent => {}
            AmbientStorageDependence::Current => {
                result.union_with(&current_allocation.flattened());
            }
            AmbientStorageDependence::Unknown => {
                result.union_with(&ValueProvenance::from_source(ProvenanceSource::Unknown));
            }
        }
        Ok(result)
    }

    pub(super) fn evaluate_pack_literal(
        &mut self,
        sequence: &CheckedPackLiteral,
        state: &mut ProvenanceState,
    ) -> Result<(ValueProvenance, bool), BodyRelationError> {
        let mut result = self.allocation_provenance(sequence.allocation(), state)?;
        let mut elements = ValueProvenance::independent();
        for element in sequence.pack().segments() {
            if !matches!(element, ArgumentPackSegment::Spread { .. }) {
                for operand in element.operands() {
                    let (value, reaches) = self.evaluate(operand, state)?;
                    if !reaches {
                        return Ok((ValueProvenance::independent(), false));
                    }
                    elements.union_with(&value);
                }
                continue;
            }
            if let ArgumentPackSegment::Spread {
                mode, iteration, ..
            } = element
            {
                let (iterator, reaches) = self.evaluate(iteration.iterator(), state)?;
                if !reaches {
                    return Ok((ValueProvenance::independent(), false));
                }
                let contribution = mode
                    .contribution_type(self.types, iteration.item())
                    .ok_or(BodyCheckInternalError::ProvenanceAnalysis)?;
                if !self.types.may_carry_storage(contribution) {
                    continue;
                }
                let value = self.iteration_item_provenance(
                    iteration.step(),
                    &iterator,
                    state.current_allocation(),
                    ProvenanceSource::StatementTemporary(iteration.iterator()),
                )?;
                elements.union_with(&value);
            }
        }
        result.insert_projection(ProvenanceProjection::Element, elements);
        Ok((result, true))
    }

    pub(super) fn allocation_provenance(
        &mut self,
        allocation: AllocationSelection,
        state: &mut ProvenanceState,
    ) -> Result<ValueProvenance, BodyRelationError> {
        match allocation {
            AllocationSelection::CurrentRegion => Ok(state.current_allocation().clone()),
            AllocationSelection::Explicit(allocator) => {
                let (value, reaches) = self.evaluate(allocator, state)?;
                if !reaches {
                    return Ok(ValueProvenance::independent());
                }
                Ok(value)
            }
        }
    }
}

fn static_callable(dispatch: StaticDispatch) -> Option<nocter_model::CallableId> {
    match dispatch {
        StaticDispatch::Direct(callable)
        | StaticDispatch::InterfaceMethod {
            method: callable, ..
        }
        | StaticDispatch::InterfaceSelfMethod {
            method: callable, ..
        }
        | StaticDispatch::InterfaceDefault {
            method: callable, ..
        }
        | StaticDispatch::OpaqueMethod {
            method: callable, ..
        } => Some(callable),
        StaticDispatch::StructuralRequirement { .. } => None,
    }
}
