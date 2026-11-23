//! Freeze ordinary input operations and formal-to-actual witness bindings at the selected call.
use super::{
    Failure, Rule,
    checker::Checker,
    ir::{Argument, ArgumentAction, Boundary, WitnessBinding},
};
use nocter_declarations::value_access::{FunctionInput, FunctionWitness, PayloadSchema};
use nocter_model::{
    AccessExpressionId, TypeId, ValueAccessMode as Mode, ValueFunctionOrigin, ValueParameter,
};
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
            let (data, parameter) = self.input_callable_data(syntax, value, witness)?;
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
            parameter.map(|parameter| WitnessBinding { parameter, witness })
        } else {
            // Keep supplied semantic identities, including through newly injected outcomes.
            expected.data = self.supplied_constraint_data(syntax, value, expected.data)?;
            None
        };
        if self
            .ty(value)
            .access()
            .is_some_and(|actual| actual.data() != expected.data)
            && matches!(
                self.program().data_schema(expected.data),
                Some(PayloadSchema::Optional(_) | PayloadSchema::Fallible(_))
            )
        {
            if expected.operation == nocter_model::ValueInputOperation::Edit
                && self.expression_referent(value).is_some()
            {
                return self.reject(Rule::InvalidUpdate, syntax);
            }
            value = self.boundary(syntax, value, Boundary::Value(expected.data))?;
        }
        self.input_plan(syntax, value, expected, binding)
    }
    fn input_callable_data(
        &mut self,
        syntax: NodeId,
        value: AccessExpressionId,
        witness: FunctionWitness,
    ) -> Result<(TypeId, Option<ValueParameter>), Failure> {
        match witness.origin {
            ValueFunctionOrigin::Parameter(parameter) => {
                let constraint = self
                    .program()
                    .function_contract(witness)
                    .ok_or(Failure::InconsistentSyntax)?;
                self.function_constraint_data(syntax, value, &constraint, false)
                    .map(|data| (data, Some(parameter)))
            }
            ValueFunctionOrigin::Contract(_) => {
                let actual = self
                    .ty(value)
                    .access()
                    .ok_or(Failure::InconsistentSyntax)?
                    .data();
                let matched = self.program().constrain_callable_contract(actual, witness);
                self.finish_function_constraint(syntax, actual, matched, false)
                    .map(|data| (data, None))
            }
            _ => Err(Failure::InconsistentSyntax),
        }
    }
    fn deferred_transfer(
        &self,
        mut value: AccessExpressionId,
    ) -> Option<(super::Referent, Vec<AccessExpressionId>)> {
        let mut adapters = Vec::new();
        loop {
            match self.expressions.get(value)?.kind {
                super::ir::ExpressionKind::Move(place) => {
                    adapters.reverse();
                    return Some((place, adapters));
                }
                super::ir::ExpressionKind::AdaptFunction { source } => {
                    adapters.push(value);
                    value = source;
                }
                super::ir::ExpressionKind::Aggregate { ref fields, .. }
                    if matches!(
                        fields.as_ref(),
                        [super::ir::FieldValue {
                            field: nocter_model::ValueField::Variant(true),
                            ..
                        }]
                    ) && matches!(
                        self.program().data_schema(self.ty(value).access()?.data()),
                        Some(PayloadSchema::Optional(_) | PayloadSchema::Fallible(_))
                    ) =>
                {
                    adapters.push(value);
                    value = fields[0].value;
                }
                super::ir::ExpressionKind::Boundary {
                    source,
                    action: super::ir::BoundaryAction::TakeValue,
                } => value = source,
                _ => return None,
            }
        }
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
            let transfer = self.deferred_transfer(value);
            let place = self.expression_referent(value);
            if transfer.is_none()
                && mode == Mode::Update
                && let Some(referent) = place
            {
                self.require_writable_referent(referent, syntax, Rule::InvalidUpdate)?;
            }
            super::checker::payload_requirements::data(
                self.program(),
                self.syntax(),
                actual.data(),
                nocter_syntax::SyntaxOrigin::Node(syntax),
            )?;
            if let Some((place, adapters)) = transfer {
                ArgumentAction::ReserveTransfer { place, adapters }
            } else if let Some(place) = place {
                match mode {
                    Mode::Read => ArgumentAction::ReserveRead(place),
                    Mode::Update => ArgumentAction::ReserveUpdate(place),
                    Mode::Own => {
                        use super::supply::PlaceSupply;
                        match super::supply::select(self.program(), actual.data())? {
                            PlaceSupply::Pending => ArgumentAction::PendingValue(place),
                            PlaceSupply::Copy => ArgumentAction::CopyPlace(place),
                            PlaceSupply::MissingCopy => {
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
                    Mode::Read | Mode::Update => ArgumentAction::AccessOwnedValue,
                    Mode::Own => ArgumentAction::TakeValue,
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
