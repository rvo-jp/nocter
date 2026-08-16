use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;

use nocter_declarations::{DeclarationGraph, NominalShape, RequirementKind};
use nocter_model::{
    BorrowCapability, BuiltinType, FieldId, GenericParameterId, NominalTypeId, TypeId, TypeKind,
    TypeStore, VariantId,
};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceOrigin, SourceRole};

use crate::type_relations::{SubstitutionError, TypeSubstitution};

mod diagnostic;

pub use diagnostic::{CopyabilityBuildError, CopyabilityRule};

/// Compile-time proof that an ordinary value use may copy its source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Copyability {
    Copy,
    MoveOnly,
}

/// Normalized condition under which a structural type is copyable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CopyCondition {
    Always,
    Requires(BTreeSet<GenericParameterId>),
    Impossible,
}

impl CopyCondition {
    fn requiring(parameter: GenericParameterId) -> Self {
        Self::Requires(BTreeSet::from([parameter]))
    }

    fn conjoin(self, another: Self) -> Self {
        match (self, another) {
            (Self::Impossible, _) | (_, Self::Impossible) => Self::Impossible,
            (Self::Always, condition) | (condition, Self::Always) => condition,
            (Self::Requires(mut left), Self::Requires(right)) => {
                left.extend(right);
                Self::Requires(left)
            }
        }
    }

    const fn classification(&self) -> Copyability {
        match self {
            Self::Always => Copyability::Copy,
            Self::Requires(_) | Self::Impossible => Copyability::MoveOnly,
        }
    }
}

enum CopyabilityAction {
    Enter(TypeId),
    Finish {
        ty: TypeId,
        dependencies: Box<[TypeId]>,
    },
}

/// The sole program-wide authority for copy proofs and concrete type classifications.
///
/// Generic proof identities come from normalized `copy` requirements. Concrete structural facts
/// are memoized by canonical `TypeId`, then retained in `CheckedProgram` for later stages.
#[derive(Debug, Default)]
pub struct CopyabilityTable {
    parameters: BTreeSet<GenericParameterId>,
    conditions: BTreeMap<TypeId, CopyCondition>,
    families: BTreeMap<NominalTypeId, CopyCondition>,
}

impl CopyabilityTable {
    pub(crate) fn build(
        graph: &DeclarationGraph,
        types: &mut TypeStore,
        source_index: &SourceIndex,
    ) -> Result<Self, CopyabilityBuildError> {
        let mut table = Self::new(graph);
        table.validate_copy_families(graph, types, source_index)?;
        Ok(table)
    }

    fn new(graph: &DeclarationGraph) -> Self {
        let mut table = Self::default();
        for (_, requirement) in graph.declarations().requirements().iter() {
            if let RequirementKind::Copy(parameter) = requirement.kind() {
                table.parameters.insert(*parameter);
            }
        }
        table
    }

    fn validate_copy_families(
        &mut self,
        graph: &DeclarationGraph,
        types: &mut TypeStore,
        source_index: &SourceIndex,
    ) -> Result<(), CopyabilityBuildError> {
        for (family, declaration) in graph.declarations().nominal_types().iter() {
            let NominalShape::Struct {
                copy_declared: true,
                fields,
            } = declaration.shape()
            else {
                continue;
            };
            let mut family_condition = CopyCondition::Always;
            for field in fields.iter().copied() {
                let field_type = graph
                    .declarations()
                    .fields()
                    .get(field)
                    .map(|declaration| declaration.ty())
                    .ok_or(CopyabilityError::UnknownField(field))?;
                let condition = self.evaluate(graph, types, field_type)?.clone();
                if condition == CopyCondition::Impossible {
                    let entity = SemanticEntity::Field(field);
                    let origin = source_origin(source_index, entity)
                        .ok_or(CopyabilityError::MissingSource(entity))?;
                    return Err(CopyabilityBuildError::Rule(
                        CopyabilityRule::UnconditionallyMoveOnlyField.diagnostic(origin),
                    ));
                }
                if let CopyCondition::Requires(parameters) = &condition
                    && let Some(parameter) = parameters
                        .iter()
                        .find(|parameter| !declaration.generic_parameters().contains(parameter))
                {
                    return Err(CopyabilityError::ForeignConditionParameter {
                        family,
                        parameter: *parameter,
                    }
                    .into());
                }
                family_condition = family_condition.conjoin(condition);
            }
            self.families.insert(family, family_condition);
        }
        Ok(())
    }

