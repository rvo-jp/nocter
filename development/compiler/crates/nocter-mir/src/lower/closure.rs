use nocter_checking::{CallTarget, CaptureMode, CheckedCall, CheckedClosure};
use nocter_model::{BodyNodeId, CaptureId, MirValueId, TypeId, TypeKind};

use super::MirLoweringError;
use super::callable_environment::CallableEnvironmentPlan;
use super::function::FunctionLowerer;
use super::place::LoweredPlacePath;
use crate::{
    MirAggregate, MirCallTarget, MirClosureCapture, MirOperationKind, MirPlaceRoot,
    MirProjectionKind,
};

impl FunctionLowerer<'_> {
    pub(super) fn lower_closure(
        &mut self,
        node: BodyNodeId,
        ty: TypeId,
        closure: &CheckedClosure,
    ) -> Result<MirValueId, MirLoweringError> {
        let body = self
            .item
            .body()
            .closure_item(closure.closure())
            .ok_or(MirLoweringError::InvalidClosure(node))?;
        let layout = self
            .executable
            .closure_layout(body)
            .cloned()
            .ok_or(MirLoweringError::InvalidClosure(node))?;
        if layout.closure() != closure.closure()
            || layout.ty() != ty
            || layout.captures().len() != closure.captures().len()
        {
            return Err(MirLoweringError::InvalidClosure(node));
        }
        let captures = closure
            .captures()
            .iter()
            .copied()
            .zip(layout.captures().iter().copied())
            .map(|(capture, expected)| {
                if capture.binding() != expected.binding() {
                    return Err(MirLoweringError::InvalidClosure(node));
                }
                let value = self.require_value(capture.initializer())?;
                if self.builder.value_type(value) != Some(expected.ty()) {
                    return Err(MirLoweringError::InvalidClosure(node));
                }
                Ok(MirClosureCapture::new(capture.binding(), value))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.append_value(
            ty,
            MirOperationKind::Aggregate(MirAggregate::Closure {
                body,
                captures: captures.into_boxed_slice(),
            }),
        )
    }

    pub(super) fn lower_closure_call(
        &mut self,
        node: BodyNodeId,
        ty: TypeId,
        call: &CheckedCall,
    ) -> Result<MirValueId, MirLoweringError> {
        let CallTarget::ClosureValue {
            value,
            closure,
            capability,
        } = call.target()
        else {
            return Err(MirLoweringError::InvalidClosure(node));
        };
        let body = self
            .item
            .body()
            .closure_item(*closure)
            .ok_or(MirLoweringError::InvalidClosure(node))?;
        let layout = self
            .executable
            .closure_layout(body)
            .ok_or(MirLoweringError::InvalidClosure(node))?;
        let signature = self
            .executable
            .items()
            .get(body)
            .ok_or(MirLoweringError::InvalidClosure(node))?
            .signature();
        let Some(environment_ty) = signature.inputs().first().map(|input| input.ty()) else {
            return Err(MirLoweringError::InvalidClosure(node));
        };
        if layout.closure() != *closure
            || layout.capability() != *capability
            || signature.result() != ty
            || signature.inputs().len() != call.arguments().len() + 1
        {
            return Err(MirLoweringError::InvalidClosure(node));
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
        if retained.is_some() {
            return Err(MirLoweringError::InvalidClosure(node));
        }
        let mut arguments = Vec::with_capacity(signature.inputs().len());
        arguments.push(environment);
        arguments.extend(explicit_arguments);
        self.emit_call(ty, MirCallTarget::Direct(body), arguments)
    }

    /// Reifies the hidden closure-environment representation beneath one checked capture root.
    pub(super) fn lower_capture_path(
        &mut self,
        capture: CaptureId,
    ) -> Result<LoweredPlacePath, MirLoweringError> {
        let layout = self
            .item
            .closure_layout()
            .ok_or(MirLoweringError::InvalidCapture(capture))?;
        let environment = *self
            .closure_environments
            .get(&layout.closure())
            .ok_or(MirLoweringError::InvalidCapture(capture))?;
        let environment_ty = self
            .builder
            .local_type(environment)
            .ok_or(MirLoweringError::InvalidCapture(capture))?;
        let mut path = LoweredPlacePath {
            root: MirPlaceRoot::Local(environment),
            projections: Vec::new(),
            ty: environment_ty,
        };
        match self.executable.types().get(environment_ty) {
            Some(TypeKind::Borrow {
                capability,
                referent,
            }) if *referent == layout.ty() => {
                path.push(MirProjectionKind::BorrowDereference(*capability), *referent);
            }
            Some(_) if environment_ty == layout.ty() => {}
            _ => return Err(MirLoweringError::InvalidCapture(capture)),
        }
        let stored = layout
            .capture(capture)
            .ok_or(MirLoweringError::InvalidCapture(capture))?;
        path.push(MirProjectionKind::ClosureCapture(capture), stored.ty());

        let checked = self
            .body
            .captures()
            .get(capture)
            .copied()
            .ok_or(MirLoweringError::InvalidCapture(capture))?;
        let exposed = self.concrete_type(checked.ty())?;
        match checked.declaration().mode() {
            CaptureMode::Readonly | CaptureMode::ReadWrite => {
                let Some(TypeKind::Borrow {
                    capability,
                    referent,
                }) = self.executable.types().get(stored.ty())
                else {
                    return Err(MirLoweringError::InvalidCapture(capture));
                };
                if *referent != exposed {
                    return Err(MirLoweringError::InvalidCapture(capture));
                }
                path.push(MirProjectionKind::BorrowDereference(*capability), exposed);
            }
            CaptureMode::Move if stored.ty() == exposed => {}
            CaptureMode::Move => return Err(MirLoweringError::InvalidCapture(capture)),
        }
        Ok(path)
    }
}
