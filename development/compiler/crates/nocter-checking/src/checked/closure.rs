use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use nocter_model::{
    Arena, ArenaBuilder, BodyId, BodyNodeId, CallableCapability, CallableContract, CaptureId,
    ClosureId, ClosureSequence, LocalBindingId, TypeId,
};
use nocter_persistent::PersistentVector;

use crate::{BodyClosureRef, BodyTypeCapture, BodyTypeRecipeError, BodyTypeRef, ReplayedBodyTypes};

/// The fully typed invocation shape of one anonymous concrete closure.
///
/// Result provenance is deliberately absent. It is a program-wide inferred relation retained by
/// `ProvenanceTable`, not a fact guessed while the closure body is still being constructed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosureSignature {
    capability: CallableCapability,
    parameters: Box<[ClosureParameter]>,
    result: TypeId,
}

/// One closure parameter whose local identity and checked type cannot diverge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClosureParameter {
    binding: LocalBindingId,
    ty: TypeId,
}

impl ClosureParameter {
    #[must_use]
    pub const fn new(binding: LocalBindingId, ty: TypeId) -> Self {
        Self { binding, ty }
    }

    #[must_use]
    pub const fn binding(self) -> LocalBindingId {
        self.binding
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

impl ClosureSignature {
    pub(crate) fn new(
        capability: CallableCapability,
        parameters: impl Into<Box<[ClosureParameter]>>,
        result: TypeId,
    ) -> Self {
        Self {
            capability,
            parameters: parameters.into(),
            result,
        }
    }

    #[must_use]
    pub const fn capability(&self) -> CallableCapability {
        self.capability
    }

    #[must_use]
    pub const fn parameters(&self) -> &[ClosureParameter] {
        &self.parameters
    }

    pub fn parameter_types(&self) -> impl ExactSizeIterator<Item = TypeId> + '_ {
        self.parameters.iter().copied().map(ClosureParameter::ty)
    }

    #[must_use]
    pub const fn result(&self) -> TypeId {
        self.result
    }
}

/// One field stored in a checked closure environment.
///
/// `ty` describes the representation held by the closure value. For a borrow capture this is a
/// borrow type, not the type of the source place that the nested body reads through `binding`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClosureEnvironmentField {
    binding: CaptureId,
    ty: TypeId,
}

impl ClosureEnvironmentField {
    pub(crate) const fn new(binding: CaptureId, ty: TypeId) -> Self {
        Self { binding, ty }
    }

    #[must_use]
    pub const fn binding(self) -> CaptureId {
        self.binding
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

/// One generated closure body and the exact environment bindings it operates on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosureDefinition {
    core: Arc<ClosureDefinitionCore>,
    callable_requirements: Box<[CallableContract]>,
}

#[derive(Debug, Eq, PartialEq)]
struct ClosureDefinitionCore {
    owner: BodyId,
    ty: TypeId,
    signature: ClosureSignature,
    environment: Box<[ClosureEnvironmentField]>,
    body: BodyNodeId,
}

impl ClosureDefinition {
    pub(crate) fn new(
        owner: BodyId,
        ty: TypeId,
        signature: ClosureSignature,
        environment: impl Into<Box<[ClosureEnvironmentField]>>,
        body: BodyNodeId,
    ) -> Self {
        Self {
            core: Arc::new(ClosureDefinitionCore {
                owner,
                ty,
                signature,
                environment: environment.into(),
                body,
            }),
            callable_requirements: Box::new([]),
        }
    }

    #[must_use]
    pub fn owner(&self) -> BodyId {
        self.core.owner
    }

    #[must_use]
    pub fn ty(&self) -> TypeId {
        self.core.ty
    }

    #[must_use]
    pub fn signature(&self) -> &ClosureSignature {
        &self.core.signature
    }

    #[must_use]
    pub fn environment(&self) -> &[ClosureEnvironmentField] {
        &self.core.environment
    }

    /// Concrete structural callable contracts this closure must satisfy at its use sites.
    ///
    /// Signature compatibility is checked when the edge is recorded. Result provenance is
    /// inferred later, so the provenance pass consumes the retained contracts without rebuilding
    /// generic-call selection.
    #[must_use]
    pub fn callable_requirements(&self) -> &[CallableContract] {
        &self.callable_requirements
    }

