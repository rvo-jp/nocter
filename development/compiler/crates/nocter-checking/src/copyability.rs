use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::ops::Deref;

use nocter_declarations::{DeclarationGraph, NominalShape};
use nocter_model::{
    BorrowCapability, BuiltinType, ClosureId, FieldId, GenericParameterId, NominalTypeId, TypeId,
    TypeKind, TypeStore, VariantId,
};
use nocter_persistent::PersistentMap;
use nocter_source_index::{DiagnosticOrigins, SemanticEntity, SourceOrigin};

use crate::CheckedPredicate;
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

    fn classification(&self, proofs: &CopyProofs) -> Copyability {
        match self {
            Self::Always => Copyability::Copy,
            Self::Requires(required) if required.is_subset(&proofs.parameters) => Copyability::Copy,
            Self::Requires(_) | Self::Impossible => Copyability::MoveOnly,
        }
    }
}

/// Exact lexical `copy` facts available while checking one body.
///
/// Structural conditions belong to [`CopyabilityTable`]. Keeping authored proof scope in this
/// separate value prevents a requirement on one member from changing sibling bodies or the rest
/// of the program.
#[derive(Clone, Debug, Default)]
pub(crate) struct CopyProofs {
    parameters: BTreeSet<GenericParameterId>,
}

