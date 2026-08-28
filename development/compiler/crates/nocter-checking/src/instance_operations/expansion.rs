use nocter_declarations::ExpansionCapability;
use nocter_model::TypeId;

use super::CheckedInstanceMember;
use super::selection::{InstanceOperationSelector, InstanceSelectionError};
use crate::interface_implementation::normalize_requirements;
use crate::type_relations::TypeSubstitution;
use crate::{CheckedPredicate, GenericArguments, StaticDispatch, StaticSelection};

/// One exact expansion result and dispatch edge after source specialization.
pub(crate) struct ExpansionCandidate {
    result: TypeId,
    selection: StaticSelection,
}

impl ExpansionCandidate {
    pub(crate) const fn result(&self) -> TypeId {
        self.result
    }

    pub(crate) const fn selection(&self) -> &StaticSelection {
        &self.selection
    }
}

impl InstanceOperationSelector<'_> {
    /// Selects visible expansion declarations and exact lexical expansion requirements.
    ///
    /// This operation never considers borrow coercions. Expansion is its own source contract, and
    /// collection iteration or spread chooses its receiver capability before entering selection.
    pub(crate) fn select_expansions(
        &mut self,
        source: TypeId,
        capability: ExpansionCapability,
    ) -> Result<Vec<ExpansionCandidate>, InstanceSelectionError> {
        let mut selected = self.structural_expansions(source, capability);
        selected.extend(self.instance_expansions(source, capability)?);
        Ok(selected)
    }

    fn structural_expansions(
        &self,
        source: TypeId,
        capability: ExpansionCapability,
    ) -> Vec<ExpansionCandidate> {
        self.body_assumptions()
            .iter()
            .filter_map(|assumption| {
                let CheckedPredicate::Expansion {
                    capability: required,
                    source: required_source,
                    result,
                } = assumption.predicate()
                else {
                    return None;
                };
                (*required == capability && *required_source == source).then(|| {
                    ExpansionCandidate {
                        result: *result,
                        selection: StaticSelection::new(
                            StaticDispatch::StructuralRequirement {
                                evidence: assumption.evidence(),
                            },
                            GenericArguments::default(),
                        ),
                    }
                })
            })
            .collect()
    }

    fn instance_expansions(
        &mut self,
        source: TypeId,
        capability: ExpansionCapability,
    ) -> Result<Vec<ExpansionCandidate>, InstanceSelectionError> {
        let mut selected = Vec::new();
        for applicable in self.applicable_instances(source)? {
            let expansions = self
                .table
                .entries()
                .get(&applicable.instance)
                .ok_or(InstanceSelectionError::MissingInstance(applicable.instance))?
                .members()
                .iter()
                .filter_map(|member| match member {
                    CheckedInstanceMember::Expansion(expansion) => Some(expansion.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            for expansion in expansions {
                if expansion.capability() != capability
                    || !self.callable_is_admissible(expansion.site())?
                {
                    continue;
                }
                let result = applicable
                    .substitution
                    .apply_type(self.types, expansion.result())?;
                let requirements = normalize_requirements(
                    self.graph,
                    self.types,
                    &applicable.substitution,
                    expansion.requirements(),
                )?;
                if !self.requirements_hold(&requirements, &TypeSubstitution::default())? {
                    continue;
                }
                selected.push(ExpansionCandidate {
                    result,
                    selection: StaticSelection::new(
                        StaticDispatch::Direct(expansion.callable()),
                        applicable.generic_arguments.clone(),
                    ),
                });
            }
        }
        Ok(selected)
    }
}