    #[must_use]
    pub fn body(&self) -> BodyNodeId {
        self.core.body
    }
}

/// Sole checked-program authority for anonymous closure identity and generated body metadata.
#[derive(Clone, Debug)]
pub struct ClosureTable {
    definitions: Arena<ClosureId, ClosureDefinition>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BodyClosureSignature {
    capability: CallableCapability,
    parameters: Box<[(LocalBindingId, BodyTypeRef)]>,
    result: BodyTypeRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BodyClosureDefinition {
    ty: BodyTypeRef,
    signature: BodyClosureSignature,
    environment: Box<[(CaptureId, BodyTypeRef)]>,
    body: BodyNodeId,
    callable_requirements: Box<[BodyCallableContract]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BodyCallableContract {
    capability: CallableCapability,
    guarantees: nocter_model::CallableGuarantees,
    parameters: Box<[BodyTypeRef]>,
    pack: Option<nocter_model::ArgumentPack<BodyTypeRef>>,
    result: BodyTypeRef,
    provenance: nocter_model::ResultProvenance,
}

/// Source-neutral closure definitions contributed by one checked body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodyClosureRecipe {
    owner: BodyId,
    definitions: Box<[BodyClosureDefinition]>,
    source_references: HashMap<ClosureId, BodyClosureRef>,
}

/// Current canonical closure identities reserved for one replayed body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayedBodyClosures {
    owner: BodyId,
    ids: Box<[ClosureId]>,
}

impl ReplayedBodyClosures {
    #[must_use]
    pub const fn ids(&self) -> &[ClosureId] {
        &self.ids
    }

    /// Resolves one body-local closure identity.
    ///
    /// # Errors
    ///
    /// Returns an integrity error for an unknown local identity.
    pub fn resolve(&self, closure: BodyClosureRef) -> Result<ClosureId, BodyClosureRecipeError> {
        self.ids
            .get(closure.index() as usize)
            .copied()
            .ok_or(BodyClosureRecipeError::UnknownLocalClosure(closure.index()))
    }
}

impl BodyClosureRecipe {
    /// Classifies one closure from the exact body branch captured by this recipe.
    ///
    /// # Errors
    ///
    /// Returns an integrity error when the identity did not originate in this body.
    pub fn reference(&self, closure: ClosureId) -> Result<BodyClosureRef, BodyClosureRecipeError> {
        self.source_references
            .get(&closure)
            .copied()
            .ok_or(BodyClosureRecipeError::UnknownClosure(closure))
    }

    /// Reserves current canonical identities before structural type replay.
    pub(crate) fn reserve(&self, target: &mut ClosureTransaction) -> ReplayedBodyClosures {
        ReplayedBodyClosures {
            owner: self.owner,
            ids: (0..self.definitions.len())
                .map(|_| target.reserve(self.owner))
                .collect(),
        }
    }

    /// Defines every previously reserved closure after body-local types have been replayed.
    ///
    /// # Errors
    ///
    /// Returns an integrity failure when closure or type maps do not cover this recipe or the
    /// target authority rejects a definition.
    pub(crate) fn define(
        &self,
        target: &mut ClosureTransaction,
        closures: &ReplayedBodyClosures,
        types: &ReplayedBodyTypes,
    ) -> Result<(), BodyClosureRecipeError> {
        if closures.owner != self.owner || closures.ids.len() != self.definitions.len() {
            return Err(BodyClosureRecipeError::ReplayDomainMismatch);
        }
        for (closure, recipe) in closures.ids.iter().copied().zip(&self.definitions) {
            let definition = ClosureDefinition::new(
                self.owner,
                types.resolve(recipe.ty)?,
                ClosureSignature::new(
                    recipe.signature.capability,
                    recipe
                        .signature
                        .parameters
                        .iter()
                        .map(|(binding, ty)| {
                            types
                                .resolve(*ty)
                                .map(|ty| ClosureParameter::new(*binding, ty))
                                .map_err(BodyClosureRecipeError::from)
                        })
                        .collect::<Result<Vec<_>, BodyClosureRecipeError>>()?,
                    types.resolve(recipe.signature.result)?,
                ),
                recipe
                    .environment
                    .iter()
                    .map(|(binding, ty)| {
                        types
                            .resolve(*ty)
                            .map(|ty| ClosureEnvironmentField::new(*binding, ty))
                            .map_err(BodyClosureRecipeError::from)
                    })
                    .collect::<Result<Vec<_>, BodyClosureRecipeError>>()?,
                recipe.body,
            );
            target.define(closure, definition)?;
            for requirement in &recipe.callable_requirements {
                target.require_callable(self.owner, closure, requirement.replay(types)?)?;
            }
        }
        Ok(())
    }

