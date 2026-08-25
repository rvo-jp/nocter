use nocter_declarations::{CallableKind, ExpansionCapability, ParameterRole};
use nocter_model::{CallableCapability, TypeId};

use super::selection::{InstanceOperationSelector, InstanceSelectionError};
use crate::conformance::normalize_requirements;
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
        self.assumptions
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
                            StaticDispatch::StructuralRequirement(assumption.declaration()),
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
            let members = self
                .table
                .entries()
                .get(&applicable.instance)
                .ok_or(InstanceSelectionError::MissingInstance(applicable.instance))?
                .members()
                .to_vec();
            for member in members {
                let callable = self
                    .graph
                    .declarations()
                    .callables()
                    .get(member)
                    .ok_or(InstanceSelectionError::MissingCallable(member))?;
                if callable.kind() != CallableKind::Expansion
                    || !self.callable_is_admissible(callable.site())?
                {
                    continue;
                }
                let receiver = callable
                    .receiver()
                    .and_then(|receiver| self.graph.declarations().parameters().get(receiver))
                    .ok_or(InstanceSelectionError::InvalidExpansionSignature(member))?;
                if receiver.role() != ParameterRole::Receiver(callable_capability(capability))
                    || !callable.parameters().is_empty()
                    || !callable.generic_parameters().is_empty()
                {
                    continue;
                }
                let result = applicable
                    .substitution
                    .apply_type(self.types, callable.result())?;
                let requirements = normalize_requirements(
                    self.graph,
                    self.types,
                    &applicable.substitution,
                    callable.requirements(),
                )?;
                if !self.requirements_hold(&requirements, &TypeSubstitution::default())? {
                    continue;
                }
                selected.push(ExpansionCandidate {
                    result,
                    selection: StaticSelection::new(
                        StaticDispatch::Direct(member),
                        applicable.generic_arguments.clone(),
                    ),
                });
            }
        }
        Ok(selected)
    }
}

const fn callable_capability(capability: ExpansionCapability) -> CallableCapability {
    match capability {
        ExpansionCapability::Readonly => CallableCapability::Readonly,
        ExpansionCapability::ReadWrite => CallableCapability::ReadWrite,
        ExpansionCapability::Owned => CallableCapability::Owned,
    }
}