impl CopyProofs {
    pub(crate) fn from_predicates<'a>(
        types: &TypeStore,
        predicates: impl IntoIterator<Item = &'a CheckedPredicate>,
    ) -> Self {
        let parameters = predicates
            .into_iter()
            .filter_map(|predicate| {
                let CheckedPredicate::Copy(ty) = predicate else {
                    return None;
                };
                match types.get(*ty) {
                    Some(TypeKind::GenericParameter(parameter)) => Some(*parameter),
                    _ => None,
                }
            })
            .collect();
        Self { parameters }
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
#[derive(Clone, Debug)]
pub struct CopyabilityTable {
    conditions: PersistentMap<TypeId, CopyCondition>,
    families: PersistentMap<NominalTypeId, CopyCondition>,
    closures: PersistentMap<ClosureId, ClosureCopyCondition>,
    authority: CopyabilityAuthorityIdentity,
}

impl Default for CopyabilityTable {
    fn default() -> Self {
        Self {
            conditions: PersistentMap::default(),
            families: PersistentMap::default(),
            closures: PersistentMap::default(),
            authority: CopyabilityAuthorityIdentity::fresh(),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct CopyabilityAuthorityIdentity(u64);

impl CopyabilityAuthorityIdentity {
    fn fresh() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT: AtomicU64 = AtomicU64::new(1);
        let identity = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .expect("copyability authority identity space exhausted");
        Self(identity)
    }
}

impl fmt::Debug for CopyabilityAuthorityIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CopyabilityAuthorityIdentity")
    }
}

#[derive(Debug)]
pub(crate) struct CopyabilityTransaction {
    base: CopyabilityAuthorityIdentity,
    branch: CopyabilityTable,
}

impl Deref for CopyabilityTransaction {
    type Target = CopyabilityTable;

    fn deref(&self) -> &Self::Target {
        &self.branch
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StaleCopyabilityTransaction;

#[derive(Clone, Debug)]
struct ClosureCopyCondition {
    parameters: Box<[GenericParameterId]>,
    condition: CopyCondition,
}

impl CopyabilityTable {
    pub(crate) fn transaction(&self) -> CopyabilityTransaction {
        CopyabilityTransaction {
            base: self.authority,
            branch: self.clone(),
        }
    }

    pub(crate) fn build(
        graph: &DeclarationGraph,
        types: &mut nocter_model::TypeTransaction,
        source_index: DiagnosticOrigins<'_>,
    ) -> Result<Self, CopyabilityBuildError> {
        let base = Self::default();
        let mut transaction = base.transaction();
        transaction.validate_copy_families(graph, types, source_index)?;
        Ok(transaction
            .commit(&base)
            .expect("copyability construction must commit to its exact empty authority"))
    }

    /// Returns a classification already fixed by checking.
    #[must_use]
    pub fn get(&self, ty: TypeId) -> Option<Copyability> {
        self.conditions
            .get(&ty)
            .map(|condition| condition.classification(&CopyProofs::default()))
    }

    /// Returns the retained symbolic condition for a copy-struct family.
    #[must_use]
    pub fn family_condition(&self, family: NominalTypeId) -> Option<&CopyCondition> {
        self.families.get(&family)
    }
}

impl CopyabilityTransaction {
    pub(crate) fn is_based_on(&self, authority: &CopyabilityTable) -> bool {
        self.base == authority.authority
    }

    pub(crate) fn commit(
        mut self,
        base: &CopyabilityTable,
    ) -> Result<CopyabilityTable, StaleCopyabilityTransaction> {
        if self.base != base.authority {
            return Err(StaleCopyabilityTransaction);
        }
        self.branch.authority = CopyabilityAuthorityIdentity::fresh();
        Ok(self.branch)
    }

    pub(crate) fn freeze(mut self) -> CopyabilityTable {
        self.branch.authority = CopyabilityAuthorityIdentity::fresh();
        self.branch
    }

    fn validate_copy_families(
        &mut self,
        graph: &DeclarationGraph,
        types: &mut nocter_model::TypeTransaction,
        source_index: DiagnosticOrigins<'_>,
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
            assert!(
                self.branch.families.insert_absent(family, family_condition),
                "copyability family insertion must be unique"
            );
        }
        Ok(())
    }

    /// Fixes the structural copy condition of one freshly checked closure environment.
    ///
    /// Closure identity is concrete, but its stored captures may still mention lexical generic
    /// parameters. Retaining the complete condition here lets later specialization answer the
    /// same question without reconstructing the environment from checked operations.
    pub(crate) fn register_closure(
        &mut self,
        graph: &DeclarationGraph,
        types: &mut nocter_model::TypeTransaction,
        closure: TypeId,
        stored_captures: impl IntoIterator<Item = TypeId>,
    ) -> Result<(), CopyabilityError> {
        let Some(TypeKind::Closure {
            definition,
            arguments,
        }) = types.get(closure)
        else {
            return Err(CopyabilityError::InvalidClosureRegistration(closure));
        };
        let definition = *definition;
        let parameters = arguments
            .iter()
            .map(|argument| match types.get(*argument) {
                Some(TypeKind::GenericParameter(parameter)) => Ok(*parameter),
                _ => Err(CopyabilityError::InvalidClosureRegistration(closure)),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if self.conditions.contains_key(&closure)
            || self.closures.contains_key(&definition)
            || parameters.iter().copied().collect::<BTreeSet<_>>().len() != parameters.len()
        {
            return Err(CopyabilityError::InvalidClosureRegistration(closure));
        }
        let mut condition = CopyCondition::Always;
        for capture in stored_captures {
            condition = condition.conjoin(self.evaluate(graph, types, capture)?.clone());
        }
        self.insert_condition(closure, condition.clone());
        self.insert_closure_condition(
            definition,
            ClosureCopyCondition {
                parameters: parameters.into_boxed_slice(),
                condition,
            },
        );
        Ok(())
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
        types: &mut nocter_model::TypeTransaction,
        root: TypeId,
    ) -> Result<Copyability, CopyabilityError> {
        if !self.conditions.contains_key(&root) {
            self.evaluate(graph, types, root)?;
        }
        self.get(root)
            .ok_or(CopyabilityError::InvalidTraversal(root))
    }

    pub(crate) fn classify_with_proofs(
        &mut self,
        graph: &DeclarationGraph,
        types: &mut nocter_model::TypeTransaction,
        root: TypeId,
        proofs: &CopyProofs,
    ) -> Result<Copyability, CopyabilityError> {
        if !self.conditions.contains_key(&root) {
            self.evaluate(graph, types, root)?;
        }
        self.conditions
            .get(&root)
            .map(|condition| condition.classification(proofs))
            .ok_or(CopyabilityError::InvalidTraversal(root))
    }

    fn evaluate(
        &mut self,
        graph: &DeclarationGraph,
        types: &mut nocter_model::TypeTransaction,
        root: TypeId,
    ) -> Result<&CopyCondition, CopyabilityError> {
        let mut active = HashSet::new();
        let mut pending = vec![CopyabilityAction::Enter(root)];
        while let Some(action) = pending.pop() {
            match action {
                CopyabilityAction::Enter(ty) => {
                    self.enter_type(graph, types, ty, &mut active, &mut pending)?;
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
                    self.insert_condition(ty, condition);
                }
            }
        }
        self.conditions
            .get(&root)
            .ok_or(CopyabilityError::InvalidTraversal(root))
    }

    fn enter_type(
        &mut self,
        graph: &DeclarationGraph,
        types: &mut nocter_model::TypeTransaction,
        ty: TypeId,
        active: &mut HashSet<TypeId>,
        pending: &mut Vec<CopyabilityAction>,
    ) -> Result<(), CopyabilityError> {
        if self.conditions.contains_key(&ty) {
            return Ok(());
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
                | BuiltinType::Void,
            )
            | TypeKind::Pointer(_)
            | TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                ..
            } => Some(CopyCondition::Always),
            TypeKind::Builtin(BuiltinType::Never | BuiltinType::Str | BuiltinType::Error)
            | TypeKind::Borrow {
                capability: BorrowCapability::ReadWrite,
                ..
            }
            | TypeKind::Slice(_)
            | TypeKind::PackEntry { .. }
            | TypeKind::Callable(_)
            | TypeKind::InterfaceSelf(_)
            | TypeKind::AssociatedProjection { .. }
            | TypeKind::Opaque { .. }
            | TypeKind::Fallible(_) => Some(CopyCondition::Impossible),
            TypeKind::Closure {
                definition,
                arguments,
            } => match self.closure_dependencies(ty, definition, &arguments)? {
                Some(dependencies) => {
                    schedule_dependencies(ty, dependencies, &self.conditions, pending);
                    None
                }
                None => Some(CopyCondition::Impossible),
            },
            TypeKind::GenericParameter(parameter) => Some(CopyCondition::requiring(parameter)),
            TypeKind::FixedArray { element, .. } | TypeKind::Optional(element) => {
                schedule_dependencies(ty, [element], &self.conditions, pending);
                None
            }
            TypeKind::Nominal {
                definition,
                arguments,
            } => match nominal_dependencies(graph, types, definition, &arguments)? {
                Some(dependencies) => {
                    schedule_dependencies(ty, dependencies, &self.conditions, pending);
                    None
                }
                None => Some(CopyCondition::Impossible),
            },
        };
        if let Some(condition) = condition {
            active.remove(&ty);
            self.insert_condition(ty, condition);
        }
        Ok(())
    }

    fn insert_condition(&mut self, ty: TypeId, condition: CopyCondition) {
        assert!(
            self.branch.conditions.insert_absent(ty, condition),
            "copy condition insertion must be unique"
        );
    }

    fn insert_closure_condition(&mut self, closure: ClosureId, condition: ClosureCopyCondition) {
        assert!(
            self.branch.closures.insert_absent(closure, condition),
            "closure copyability insertion must be unique"
        );
    }

    fn closure_dependencies(
        &self,
        ty: TypeId,
        definition: ClosureId,
        arguments: &[TypeId],
    ) -> Result<Option<Vec<TypeId>>, CopyabilityError> {
        let closure = self
            .closures
            .get(&definition)
            .ok_or(CopyabilityError::MissingClosureCondition(ty))?;
        if closure.parameters.len() != arguments.len() {
            return Err(CopyabilityError::MissingClosureCondition(ty));
        }
        Ok(match &closure.condition {
            CopyCondition::Always => Some(Vec::new()),
            CopyCondition::Impossible => None,
            CopyCondition::Requires(required) => Some(
                required
                    .iter()
                    .map(|required| {
                        closure
                            .parameters
                            .iter()
                            .position(|parameter| parameter == required)
                            .map(|index| arguments[index])
                            .ok_or(CopyabilityError::MissingClosureCondition(ty))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        })
    }

    /// Closes the table over every type in the final store, including types interned by
    /// substitution while classification is running.
    pub(crate) fn complete(
        &mut self,
        graph: &DeclarationGraph,
        types: &mut nocter_model::TypeTransaction,
    ) -> Result<(), CopyabilityError> {
        let mut visited = 0;
        loop {
            let pending = types
                .iter_from(visited)
                .map(|(ty, _)| ty)
                .filter(|ty| !self.conditions.contains_key(ty))
                .collect::<Vec<_>>();
            visited = types.type_count();
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
    finished: &PersistentMap<TypeId, CopyCondition>,
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
    types: &mut nocter_model::TypeTransaction,
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
    types: &mut nocter_model::TypeTransaction,
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
    InvalidClosureRegistration(TypeId),
    MissingClosureCondition(TypeId),
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

fn source_origin(
    source_index: DiagnosticOrigins<'_>,
    entity: SemanticEntity,
) -> Option<SourceOrigin> {
    source_index.declaration(entity)
}

#[cfg(test)]
mod tests {
    use nocter_model::{BuiltinType, TypeStore};

    use super::{CopyCondition, Copyability, CopyabilityTable};

    #[test]
    fn discarded_branch_cannot_change_accepted_conditions() {
        let types = TypeStore::new();
        let retained = types.builtin(BuiltinType::Bool);
        let discarded = types.builtin(BuiltinType::I32);
        let initial = CopyabilityTable::default();
        let mut accepted = initial.transaction();
        accepted.insert_condition(retained, CopyCondition::Always);
        let accepted = accepted.commit(&initial).unwrap();
        let mut discarded_branch = accepted.transaction();
        discarded_branch.insert_condition(discarded, CopyCondition::Always);

        drop(discarded_branch);

        assert_eq!(accepted.get(retained), Some(Copyability::Copy));
        assert_eq!(accepted.get(discarded), None);
    }

    #[test]
    fn frozen_sibling_branches_isolate_their_conditions() {
        let types = TypeStore::new();
        let first_type = types.builtin(BuiltinType::Bool);
        let second_type = types.builtin(BuiltinType::I32);
        let base = CopyabilityTable::default();
        let mut first = base.transaction();
        let mut second = base.transaction();
        first.insert_condition(first_type, CopyCondition::Always);
        second.insert_condition(second_type, CopyCondition::Always);

        let first = first.freeze();
        let second = second.freeze();

        assert_eq!(first.get(first_type), Some(Copyability::Copy));
        assert_eq!(first.get(second_type), None);
        assert_eq!(second.get(first_type), None);
        assert_eq!(second.get(second_type), Some(Copyability::Copy));
    }

    #[test]
    fn transaction_cannot_commit_to_another_authority() {
        let first = CopyabilityTable::default();
        let second = CopyabilityTable::default();
        let transaction = first.transaction();

        assert!(transaction.commit(&second).is_err());
    }

    #[test]
    fn sibling_transaction_is_stale_after_accepted_authority_advances() {
        let base = CopyabilityTable::default();
        let accepted = base.transaction();
        let stale = base.transaction();
        let accepted = accepted.commit(&base).unwrap();

        assert!(stale.commit(&accepted).is_err());
    }

    #[test]
    fn base_compatibility_distinguishes_authorities_but_accepts_an_immutable_clone() {
        let base = CopyabilityTable::default();
        let same = base.clone();
        let foreign = CopyabilityTable::default();
        let transaction = base.transaction();

        assert!(transaction.is_based_on(&base));
        assert!(transaction.is_based_on(&same));
        assert!(!transaction.is_based_on(&foreign));
    }
}
