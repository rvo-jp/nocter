use std::fmt;
use std::sync::Arc;

pub use nocter_language::BuiltinType;
use nocter_persistent::{PersistentMap, PersistentVector};

use crate::id::SemanticId;
use crate::{
    AssociatedTypeId, ClosureId, GenericParameterId, InterfaceId, NominalTypeId, OpaqueTypeId,
    ResultProvenance, TypeId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BorrowCapability {
    Readonly,
    ReadWrite,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableCapability {
    Readonly,
    ReadWrite,
    Owned,
}

/// The semantic shape of one compiler-owned final argument pack.
///
/// A keyed pack retains each key/value pair as one entry. It is never represented as alternating
/// values or as two independently ordered packs.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ArgumentPack<T> {
    Values(T),
    Keyed { key: T, value: T },
}

pub type ArgumentPackType = ArgumentPack<TypeId>;

impl<T: Copy> ArgumentPack<T> {
    #[must_use]
    pub const fn primary(self) -> T {
        match self {
            Self::Values(element) => element,
            Self::Keyed { key, .. } => key,
        }
    }

    #[must_use]
    pub const fn value(self) -> Option<T> {
        match self {
            Self::Values(_) => None,
            Self::Keyed { value, .. } => Some(value),
        }
    }

    /// Returns the component types in their source and ABI order.
    pub fn components(self) -> impl Iterator<Item = T> {
        [Some(self.primary()), self.value()].into_iter().flatten()
    }
}

impl<T> ArgumentPack<T> {
    /// Applies one fallible transformation to every component while preserving the pack shape.
    ///
    /// # Errors
    ///
    /// Returns the first error produced by `map`, in source and ABI component order.
    pub fn try_map<U, E>(
        self,
        mut map: impl FnMut(T) -> Result<U, E>,
    ) -> Result<ArgumentPack<U>, E> {
        match self {
            Self::Values(element) => map(element).map(ArgumentPack::Values),
            Self::Keyed { key, value } => Ok(ArgumentPack::Keyed {
                key: map(key)?,
                value: map(value)?,
            }),
        }
    }

    pub(crate) fn visit(self, visit: &mut impl FnMut(T)) {
        match self {
            Self::Values(element) => visit(element),
            Self::Keyed { key, value } => {
                visit(key);
                visit(value);
            }
        }
    }
}

impl CallableCapability {
    /// Whether this caller-side access can invoke a body with `required` environment access.
    #[must_use]
    pub const fn permits(self, required: Self) -> bool {
        callable_capability_rank(required) <= callable_capability_rank(self)
    }
}

const fn callable_capability_rank(capability: CallableCapability) -> u8 {
    match capability {
        CallableCapability::Readonly => 0,
        CallableCapability::ReadWrite => 1,
        CallableCapability::Owned => 2,
    }
}

/// The authored allocation guarantee carried by a callable contract.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AllocationGuarantee {
    /// The contract makes no allocation guarantee.
    #[default]
    Unspecified,
    /// Calling the value does not request new storage.
    NoAllocation,
}

/// Source-level guarantees that participate in structural callable identity.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableGuarantees {
    allocation: AllocationGuarantee,
}

impl CallableGuarantees {
    #[must_use]
    pub const fn no_allocation() -> Self {
        Self {
            allocation: AllocationGuarantee::NoAllocation,
        }
    }

    #[must_use]
    pub const fn allocation(self) -> AllocationGuarantee {
        self.allocation
    }

    /// Whether a value carrying these guarantees can be used through `expected`.
    ///
    /// Guarantees may be forgotten, but never synthesized. This relation is intentionally
    /// directional: an authored `noalloc` callable may be viewed as an unqualified callable,
    /// while the inverse would make an unproved promise.
    #[must_use]
    pub const fn can_weaken_to(self, expected: Self) -> bool {
        match (self.allocation, expected.allocation) {
            (AllocationGuarantee::NoAllocation, AllocationGuarantee::Unspecified)
            | (AllocationGuarantee::NoAllocation, AllocationGuarantee::NoAllocation)
            | (AllocationGuarantee::Unspecified, AllocationGuarantee::Unspecified) => true,
            (AllocationGuarantee::Unspecified, AllocationGuarantee::NoAllocation) => false,
        }
    }
}

