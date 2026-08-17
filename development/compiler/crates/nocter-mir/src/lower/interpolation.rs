use nocter_checking::{CheckedInterpolation, InterpolationPart};
use nocter_model::{
    BodyNodeId, BorrowCapability, BuiltinType, MirPlaceId, MirValueId, TypeId, TypeKind,
};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{MirConstant, MirOperationKind};

impl FunctionLowerer<'_> {
    /// Lowers interpolation exclusively through its frozen standard-library operations.
    ///
    /// The partially built output lives in the interpolation node's canonical temporary. Outcome
    /// propagation from any embedded expression can therefore run the checker's cleanup plan
    /// against the same storage without teaching MIR about `String`'s representation.
    pub(super) fn lower_interpolation(
        &mut self,
        node: BodyNodeId,
        node_ty: TypeId,
        interpolation: &CheckedInterpolation,
    ) -> Result<Option<MirValueId>, MirLoweringError> {
        let output_ty = self.concrete_type(interpolation.output())?;
        let never = self.executable.types().builtin(BuiltinType::Never);
        let diverges = interpolation
            .parts()
            .iter()
            .any(|part| matches!(part, InterpolationPart::Diverging(_)));
        if (diverges && node_ty != never) || (!diverges && node_ty != output_ty) {
            return Err(MirLoweringError::InvalidInterpolation(node));
        }

        let allocation = self.lower_call_allocation(interpolation.allocation())?;
        let constructor = self.invocation_step(node, interpolation.constructor())?;
        let signature = self.step_signature(&constructor)?;
        if !signature.parameters().is_empty() || signature.result() != output_ty {
            return Err(MirLoweringError::InvalidInterpolation(node));
        }
        let output =
            self.emit_dispatch_step_with_allocation(node, output_ty, &constructor, [], allocation)?;
        let output_place = self.materialize_value_storage(node, output)?;

        for part in interpolation.parts() {
            match part {
                InterpolationPart::Text(text) => {
                    self.append_interpolation_text(
                        node,
                        output_ty,
                        output_place,
                        interpolation,
                        text,
                    )?;
                }
                InterpolationPart::Formatted { operand, formatter } => {
                    let plan = self.invocation_plan(node, formatter)?;
                    let signature = self.step_signature(&plan.step)?;
                    let [receiver_ty, formatter_output_ty] = signature.parameters() else {
                        return Err(MirLoweringError::InvalidInterpolation(node));
                    };
                    if signature.result() != self.executable.types().builtin(BuiltinType::Void)
                        || !self.is_borrow_of(
                            *formatter_output_ty,
                            BorrowCapability::ReadWrite,
                            output_ty,
                        )
                    {
                        return Err(MirLoweringError::InvalidInterpolation(node));
                    }
                    let source_ty = plan.opaque_receiver.map_or(
                        *receiver_ty,
                        nocter_target_program::ExecutableOpaqueReceiver::source,
                    );
                    let receiver = self.lower_readonly_operand(
                        node,
                        operand.value(),
                        operand.preparation(),
                        source_ty,
                    )?;
                    let receiver = if let Some(opaque) = plan.opaque_receiver {
                        self.lower_opaque_receiver(
                            node,
                            operand.value(),
                            receiver,
                            opaque,
                            *receiver_ty,
                        )?
                    } else {
                        receiver
                    };
                    let output = self.borrow_place(
                        output_place,
                        BorrowCapability::ReadWrite,
                        *formatter_output_ty,
                    )?;
                    self.emit_dispatch_step(
                        node,
                        signature.result(),
                        &plan.step,
                        [receiver, output],
                    )?;
                }
                InterpolationPart::Diverging(value) => {
                    let _ = self.lower_node(*value)?;
                    if self.current.is_some() {
                        return Err(MirLoweringError::InvalidInterpolation(node));
                    }
                    return Ok(None);
                }
            }
        }

        self.take_value_storage(node, output_ty).map(Some)
    }

    fn append_interpolation_text(
        &mut self,
        node: BodyNodeId,
        output_ty: TypeId,
        output_place: MirPlaceId,
        interpolation: &CheckedInterpolation,
        text: &str,
    ) -> Result<(), MirLoweringError> {
        let appender = self.invocation_step(node, interpolation.text_appender())?;
        let signature = self.step_signature(&appender)?;
        let [receiver_ty, text_ty] = signature.parameters() else {
            return Err(MirLoweringError::InvalidInterpolation(node));
        };
        if signature.result() != self.executable.types().builtin(BuiltinType::Void)
            || !self.is_borrow_of(*receiver_ty, BorrowCapability::ReadWrite, output_ty)
            || !matches!(
                self.executable.types().get(*text_ty),
                Some(TypeKind::Borrow {
                    capability: BorrowCapability::Readonly,
                    referent,
                }) if *referent == self.executable.types().builtin(BuiltinType::Str)
            )
        {
            return Err(MirLoweringError::InvalidInterpolation(node));
        }
        let receiver =
            self.borrow_place(output_place, BorrowCapability::ReadWrite, *receiver_ty)?;
        let text = self.append_value(
            *text_ty,
            MirOperationKind::Constant(MirConstant::Text(text.into())),
        )?;
        self.emit_dispatch_step(node, signature.result(), &appender, [receiver, text])?;
        Ok(())
    }

    fn is_borrow_of(&self, ty: TypeId, capability: BorrowCapability, referent: TypeId) -> bool {
        matches!(
            self.executable.types().get(ty),
            Some(TypeKind::Borrow {
                capability: actual,
                referent: actual_referent,
            }) if *actual == capability && *actual_referent == referent
        )
    }
}
