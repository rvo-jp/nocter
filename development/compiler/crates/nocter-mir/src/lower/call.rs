use nocter_checking::{CallTarget, CheckedCall};
use nocter_model::{BodyNodeId, BuiltinType, MirValueId, TypeId, TypeKind};
use nocter_target_program::ExecutableDispatchStep;

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{MirCall, MirCallTarget, MirOperationKind, MirTerminator};

impl FunctionLowerer<'_> {
    pub(super) fn lower_call(
        &mut self,
        node: BodyNodeId,
        ty: TypeId,
        call: &CheckedCall,
    ) -> Result<MirValueId, MirLoweringError> {
        let CallTarget::Static(selection) = call.target() else {
            return Err(MirLoweringError::UnsupportedOperation(node));
        };
        if call.receiver().is_some() {
            return Err(MirLoweringError::UnsupportedOperation(node));
        }
        let plan = self
            .item
            .body()
            .dispatch(selection)
            .ok_or(MirLoweringError::InvalidDispatch(node))?;
        let [ExecutableDispatchStep::Direct(callee)] = plan.steps() else {
            return Err(MirLoweringError::UnsupportedOperation(node));
        };
        let arguments = call
            .arguments()
            .iter()
            .map(|argument| self.require_value(*argument))
            .collect::<Result<Vec<_>, _>>()?;
        let value = self.append_value(
            ty,
            MirOperationKind::Call(MirCall::new(MirCallTarget::Direct(*callee), arguments)),
        )?;
        if self.executable.types().get(ty) == Some(&TypeKind::Builtin(BuiltinType::Never)) {
            let block = self.current.ok_or(MirLoweringError::MissingCurrentBlock)?;
            self.builder.terminate(block, MirTerminator::Unreachable)?;
            self.current = None;
        }
        Ok(value)
    }
}