/// A structural callable contract after parameter names have been resolved to positions.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableContract {
    capability: CallableCapability,
    guarantees: CallableGuarantees,
    parameters: Box<[TypeId]>,
    pack: Option<ArgumentPackType>,
    result: TypeId,
    provenance: ResultProvenance,
}

impl CallableContract {
    /// Creates a normalized structural callable contract.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidParameterOrigin`] when result provenance refers to a position outside the
    /// parameter list.
    pub fn new(
        capability: CallableCapability,
        guarantees: CallableGuarantees,
        parameters: impl Into<Box<[TypeId]>>,
        pack: Option<ArgumentPackType>,
        result: TypeId,
        provenance: ResultProvenance,
    ) -> Result<Self, InvalidParameterOrigin> {
        let parameters = parameters.into();
        if let Some(origin) = provenance
            .origins()
            .iter()
            .copied()
            .find(|origin| origin.position() >= parameters.len() + usize::from(pack.is_some()))
        {
            return Err(InvalidParameterOrigin {
                origin,
                parameter_count: parameters.len() + usize::from(pack.is_some()),
            });
        }
        Ok(Self {
            capability,
            guarantees,
            parameters,
            pack,
            result,
            provenance,
        })
    }

    #[must_use]
    pub const fn capability(&self) -> CallableCapability {
        self.capability
    }

    #[must_use]
    pub const fn guarantees(&self) -> CallableGuarantees {
        self.guarantees
    }

    #[must_use]
    pub const fn parameters(&self) -> &[TypeId] {
        &self.parameters
    }

    /// Returns the semantic shape of the final compiler-owned argument pack, when present.
    #[must_use]
    pub const fn pack(&self) -> Option<ArgumentPackType> {
        self.pack
    }

    #[must_use]
    pub const fn result(&self) -> TypeId {
        self.result
    }

    #[must_use]
    pub const fn provenance(&self) -> &ResultProvenance {
        &self.provenance
    }

    /// Whether this callable contract can be exposed as `expected` by forgetting guarantees.
    #[must_use]
    pub fn can_weaken_to(&self, expected: &Self) -> bool {
        self.capability == expected.capability
            && self.parameters == expected.parameters
            && self.pack == expected.pack
            && self.result == expected.result
            && self.provenance == expected.provenance
            && self.guarantees.can_weaken_to(expected.guarantees)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeKind {
    Builtin(BuiltinType),
    GenericParameter(GenericParameterId),
    InterfaceSelf(InterfaceId),
    Nominal {
        definition: NominalTypeId,
        arguments: Box<[TypeId]>,
    },
    AssociatedProjection {
        base: TypeId,
        associated: AssociatedTypeId,
    },
    Opaque {
        definition: OpaqueTypeId,
        arguments: Box<[TypeId]>,
    },
    Pointer(TypeId),
    Borrow {
        capability: BorrowCapability,
        referent: TypeId,
    },
    Slice(TypeId),
    FixedArray {
        element: TypeId,
        length: u64,
    },
    /// One compiler-owned keyed-pack entry. Source syntax cannot name or construct this type.
    PackEntry {
        key: TypeId,
        value: TypeId,
    },
    /// One concrete anonymous closure environment and its statically generated body.
    ///
    /// The signature and environment layout live in the checked-program closure authority. A
    /// [`CallableContract`] may provide the source annotation, but the value retains this concrete
    /// closure identity; callable annotations do not erase or replace its storage layout.
    Closure {
        definition: ClosureId,
        arguments: Box<[TypeId]>,
    },
    Callable(CallableContract),
    Optional(TypeId),
    Fallible(TypeId),
}

impl TypeKind {
    fn references(&self, visit: &mut impl FnMut(TypeId)) {
        match self {
            Self::Builtin(_) | Self::GenericParameter(_) | Self::InterfaceSelf(_) => {}
            Self::Nominal { arguments, .. }
            | Self::Opaque { arguments, .. }
            | Self::Closure { arguments, .. } => {
                arguments.iter().copied().for_each(visit);
            }
            Self::AssociatedProjection { base, .. }
            | Self::Pointer(base)
            | Self::Borrow { referent: base, .. }
            | Self::Slice(base)
            | Self::FixedArray { element: base, .. }
            | Self::Optional(base)
            | Self::Fallible(base) => visit(*base),
            Self::PackEntry { key, value } => {
                visit(*key);
                visit(*value);
            }
            Self::Callable(contract) => {
                contract.parameters().iter().copied().for_each(&mut *visit);
                if let Some(pack) = contract.pack() {
                    pack.visit(visit);
                }
                visit(contract.result());
            }
        }
    }
}

/// Immutable, read-only view of one canonical structural type sequence.
///
/// This value deliberately has no branch-opening or mutation API. Compiler phases that own type
/// construction retain a [`crate::TypeAuthority`]; downstream phases receive only this snapshot.
#[derive(Clone)]
pub struct TypeStore {
    kinds: PersistentVector<TypeKind>,
    properties: PersistentVector<TypeProperties>,
    interned: PersistentMap<TypeKind, TypeId>,
    builtins: [TypeId; BuiltinType::ALL.len()],
}

impl fmt::Debug for TypeStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TypeStore")
            .field("type_count", &self.type_count())
            .finish()
    }
}

