//! Freeze ordinary input operations and formal-to-actual witness bindings at the selected call.
use super::{
    Failure, Rule,
    checker::Checker,
    ir::{Argument, ArgumentAction, Boundary, WitnessBinding},
};
use nocter_declarations::value_access::{FunctionInput, PayloadSchema};
use nocter_model::{AccessExpressionId, ValueAccessMode as Mode, ValueFunctionOrigin};
use nocter_syntax::NodeId;
impl Checker<'_, '_> {
    pub(super) fn prepare_input(
        &mut self,
        syntax: NodeId,
        mut value: AccessExpressionId,
        mut expected: FunctionInput,
    ) -> Result<Argument, Failure> {
        // An unknown-shaped terminating input supplies no environment and no witness binding.
        // Known data still goes through contract matching even when normal completion is absent.
        if self.ty(value).access().is_none() {
            return self.input_plan(syntax, value, expected, None);
        }
        let binding = if let Some(PayloadSchema::Function(witness)) =
            self.program().data_schema(expected.data)
        {
            let ValueFunctionOrigin::Parameter(parameter) = witness.origin else {
                return Err(Failure::InconsistentSyntax);
            };
            let constraint = self
                .program()
                .function_contract(witness)
                .ok_or(Failure::InconsistentSyntax)?;
            let data = self.function_constraint_data(syntax, value, &constraint, false)?;
            if self
                .ty(value)
                .access()
                .is_some_and(|actual| actual.data() != data)
            {
                value = self.boundary(syntax, value, Boundary::Value(data))?;
            }
            expected.data = data;
            let Some(PayloadSchema::Function(witness)) = self.program().data_schema(data) else {
                return Err(Failure::InconsistentSyntax);
            };
            Some(WitnessBinding { parameter, witness })
        } else {
            None
        };
        self.input_plan(syntax, value, expected, binding)
    }
    pub(super) fn input_plan(
        &self,
        syntax: NodeId,
        value: AccessExpressionId,
        expected: FunctionInput,
        binding: Option<WitnessBinding>,
    ) -> Result<Argument, Failure> {
        self.require_data(value, expected.data, syntax)?;
        let mode = expected.operation.access_mode();
        let action = if let Some(actual) = self.ty(value).normal() {
            if let Some(place) = self.place_expression(value) {
                match mode {
                    Mode::Read => ArgumentAction::ReserveRead(place),
                    Mode::Update => {
                        if !self.writable_place(place) {
                            return self.reject_local(
                                Rule::InvalidUpdate,
                                syntax,
                                self.selected_place(place).local,
                            );
                        }
                        ArgumentAction::ReserveUpdate(place)
                    }
                    Mode::Own => {
                        use super::supply::PlaceSupply;
                        match super::supply::select(
                            self.program(),
                            actual.data(),
                            self.place_ty(place).mode(),
                        )? {
                            PlaceSupply::Pending => ArgumentAction::PendingValue(place),
                            PlaceSupply::Copy => ArgumentAction::CopyPlace(place),
                            PlaceSupply::Transfer => ArgumentAction::ReserveTransfer(place),
                            PlaceSupply::MissingOwner => {
                                return self.reject(Rule::MissingTransfer, syntax);
                            }
                        }
                    }
                }
            } else {
                if actual.mode() != Mode::Own {
                    return Err(Failure::InconsistentSyntax);
                }
                match mode {
                    Mode::Read => ArgumentAction::ReadOwnedValue,
                    Mode::Own => ArgumentAction::TakeValue,
                    Mode::Update => return self.reject(Rule::InvalidUpdate, syntax),
                }
            }
        } else {
            ArgumentAction::Interrupted
        };
        Ok(Argument {
            value,
            operation: expected.operation,
            binding,
            action,
        })
    }
}