    pub(crate) fn register_copyability(
        &self,
        graph: &nocter_declarations::DeclarationGraph,
        type_store: &mut nocter_model::TypeTransaction,
        copyabilities: &mut crate::copyability::CopyabilityTransaction,
        types: &ReplayedBodyTypes,
    ) -> Result<(), BodyClosureRecipeError> {
        for definition in &self.definitions {
            copyabilities.register_closure(
                graph,
                type_store,
                types.resolve(definition.ty)?,
                definition
                    .environment
                    .iter()
                    .map(|(_, ty)| types.resolve(*ty))
                    .collect::<Result<Vec<_>, _>>()?,
            )?;
        }
        Ok(())
    }
}

impl BodyCallableContract {
    fn capture(
        contract: &CallableContract,
        types: &BodyTypeCapture,
    ) -> Result<Self, BodyClosureRecipeError> {
        Ok(Self {
            capability: contract.capability(),
            guarantees: contract.guarantees(),
            parameters: contract
                .parameters()
                .iter()
                .copied()
                .map(|ty| types.reference(ty))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
            pack: contract
                .pack()
                .map(|pack| pack.try_map(|ty| types.reference(ty)))
                .transpose()?,
            result: types.reference(contract.result())?,
            provenance: contract.provenance().clone(),
        })
    }

    fn replay(
        &self,
        types: &ReplayedBodyTypes,
    ) -> Result<CallableContract, BodyClosureRecipeError> {
        Ok(CallableContract::new(
            self.capability,
            self.guarantees,
            self.parameters
                .iter()
                .copied()
                .map(|ty| types.resolve(ty))
                .collect::<Result<Vec<_>, _>>()?,
            self.pack
                .map(|pack| pack.try_map(|ty| types.resolve(ty)))
                .transpose()?,
            types.resolve(self.result)?,
            self.provenance.clone(),
        )?)
    }
}

impl ClosureTable {
    const fn new(definitions: Arena<ClosureId, ClosureDefinition>) -> Self {
        Self { definitions }
    }

    #[must_use]
    pub const fn definitions(&self) -> &Arena<ClosureId, ClosureDefinition> {
        &self.definitions
    }