impl Default for TypeStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeStore {
    #[must_use]
    pub fn new() -> Self {
        let mut store = Self {
            kinds: PersistentVector::default(),
            properties: PersistentVector::default(),
            interned: PersistentMap::default(),
            builtins: [TypeId::new(0); BuiltinType::ALL.len()],
        };
        for (index, builtin) in BuiltinType::ALL.iter().copied().enumerate() {
            let id = store.insert_known(TypeKind::Builtin(builtin));
            store.builtins[index] = id;
        }
        store
    }

    #[must_use]
    pub const fn builtin(&self, builtin: BuiltinType) -> TypeId {
        self.builtins[builtin.index()]
    }

    /// Reports whether a value of this type can retain a storage origin.
    ///
    /// This conservative structural property does not inspect declaration bodies or allocation.
    #[must_use]
    pub fn may_carry_storage(&self, root: TypeId) -> bool {
        self.properties
            .get(root.index())
            .is_some_and(|properties| properties.may_carry_storage)
    }

    /// Reports whether a type is closed over concrete semantic identities.
    ///
    /// The result is computed once when the type is interned. Structural type references always
    /// point into the existing authority prefix, so this property never requires a graph walk.
    #[must_use]
    pub fn is_concrete(&self, root: TypeId) -> Option<bool> {
        self.properties
            .get(root.index())
            .map(|properties| properties.concrete)
    }

    /// Interns one structural type after checking its referenced type IDs.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownTypeId`] when `kind` refers to a type absent from this store.
    pub(super) fn intern_branch(&mut self, kind: TypeKind) -> Result<TypeId, UnknownTypeId> {
        let mut invalid = None;
        kind.references(&mut |referenced| {
            if invalid.is_none() && self.get(referenced).is_none() {
                invalid = Some(referenced);
            }
        });
        if let Some(invalid) = invalid {
            return Err(UnknownTypeId(invalid));
        }
        if let Some(id) = self.interned.get(&kind) {
            return Ok(*id);
        }
        Ok(self.insert_known(kind))
    }

