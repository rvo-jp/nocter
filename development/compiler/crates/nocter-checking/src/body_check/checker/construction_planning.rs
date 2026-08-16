use nocter_model::{GenericParameterId, NominalTypeId, TypeId, TypeKind};
use nocter_syntax::NodeId;

use super::BodyChecker;
use super::type_uses::{NominalConstructionOwner, NominalOwnerArguments};
use crate::GenericArguments;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::type_relations::TypeSubstitution;

pub(super) struct NominalConstructionPlan {
    pub(super) definition: NominalTypeId,
    pub(super) result_pattern: TypeId,
    pub(super) inference_parameters: Box<[GenericParameterId]>,
    pub(super) substitution: TypeSubstitution,
}

impl BodyChecker<'_, '_> {
    pub(super) fn nominal_construction_plan(
        &mut self,
        node: NodeId,
        owner: NominalConstructionOwner,
    ) -> Result<NominalConstructionPlan, BodyCheckError> {
        let declaration = self
            .graph
            .declarations()
            .nominal_types()
            .get(owner.definition)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let parameters = declaration.generic_parameters().to_vec();
        let (arguments, inference_parameters, substitution) = match owner.arguments {
            NominalOwnerArguments::Inferred(inference_parameters) => {
                let arguments = inference_parameters
                    .iter()
                    .copied()
                    .map(|parameter| {
                        self.types
                            .intern(TypeKind::GenericParameter(parameter))
                            .map_err(|_| BodyCheckInternalError::InvalidSyntax(node))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                (arguments, inference_parameters, TypeSubstitution::default())
            }
            NominalOwnerArguments::Fixed(arguments) => {
                if parameters.len() != arguments.len() {
                    return Err(self.rule(BodyRule::InvalidConstruction, node)?);
                }
                let mut substitution = TypeSubstitution::default();
                for (parameter, argument) in
                    parameters.iter().copied().zip(arguments.iter().copied())
                {
                    substitution.bind_generic(parameter, argument);
                }
                (
                    arguments.into_vec(),
                    Box::<[GenericParameterId]>::default(),
                    substitution,
                )
            }
        };
        if parameters.len() != arguments.len() {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        }
        let result_pattern = self
            .types
            .intern(TypeKind::Nominal {
                definition: owner.definition,
                arguments: arguments.into_boxed_slice(),
            })
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(node))?;
        Ok(NominalConstructionPlan {
            definition: owner.definition,
            result_pattern,
            inference_parameters,
            substitution,
        })
    }

    pub(super) fn nominal_construction_requirements_hold(
        &mut self,
        definition: NominalTypeId,
        substitution: &TypeSubstitution,
    ) -> Result<bool, BodyCheckError> {
        let requirements = self
            .graph
            .declarations()
            .nominal_types()
            .get(definition)
            .ok_or(BodyCheckInternalError::InvalidSyntax(self.source.block()))?
            .requirements()
            .to_vec();
        self.requirements_hold(&requirements, substitution)
    }
}

pub(super) fn bind_inferred_arguments(
    substitution: &mut TypeSubstitution,
    arguments: &GenericArguments,
) {
    for argument in arguments.as_slice() {
        substitution.bind_generic(argument.parameter(), argument.ty());
    }
}
