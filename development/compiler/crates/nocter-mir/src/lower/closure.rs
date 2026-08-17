use nocter_checking::{CallTarget, CaptureMode, CheckedCall, CheckedClosure, CheckedOperation};
use nocter_model::{
    BodyNodeId, BorrowCapability, CallableCapability, CaptureId, MirValueId, TypeId, TypeKind,
};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use super::place::LoweredPlacePath;
use crate::{
    MirAggregate, MirCallTarget, MirClosureCapture, MirOperationKind, MirPlaceRoot,
    MirProjectionKind, MirReadMode,
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
        let place = self.lower_place_node(*value)?;
        let environment = match capability {
            CallableCapability::Readonly => {
                self.borrow_place(place, BorrowCapability::Readonly, environment_ty)?
            }
            CallableCapability::ReadWrite => {
                self.borrow_place(place, BorrowCapability::ReadWrite, environment_ty)?
            }
            CallableCapability::Owned => {
                let checked_value = *value;
                let environment = self.append_value(
                    layout.ty(),
                    MirOperationKind::Read {
                        place,
                        mode: MirReadMode::Move,
                    },
                )?;
                let checked = self
                    .body
                    .nodes()
                    .get(checked_value)
                    .ok_or(MirLoweringError::UnknownNode(checked_value))?;
                let CheckedOperation::Place(place) = checked.operation() else {
                    return Err(MirLoweringError::InvalidClosure(node));
                };
                self.mark_place_initialized(*place, false)?;
                environment
            }
        };
        let mut arguments = Vec::with_capacity(signature.inputs().len());
        arguments.push(environment);
        arguments.extend(
            call.arguments()
                .iter()
                .map(|argument| self.require_value(*argument))
                .collect::<Result<Vec<_>, _>>()?,
        );
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
