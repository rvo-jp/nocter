use nocter_checking::{
    BorrowConversionImplementation, BorrowConversionPreparation, CheckedBorrowConversion,
};
use nocter_model::{BodyNodeId, MirValueId, TypeId};
use nocter_target_program::ExecutableDispatchPlan;

use super::MirLoweringError;
use super::function::FunctionLowerer;

impl FunctionLowerer<'_> {
    pub(super) fn lower_borrow_conversion(
        &mut self,
        node: BodyNodeId,
        conversion: &CheckedBorrowConversion,
    ) -> Result<MirValueId, MirLoweringError> {
        let target = self.concrete_type(conversion.target())?;
        match conversion.implementation() {
            BorrowConversionImplementation::CapabilityWeakening => {
                let value = self.lower_place_carrier(conversion.value())?;
                self.weaken_borrow(value, target)
            }
            BorrowConversionImplementation::Selected(selection) => {
                let step = {
                    let plan = self
                        .item
                        .body()
                        .dispatch(selection)
                        .ok_or(MirLoweringError::InvalidDispatch(node))?;
                    let ExecutableDispatchPlan::Invocation(step) = plan else {
                        return Err(MirLoweringError::UnsupportedOperation(node));
                    };
                    step.clone()
                };
                let signature = self.step_signature(&step)?;
                let [input] = signature.parameters() else {
                    return Err(MirLoweringError::InvalidDispatch(node));
                };
                if signature.result() != target {
                    return Err(MirLoweringError::InvalidDispatch(node));
                }
                let value = self.lower_conversion_input(
                    conversion.value(),
                    conversion.preparation(),
                    *input,
                )?;
                self.emit_dispatch_step(node, target, &step, [value])
            }
        }
    }

    fn lower_conversion_input(
        &mut self,
        node: BodyNodeId,
        preparation: BorrowConversionPreparation,
        expected: TypeId,
    ) -> Result<MirValueId, MirLoweringError> {
        let value = self.lower_place_carrier(node)?;
        match preparation {
            BorrowConversionPreparation::PreserveReadonly
            | BorrowConversionPreparation::PreserveReadwrite => {
                if self.builder.value_type(value) == Some(expected) {
                    Ok(value)
                } else {
                    Err(MirLoweringError::InvalidDispatch(node))
                }
            }
            BorrowConversionPreparation::WeakenReadwrite => self.weaken_borrow(value, expected),
        }
    }
}
