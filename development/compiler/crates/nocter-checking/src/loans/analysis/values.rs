use std::collections::BTreeSet;

use super::Analyzer;
use crate::loans::state::LoanState;
use crate::loans::value::LoanValue;
use crate::{
    AggregateConstruction, BodyCheckError, BodyCheckInternalError, CheckedOutcome, LoanId,
    PlaceRoot, ProvenanceProjection,
};

impl Analyzer<'_, '_> {
    pub(super) fn evaluate_primitive(
        &mut self,
        operation: &crate::PrimitiveOperation,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyCheckError> {
        match operation {
            crate::PrimitiveOperation::Unary { operand, .. }
            | crate::PrimitiveOperation::IntegerConversion { operand, .. } => {
                let (_, reaches) = self.evaluate(*operand, state, extra)?;
                Ok((LoanValue::independent(), reaches))
            }
            crate::PrimitiveOperation::Binary { left, right, .. } => {
                if !self.evaluate(*left, state, extra)?.1 {
                    return Ok((LoanValue::independent(), false));
                }
                let (_, reaches) = self.evaluate(*right, state, extra)?;
                Ok((LoanValue::independent(), reaches))
            }
        }
    }

    pub(super) fn evaluate_aggregate(
        &mut self,
        aggregate: &AggregateConstruction,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyCheckError> {
        let mut result = LoanValue::independent();
        match aggregate {
            AggregateConstruction::Struct { fields, .. } => {
                for (field, node) in fields {
                    let (value, reaches) = self.evaluate(*node, state, extra)?;
                    if !reaches {
                        return Ok((LoanValue::independent(), false));
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
                    .ok_or(BodyCheckInternalError::LoanAnalysis)?;
                for (parameter, node) in declaration.payload().iter().zip(payload) {
                    let (value, reaches) = self.evaluate(*node, state, extra)?;
                    if !reaches {
                        return Ok((LoanValue::independent(), false));
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
                let mut values = LoanValue::independent();
                for element in elements {
                    let (value, reaches) = self.evaluate(*element, state, extra)?;
                    if !reaches {
                        return Ok((LoanValue::independent(), false));
                    }
                    values.union_with(&value);
                }
                result.insert_projection(ProvenanceProjection::Element, values);
            }
        }
        Ok((result, true))
    }

    pub(super) fn evaluate_outcome(
        &mut self,
        outcome: &CheckedOutcome,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyCheckError> {
        match outcome {
            CheckedOutcome::Inject { payload, .. } => {
                let (value, reaches) = self.evaluate(*payload, state, extra)?;
                Ok((
                    LoanValue::from_projection(ProvenanceProjection::OutcomeValue, value),
                    reaches,
                ))
            }
            CheckedOutcome::Absent => Ok((LoanValue::independent(), true)),
            CheckedOutcome::Failure(payload) => {
                let (value, reaches) = self.evaluate(*payload, state, extra)?;
                Ok((
                    LoanValue::from_projection(ProvenanceProjection::OutcomeFailure, value),
                    reaches,
                ))
            }
            CheckedOutcome::Propagate { operand, .. } | CheckedOutcome::Force { operand, .. } => {
                let (value, reaches) = self.evaluate(*operand, state, extra)?;
                Ok((value.projected(ProvenanceProjection::OutcomeValue), reaches))
            }
            CheckedOutcome::Recover {
                operand,
                binding,
                fallback,
                ..
            } => {
                let (value, reaches) = self.evaluate(*operand, state, extra)?;
                if !reaches {
                    return Ok((LoanValue::independent(), false));
                }
                let success = value.projected(ProvenanceProjection::OutcomeValue);
                let entry = state.clone();
                let mut fallback_state = entry.clone();
                if let Some(binding) = binding {
                    fallback_state.set_root(
                        PlaceRoot::Local(*binding),
                        value.projected(ProvenanceProjection::OutcomeFailure),
                    );
                }
                let (fallback_value, fallback_reaches) =
                    self.evaluate(*fallback, &mut fallback_state, extra)?;
                if let Some(binding) = binding {
                    fallback_state.remove_root(PlaceRoot::Local(*binding));
                }
                let mut incoming = vec![entry];
                if fallback_reaches {
                    incoming.push(fallback_state);
                }
                state.join(&incoming);
                let mut result = success;
                result.union_with(&fallback_value);
                Ok((result, true))
            }
        }
    }

    pub(super) fn evaluate_allocation(
        &mut self,
        selection: crate::AllocationSelection,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(), BodyCheckError> {
        if let crate::AllocationSelection::Explicit(value) = selection {
            self.evaluate(value, state, extra)?;
        }
        Ok(())
    }

    pub(super) fn evaluate_sequence(
        &mut self,
        sequence: &crate::CheckedSequence,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyCheckError> {
        self.evaluate_allocation(sequence.allocation(), state, extra)?;
        let mut elements = LoanValue::independent();
        for element in sequence.elements() {
            match element {
                crate::SequenceElement::Value(value) => {
                    let (value, reaches) = self.evaluate(*value, state, extra)?;
                    if !reaches {
                        return Ok((LoanValue::independent(), false));
                    }
                    elements.union_with(&value);
                }
                crate::SequenceElement::Spread {
                    mode, iteration, ..
                } => {
                    let (iterator, reaches) = self.evaluate(iteration.iterator(), state, extra)?;
                    if !reaches {
                        return Ok((LoanValue::independent(), false));
                    }
                    let contribution = mode
                        .contribution_type(self.types, iteration.item())
                        .ok_or(BodyCheckInternalError::LoanAnalysis)?;
                    if !self.types.may_carry_storage(contribution)
                        || (*mode == crate::SpreadMode::Move
                            && matches!(
                                self.types.get(iteration.item()),
                                Some(nocter_model::TypeKind::Borrow { .. })
                            ))
                    {
                        continue;
                    }
                    let callable = match iteration.next().dispatch() {
                        crate::StaticDispatch::Direct(callable)
                        | crate::StaticDispatch::InterfaceMethod {
                            method: callable, ..
                        } => callable,
                        crate::StaticDispatch::StructuralRequirement(_) => {
                            return Err(BodyCheckInternalError::LoanAnalysis.into());
                        }
                    };
                    let value = self
                        .map_callable_result(callable, Some(&iterator), &[])?
                        .projected(ProvenanceProjection::OutcomeValue);
                    elements.union_with(&value);
                }
            }
        }
        Ok((
            LoanValue::from_projection(ProvenanceProjection::Element, elements),
            true,
        ))
    }
}