    #[must_use]
    pub fn get(&self, closure: ClosureId) -> Option<&ClosureDefinition> {
        self.definitions.get(closure)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ClosureAuthority {
    slots: ClosureSequence<ClosureSlot>,
    authority: ClosureAuthorityIdentity,
}

impl ClosureAuthority {
    pub(crate) fn new() -> Self {
        Self {
            slots: ClosureSequence::default(),
            authority: ClosureAuthorityIdentity::fresh(),
        }
    }

    pub(crate) fn transaction(&self) -> ClosureTransaction {
        ClosureTransaction {
            base: self.authority,
            branch: self.clone(),
        }
    }

    pub(crate) fn signature(&self, closure: ClosureId) -> Option<&ClosureSignature> {
        match self.slots.get(closure) {
            Some(ClosureSlot::Defined(draft)) => Some(&draft.core.signature),
            Some(ClosureSlot::Reserved(_)) | None => None,
        }
    }

    pub(crate) fn body_identities(
        &self,
        owner: BodyId,
    ) -> Result<HashMap<ClosureId, BodyClosureRef>, BodyClosureRecipeError> {
        self.slots
            .iter()
            .enumerate()
            .map(|(index, (closure, slot))| {
                let ClosureSlot::Defined(draft) = slot else {
                    return Err(BodyClosureRecipeError::IncompleteClosure(closure));
                };
                if draft.core.owner != owner {
                    return Err(BodyClosureRecipeError::OwnerMismatch(closure));
                }
                let index =
                    u32::try_from(index).map_err(|_| BodyClosureRecipeError::TooManyClosures)?;
                Ok((closure, BodyClosureRef::new(index)))
            })
            .collect()
    }

    pub(crate) fn capture_body_recipe(
        &self,
        owner: BodyId,
        identities: &HashMap<ClosureId, BodyClosureRef>,
        types: &BodyTypeCapture,
    ) -> Result<BodyClosureRecipe, BodyClosureRecipeError> {
        if identities.len() != self.slots.iter().len() {
            return Err(BodyClosureRecipeError::IdentityDomainMismatch);
        }
        let definitions = self
            .slots
            .iter()
            .enumerate()
            .map(|(index, (closure, slot))| {
                let ClosureSlot::Defined(draft) = slot else {
                    return Err(BodyClosureRecipeError::IncompleteClosure(closure));
                };
                if draft.core.owner != owner {
                    return Err(BodyClosureRecipeError::OwnerMismatch(closure));
                }
                let expected = u32::try_from(index)
                    .map(BodyClosureRef::new)
                    .map_err(|_| BodyClosureRecipeError::TooManyClosures)?;
                if identities.get(&closure) != Some(&expected) {
                    return Err(BodyClosureRecipeError::IdentityDomainMismatch);
                }
                Ok(BodyClosureDefinition {
                    ty: types.reference(draft.core.ty)?,
                    signature: BodyClosureSignature {
                        capability: draft.core.signature.capability(),
                        parameters: draft
                            .core
                            .signature
                            .parameters()
                            .iter()
                            .map(|parameter| {
                                types
                                    .reference(parameter.ty())
                                    .map(|ty| (parameter.binding(), ty))
                            })
                            .collect::<Result<Vec<_>, _>>()?
                            .into_boxed_slice(),
                        result: types.reference(draft.core.signature.result())?,
                    },
                    environment: draft
                        .core
                        .environment
                        .iter()
                        .map(|field| types.reference(field.ty()).map(|ty| (field.binding(), ty)))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice(),
                    body: draft.core.body,
                    callable_requirements: draft
                        .requirements
                        .iter()
                        .map(|requirement| BodyCallableContract::capture(requirement, types))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice(),
                })
            })
            .collect::<Result<Vec<_>, BodyClosureRecipeError>>()?;
        Ok(BodyClosureRecipe {
            owner,
            definitions: definitions.into_boxed_slice(),
            source_references: identities.clone(),
        })
    }

    pub(crate) fn finish(self) -> Result<ClosureTable, ClosureTableBuildError> {
        let mut definitions = ArenaBuilder::new();
        for (closure, slot) in &self.slots {
            let ClosureSlot::Defined(draft) = slot else {
                return Err(ClosureTableBuildError::IncompleteClosure(closure));
            };
            let actual = definitions.insert(draft.freeze());
            assert_eq!(
                actual, closure,
                "persistent closure order must preserve canonical identity"
            );
        }
        Ok(ClosureTable::new(definitions.finish()))
    }
}

#[derive(Debug)]
pub(crate) struct ClosureTransaction {
    base: ClosureAuthorityIdentity,
    branch: ClosureAuthority,
}

impl Deref for ClosureTransaction {
    type Target = ClosureAuthority;

    fn deref(&self) -> &Self::Target {
        &self.branch
    }
}

impl ClosureTransaction {
    pub(crate) fn reserve(&mut self, owner: BodyId) -> ClosureId {
        self.branch.slots.insert(ClosureSlot::Reserved(owner))
    }

    pub(crate) fn commit(
        mut self,
        base: &ClosureAuthority,
    ) -> Result<ClosureAuthority, StaleClosureTransaction> {
        if self.base != base.authority {
            return Err(StaleClosureTransaction);
        }
        self.branch.authority = ClosureAuthorityIdentity::fresh();
        Ok(self.branch)
    }

    pub(crate) fn define(
        &mut self,
        closure: ClosureId,
        definition: ClosureDefinition,
    ) -> Result<(), ClosureTableBuildError> {
        let slot = self
            .branch
            .slots
            .get(closure)
            .cloned()
            .ok_or(ClosureTableBuildError::UnknownClosure(closure))?;
        let ClosureSlot::Reserved(owner) = slot else {
            return Err(ClosureTableBuildError::DuplicateClosure(closure));
        };
        if owner != definition.owner() {
            return Err(ClosureTableBuildError::OwnerMismatch(closure));
        }
        self.branch
            .slots
            .replace(closure, ClosureSlot::Defined(ClosureDraft::new(definition)))
            .map_err(|_| ClosureTableBuildError::UnknownClosure(closure))?;
        Ok(())
    }

    pub(crate) fn require_callable(
        &mut self,
        owner: BodyId,
        closure: ClosureId,
        contract: CallableContract,
    ) -> Result<(), ClosureTableBuildError> {
        let slot = self
            .branch
            .slots
            .get(closure)
            .cloned()
            .ok_or(ClosureTableBuildError::UnknownClosure(closure))?;
        let ClosureSlot::Defined(mut draft) = slot else {
            return Err(ClosureTableBuildError::IncompleteClosure(closure));
        };
        if draft.core.owner != owner {
            return Err(ClosureTableBuildError::OwnerMismatch(closure));
        }
        if !draft
            .requirements
            .iter()
            .any(|existing| existing == &contract)
        {
            draft.requirements.push(contract);
            self.branch
                .slots
                .replace(closure, ClosureSlot::Defined(draft))
                .map_err(|_| ClosureTableBuildError::UnknownClosure(closure))?;
        }
        Ok(())
    }

    #[cfg(test)]
    fn callable_requirement_count(&self, closure: ClosureId) -> Option<usize> {
        match self.branch.slots.get(closure) {
            Some(ClosureSlot::Defined(draft)) => Some(draft.requirements.len()),
            Some(ClosureSlot::Reserved(_)) | None => None,
        }
    }
}

#[derive(Clone, Debug)]
enum ClosureSlot {
    Reserved(BodyId),
    Defined(ClosureDraft),
}

#[derive(Clone, Debug)]
struct ClosureDraft {
    core: Arc<ClosureDefinitionCore>,
    requirements: PersistentVector<CallableContract>,
}

impl ClosureDraft {
    fn new(definition: ClosureDefinition) -> Self {
        debug_assert!(definition.callable_requirements.is_empty());
        Self {
            core: definition.core,
            requirements: PersistentVector::default(),
        }
    }

    fn freeze(&self) -> ClosureDefinition {
        ClosureDefinition {
            core: Arc::clone(&self.core),
            callable_requirements: self.requirements.iter().cloned().collect(),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct ClosureAuthorityIdentity(u64);

impl ClosureAuthorityIdentity {
    fn fresh() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT: AtomicU64 = AtomicU64::new(1);
        let identity = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .expect("closure authority identity space exhausted");
        Self(identity)
    }
}

impl fmt::Debug for ClosureAuthorityIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ClosureAuthorityIdentity")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StaleClosureTransaction;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosureTableBuildError {
    UnknownClosure(ClosureId),
    DuplicateClosure(ClosureId),
    OwnerMismatch(ClosureId),
    IncompleteClosure(ClosureId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BodyClosureRecipeError {
    TooManyClosures,
    IdentityDomainMismatch,
    ReplayDomainMismatch,
    UnknownLocalClosure(u32),
    UnknownClosure(ClosureId),
    IncompleteClosure(ClosureId),
    OwnerMismatch(ClosureId),
    Type(BodyTypeRecipeError),
    Callable(nocter_model::InvalidParameterOrigin),
    Build(ClosureTableBuildError),
    Copyability(crate::CopyabilityError),
}

impl From<BodyTypeRecipeError> for BodyClosureRecipeError {
    fn from(error: BodyTypeRecipeError) -> Self {
        Self::Type(error)
    }
}

impl From<nocter_model::InvalidParameterOrigin> for BodyClosureRecipeError {
    fn from(error: nocter_model::InvalidParameterOrigin) -> Self {
        Self::Callable(error)
    }
}

impl From<ClosureTableBuildError> for BodyClosureRecipeError {
    fn from(error: ClosureTableBuildError) -> Self {
        Self::Build(error)
    }
}

impl From<crate::CopyabilityError> for BodyClosureRecipeError {
    fn from(error: crate::CopyabilityError) -> Self {
        Self::Copyability(error)
    }
}

impl fmt::Display for BodyClosureRecipeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid body closure recipe: {self:?}")
    }
}

impl std::error::Error for BodyClosureRecipeError {}

impl fmt::Display for ClosureTableBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid closure-table construction: {self:?}")
    }
}

impl std::error::Error for ClosureTableBuildError {}

#[cfg(test)]
mod tests {
    use nocter_model::{
        ArenaBuilder, BodyId, BodyNodeId, BuiltinType, CallableCapability, CallableContract,
        ResultProvenance, TypeAuthority, TypeKind,
    };