    /// Returns a classification already fixed by checking.
    #[must_use]
    pub fn get(&self, ty: TypeId) -> Option<Copyability> {
        self.conditions.get(&ty).map(CopyCondition::classification)
    }

    /// Returns the retained symbolic condition for a copy-struct family.
    #[must_use]
    pub fn family_condition(&self, family: NominalTypeId) -> Option<&CopyCondition> {
        self.families.get(&family)
    }

    /// Classifies one complete type using declaration structure and normalized generic proofs.
    ///
    /// An ordinary struct stays move-only regardless of its fields. A `copy struct`
    /// specialization opens structural classification after substituting its actual arguments.
    /// Callable contracts remain move-only here because closure-environment copyability belongs
    /// to a checked value, not its callable signature.
    pub(crate) fn classify(
        &mut self,
        graph: &DeclarationGraph,
        types: &mut TypeStore,
        root: TypeId,
    ) -> Result<Copyability, CopyabilityError> {
        if !self.conditions.contains_key(&root) {
            self.evaluate(graph, types, root)?;
        }
        self.get(root)
            .ok_or(CopyabilityError::InvalidTraversal(root))
    }

    fn evaluate(
        &mut self,
        graph: &DeclarationGraph,
        types: &mut TypeStore,
        root: TypeId,
    ) -> Result<&CopyCondition, CopyabilityError> {
        let mut active = HashSet::new();
        let mut pending = vec![CopyabilityAction::Enter(root)];
        while let Some(action) = pending.pop() {
            match action {
                CopyabilityAction::Enter(ty) => {
                    if self.conditions.contains_key(&ty) {
                        continue;
                    }
                    if !active.insert(ty) {
                        return Err(CopyabilityError::CyclicType(ty));
                    }
                    let kind = types
                        .get(ty)
                        .cloned()
                        .ok_or(CopyabilityError::UnknownType(ty))?;
                    let condition = match kind {
                        TypeKind::Builtin(
                            BuiltinType::Bool
                            | BuiltinType::I8
                            | BuiltinType::I16
                            | BuiltinType::I32
                            | BuiltinType::I64
                            | BuiltinType::U8
                            | BuiltinType::U16
                            | BuiltinType::U32
                            | BuiltinType::U64
                            | BuiltinType::Usize
                            | BuiltinType::Isize
                            | BuiltinType::Error
                            | BuiltinType::Void,
                        )
                        | TypeKind::Pointer(_)
                        | TypeKind::Borrow {
                            capability: BorrowCapability::Readonly,
                            ..
                        } => Some(CopyCondition::Always),
                        TypeKind::Builtin(BuiltinType::Never | BuiltinType::Str)
                        | TypeKind::Borrow {
                            capability: BorrowCapability::ReadWrite,
                            ..
                        }
                        | TypeKind::Slice(_)
                        | TypeKind::Callable(_)
                        | TypeKind::InterfaceSelf(_)
                        | TypeKind::AssociatedProjection { .. }
                        | TypeKind::Opaque { .. } => Some(CopyCondition::Impossible),
                        TypeKind::GenericParameter(parameter) => {
                            Some(if self.parameters.contains(&parameter) {
                                CopyCondition::Always
                            } else {
                                CopyCondition::requiring(parameter)
                            })
                        }
                        TypeKind::FixedArray { element, .. }
                        | TypeKind::Optional(element)
                        | TypeKind::Fallible(element) => {
                            schedule_dependencies(ty, [element], &self.conditions, &mut pending);
                            None
                        }
                        TypeKind::Nominal {
                            definition,
                            arguments,
                        } => match nominal_dependencies(graph, types, definition, &arguments)? {
                            Some(dependencies) => {
                                schedule_dependencies(
                                    ty,
                                    dependencies,
                                    &self.conditions,
                                    &mut pending,
                                );
                                None
                            }
                            None => Some(CopyCondition::Impossible),
                        },
                    };
                    if let Some(condition) = condition {
                        active.remove(&ty);
                        self.conditions.insert(ty, condition);
                    }
                }
                CopyabilityAction::Finish { ty, dependencies } => {
                    let condition = dependencies.iter().try_fold(
                        CopyCondition::Always,
                        |condition, dependency| {
                            self.conditions
                                .get(dependency)
                                .cloned()
                                .map(|dependency| condition.conjoin(dependency))
                                .ok_or(CopyabilityError::InvalidTraversal(ty))
                        },
                    )?;
                    active.remove(&ty);
                    self.conditions.insert(ty, condition);
                }
            }
        }
        self.conditions
            .get(&root)
            .ok_or(CopyabilityError::InvalidTraversal(root))
    }

