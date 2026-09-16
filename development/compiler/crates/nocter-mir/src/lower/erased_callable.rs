use nocter_checking::CheckedCallableErasure;
use nocter_model::{BodyNodeId, TypeId};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{MirCallSignature, MirErasedCallable, MirOperationKind};

impl FunctionLowerer<'_> {
    pub(super) fn lower_callable_erasure(
        &mut self,
        node: BodyNodeId,
        ty: TypeId,
        erasure: CheckedCallableErasure,
    ) -> Result<nocter_model::MirValueId, MirLoweringError> {
        let descriptor = self
            .item
            .body()
            .erased_callable(node)
            .ok_or(MirLoweringError::InvalidCallable(node))?;
        if descriptor.ty() != ty {
            return Err(MirLoweringError::InvalidCallable(node));
        }
        let Some(nocter_model::TypeKind::Callable(callable)) = self.executable.types().get(ty)
        else {
            return Err(MirLoweringError::InvalidCallable(node));
        };
        let environment = self.require_value(erasure.value())?;
        let actual = self
            .body
            .nodes()
            .get(erasure.value())
            .ok_or(MirLoweringError::UnknownNode(erasure.value()))?;
        if self.concrete_type(actual.ty())? != descriptor.environment() {
            return Err(MirLoweringError::InvalidCallable(node));
        }
        let destruction = descriptor
            .environment_destruction()
            .map(|plan| self.lower_deferred_destruction(node, plan))
            .transpose()?;
        self.append_value(
            ty,
            MirOperationKind::EraseCallable(MirErasedCallable::new(
                environment,
                descriptor.environment(),
                descriptor.body(),
                descriptor.capability(),
                descriptor.source_capability(),
                MirCallSignature::new(callable.parameters().to_vec(), callable.result()),
                destruction,
            )),
        )
    }
}
