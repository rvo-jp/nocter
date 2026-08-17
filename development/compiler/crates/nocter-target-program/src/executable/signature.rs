use nocter_checking::{ConcreteDispatchResolver, TypeSubstitution};
use nocter_declarations::{Parameter, ParameterRole};
use nocter_model::{
    BodyId, BorrowCapability, BuiltinType, CallableCapability, ClosureId, LocalBindingId,
    ParameterId, TypeId, TypeKind,
};

use super::{ExecutableItemKey, ExecutableProgramError};
use crate::{CallableInstanceKey, ClosureInstanceKey, DropInstanceKey, TargetProgram};

/// The semantic binding initialized by one concrete function input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutableInputSource {
    Parameter(ParameterId),
    ClosureEnvironment(ClosureId),
    ClosureParameter(LocalBindingId),
}

/// One concrete runtime input and the checked binding it initializes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutableInput {
    source: ExecutableInputSource,
    ty: TypeId,
}

impl ExecutableInput {
    #[must_use]
    pub const fn source(self) -> ExecutableInputSource {
        self.source
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

/// One fully specialized runtime signature. No later stage applies generic substitution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableSignature {
    inputs: Box<[ExecutableInput]>,
    result: TypeId,
}

impl ExecutableSignature {
    #[must_use]
    pub const fn inputs(&self) -> &[ExecutableInput] {
        &self.inputs
    }

    #[must_use]
    pub const fn result(&self) -> TypeId {
        self.result
    }
}

pub(super) fn build_signature(
    target: &TargetProgram,
    resolver: &mut ConcreteDispatchResolver<'_>,
    key: &ExecutableItemKey,
    body: BodyId,
    root: nocter_model::BodyNodeId,
) -> Result<ExecutableSignature, ExecutableProgramError> {
    let substitution = item_substitution(key);
    match key {
        ExecutableItemKey::Callable(key) => {
            callable_signature(target, resolver, key, &substitution)
        }
        ExecutableItemKey::Closure(key) => closure_signature(target, resolver, key, &substitution),
        ExecutableItemKey::Drop(key) => drop_signature(target, resolver, key, &substitution),
        ExecutableItemKey::Test(_) => test_signature(target, resolver, body, root, &substitution),
    }
}

pub(super) fn callable_signature(
    target: &TargetProgram,
    resolver: &mut ConcreteDispatchResolver<'_>,
    key: &CallableInstanceKey,
    substitution: &TypeSubstitution,
) -> Result<ExecutableSignature, ExecutableProgramError> {
    let declarations = target.checked().graph().declarations();
    let callable = declarations
        .callables()
        .get(key.callable())
        .ok_or_else(|| {
            ExecutableProgramError::UnknownItem(ExecutableItemKey::Callable(key.clone()))
        })?;
    let inputs = callable
        .receiver()
        .iter()
        .chain(callable.parameters())
        .copied()
        .map(|parameter| {
            let declaration = declarations
                .parameters()
                .get(parameter)
                .copied()
                .ok_or(ExecutableProgramError::MissingParameter(parameter))?;
            Ok(ExecutableInput {
                source: ExecutableInputSource::Parameter(parameter),
                ty: runtime_parameter_type(resolver, declaration, substitution)?,
            })
        })
        .collect::<Result<Vec<_>, ExecutableProgramError>>()?;
    Ok(ExecutableSignature {
        inputs: inputs.into_boxed_slice(),
        result: resolver.specialize_type(callable.result(), substitution)?,
    })
}

fn closure_signature(
    target: &TargetProgram,
    resolver: &mut ConcreteDispatchResolver<'_>,
    key: &ClosureInstanceKey,
    substitution: &TypeSubstitution,
) -> Result<ExecutableSignature, ExecutableProgramError> {
    let definition = target
        .checked()
        .closures()
        .get(key.closure())
        .ok_or_else(|| {
            ExecutableProgramError::UnknownItem(ExecutableItemKey::Closure(key.clone()))
        })?;
    let environment = resolver.specialize_type(definition.ty(), substitution)?;
    let environment = match definition.signature().capability() {
        nocter_model::CallableCapability::Readonly => {
            resolver.intern_concrete(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent: environment,
            })?
        }
        nocter_model::CallableCapability::ReadWrite => {
            resolver.intern_concrete(TypeKind::Borrow {
                capability: BorrowCapability::ReadWrite,
                referent: environment,
            })?
        }
        nocter_model::CallableCapability::Owned => environment,
    };
    let mut inputs = vec![ExecutableInput {
        source: ExecutableInputSource::ClosureEnvironment(key.closure()),
        ty: environment,
    }];
    if definition.parameters().len() != definition.signature().parameters().len() {
        return Err(ExecutableProgramError::InvalidClosureSignature(
            key.closure(),
        ));
    }
    for (binding, ty) in definition
        .parameters()
        .iter()
        .copied()
        .zip(definition.signature().parameters().iter().copied())
    {
        inputs.push(ExecutableInput {
            source: ExecutableInputSource::ClosureParameter(binding),
            ty: resolver.specialize_type(ty, substitution)?,
        });
    }
    Ok(ExecutableSignature {
        inputs: inputs.into_boxed_slice(),
        result: resolver.specialize_type(definition.signature().result(), substitution)?,
    })
}