    use super::{ClosureAuthority, ClosureDefinition, ClosureSignature};

    #[test]
    fn reservation_fixes_identity_before_a_nested_body_is_defined() {
        let mut bodies = ArenaBuilder::<BodyId, _>::new();
        let owner = bodies.insert(());
        let _ = bodies.finish();
        let mut nodes = ArenaBuilder::<BodyNodeId, _>::new();
        let root = nodes.insert(());
        let _ = nodes.finish();
        let mut types = TypeAuthority::new().transaction();
        let base = ClosureAuthority::new();
        let mut closures = base.transaction();
        let closure = closures.reserve(owner);
        let ty = types
            .intern(TypeKind::Closure {
                definition: closure,
                arguments: Box::new([]),
            })
            .unwrap();
        closures
            .define(
                closure,
                ClosureDefinition::new(
                    owner,
                    ty,
                    ClosureSignature::new(
                        nocter_model::CallableCapability::Readonly,
                        [],
                        types.builtin(BuiltinType::Void),
                    ),
                    [],
                    root,
                ),
            )
            .unwrap();

        let table = closures.commit(&base).unwrap().finish().unwrap();
        assert_eq!(table.get(closure).unwrap().ty(), ty);
    }

    #[test]
    fn discarded_branch_cannot_mutate_a_preexisting_closure() {
        let mut bodies = ArenaBuilder::<BodyId, _>::new();
        let owner = bodies.insert(());
        let mut nodes = ArenaBuilder::<BodyNodeId, _>::new();
        let root = nodes.insert(());
        let mut types = TypeAuthority::new().transaction();
        let base = ClosureAuthority::new();
        let mut closures = base.transaction();
        let closure = closures.reserve(owner);
        let ty = types
            .intern(TypeKind::Closure {
                definition: closure,
                arguments: Box::new([]),
            })
            .unwrap();
        closures
            .define(
                closure,
                ClosureDefinition::new(
                    owner,
                    ty,
                    ClosureSignature::new(
                        CallableCapability::Readonly,
                        [],
                        types.builtin(BuiltinType::Void),
                    ),
                    [],
                    root,
                ),
            )
            .unwrap();
        let contract = CallableContract::new(
            CallableCapability::Readonly,
            nocter_model::CallableGuarantees::default(),
            [],
            None,
            types.builtin(BuiltinType::Void),
            ResultProvenance::empty(),
        )
        .unwrap();
        let another_contract = CallableContract::new(
            CallableCapability::Owned,
            nocter_model::CallableGuarantees::default(),
            [],
            None,
            types.builtin(BuiltinType::I32),
            ResultProvenance::empty(),
        )
        .unwrap();
        let accepted = closures.commit(&base).unwrap();
        let mut branch = accepted.transaction();

        branch
            .require_callable(owner, closure, contract.clone())
            .unwrap();
        branch.require_callable(owner, closure, contract).unwrap();
        branch
            .require_callable(owner, closure, another_contract)
            .unwrap();
        assert_eq!(branch.callable_requirement_count(closure), Some(2));
        drop(branch);

        let accepted = accepted.transaction();
        assert_eq!(accepted.callable_requirement_count(closure), Some(0));
    }

    #[test]
    fn sibling_transaction_is_stale_after_authority_advances() {
        let base = ClosureAuthority::new();
        let first = base.transaction();
        let second = base.transaction();
        let accepted = first.commit(&base).unwrap();

        assert!(second.commit(&accepted).is_err());
    }
}
