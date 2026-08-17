use std::collections::BTreeSet;

use nocter_declarations::{ProvenanceOrigin, RequirementKind, StructuralCapability};
use nocter_model::{BodyNodeId, BorrowCapability, CallableId};

use super::Analyzer;
use crate::loans::state::LoanState;
use crate::loans::value::LoanValue;
use crate::{
    BodyCheckError, BodyCheckInternalError, CallTarget, CheckedCall, CheckedOperation, LoanId,
    ReceiverPreparation, StaticDispatch,
};

impl Analyzer<'_, '_> {
    pub(super) fn evaluate_comparison(
        &mut self,
        node: BodyNodeId,
        comparison: &crate::CheckedComparison,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyCheckError> {
        let (left, reaches) =
            self.evaluate_readonly_operand(node, 0, comparison.left(), state, extra)?;
        if !reaches {
            return Ok((LoanValue::independent(), false));
        }
        let mut right_extra = extra.clone();
        right_extra.extend(left.all_loans());
        let (_, reaches) =
            self.evaluate_readonly_operand(node, 1, comparison.right(), state, &right_extra)?;
        Ok((LoanValue::independent(), reaches))
    }

    pub(super) fn evaluate_readonly_operand(
        &mut self,
        owner: BodyNodeId,
        position: u16,
        operand: &crate::CheckedComparisonOperand,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyCheckError> {
        match operand.preparation() {
            crate::ReadonlyOperandPreparation::BorrowPlace => {
                let place = self.place_node(operand.value())?;
                self.evaluate_place_indices(place, state, extra)?;
                let value = self.issue_loan_as(
                    LoanId::Operand {
                        node: owner,
                        position,
                    },
                    owner,
                    place,
                    BorrowCapability::Readonly,
                    state,
                    extra,
                )?;
                state.set_node(operand.value(), self.read_place(place, state)?);
                Ok((value, true))
            }
            crate::ReadonlyOperandPreparation::BorrowTemporary
            | crate::ReadonlyOperandPreparation::UseReadonlyBorrow
            | crate::ReadonlyOperandPreparation::WeakenReadwriteBorrow => {
                self.evaluate(operand.value(), state, extra)
            }
        }
    }

    pub(super) fn evaluate_call(
        &mut self,
        node: BodyNodeId,
        call: &CheckedCall,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(LoanValue, bool), BodyCheckError> {
        let _callable_value = match call.target() {
            CallTarget::CallableValue { value, .. } => {
                let (loans, reaches) = self.evaluate(*value, state, extra)?;
                if !reaches {
                    return Ok((LoanValue::independent(), false));
                }
                Some(loans)
            }
            CallTarget::Static(_) => None,
        };
        let receiver = self.evaluate_call_receiver(node, call, state, extra)?;
        if call.receiver().is_some() && receiver.is_none() {
            return Ok((LoanValue::independent(), false));
        }
        let Some(arguments) =
            self.evaluate_call_arguments(call, receiver.as_ref(), state, extra)?
        else {
            return Ok((LoanValue::independent(), false));
        };
        let result = self.map_call_target(call, receiver.as_ref(), &arguments)?;
        Ok((result, true))
    }

    fn evaluate_call_receiver(
        &mut self,
        node: BodyNodeId,
        call: &CheckedCall,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<Option<LoanValue>, BodyCheckError> {
        let Some(receiver) = call.receiver() else {
            return Ok(None);
        };
        let value = match receiver.preparation() {
            ReceiverPreparation::BorrowPlace(capability) => {
                let place = self.place_node(receiver.value())?;
                self.evaluate_place_indices(place, state, extra)?;
                let loans = self.issue_loan_as(
                    LoanId::Operand { node, position: 0 },
                    node,
                    place,
                    capability,
                    state,
                    extra,
                )?;
                state.set_node(receiver.value(), self.read_place(place, state)?);
                loans
            }
            ReceiverPreparation::BorrowTemporary(_) => {
                let (_, reaches) = self.evaluate(receiver.value(), state, extra)?;
                if !reaches {
                    return Ok(None);
                }
                LoanValue::independent()
            }
            ReceiverPreparation::Owned
            | ReceiverPreparation::PreserveBorrow(_)
            | ReceiverPreparation::WeakenReadwriteBorrow => {
                let (value, reaches) = self.evaluate(receiver.value(), state, extra)?;
                if !reaches {
                    return Ok(None);
                }
                value
            }
        };
        Ok(Some(value))
    }

    fn evaluate_call_arguments(
        &mut self,
        call: &CheckedCall,
        receiver: Option<&LoanValue>,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<Option<Vec<LoanValue>>, BodyCheckError> {
        let mut invocation_active = extra.clone();
        if let Some(receiver) = receiver {
            invocation_active.extend(receiver.all_loans());
        }
        let mut arguments = Vec::with_capacity(call.arguments().len());
        for argument in call.arguments() {
            let (value, reaches) = self.evaluate(*argument, state, &invocation_active)?;
            if !reaches {
                return Ok(None);
            }
            invocation_active.extend(value.all_loans());
            arguments.push(value);
        }
        Ok(Some(arguments))
    }

    fn map_call_target(
        &self,
        call: &CheckedCall,
        receiver: Option<&LoanValue>,
        arguments: &[LoanValue],
    ) -> Result<LoanValue, BodyCheckError> {
        Ok(match call.target() {
            CallTarget::Static(selection) => {
                let callable = match selection.dispatch() {
                    StaticDispatch::Direct(callable)
                    | StaticDispatch::InterfaceMethod {
                        method: callable, ..
                    } => callable,
                    StaticDispatch::StructuralRequirement(_) => {
                        return Err(BodyCheckInternalError::LoanAnalysis.into());
                    }
                };
                self.map_callable_result(callable, receiver, arguments)?
            }
            CallTarget::CallableValue { dispatch, .. } => {
                let StaticDispatch::StructuralRequirement(requirement) = dispatch.dispatch() else {
                    return Err(BodyCheckInternalError::LoanAnalysis.into());
                };
                let contract = self
                    .graph
                    .declarations()
                    .requirements()
                    .get(requirement)
                    .and_then(|requirement| match requirement.kind() {
                        RequirementKind::Capability {
                            capability: StructuralCapability::Callable(contract),
                            ..
                        } => Some(contract),
                        _ => None,
                    })
                    .ok_or(BodyCheckInternalError::LoanAnalysis)?;
                let mut result = LoanValue::independent();
                for origin in contract.provenance().origins() {
                    result.union_with(
                        arguments
                            .get(origin.position())
                            .ok_or(BodyCheckInternalError::LoanAnalysis)?,
                    );
                }
                result
            }
        })
    }

    pub(super) fn map_callable_result(
        &self,
        callable: CallableId,
        receiver: Option<&LoanValue>,
        arguments: &[LoanValue],
    ) -> Result<LoanValue, BodyCheckInternalError> {
        let declaration = self
            .graph
            .declarations()
            .callables()
            .get(callable)
            .ok_or(BodyCheckInternalError::MissingCallable(callable))?;
        let summary = self
            .provenance
            .callables()
            .get(callable)
            .ok_or(BodyCheckInternalError::LoanAnalysis)?;
        let mut result = LoanValue::independent();
        for origin in summary.origins() {
            match origin {
                ProvenanceOrigin::Receiver => {
                    result.union_with(receiver.ok_or(BodyCheckInternalError::LoanAnalysis)?);
                }
                ProvenanceOrigin::Parameter(parameter) => {
                    let position = declaration
                        .parameters()
                        .iter()
                        .position(|candidate| candidate == parameter)
                        .ok_or(BodyCheckInternalError::LoanAnalysis)?;
                    result.union_with(
                        arguments
                            .get(position)
                            .ok_or(BodyCheckInternalError::LoanAnalysis)?,
                    );
                }
            }
        }
        Ok(result)
    }

    pub(super) fn place_node(
        &self,
        node: BodyNodeId,
    ) -> Result<nocter_model::PlaceId, BodyCheckInternalError> {
        match self
            .input
            .body
            .nodes()
            .get(node)
            .map(crate::CheckedNode::operation)
        {
            Some(
                CheckedOperation::Place(place)
                | CheckedOperation::Copy(place)
                | CheckedOperation::Move(place),
            ) => Ok(*place),
            _ => Err(BodyCheckInternalError::LoanAnalysis),
        }
    }
}
