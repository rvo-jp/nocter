use nocter_checking::CheckedOperation;
use nocter_model::{
    BodyNodeId, BorrowCapability, CallableCapability, MirPlaceId, MirValueId, TypeId,
};

use super::MirLoweringError;
use super::function::FunctionLowerer;

#[derive(Clone, Copy)]
pub(super) struct CallableEnvironmentPlan {
    pub(super) source: CallableCapability,
    pub(super) body: CallableCapability,
    pub(super) closure_ty: TypeId,
    pub(super) environment_ty: TypeId,
}

#[derive(Clone, Copy)]
pub(super) enum PreparedCallableEnvironment {
    Ready {
        value: MirValueId,
    },
    BorrowStaged {
        node: BodyNodeId,
        place: MirPlaceId,
        capability: BorrowCapability,
        environment_ty: TypeId,
    },
    Transfer {
        node: BodyNodeId,
        ty: TypeId,
    },
}

impl FunctionLowerer<'_> {
    /// Evaluates a callable operand before its arguments and preserves owned state in addressable
    /// storage until every argument has completed successfully.
    pub(super) fn prepare_callable_environment(
        &mut self,
        owner: BodyNodeId,
        source_node: BodyNodeId,
        plan: CallableEnvironmentPlan,
    ) -> Result<PreparedCallableEnvironment, MirLoweringError> {
        if !plan.source.permits(plan.body) {
            return Err(MirLoweringError::InvalidCallable(owner));
        }
        if plan.source != CallableCapability::Owned {
            let place = self.lower_place_node(source_node)?;
            let capability =
                borrow_capability(plan.body).ok_or(MirLoweringError::InvalidCallable(owner))?;
            let value = self.borrow_place(place, capability, plan.environment_ty)?;
            return Ok(PreparedCallableEnvironment::Ready { value });
        }

        let checked = self
            .body
            .nodes()
            .get(source_node)
            .ok_or(MirLoweringError::UnknownNode(source_node))?;
        let CheckedOperation::Place(checked_place) = checked.operation() else {
            return Err(MirLoweringError::InvalidCallable(owner));
        };
        let storage = self.stage_moved_place(source_node, *checked_place, plan.closure_ty)?;
        if plan.body == CallableCapability::Owned {
            if plan.environment_ty != plan.closure_ty {
                return Err(MirLoweringError::InvalidCallable(owner));
            }
            return Ok(PreparedCallableEnvironment::Transfer {
                node: source_node,
                ty: plan.closure_ty,
            });
        }

        let capability =
            borrow_capability(plan.body).ok_or(MirLoweringError::InvalidCallable(owner))?;
        Ok(PreparedCallableEnvironment::BorrowStaged {
            node: source_node,
            place: storage,
            capability,
            environment_ty: plan.environment_ty,
        })
    }

    /// Performs the ownership transfer only after all later call operands have succeeded.
    pub(super) fn finalize_callable_environment(
        &mut self,
        prepared: PreparedCallableEnvironment,
    ) -> Result<(MirValueId, Option<(BodyNodeId, MirPlaceId)>), MirLoweringError> {
        match prepared {
            PreparedCallableEnvironment::Ready { value } => Ok((value, None)),
            PreparedCallableEnvironment::BorrowStaged {
                node,
                place,
                capability,
                environment_ty,
            } => self
                .borrow_place(place, capability, environment_ty)
                .map(|value| (value, Some((node, place)))),
            PreparedCallableEnvironment::Transfer { node, ty } => {
                self.take_value_storage(node, ty).map(|value| (value, None))
            }
        }
    }
}

const fn borrow_capability(capability: CallableCapability) -> Option<BorrowCapability> {
    match capability {
        CallableCapability::Readonly => Some(BorrowCapability::Readonly),
        CallableCapability::ReadWrite => Some(BorrowCapability::ReadWrite),
        CallableCapability::Owned => None,
    }
}
