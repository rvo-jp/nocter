use nocter_declarations::{CallableDeclaration, DeclarationGraph, ParameterRole};
use nocter_model::{
    ArgumentPackType, CallableCapability, CallableId, GenericParameterId,
    InterfaceImplementationId, ParameterId, TypeId, TypeKind,
};

use super::build::InterfaceImplementationInternalError;
use super::predicate::{CheckedPredicate, normalize_requirements};
use crate::type_relations::TypeSubstitution;

/// One ordinary parameter in the exact signature required by a interface implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequiredInterfaceImplementationParameter {
    declaration: ParameterId,
    ty: TypeId,
    argument_pack: Option<ArgumentPackType>,
}

impl RequiredInterfaceImplementationParameter {
    #[must_use]
    pub const fn declaration(self) -> ParameterId {
        self.declaration
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn argument_pack(self) -> Option<ArgumentPackType> {
        self.argument_pack
    }
}

/// The canonical, owner-specialized signature required for one missing interface implementation method.
///
/// This value is captured while interface implementation selection owns the authoritative substitution. Tooling
/// therefore never needs to repeat dispatch rules or recover a signature from diagnostic text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequiredInterfaceImplementationMethod {
    interface_implementation: InterfaceImplementationId,
    interface_method: CallableId,
    receiver: CallableCapability,
    generic_parameters: Box<[GenericParameterId]>,
    parameters: Box<[RequiredInterfaceImplementationParameter]>,
    result: TypeId,
    requirements: Box<[CheckedPredicate]>,
}

impl RequiredInterfaceImplementationMethod {
    pub(super) fn build(
        graph: &DeclarationGraph,
        types: &mut nocter_model::TypeTransaction,
        interface_implementation: InterfaceImplementationId,
        interface_method: CallableId,
        expected: &CallableDeclaration,
        owner_substitution: &TypeSubstitution,
    ) -> Result<Self, InterfaceImplementationInternalError> {
        let declarations = graph.declarations();
        let receiver_id =
            expected
                .receiver()
                .ok_or(InterfaceImplementationInternalError::MissingCallable(
                    interface_method,
                ))?;
        let receiver = declarations
            .parameters()
            .get(receiver_id)
            .and_then(|parameter| match parameter.role() {
                ParameterRole::Receiver(capability) => Some(capability),
                ParameterRole::Ordinary { .. } | ParameterRole::ArgumentPack { .. } => None,
            })
            .ok_or(InterfaceImplementationInternalError::MissingParameter(
                receiver_id,
            ))?;

        let mut substitution = owner_substitution.clone();
        for parameter in expected.generic_parameters() {
            let ty = types
                .intern(TypeKind::GenericParameter(*parameter))
                .map_err(|_| {
                    InterfaceImplementationInternalError::InvalidGenericType(*parameter)
                })?;
            substitution.bind_generic(*parameter, ty);
        }

        let parameters = expected
            .parameters()
            .iter()
            .map(|id| {
                let parameter = declarations
                    .parameters()
                    .get(*id)
                    .ok_or(InterfaceImplementationInternalError::MissingParameter(*id))?;
                let argument_pack = match parameter.role() {
                    ParameterRole::Ordinary { .. } => None,
                    ParameterRole::ArgumentPack { .. } => parameter
                        .argument_pack()
                        .map(|pack| pack.try_map(|ty| substitution.apply_type(types, ty)))
                        .transpose()?,
                    ParameterRole::Receiver(_) => {
                        return Err(InterfaceImplementationInternalError::MissingParameter(*id));
                    }
                };
                Ok(RequiredInterfaceImplementationParameter {
                    declaration: *id,
                    ty: substitution.apply_type(types, parameter.ty())?,
                    argument_pack,
                })
            })
            .collect::<Result<Vec<_>, InterfaceImplementationInternalError>>()?;
        let result = substitution.apply_type(types, expected.result())?;
        let requirements =
            normalize_requirements(graph, types, &substitution, expected.requirements())?
                .into_iter()
                .map(|requirement| requirement.predicate().clone())
                .collect::<Vec<_>>();

        Ok(Self {
            interface_implementation,
            interface_method,
            receiver,
            generic_parameters: expected.generic_parameters().into(),
            parameters: parameters.into_boxed_slice(),
            result,
            requirements: requirements.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn interface_implementation(&self) -> InterfaceImplementationId {
        self.interface_implementation
    }

    #[must_use]
    pub const fn interface_method(&self) -> CallableId {
        self.interface_method
    }

    #[must_use]
    pub const fn receiver(&self) -> CallableCapability {
        self.receiver
    }

    #[must_use]
    pub const fn generic_parameters(&self) -> &[GenericParameterId] {
        &self.generic_parameters
    }

    #[must_use]
    pub const fn parameters(&self) -> &[RequiredInterfaceImplementationParameter] {
        &self.parameters
    }

    #[must_use]
    pub const fn result(&self) -> TypeId {
        self.result
    }

    #[must_use]
    pub const fn requirements(&self) -> &[CheckedPredicate] {
        &self.requirements
    }
}
