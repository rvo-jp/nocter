use std::collections::BTreeSet;

use nocter_declarations::ProvenanceOrigin;
use nocter_model::{BodyNodeId, BorrowCapability, CallableCapability, CallableId};

use super::Analyzer;
use crate::loans::liveness::{LivePlace, LiveSlot};
use crate::loans::state::LoanState;
use crate::loans::value::LoanValue;
use crate::provenance::{invocation_place_can_reach_result, type_can_carry_loan};
use crate::{
    BodyCheckError, BodyCheckInternalError, CallTarget, CheckedCall, CheckedOperation, LoanId,
    PlaceRoot, ReceiverPreparation, StaticDispatch,
};

pub(super) struct InvocationLoan {
    carried: LoanValue,
    place: Option<LoanValue>,
}

impl InvocationLoan {
    pub(super) fn carried(value: LoanValue) -> Self {
        Self {
            carried: value,
            place: None,
        }
    }

    fn retained(&self, retain_invocation_place: bool) -> &LoanValue {
        if retain_invocation_place {
            self.place.as_ref().unwrap_or(&self.carried)
        } else {
            &self.carried
        }
    }

    pub(super) fn into_carried(self) -> LoanValue {
        self.carried
    }

    fn extend_active(&self, active: &mut BTreeSet<LoanId>) {
        active.extend(self.carried.all_loans());
        if let Some(place) = &self.place {
            active.extend(place.all_loans());
        }
    }
}

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
        operand: &crate::CheckedReadonlyOperand,
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
        let (callable_value, callable_environment) = match call.target() {
            CallTarget::CallableValue {
                value, capability, ..
            }
            | CallTarget::ClosureValue {
                value, capability, ..
            } => {
                let place = self.place_node(*value)?;
                self.evaluate_place_indices(place, state, extra)?;
                let carried = self.read_place(place, state)?;
                let environment = match capability {
                    CallableCapability::Readonly | CallableCapability::ReadWrite => {
                        let capability = match capability {
                            CallableCapability::Readonly => BorrowCapability::Readonly,
                            CallableCapability::ReadWrite => BorrowCapability::ReadWrite,
                            CallableCapability::Owned => unreachable!(),
                        };
                        let loan = self.issue_loan_as(
                            LoanId::Operand { node, position: 0 },
                            node,
                            place,
                            capability,
                            state,
                            extra,
                        )?;
                        state.set_node(*value, carried.clone());
                        Some(loan)
                    }
                    CallableCapability::Owned => {
                        self.check_place_access(
                            node,
                            place,
                            super::AccessKind::Write,
                            state,
                            extra,
                        )?;
                        self.remove_place(place, state)?;
                        state.set_node(*value, carried.clone());
                        None
                    }
                };
                (Some(carried), environment)
            }
            CallTarget::Static(_) => (None, None),
        };
        let mut invocation_extra = extra.clone();
        if let Some(value) = &callable_value {
            invocation_extra.extend(value.all_loans());
        }
        if let Some(environment) = &callable_environment {
            invocation_extra.extend(environment.all_loans());
        }
        let receiver = self.evaluate_call_receiver(node, call, state, &invocation_extra)?;
        if call.receiver().is_some() && receiver.is_none() {
            return Ok((LoanValue::independent(), false));
        }
        let Some(arguments) =
            self.evaluate_call_arguments(call, receiver.as_ref(), state, &invocation_extra)?
        else {
            return Ok((LoanValue::independent(), false));
        };
        let result_type = self
            .input
            .body
            .nodes()
            .get(node)
            .ok_or(BodyCheckInternalError::MissingNode(node))?
            .ty();
        let mut result = self.map_call_target(
            call,
            callable_value.as_ref(),
            callable_environment.as_ref(),
            receiver.as_ref(),
            &arguments,
            result_type,
        )?;
        if !type_can_carry_loan(self.graph, self.types, result_type) {
            result = LoanValue::independent();
        }
        Ok((result, true))
    }

    fn evaluate_call_receiver(
        &mut self,
        node: BodyNodeId,
        call: &CheckedCall,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<Option<InvocationLoan>, BodyCheckError> {
        let Some(receiver) = call.receiver() else {
            return Ok(None);
        };
        self.evaluate_receiver(node, 0, receiver, state, extra)
    }

    pub(super) fn evaluate_receiver(
        &mut self,
        node: BodyNodeId,
        position: u16,
        receiver: &crate::CheckedReceiver,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<Option<InvocationLoan>, BodyCheckError> {
        let value = match receiver.preparation() {
            ReceiverPreparation::BorrowPlace(capability) => {
                let place = self.place_node(receiver.value())?;
                self.evaluate_place_indices(place, state, extra)?;
                let carried = self.read_place(place, state)?;
                let loans = self.issue_loan_as(
                    LoanId::Operand { node, position },
                    node,
                    place,
                    capability,
                    state,
                    extra,
                )?;
                state.set_node(receiver.value(), carried.clone());
                InvocationLoan {
                    carried,
                    place: Some(loans),
                }
            }
            ReceiverPreparation::BorrowTemporary(_)
            | ReceiverPreparation::Owned
            | ReceiverPreparation::PreserveBorrow(_)
            | ReceiverPreparation::WeakenReadwriteBorrow => {
                let (value, reaches) = self.evaluate(receiver.value(), state, extra)?;
                if !reaches {
                    return Ok(None);
                }
                InvocationLoan::carried(value)
            }
        };
        Ok(Some(value))
    }

    fn evaluate_call_arguments(
        &mut self,
        call: &CheckedCall,
        receiver: Option<&InvocationLoan>,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<Option<Vec<InvocationLoan>>, BodyCheckError> {
        let mut invocation_active = extra.clone();
        if let Some(receiver) = receiver {
            receiver.extend_active(&mut invocation_active);
        }
        let mut arguments = Vec::with_capacity(call.arguments().len());
        for argument in call.arguments() {
            let (value, reaches) = self.evaluate(*argument, state, &invocation_active)?;
            if !reaches {
                return Ok(None);
            }
            let checked = self
                .input
                .body
                .nodes()
                .get(*argument)
                .ok_or(BodyCheckInternalError::MissingNode(*argument))?;
            let argument = match checked.operation() {
                CheckedOperation::Borrow { place, .. } => InvocationLoan {
                    carried: self.read_place(*place, state)?,
                    place: Some(value),
                },
                _ => InvocationLoan::carried(value),
            };
            argument.extend_active(&mut invocation_active);
            arguments.push(argument);
        }
        if let Some(pack) = call.pack() {
            let mut elements =
                pack.forwarded_parameter()
                    .map_or_else(LoanValue::independent, |parameter| {
                        state.value(&LiveSlot::Place(LivePlace::from_parts(
                            PlaceRoot::Parameter(parameter),
                            Box::new([]),
                        )))
                    });
            if pack.forwarded_parameter().is_none() {
                for segment in pack.segments() {
                    match segment {
                        crate::ArgumentPackSegment::Value(value) => {
                            let (carried, reaches) =
                                self.evaluate(*value, state, &invocation_active)?;
                            if !reaches {
                                return Ok(None);
                            }
                            let checked = self
                                .input
                                .body
                                .nodes()
                                .get(*value)
                                .ok_or(BodyCheckInternalError::MissingNode(*value))?;
                            let argument = match checked.operation() {
                                CheckedOperation::Borrow { place, .. } => InvocationLoan {
                                    carried: self.read_place(*place, state)?,
                                    place: Some(carried),
                                },
                                _ => InvocationLoan::carried(carried),
                            };
                            elements.union_with(argument.retained(true));
                            argument.extend_active(&mut invocation_active);
                        }
                        crate::ArgumentPackSegment::Spread {
                            mode, iteration, ..
                        } => {
                            let (iterator, reaches) =
                                self.evaluate(iteration.iterator(), state, &invocation_active)?;
                            if !reaches {
                                return Ok(None);
                            }
                            let contribution = mode
                                .contribution_type(self.types, iteration.item())
                                .ok_or(BodyCheckInternalError::LoanAnalysis)?;
                            if type_can_carry_loan(self.graph, self.types, contribution) {
                                let item = self.iteration_item_loans(iteration, &iterator)?;
                                invocation_active.extend(item.all_loans());
                                elements.union_with(&item);
                            }
                        }
                    }
                }
            }
            arguments.push(InvocationLoan::carried(elements));
        }
        Ok(Some(arguments))
    }

    fn map_call_target(
        &self,
        call: &CheckedCall,
        callable_value: Option<&LoanValue>,
        callable_environment: Option<&LoanValue>,
        receiver: Option<&InvocationLoan>,
        arguments: &[InvocationLoan],
        result_type: nocter_model::TypeId,
    ) -> Result<LoanValue, BodyCheckError> {
        Ok(match call.target() {
            CallTarget::Static(selection) => {
                let callable = match selection.dispatch() {
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
                        return Err(BodyCheckInternalError::LoanAnalysis.into());
                    }
                };
                self.map_callable_result(callable, receiver, arguments)?
            }
            CallTarget::CallableValue { dispatch, .. } => {
                let StaticDispatch::StructuralRequirement { evidence } = dispatch.dispatch() else {
                    return Err(BodyCheckInternalError::LoanAnalysis.into());
                };
                let contract = self
                    .capability_evidence
                    .get(evidence)
                    .and_then(|evidence| match evidence.predicate() {
                        crate::CheckedPredicate::Callable { contract, .. } => Some(contract),
                        _ => None,
                    })
                    .ok_or(BodyCheckInternalError::LoanAnalysis)?;
                let mut result = LoanValue::independent();
                let retain_place =
                    invocation_place_can_reach_result(self.graph, self.types, result_type);
                for origin in contract.provenance().origins() {
                    let argument = arguments
                        .get(origin.position())
                        .ok_or(BodyCheckInternalError::LoanAnalysis)?;
                    result.union_with(&argument.retained(retain_place).flattened());
                }
                result.union_with(
                    &callable_value
                        .ok_or(BodyCheckInternalError::LoanAnalysis)?
                        .flattened(),
                );
                if retain_place && let Some(environment) = callable_environment {
                    result.union_with(&environment.flattened());
                }
                result
            }
            CallTarget::ClosureValue { closure, .. } => {
                let summary = self
                    .provenance
                    .closures()
                    .get(*closure)
                    .ok_or(BodyCheckInternalError::LoanAnalysis)?;
                let callable = callable_value.ok_or(BodyCheckInternalError::LoanAnalysis)?;
                let mut result = LoanValue::independent();
                let retain_place =
                    invocation_place_can_reach_result(self.graph, self.types, result_type);
                for origin in summary.parameters().origins() {
                    let argument = arguments
                        .get(origin.position())
                        .ok_or(BodyCheckInternalError::LoanAnalysis)?;
                    result.union_with(&argument.retained(retain_place).flattened());
                }
                for capture in summary.captures() {
                    let capture = callable
                        .projected(crate::ProvenanceProjection::ClosureCaptureValue(*capture));
                    result.union_with(&capture.flattened());
                }
                if summary.retains_environment() {
                    result.union_with(
                        &callable_environment
                            .ok_or(BodyCheckInternalError::LoanAnalysis)?
                            .flattened(),
                    );
                }
                result
            }
        })
    }

    pub(super) fn map_callable_result(
        &self,
        callable: CallableId,
        receiver: Option<&InvocationLoan>,
        arguments: &[InvocationLoan],
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
                    let receiver = receiver.ok_or(BodyCheckInternalError::LoanAnalysis)?;
                    result.union_with(
                        &receiver
                            .retained(invocation_place_can_reach_result(
                                self.graph,
                                self.types,
                                declaration.result(),
                            ))
                            .flattened(),
                    );
                }
                ProvenanceOrigin::Parameter(parameter) => {
                    let position = declaration
                        .parameters()
                        .iter()
                        .position(|candidate| candidate == parameter)
                        .ok_or(BodyCheckInternalError::LoanAnalysis)?;
                    let argument = arguments
                        .get(position)
                        .ok_or(BodyCheckInternalError::LoanAnalysis)?;
                    result.union_with(
                        &argument
                            .retained(invocation_place_can_reach_result(
                                self.graph,
                                self.types,
                                declaration.result(),
                            ))
                            .flattened(),
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