    #[must_use]
    pub fn get(&self, id: TypeId) -> Option<&TypeKind> {
        self.kinds.get(id.index())
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (TypeId, &TypeKind)> {
        self.kinds
            .iter()
            .enumerate()
            .map(|(index, kind)| (TypeId::new(index), kind))
    }

    /// Iterates identities at or after `start` without visiting the preceding prefix.
    ///
    /// Persistent snapshots use this cursor contract when an analysis has already closed an
    /// immutable prefix and needs to inspect only types appended by its own transaction.
    ///
    /// # Panics
    ///
    /// Panics only if the store's private identity sequence is internally sparse.
    #[must_use]
    pub fn iter_from(&self, start: usize) -> impl ExactSizeIterator<Item = (TypeId, &TypeKind)> {
        let end = self.kinds.len();
        let start = start.min(end);
        (start..end).map(|index| {
            let kind = self
                .kinds
                .get(index)
                .expect("in-range type identity must exist in the persistent sequence");
            (TypeId::new(index), kind)
        })
    }

    #[must_use]
    pub const fn type_count(&self) -> usize {
        self.kinds.len()
    }

    fn insert_known(&mut self, kind: TypeKind) -> TypeId {
        let id = TypeId::new(self.kinds.len());
        let properties = TypeProperties::for_kind(self, &kind);
        let kind = Arc::new(kind);
        self.kinds.push_shared(Arc::clone(&kind));
        self.properties.push(properties);
        assert!(
            self.interned.insert_shared_key_absent(kind, id),
            "known type insertion must be unique"
        );
        debug_assert_eq!(self.interned.len(), self.kinds.len());
        debug_assert_eq!(self.properties.len(), self.kinds.len());
        id
    }
}

#[derive(Clone, Copy, Debug)]
struct TypeProperties {
    may_carry_storage: bool,
    concrete: bool,
}

impl TypeProperties {
    fn for_kind(types: &TypeStore, kind: &TypeKind) -> Self {
        let child = |ty: TypeId| {
            types
                .properties
                .get(ty.index())
                .copied()
                .expect("validated type references must already have structural properties")
        };
        let children_concrete =
            |children: &[TypeId]| children.iter().copied().all(|ty| child(ty).concrete);
        let concrete = match kind {
            TypeKind::GenericParameter(_)
            | TypeKind::InterfaceSelf(_)
            | TypeKind::AssociatedProjection { .. } => false,
            TypeKind::Builtin(_) => true,
            TypeKind::Nominal { arguments, .. }
            | TypeKind::Opaque { arguments, .. }
            | TypeKind::Closure { arguments, .. } => children_concrete(arguments),
            TypeKind::Pointer(base)
            | TypeKind::Borrow { referent: base, .. }
            | TypeKind::Slice(base)
            | TypeKind::FixedArray { element: base, .. }
            | TypeKind::Optional(base)
            | TypeKind::Fallible(base) => child(*base).concrete,
            TypeKind::PackEntry { key, value } => child(*key).concrete && child(*value).concrete,
            TypeKind::Callable(contract) => {
                child(contract.result()).concrete
                    && children_concrete(contract.parameters())
                    && contract.pack().is_none_or(|pack| {
                        child(pack.primary()).concrete
                            && pack.value().is_none_or(|value| child(value).concrete)
                    })
            }
        };
        let may_carry_storage = match kind {
            TypeKind::Builtin(BuiltinType::Str)
            | TypeKind::GenericParameter(_)
            | TypeKind::InterfaceSelf(_)
            | TypeKind::Nominal { .. }
            | TypeKind::AssociatedProjection { .. }
            | TypeKind::Opaque { .. }
            | TypeKind::Pointer(_)
            | TypeKind::Borrow { .. }
            | TypeKind::Slice(_)
            | TypeKind::Closure { .. }
            | TypeKind::Callable(_) => true,
            TypeKind::PackEntry { key, value } => {
                child(*key).may_carry_storage || child(*value).may_carry_storage
            }
            TypeKind::FixedArray { element, .. }
            | TypeKind::Optional(element)
            | TypeKind::Fallible(element) => child(*element).may_carry_storage,
            TypeKind::Builtin(_) => false,
        };
        Self {
            may_carry_storage,
            concrete,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct UnknownTypeId(TypeId);

impl UnknownTypeId {
    pub(crate) const fn new(id: TypeId) -> Self {
        Self(id)
    }

    #[must_use]
    pub const fn id(self) -> TypeId {
        self.0
    }
}

impl fmt::Debug for UnknownTypeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("UnknownTypeId")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for UnknownTypeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "type ID {:?} is not part of this store", self.0)
    }
}

impl std::error::Error for UnknownTypeId {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidParameterOrigin {
    origin: crate::ParameterOrigin,
    parameter_count: usize,
}

impl InvalidParameterOrigin {
    #[must_use]
    pub const fn origin(self) -> crate::ParameterOrigin {
        self.origin
    }

    #[must_use]
    pub const fn parameter_count(self) -> usize {
        self.parameter_count
    }
}

impl fmt::Display for InvalidParameterOrigin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "parameter origin {} is outside a {}-parameter contract",
            self.origin.position(),
            self.parameter_count
        )
    }
}

impl std::error::Error for InvalidParameterOrigin {}

