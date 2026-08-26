use std::collections::HashMap;
use std::fmt;

pub use nocter_language::BuiltinType;

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

/// A structural callable contract after parameter names have been resolved to positions.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableContract {
    capability: CallableCapability,
    parameters: Box<[TypeId]>,
    pack: Option<TypeId>,
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
        parameters: impl Into<Box<[TypeId]>>,
        pack: Option<TypeId>,
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
    pub const fn parameters(&self) -> &[TypeId] {
        &self.parameters
    }

    /// Returns the element type of the final compiler-owned argument pack, when present.
    #[must_use]
    pub const fn pack(&self) -> Option<TypeId> {
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
            Self::Callable(contract) => {
                contract.parameters().iter().copied().for_each(&mut *visit);
                contract.pack().into_iter().for_each(&mut *visit);
                visit(contract.result());
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct TypeStore {
    kinds: Vec<TypeKind>,
    interned: HashMap<TypeKind, TypeId>,
    builtins: [TypeId; BuiltinType::ALL.len()],
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
            kinds: Vec::new(),
            interned: HashMap::new(),
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
        let mut pending = vec![root];
        let mut visited = std::collections::HashSet::new();
        while let Some(ty) = pending.pop() {
            if !visited.insert(ty) {
                continue;
            }
            match self.get(ty) {
                Some(
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
                    | TypeKind::Callable(_),
                ) => return true,
                Some(
                    TypeKind::FixedArray { element, .. }
                    | TypeKind::Optional(element)
                    | TypeKind::Fallible(element),
                ) => pending.push(*element),
                Some(TypeKind::Builtin(_)) | None => {}
            }
        }
        false
    }

    /// Interns one structural type after checking its referenced type IDs.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownTypeId`] when `kind` refers to a type absent from this store.
    pub fn intern(&mut self, kind: TypeKind) -> Result<TypeId, UnknownTypeId> {
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

    #[must_use]
    pub const fn len(&self) -> usize {
        self.kinds.len()
    }

    /// Returns whether the store contains no types.
    ///
    /// A constructed store contains the closed built-in set, so this is always false for a valid
    /// `TypeStore`; the method accompanies [`Self::len`] for collection-style inspection.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    fn insert_known(&mut self, kind: TypeKind) -> TypeId {
        let id = TypeId::new(self.kinds.len());
        self.kinds.push(kind.clone());
        assert!(
            self.interned.insert(kind, id).is_none(),
            "known type insertion must be unique"
        );
        id
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
    use crate::{ParameterOrigin, ResultProvenance};

    use super::{
        BorrowCapability, BuiltinType, CallableCapability, CallableContract, TypeKind, TypeStore,
    };

    #[test]
    fn structural_types_are_interned_without_rendered_names() {
        let mut types = TypeStore::new();
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
        assert_eq!(types.len(), BuiltinType::ALL.len() + 1);
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
        assert_eq!(types.len(), BuiltinType::ALL.len());
        assert!(!types.is_empty());
    }

    #[test]
    fn callable_identity_uses_parameter_positions_for_provenance() {
        let mut types = TypeStore::new();
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
        let mut types = TypeStore::new();
        let value = types.builtin(BuiltinType::I32);
        let ordinary = CallableContract::new(
            CallableCapability::Owned,
            [value],
            None,
            value,
            ResultProvenance::empty(),
        )
        .unwrap();
        let packed = CallableContract::new(
            CallableCapability::Owned,
            [],
            Some(value),
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
            [value],
            Some(value),
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
        let mut types = TypeStore::new();
        let unknown = crate::TypeId::new(types.len() + 10);
        let error = types.intern(TypeKind::Optional(unknown)).unwrap_err();

        assert_eq!(error.id(), unknown);
    }

    #[test]
    fn interface_self_is_keyed_by_its_declaring_interface() {
        let mut types = TypeStore::new();
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
}
