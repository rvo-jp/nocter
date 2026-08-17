use nocter_checking::{AllocationSelection, StaticSelection};
use nocter_model::{BodyNodeId, BorrowCapability, BuiltinType, MirValueId, TypeId, TypeKind};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{MirConstant, MirOperationKind};

impl FunctionLowerer<'_> {
    pub(super) fn lower_typed_string(
        &mut self,
        node: BodyNodeId,
        ty: TypeId,
        constructor: &StaticSelection,
        text: &str,
        allocation: AllocationSelection,
    ) -> Result<MirValueId, MirLoweringError> {
        // The explicit context is evaluated before the literal payload and becomes active only for
        // the constructor call.
        let allocation = self.lower_call_allocation(allocation)?;
        let step = self.invocation_step(node, constructor)?;
        let signature = self.step_signature(&step)?;
        let [text_ty] = signature.parameters() else {
            return Err(MirLoweringError::InvalidStringLiteral(node));
        };
        if signature.result() != ty
            || !matches!(
                self.executable.types().get(*text_ty),
                Some(TypeKind::Borrow {
                    capability: BorrowCapability::Readonly,
                    referent,
                }) if *referent == self.executable.types().builtin(BuiltinType::Str)
            )
        {
            return Err(MirLoweringError::InvalidStringLiteral(node));
        }
        let text = self.append_value(
            *text_ty,
            MirOperationKind::Constant(MirConstant::Text(text.into())),
        )?;
        self.emit_dispatch_step_with_allocation(node, ty, &step, [text], allocation)
    }
}