#[cfg(test)]
mod tests {
    use crate::id::SemanticId;
    use crate::{ParameterOrigin, ResultProvenance, TypeAuthority};

    use super::{
        ArgumentPackType, BorrowCapability, BuiltinType, CallableCapability, CallableContract,
        CallableGuarantees, TypeKind, TypeStore,
    };

    #[test]
    fn structural_types_are_interned_without_rendered_names() {
        let base = TypeAuthority::new();
        let mut types = base.transaction();
        let value = types.builtin(BuiltinType::I32);
        let first = types
            .intern(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent: value,
            })
            .unwrap();
        let second = types
            .intern(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent: value,
            })
            .unwrap();

        assert_eq!(first, second);
        assert_eq!(types.type_count(), BuiltinType::ALL.len() + 1);
    }

    #[test]
    fn callable_allocation_guarantee_participates_in_structural_identity() {
        let base = TypeAuthority::new();
        let mut types = base.transaction();
        let result = types.builtin(BuiltinType::Void);
        let ordinary = CallableContract::new(
            CallableCapability::Owned,
            CallableGuarantees::default(),
            [],
            None,
            result,
            ResultProvenance::empty(),
        )
        .unwrap();
        let noalloc = CallableContract::new(
            CallableCapability::Owned,
            CallableGuarantees::no_allocation(),
            [],
            None,
            result,
            ResultProvenance::empty(),
        )
        .unwrap();

        assert_ne!(
            types.intern(TypeKind::Callable(ordinary)).unwrap(),
            types.intern(TypeKind::Callable(noalloc)).unwrap()
        );
    }

    #[test]
    fn transactions_share_the_prefix_and_isolate_sibling_extensions() {
        let base = TypeAuthority::new();
        let value = base.store().builtin(BuiltinType::I32);
        let mut first = base.transaction();
        let mut second = base.transaction();

        let first_extension = first.intern(TypeKind::Optional(value)).unwrap();
        let second_extension = second.intern(TypeKind::Fallible(value)).unwrap();

        assert_eq!(first_extension, second_extension);
        assert_eq!(base.store().get(first_extension), None);
        assert_eq!(first.get(first_extension), Some(&TypeKind::Optional(value)));
        assert_eq!(
            second.get(second_extension),
            Some(&TypeKind::Fallible(value))
        );
    }

    #[test]
    fn builtin_lookup_is_closed_and_canonical() {
        let types = TypeStore::new();

        for builtin in BuiltinType::ALL.iter().copied() {
            assert_eq!(
                types.get(types.builtin(builtin)),
                Some(&TypeKind::Builtin(builtin))
            );
        }
        assert_eq!(types.type_count(), BuiltinType::ALL.len());
    }

    #[test]
    fn callable_identity_uses_parameter_positions_for_provenance() {
        let base = TypeAuthority::new();
        let mut types = base.transaction();
        let text = types.builtin(BuiltinType::Str);
        let borrowed = types
            .intern(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent: text,
            })
            .unwrap();
        let origin = ParameterOrigin::new(0);
        let provenance = ResultProvenance::from_origins([origin]).unwrap();
        let contract = CallableContract::new(
            CallableCapability::Readonly,
            CallableGuarantees::default(),
            [borrowed],
            None,
            borrowed,
            provenance,
        )
        .unwrap();
        let first = types.intern(TypeKind::Callable(contract.clone())).unwrap();
        let second = types.intern(TypeKind::Callable(contract)).unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn callable_identity_distinguishes_a_final_pack_from_an_ordinary_parameter() {
        let base = TypeAuthority::new();
        let mut types = base.transaction();
        let value = types.builtin(BuiltinType::I32);
        let ordinary = CallableContract::new(
            CallableCapability::Owned,
            CallableGuarantees::default(),
            [value],
            None,
            value,
            ResultProvenance::empty(),
        )
        .unwrap();
        let packed = CallableContract::new(
            CallableCapability::Owned,
            CallableGuarantees::default(),
            [],
            Some(ArgumentPackType::Values(value)),
            value,
            ResultProvenance::empty(),
        )
        .unwrap();

        assert_ne!(
            types.intern(TypeKind::Callable(ordinary)).unwrap(),
            types.intern(TypeKind::Callable(packed)).unwrap()
        );
    }

    #[test]
    fn callable_pack_occupies_the_final_provenance_parameter_position() {
        let types = TypeStore::new();
        let value = types.builtin(BuiltinType::I32);
        let origin = ParameterOrigin::new(1);
        let provenance = ResultProvenance::from_origins([origin]).unwrap();

        CallableContract::new(
            CallableCapability::Owned,
            CallableGuarantees::default(),
            [value],
            Some(ArgumentPackType::Values(value)),
            value,
            provenance,
        )
        .unwrap();
    }

    #[test]
    fn stronger_callable_access_permits_every_weaker_body() {
        assert!(CallableCapability::Readonly.permits(CallableCapability::Readonly));
        assert!(!CallableCapability::Readonly.permits(CallableCapability::ReadWrite));
        assert!(CallableCapability::ReadWrite.permits(CallableCapability::Readonly));
        assert!(CallableCapability::ReadWrite.permits(CallableCapability::ReadWrite));
        assert!(!CallableCapability::ReadWrite.permits(CallableCapability::Owned));
        assert!(CallableCapability::Owned.permits(CallableCapability::Readonly));
        assert!(CallableCapability::Owned.permits(CallableCapability::ReadWrite));
        assert!(CallableCapability::Owned.permits(CallableCapability::Owned));
    }

    #[test]
    fn callable_provenance_cannot_escape_its_parameter_list() {
        let types = TypeStore::new();
        let result = types.builtin(BuiltinType::I32);
        let origin = ParameterOrigin::new(1);
        let provenance = ResultProvenance::from_origins([origin]).unwrap();
        let error = CallableContract::new(
            CallableCapability::Owned,
            CallableGuarantees::default(),
            [result],
            None,
            result,
            provenance,
        )
        .unwrap_err();

        assert_eq!(error.origin(), origin);
        assert_eq!(error.parameter_count(), 1);
    }

    #[test]
    fn references_must_belong_to_the_store() {
        let base = TypeAuthority::new();
        let mut types = base.transaction();
        let unknown = crate::TypeId::new(types.type_count() + 10);
        let error = types.intern(TypeKind::Optional(unknown)).unwrap_err();

        assert_eq!(error.id(), unknown);
    }

    #[test]
    fn interface_self_is_keyed_by_its_declaring_interface() {
        let base = TypeAuthority::new();
        let mut types = base.transaction();
        let first_interface = crate::InterfaceId::new(0);
        let second_interface = crate::InterfaceId::new(1);
        let first = types
            .intern(TypeKind::InterfaceSelf(first_interface))
            .unwrap();
        let repeated = types
            .intern(TypeKind::InterfaceSelf(first_interface))
            .unwrap();
        let second = types
            .intern(TypeKind::InterfaceSelf(second_interface))
            .unwrap();

        assert_eq!(first, repeated);
        assert_ne!(first, second);
    }

    #[test]
    fn structural_properties_are_fixed_when_a_type_is_interned() {
        let mut types = TypeAuthority::new().transaction();
        let generic = types
            .intern(TypeKind::GenericParameter(crate::GenericParameterId::new(
                0,
            )))
            .unwrap();
        let generic_optional = types.intern(TypeKind::Optional(generic)).unwrap();
        let integer = types.builtin(BuiltinType::I32);
        let integer_optional = types.intern(TypeKind::Optional(integer)).unwrap();

        assert_eq!(types.is_concrete(generic_optional), Some(false));
        assert_eq!(types.is_concrete(integer_optional), Some(true));
        assert!(types.may_carry_storage(generic_optional));
        assert!(!types.may_carry_storage(integer_optional));
    }

    #[test]
    fn suffix_iteration_starts_at_the_exact_identity_cursor() {
        let mut types = TypeAuthority::new().transaction();
        let cursor = types.type_count();
        let integer = types.builtin(BuiltinType::I32);
        let optional = types.intern(TypeKind::Optional(integer)).unwrap();
        let fallible = types.intern(TypeKind::Fallible(optional)).unwrap();

        assert_eq!(
            types
                .iter_from(cursor)
                .map(|(ty, _)| ty)
                .collect::<Vec<_>>(),
            [optional, fallible]
        );
        assert_eq!(types.iter_from(types.type_count()).len(), 0);
    }
}
