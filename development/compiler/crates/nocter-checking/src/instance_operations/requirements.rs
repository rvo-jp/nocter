use nocter_model::{BorrowCapability, TypeId};

use super::selection::{
    InstanceOperationSelector, InstanceSelectionError, borrow_result, builtin_index_result,
    retain_direct_candidates,
};
use crate::conformance::{proves_predicate, substitute_predicate};
use crate::type_relations::TypeSubstitution;
use crate::{CheckedPredicate, CheckedRequirement, ComparisonOperation, Copyability};

impl InstanceOperationSelector<'_> {
    pub(crate) fn requirements_hold(
        &mut self,
        requirements: &[CheckedRequirement],
        substitution: &TypeSubstitution,
    ) -> Result<bool, InstanceSelectionError> {
        for requirement in requirements {
            let predicate =
                substitute_predicate(self.types, substitution, requirement.predicate())?;
            if !self.proves_requirement(&predicate, requirement.declaration())? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn proves_requirement(
        &mut self,
        predicate: &CheckedPredicate,
        declaration: nocter_model::RequirementId,
    ) -> Result<bool, InstanceSelectionError> {
        if self
            .assumptions
            .iter()
            .any(|assumption| assumption.predicate() == predicate)
        {
            return Ok(true);
        }
        if !self.active.insert(predicate.clone()) {
            return Ok(false);
        }
        let proven = match predicate {
            CheckedPredicate::Copy(ty) => {
                self.copyabilities
                    .classify(self.graph, self.types, *ty)
                    .map_err(InstanceSelectionError::Copyability)?
                    == Copyability::Copy
            }
            CheckedPredicate::Index {
                capability,
                container,
                index,
                result,
            } => self.proves_index(*container, *index, *result, *capability, declaration)?,
            CheckedPredicate::Coercion { source, target } => {
                self.proves_coercion(*source, *target)?
            }
            CheckedPredicate::Equality(ty) => {
                self.proves_comparison(*ty, ComparisonOperation::Equal)?
            }
            CheckedPredicate::Ordering(ty) => {
                self.proves_comparison(*ty, ComparisonOperation::Less)?
            }
            _ => proves_predicate(self.types, self.conformances, self.assumptions, predicate)?,
        };
        self.active.remove(predicate);
        Ok(proven)
    }

    fn proves_index(
        &mut self,
        container: TypeId,
        index: TypeId,
        result: TypeId,
        capability: BorrowCapability,
        declaration: nocter_model::RequirementId,
    ) -> Result<bool, InstanceSelectionError> {
        let (result_capability, referent) = borrow_result(self.types, result)
            .ok_or(InstanceSelectionError::InvalidStructuralIndex(declaration))?;
        if result_capability != capability {
            return Err(InstanceSelectionError::InvalidStructuralIndex(declaration));
        }
        if let Some(builtin_result) = builtin_index_result(self.types, container, capability) {
            return Ok(
                index == self.types.builtin(nocter_model::BuiltinType::Usize)
                    && referent == builtin_result,
            );
        }
        let mut candidates = self.select_index_operations(container, capability)?;
        candidates.extend(self.select_coerced_index_operations(container, capability)?);
        retain_direct_candidates(&mut candidates);
        candidates.retain(|candidate| candidate.index() == index && candidate.result() == referent);
        Ok(candidates.len() == 1)
    }

    fn proves_coercion(
        &mut self,
        source: TypeId,
        target: TypeId,
    ) -> Result<bool, InstanceSelectionError> {
        let Some((source_capability, source)) = borrow_result(self.types, source) else {
            return Ok(false);
        };
        let Some((target_capability, target)) = borrow_result(self.types, target) else {
            return Ok(false);
        };
        if source_capability != target_capability {
            return Ok(false);
        }
        let candidates = self.select_coercions(source, source_capability)?;
        Ok(candidates
            .iter()
            .filter(|candidate| candidate.target == target)
            .count()
            == 1)
    }

    fn proves_comparison(
        &mut self,
        ty: TypeId,
        operation: ComparisonOperation,
    ) -> Result<bool, InstanceSelectionError> {
        Ok(self.select_comparison_operations(ty, ty, operation)?.len() == 1)
    }
}
