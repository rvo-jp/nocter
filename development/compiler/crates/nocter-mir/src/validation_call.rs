use nocter_model::{BorrowCapability, BuiltinType, MirOperationId, MirValueId, TypeId, TypeKind};

use crate::{
    MirCallTarget, MirFunction, MirStructuralCall, MirValidationEnvironment, MirValidationError,
};

pub(crate) fn validate_call(
    environment: &(impl MirValidationEnvironment + ?Sized),
    function: &MirFunction,
    operation: MirOperationId,
    target: &MirCallTarget,
    arguments: &[MirValueId],
    result: TypeId,
) -> Result<(), MirValidationError> {
    CallValidation {
        environment,
        function,
        operation,
        result,
    }
    .validate(target, arguments)
}

struct CallValidation<'a, E: ?Sized> {
    environment: &'a E,
    function: &'a MirFunction,
    operation: MirOperationId,
    result: TypeId,
}

impl<E: MirValidationEnvironment + ?Sized> CallValidation<'_, E> {
    fn validate(
        &self,
        target: &MirCallTarget,
        arguments: &[MirValueId],
    ) -> Result<(), MirValidationError> {
        match target {
            MirCallTarget::Direct(item) => {
                if !self.environment.contains_item(*item) {
                    return Err(MirValidationError::UnknownItem(*item));
                }
            }
            MirCallTarget::StandardPrimitive {
                type_arguments,
                signature,
                ..
            } => {
                for ty in type_arguments {
                    self.require_type(*ty)?;
                }
                for ty in signature.parameters() {
                    self.require_type(*ty)?;
                }
                self.require_type(signature.result())?;
                if arguments.len() != signature.parameters().len()
                    || arguments
                        .iter()
                        .copied()
                        .zip(signature.parameters().iter().copied())
                        .any(|(argument, expected)| self.value_type(argument) != Ok(expected))
                    || self.result != signature.result()
                {
                    return Err(self.invalid());
                }
            }
            MirCallTarget::Structural(structural) => {
                self.validate_structural(structural, arguments)?;
            }
            MirCallTarget::Indirect { callee, contract } => {
                self.value_type(*callee)?;
                if contract.parameters().len() != arguments.len()
                    || contract
                        .parameters()
                        .iter()
                        .copied()
                        .zip(arguments.iter().copied())
                        .any(|(expected, argument)| self.value_type(argument) != Ok(expected))
                    || self.result != contract.result()
                {
                    return Err(self.invalid());
                }
            }
        }
        Ok(())
    }

    fn validate_structural(
        &self,
        structural: &MirStructuralCall,
        arguments: &[MirValueId],
    ) -> Result<(), MirValidationError> {
        let types = self.environment.types();
        match structural {
            MirStructuralCall::Equality { subject, operand }
            | MirStructuralCall::Ordering { subject, operand } => {
                self.require_type(*subject)?;
                self.require_type(*operand)?;
                if arguments.len() != 2
                    || arguments
                        .iter()
                        .copied()
                        .any(|argument| self.value_type(argument) != Ok(*operand))
                    || !matches!(
                        types.get(*operand),
                        Some(TypeKind::Borrow {
                            capability: BorrowCapability::Readonly,
                            referent,
                        }) if referent == subject
                    )
                    || self.result != types.builtin(BuiltinType::Bool)
                {
                    return Err(self.invalid());
                }
            }
            MirStructuralCall::Index {
                container,
                receiver,
                index,
                result,
                capability,
            } => {
                for ty in [*container, *receiver, *index, *result] {
                    self.require_type(ty)?;
                }
                if arguments.len() != 2
                    || self.value_type(arguments[0])? != *receiver
                    || self.value_type(arguments[1])? != *index
                    || self.result != *result
                    || !matches!(
                        types.get(*receiver),
                        Some(TypeKind::Borrow {
                            capability: actual,
                            referent,
                        }) if actual == capability && referent == container
                    )
                {
                    return Err(self.invalid());
                }
            }
            MirStructuralCall::BorrowWeakening { source, target } => {
                if arguments.len() != 1
                    || self.value_type(arguments[0])? != *source
                    || self.result != *target
                    || !matches!(
                        (types.get(*source), types.get(*target)),
                        (
                            Some(TypeKind::Borrow {
                                capability: BorrowCapability::ReadWrite,
                                referent: source_referent,
                            }),
                            Some(TypeKind::Borrow {
                                capability: BorrowCapability::Readonly,
                                referent: target_referent,
                            })
                        ) if source_referent == target_referent
                    )
                {
                    return Err(self.invalid());
                }
            }
        }
        Ok(())
    }

    fn require_type(&self, ty: TypeId) -> Result<(), MirValidationError> {
        self.environment
            .types()
            .get(ty)
            .map(|_| ())
            .ok_or(MirValidationError::UnknownType(ty))
    }

    fn value_type(&self, value: MirValueId) -> Result<TypeId, MirValidationError> {
        self.function
            .values()
            .get(value)
            .copied()
            .map(crate::MirValue::ty)
            .ok_or(MirValidationError::UnknownValue(value))
    }

    const fn invalid(&self) -> MirValidationError {
        MirValidationError::OperationType(self.operation)
    }
}