fn drop_signature(
    target: &TargetProgram,
    resolver: &mut ConcreteDispatchResolver<'_>,
    key: &DropInstanceKey,
    substitution: &TypeSubstitution,
) -> Result<ExecutableSignature, ExecutableProgramError> {
    let declarations = target.checked().graph().declarations();
    let declaration = declarations
        .drops()
        .get(key.drop())
        .ok_or_else(|| ExecutableProgramError::UnknownItem(ExecutableItemKey::Drop(key.clone())))?;
    let parameter = declarations
        .parameters()
        .get(declaration.receiver())
        .ok_or(ExecutableProgramError::MissingParameter(
            declaration.receiver(),
        ))?;
    Ok(ExecutableSignature {
        inputs: Box::new([ExecutableInput {
            source: ExecutableInputSource::Parameter(declaration.receiver()),
            ty: runtime_parameter_type(resolver, *parameter, substitution)?,
        }]),
        result: resolver.types().builtin(BuiltinType::Void),
    })
}

fn runtime_parameter_type(
    resolver: &mut ConcreteDispatchResolver<'_>,
    parameter: Parameter,
    substitution: &TypeSubstitution,
) -> Result<TypeId, ExecutableProgramError> {
    let owner = resolver.specialize_type(parameter.ty(), substitution)?;
    let capability = match parameter.role() {
        ParameterRole::Ordinary { .. } | ParameterRole::Receiver(CallableCapability::Owned) => {
            return Ok(owner);
        }
        ParameterRole::Receiver(CallableCapability::Readonly) => BorrowCapability::Readonly,
        ParameterRole::Receiver(CallableCapability::ReadWrite) => BorrowCapability::ReadWrite,
    };
    resolver
        .intern_concrete(TypeKind::Borrow {
            capability,
            referent: owner,
        })
        .map_err(Into::into)
}

fn test_signature(
    target: &TargetProgram,
    resolver: &mut ConcreteDispatchResolver<'_>,
    body: BodyId,
    root: nocter_model::BodyNodeId,
    substitution: &TypeSubstitution,
) -> Result<ExecutableSignature, ExecutableProgramError> {
    let checked = target
        .checked()
        .bodies()
        .get(body)
        .and_then(|body| body.nodes().get(root))
        .ok_or(ExecutableProgramError::MissingRoot(root))?;
    Ok(ExecutableSignature {
        inputs: Box::new([]),
        result: resolver.specialize_type(checked.ty(), substitution)?,
    })
}

fn item_substitution(key: &ExecutableItemKey) -> TypeSubstitution {
    match key {
        ExecutableItemKey::Callable(key) => key.substitution(),
        ExecutableItemKey::Closure(key) => key.substitution(),
        ExecutableItemKey::Drop(key) => key.substitution(),
        ExecutableItemKey::Test(_) => TypeSubstitution::default(),
    }
}
