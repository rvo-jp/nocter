use std::collections::BTreeMap;

use nocter_declarations::{
    AssociatedTypeBinding, CallableOwner, DeclarationGraph, InterfaceApplication,
};
use nocter_model::{ConformanceId, InstanceId, TypeStore};

use crate::conformance::normalize_requirements;
use crate::pattern_requirements::PatternRequirements;
use crate::type_relations::{SubstitutionError, TypeSubstitution};
use crate::{CheckedRequirement, GenericArgument};

/// Normalized predicates visible inside one declaration container's bodies.
#[derive(Debug)]
pub(crate) struct LexicalPattern {
    refinements: Box<[GenericArgument]>,
    requirements: Box<[CheckedRequirement]>,
}

impl LexicalPattern {
    pub(crate) const fn refinements(&self) -> &[GenericArgument] {
        &self.refinements
    }

    pub(crate) const fn requirements(&self) -> &[CheckedRequirement] {
        &self.requirements
    }
}

/// One normalized instance pattern shared by lexical body analysis and global operation indexing.
#[derive(Debug)]
pub(crate) struct InstanceDeclarationPattern {
    lexical: LexicalPattern,
    substitution: TypeSubstitution,
    target: nocter_model::TypeId,
}

impl InstanceDeclarationPattern {
    pub(crate) const fn lexical(&self) -> &LexicalPattern {
        &self.lexical
    }

    pub(crate) const fn substitution(&self) -> &TypeSubstitution {
        &self.substitution
    }

    pub(crate) const fn target(&self) -> nocter_model::TypeId {
        self.target
    }
}

/// One normalized conformance pattern shared by lexical body analysis and dispatch validation.
#[derive(Debug)]
pub(crate) struct ConformanceDeclarationPattern {
    lexical: LexicalPattern,
    substitution: TypeSubstitution,
    interface: InterfaceApplication,
    target: nocter_model::TypeId,
    associated_types: Box<[AssociatedTypeBinding]>,
}

impl ConformanceDeclarationPattern {
    pub(crate) const fn lexical(&self) -> &LexicalPattern {
        &self.lexical
    }

    pub(crate) const fn substitution(&self) -> &TypeSubstitution {
        &self.substitution
    }

    pub(crate) const fn interface(&self) -> &InterfaceApplication {
        &self.interface
    }

    pub(crate) const fn target(&self) -> nocter_model::TypeId {
        self.target
    }

    pub(crate) const fn associated_types(&self) -> &[AssociatedTypeBinding] {
        &self.associated_types
    }
}

/// Sole normalization authority for instance and conformance declaration patterns.
///
/// Every structurally valid container receives lexical facts. Program-wide operation builders
/// consume the same normalized entry only when `AdmittedOperations` supplies its identity, so
/// quarantine neither erases local semantics nor triggers a second normalization pass.
#[derive(Debug)]
pub(crate) struct DeclarationPatternTable {
    instances: BTreeMap<InstanceId, InstanceDeclarationPattern>,
    conformances: BTreeMap<ConformanceId, ConformanceDeclarationPattern>,
}

impl DeclarationPatternTable {
    pub(crate) fn build(
        graph: &DeclarationGraph,
        types: &mut TypeStore,
    ) -> Result<Self, SubstitutionError> {
        let declarations = graph.declarations();
        let mut instances = BTreeMap::new();
        for (id, declaration) in declarations.instances().iter() {
            let (lexical, substitution) = normalize_pattern(
                graph,
                types,
                declaration.requirements(),
                declaration.generic_parameters(),
            )?;
            let target = substitution.apply_type(types, declaration.target())?;
            instances.insert(
                id,
                InstanceDeclarationPattern {
                    lexical,
                    substitution,
                    target,
                },
            );
        }

        let mut conformances = BTreeMap::new();
        for (id, declaration) in declarations.conformances().iter() {
            let (lexical, substitution) = normalize_pattern(
                graph,
                types,
                declaration.requirements(),
                declaration.generic_parameters(),
            )?;
            let interface = InterfaceApplication::new(
                declaration.interface().interface(),
                declaration
                    .interface()
                    .arguments()
                    .iter()
                    .map(|argument| substitution.apply_type(types, *argument))
                    .collect::<Result<Vec<_>, _>>()?,
            );
            let target = substitution.apply_type(types, declaration.target())?;
            let mut associated_types = declaration
                .associated_types()
                .iter()
                .map(|binding| {
                    substitution
                        .apply_type(types, binding.ty())
                        .map(|ty| AssociatedTypeBinding::new(binding.declaration(), ty))
                })
                .collect::<Result<Vec<_>, _>>()?;
            associated_types.sort_unstable_by_key(|binding| binding.declaration());
            conformances.insert(
                id,
                ConformanceDeclarationPattern {
                    lexical,
                    substitution,
                    interface,
                    target,
                    associated_types: associated_types.into_boxed_slice(),
                },
            );
        }
        Ok(Self {
            instances,
            conformances,
        })
    }

    pub(crate) fn instance(&self, id: InstanceId) -> Option<&InstanceDeclarationPattern> {
        self.instances.get(&id)
    }

    pub(crate) fn conformance(&self, id: ConformanceId) -> Option<&ConformanceDeclarationPattern> {
        self.conformances.get(&id)
    }

    pub(crate) fn lexical(&self, owner: CallableOwner) -> Option<&LexicalPattern> {
        match owner {
            CallableOwner::Instance(instance) => self
                .instance(instance)
                .map(InstanceDeclarationPattern::lexical),
            CallableOwner::Conformance(conformance) => self
                .conformance(conformance)
                .map(ConformanceDeclarationPattern::lexical),
            CallableOwner::Module(_)
            | CallableOwner::Construction(_)
            | CallableOwner::Interface(_) => None,
        }
    }
}

fn normalize_pattern(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    requirements: &[nocter_model::RequirementId],
    generic_parameters: &[nocter_model::GenericParameterId],
) -> Result<(LexicalPattern, TypeSubstitution), SubstitutionError> {
    let pattern = PatternRequirements::collect(graph, requirements)?;
    let substitution = pattern.substitution();
    let lexical = LexicalPattern {
        refinements: pattern
            .normalized_refinements(types, generic_parameters)?
            .into_boxed_slice(),
        requirements: normalize_requirements(graph, types, &substitution, pattern.retained())?
            .into_boxed_slice(),
    };
    Ok((lexical, substitution))
}
