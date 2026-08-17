use nocter_checking::{CallTarget, CheckedCall};
use nocter_model::{BodyNodeId, MirValueId, TypeId};
use nocter_target_program::{ExecutableDispatchPlan, ExecutableDispatchStep};

use super::MirLoweringError;
use super::callable_environment::CallableEnvironmentPlan;
use super::function::FunctionLowerer;
use crate::MirCallTarget;

impl FunctionLowerer<'_> {
    pub(super) fn lower_callable_value_call(
        &mut self,
        node: BodyNodeId,
        ty: TypeId,
        call: &CheckedCall,
    ) -> Result<MirValueId, MirLoweringError> {
        let CallTarget::CallableValue {
            value,
            capability,
            dispatch,
        } = call.target()
        else {
            return Err(MirLoweringError::InvalidCallable(node));
        };
        let invocation = match self.item.body().dispatch(dispatch) {
            Some(ExecutableDispatchPlan::Invocation(ExecutableDispatchStep::CallableValue(
                invocation,
            ))) => invocation.clone(),
            _ => return Err(MirLoweringError::InvalidCallable(node)),
        };
        let layout = self
            .executable
            .closure_layout(invocation.body())
            .cloned()
            .ok_or(MirLoweringError::InvalidCallable(node))?;
        let signature = self
            .executable
            .items()
            .get(invocation.body())
            .ok_or(MirLoweringError::InvalidCallable(node))?
            .signature();
        let Some(environment_ty) = signature.inputs().first().map(|input| input.ty()) else {
            return Err(MirLoweringError::InvalidCallable(node));
        };
        let source = self
            .body
            .nodes()
            .get(*value)
            .ok_or(MirLoweringError::UnknownNode(*value))?;
        if self.concrete_type(source.ty())? != invocation.subject()
            || invocation.contract().capability() != *capability
            || invocation.contract().parameters().len() != signature.inputs().len() - 1
            || invocation
                .contract()
                .parameters()
                .iter()
                .copied()
                .zip(signature.inputs()[1..].iter().map(|input| input.ty()))
                .any(|(expected, actual)| expected != actual)
            || invocation.contract().result() != ty
            || signature.result() != ty
            || signature.inputs().len() != call.arguments().len() + 1
        {
            return Err(MirLoweringError::InvalidCallable(node));
        }

        let environment = self.prepare_callable_environment(
            node,
            *value,
            CallableEnvironmentPlan {
                source: *capability,
                body: layout.capability(),
                closure_ty: layout.ty(),
                environment_ty,
            },
        )?;
        let explicit_arguments = call
            .arguments()
            .iter()
            .map(|argument| self.require_value(*argument))
            .collect::<Result<Vec<_>, _>>()?;
        let (environment, retained) = self.finalize_callable_environment(environment)?;
        let mut arguments = Vec::with_capacity(signature.inputs().len());
        arguments.push(environment);
        arguments.extend(explicit_arguments);
        let result = self.emit_call(ty, MirCallTarget::Direct(invocation.body()), arguments)?;
        if self.current.is_some()
            && let Some((source, place)) = retained
        {
            if let Some(plan) = invocation.post_call_destruction() {
                self.lower_destruction(node, place, plan)?;
            }
            self.deactivate_value_storage(source)?;
        }
        Ok(result)
    }
}
