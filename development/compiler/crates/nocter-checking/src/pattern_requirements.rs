use nocter_declarations::{DeclarationGraph, RequirementKind};
use nocter_model::{GenericParameterId, RequirementId, TypeId, TypeKind, TypeStore};

use crate::GenericArgument;
use crate::type_relations::{SubstitutionError, TypeSubstitution};

/// The directed binder refinements and retained capability predicates of one declaration pattern.
///
/// Binder refinements shape applicability and are consumed into the normalized target. They are
/// not runtime predicates. Every other requirement remains available for proof after a concrete
/// pattern substitution has been selected.
pub(crate) struct PatternRequirements {
    refinements: Box<[(GenericParameterId, TypeId)]>,
    retained: Box<[RequirementId]>,
}

impl PatternRequirements {
    pub(crate) fn collect(
        graph: &DeclarationGraph,
        requirements: &[RequirementId],
    ) -> Result<Self, SubstitutionError> {
        let mut refinements = Vec::new();
        let mut retained = Vec::new();
        for requirement_id in requirements {
            let requirement = graph
                .declarations()
                .requirements()
                .get(*requirement_id)
                .ok_or(SubstitutionError::InvalidStore)?;
            match requirement.kind() {
                RequirementKind::BinderRefinement {
                    parameter,
                    replacement,
                } => refinements.push((*parameter, *replacement)),
                _ => retained.push(*requirement_id),
            }
        }
        refinements.sort_unstable_by_key(|(parameter, _)| *parameter);
        if refinements.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(SubstitutionError::InvalidStore);
        }
        Ok(Self {
            refinements: refinements.into_boxed_slice(),
            retained: retained.into_boxed_slice(),
        })
    }

    pub(crate) fn substitution(&self) -> TypeSubstitution {
        let mut substitution = TypeSubstitution::default();
        for (parameter, replacement) in &self.refinements {
            substitution.bind_generic(*parameter, *replacement);
        }
        substitution
    }

    pub(crate) const fn retained(&self) -> &[RequirementId] {
        &self.retained
    }

    /// Materializes the declaration's directed binder refinements in canonical parameter order.
    ///
    /// Keeping this representation beside the normalized target lets both operation selection and
    /// body checking reconstruct the same lexical generic environment without reinterpreting the
    /// authored requirement list.
    pub(crate) fn normalized_refinements(
        &self,
        types: &mut TypeStore,
        generic_parameters: &[GenericParameterId],
    ) -> Result<Vec<GenericArgument>, SubstitutionError> {
        let substitution = self.substitution();
        let mut normalized = Vec::with_capacity(self.refinements.len());
        for (parameter, _) in &self.refinements {
            if !generic_parameters.contains(parameter) {
                return Err(SubstitutionError::InvalidStore);
            }
            let generic = types
                .intern(TypeKind::GenericParameter(*parameter))
                .map_err(|_| SubstitutionError::InvalidStore)?;
            normalized.push(GenericArgument::new(
                *parameter,
                substitution.apply_type(types, generic)?,
            ));
        }
        normalized.sort_unstable_by_key(|argument| argument.parameter());
        Ok(normalized)
    }
}