    /// Closes the table over every type in the final store, including types interned by
    /// substitution while classification is running.
    pub(crate) fn complete(
        &mut self,
        graph: &DeclarationGraph,
        types: &mut TypeStore,
    ) -> Result<(), CopyabilityError> {
        loop {
            let pending = types
                .iter()
                .map(|(ty, _)| ty)
                .filter(|ty| !self.conditions.contains_key(ty))
                .collect::<Vec<_>>();
            if pending.is_empty() {
                return Ok(());
            }
            for ty in pending {
                self.evaluate(graph, types, ty)?;
            }
        }
    }
}

fn schedule_dependencies(
    ty: TypeId,
    dependencies: impl IntoIterator<Item = TypeId>,
    finished: &BTreeMap<TypeId, CopyCondition>,
    pending: &mut Vec<CopyabilityAction>,
) {
    let dependencies = dependencies.into_iter().collect::<Vec<_>>();
    pending.push(CopyabilityAction::Finish {
        ty,
        dependencies: dependencies.clone().into_boxed_slice(),
    });
    pending.extend(
        dependencies
            .into_iter()
            .rev()
            .filter(|dependency| !finished.contains_key(dependency))
            .map(CopyabilityAction::Enter),
    );
}

fn nominal_dependencies(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    definition: NominalTypeId,
    arguments: &[TypeId],
) -> Result<Option<Vec<TypeId>>, CopyabilityError> {
    let declaration = graph
        .declarations()
        .nominal_types()
        .get(definition)
        .ok_or(CopyabilityError::UnknownNominal(definition))?;
    match declaration.shape() {
        NominalShape::Struct {
            copy_declared: false,
            ..
        } => Ok(None),
        NominalShape::Struct {
            copy_declared: true,
            fields,
        } => {
            if declaration.generic_parameters().len() != arguments.len() {
                return Err(CopyabilityError::GenericArity(definition));
            }
            let mut substitution = TypeSubstitution::default();
            for (parameter, argument) in declaration
                .generic_parameters()
                .iter()
                .copied()
                .zip(arguments.iter().copied())
            {
                substitution.bind_generic(parameter, argument);
            }
            fields
                .iter()
                .copied()
                .map(|field| substituted_field_type(graph, types, &substitution, field))
                .collect::<Result<Vec<_>, _>>()
                .map(Some)
        }
        NominalShape::Enum { variants } => {
            for variant in variants {
                let declaration = graph
                    .declarations()
                    .variants()
                    .get(*variant)
                    .ok_or(CopyabilityError::UnknownVariant(*variant))?;
                if !declaration.payload().is_empty() {
                    return Ok(None);
                }
            }
            Ok(Some(Vec::new()))
        }
    }
}

fn substituted_field_type(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    substitution: &TypeSubstitution,
    field: FieldId,
) -> Result<TypeId, CopyabilityError> {
    let ty = graph
        .declarations()
        .fields()
        .get(field)
        .map(|declaration| declaration.ty())
        .ok_or(CopyabilityError::UnknownField(field))?;
    substitution
        .apply_type(types, ty)
        .map_err(CopyabilityError::Substitution)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CopyabilityError {
    UnknownType(TypeId),
    UnknownNominal(NominalTypeId),
    UnknownField(FieldId),
    UnknownVariant(VariantId),
    GenericArity(NominalTypeId),
    CyclicType(TypeId),
    InvalidTraversal(TypeId),
    MissingSource(SemanticEntity),
    ForeignConditionParameter {
        family: NominalTypeId,
        parameter: GenericParameterId,
    },
    Substitution(SubstitutionError),
}

impl fmt::Display for CopyabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "copyability invariant failed: {self:?}")
    }
}

impl std::error::Error for CopyabilityError {}

fn source_origin(source_index: &SourceIndex, entity: SemanticEntity) -> Option<SourceOrigin> {
    source_index
        .bindings_for(entity)
        .iter()
        .find(|binding| {
            matches!(
                binding.role(),
                SourceRole::Declaration | SourceRole::Implementation
            )
        })
        .map(|binding| binding.origin())
}
