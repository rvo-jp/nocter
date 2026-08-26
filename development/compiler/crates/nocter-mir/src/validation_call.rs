use nocter_model::{BorrowCapability, BuiltinType, MirOperationId, MirValueId, TypeId, TypeKind};

use crate::validation_pack::validate_call_pack;
use crate::validation_region::validate_region_selection;
use crate::{
    MirBody, MirCall, MirCallAllocation, MirCallTarget, MirPrimitiveDependency, MirStructuralCall,
    MirValidationEnvironment, MirValidationError,
};

pub(crate) fn validate_call(
    environment: &(impl MirValidationEnvironment + ?Sized),
    function: &MirBody,
    operation: MirOperationId,
    call: &MirCall,
    result: TypeId,
) -> Result<(), MirValidationError> {
    CallValidation {
        environment,
        function,
        operation,
        result,
    }
    .validate(call)
}

struct CallValidation<'a, E: ?Sized> {
    environment: &'a E,
    function: &'a MirBody,
    operation: MirOperationId,
    result: TypeId,
}

impl<E: MirValidationEnvironment + ?Sized> CallValidation<'_, E> {
    fn validate(&self, call: &MirCall) -> Result<(), MirValidationError> {
        self.validate_allocation(call)?;
        let target = call.target();
        let arguments = call.arguments();
        match target {
            MirCallTarget::Direct(item) => {
                if !self.environment.contains_item(*item) {
                    return Err(MirValidationError::UnknownItem(*item));
                }
            }
            MirCallTarget::StandardPrimitive {
                role,
                type_arguments,
                signature,
                dependency,
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
                self.validate_primitive_dependency(*role, type_arguments, dependency)?;
            }
            MirCallTarget::Structural(structural) => {
                self.validate_structural(structural, arguments)?;
            }
        }
        validate_call_pack(self.environment, self.function, self.operation, call)?;
        Ok(())
    }

    fn validate_primitive_dependency(
        &self,
        role: nocter_runtime_contract::PrimitiveRole,
        type_arguments: &[TypeId],
        dependency: &MirPrimitiveDependency,
    ) -> Result<(), MirValidationError> {
        match (role, dependency) {
            (
                nocter_runtime_contract::PrimitiveRole::DropValueAtPointer,
                MirPrimitiveDependency::Destruction { subject, plan },
            ) if type_arguments == [*subject] => {
                self.require_type(*subject)?;
                if let Some(plan) = plan {
                    if plan.ty() != *subject {
                        return Err(self.invalid());
                    }
                    crate::validation_destruction::validate_destruction_plan(
                        self.environment,
                        plan,
                    )?;
                }
                Ok(())
            }
            (nocter_runtime_contract::PrimitiveRole::DropValueAtPointer, _)
            | (_, MirPrimitiveDependency::Destruction { .. }) => Err(self.invalid()),
            (_, MirPrimitiveDependency::None) => Ok(()),
        }
    }

    fn validate_allocation(&self, call: &MirCall) -> Result<(), MirValidationError> {
        let place = match call.allocation() {
            MirCallAllocation::Inherit => return Ok(()),
            MirCallAllocation::Region(region) => {
                return validate_region_selection(
                    self.environment,
                    self.function,
                    self.operation,
                    region,
                );
            }
            MirCallAllocation::Explicit(place) => place,
        };
        let MirCallTarget::Direct(item) = call.target() else {
            return Err(self.invalid());
        };
        if !self.environment.item_accepts_allocation_override(*item) {
            return Err(self.invalid());
        }
        let ty = self
            .function
            .places()
            .get(place)
            .ok_or(MirValidationError::UnknownPlace(place))?
            .ty();
        let referent = match self.environment.types().get(ty) {
            Some(TypeKind::Borrow { referent, .. }) => *referent,
            Some(_) => ty,
            None => return Err(self.invalid()),
        };
        let Some(TypeKind::Nominal {
            definition,
            arguments,
        }) = self.environment.types().get(referent)
        else {
            return Err(self.invalid());
        };
        let valid_role = [
            self.environment.aborting_allocator_nominal(),
            self.environment.allocation_context_nominal(),
        ]
        .into_iter()
        .flatten()
        .any(|expected| expected == *definition);
        if !arguments.is_empty() || !valid_role {
            return Err(self.invalid());
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
