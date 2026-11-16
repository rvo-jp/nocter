use std::collections::BTreeMap;
#[cfg(test)]
mod callable_alternatives_tests;

use nocter_model::{BorrowCapability, TypeId};

/// Source-independent data identity within one frozen runtime environment.
/// Direct source shapes preserve their semantic ID; contextual shapes use a local representation
/// index and never manufacture another semantic type in the declaration authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeDataId {
    Source(TypeId),
    Instance(u32),
}
impl RuntimeDataId {
    #[must_use]
    pub const fn source(self) -> Option<TypeId> {
        match self {
            Self::Source(data) => Some(data),
            Self::Instance(_) => None,
        }
    }
}
impl From<TypeId> for RuntimeDataId {
    fn from(data: TypeId) -> Self {
        Self::Source(data)
    }
}

/// Closed primitive identities retained past semantic lowering.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimePrimitive {
    Bool,
    Char,
    Float32,
    Float64,
    Signed(u16),
    Unsigned(u16),
    Isize,
    Usize,
    Error,
    Text,
    Void,
    Never,
}

impl RuntimePrimitive {
    /// Unsigned index representation for a non-singleton finite callable domain.
    /// This numeric contract supplies no selected case or primitive runtime identity.
    #[must_use]
    pub fn callable_tag(case_count: usize) -> Option<Self> {
        let largest = case_count.checked_sub(1).filter(|_| case_count >= 2)?;
        let bits = if u8::try_from(largest).is_ok() {
            8
        } else if u16::try_from(largest).is_ok() {
            16
        } else if u32::try_from(largest).is_ok() {
            32
        } else {
            64
        };
        Some(Self::Unsigned(bits))
    }

    /// The representation boundary for exact source builtin identities.
    #[must_use]
    pub fn for_builtin(builtin: nocter_model::BuiltinType) -> Self {
        use nocter_model::BuiltinType;
        match builtin {
            BuiltinType::Bool => Self::Bool,
            BuiltinType::Char => Self::Char,
            BuiltinType::F32 => Self::Float32,
            BuiltinType::F64 => Self::Float64,
            BuiltinType::I8
            | BuiltinType::I16
            | BuiltinType::I32
            | BuiltinType::I64
            | BuiltinType::U8
            | BuiltinType::U16
            | BuiltinType::U32
            | BuiltinType::U64 => {
                let Some(nocter_model::BuiltinNumericType::Integer { bits, signed }) =
                    nocter_model::BuiltinNumericType::for_builtin(builtin)
                else {
                    unreachable!("integer builtin has an integer profile")
                };
                if signed {
                    Self::Signed(u16::from(bits))
                } else {
                    Self::Unsigned(u16::from(bits))
                }
            }
            BuiltinType::Isize => Self::Isize,
            BuiltinType::Usize => Self::Usize,
            BuiltinType::Error => Self::Error,
            BuiltinType::Str => Self::Text,
            BuiltinType::Void => Self::Void,
            BuiltinType::Never => Self::Never,
        }
    }

    /// Integer range policy shared with source typing and constant evaluation.
    #[must_use]
    pub fn integer(self) -> Option<nocter_model::BuiltinNumericType> {
        nocter_model::BuiltinType::ALL
            .iter()
            .copied()
            .find(|&builtin| Self::for_builtin(builtin) == self)
            .and_then(nocter_model::BuiltinNumericType::for_builtin)
            .filter(|numeric| numeric.integer_bounds().is_some())
    }
}

/// One fully concrete runtime type. Symbolic semantic forms are intentionally unrepresentable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeType {
    /// An explicitly empty represented domain. It has no value, addressable payload or layout.
    /// Outcome descriptors may retain this identity for an excluded alternative.
    Uninhabited,
    Primitive(RuntimePrimitive),
    /// A storable unit value, distinct from completion-only primitive Void.
    Unit,
    /// The code witness is selected statically; only the optional environment occupies storage.
    StaticCallable {
        environment: Option<RuntimeDataId>,
    },
    /// Inline finite choice of complete static callable identities. Order is the upstream
    /// canonical witness order, not runtime-ID order. The selected case needs a runtime tag.
    CallableAlternatives(Box<[RuntimeDataId]>),
    Pointer(RuntimeDataId),
    Borrow {
        capability: BorrowCapability,
        referent: RuntimeDataId,
    },
    Slice(RuntimeDataId),
    FixedArray {
        element: RuntimeDataId,
        length: u64,
    },
    /// A v2 zero-count array. The established element identity is metadata, not a stored-payload
    /// dependency; no runtime entry or geometry for that element is required by this shape.
    EmptyArray {
        element: RuntimeDataId,
    },
    Tuple(Box<[RuntimeDataId]>),
    PackEntry {
        key: RuntimeDataId,
        value: RuntimeDataId,
    },
    Aggregate,
    Closure,
    /// Owning erased callable: environment, invoke entry, destroy entry, and mapped byte count.
    ErasedCallable,
    Callable,
    Optional(RuntimeDataId),
    Fallible(RuntimeDataId),
    /// One owning, type-erased deferred computation whose eventual output has this type.
    Future(RuntimeDataId),
    /// One compiler-owned, source-opaque storage representation.
    Storage(super::RuntimeStorageRole),
    Opaque,
}

impl RuntimeType {
    /// Homogeneous source component identity and logical count, independent of empty geometry.
    #[must_use]
    pub const fn array(&self) -> Option<(RuntimeDataId, u64)> {
        match self {
            Self::FixedArray { element, length } => Some((*element, *length)),
            Self::EmptyArray { element } => Some((*element, 0)),
            _ => None,
        }
    }
}

/// Closed semantic IDs paired only with concrete runtime shapes.
#[derive(Clone, Debug, Default)]
pub struct RuntimeTypeTable {
    entries: BTreeMap<RuntimeDataId, RuntimeType>,
    primitives: BTreeMap<RuntimePrimitive, RuntimeDataId>,
}

impl RuntimeTypeTable {
    #[must_use]
    pub fn get(&self, ty: RuntimeDataId) -> Option<&RuntimeType> {
        self.entries.get(&ty)
    }

    #[must_use]
    pub fn primitive(&self, primitive: RuntimePrimitive) -> Option<RuntimeDataId> {
        self.primitives.get(&primitive).copied()
    }

    /// The semantic primary/alternate payload of a builtin outcome, independently of tag bytes.
    #[must_use]
    pub fn outcome_payload(&self, data: RuntimeDataId, primary: bool) -> Option<RuntimeDataId> {
        match self.get(data)? {
            RuntimeType::Optional(payload) if primary => Some(*payload),
            RuntimeType::Fallible(payload) if primary => Some(*payload),
            RuntimeType::Fallible(_) => self.primitive(RuntimePrimitive::Error),
            _ => None,
        }
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (RuntimeDataId, &RuntimeType)> {
        self.entries.iter().map(|(ty, kind)| (*ty, kind))
    }
}

#[derive(Debug, Default)]
pub struct RuntimeTypeTableBuilder {
    entries: BTreeMap<RuntimeDataId, RuntimeType>,
    primitives: BTreeMap<RuntimePrimitive, RuntimeDataId>,
}

impl RuntimeTypeTableBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one concrete type identity without replacing an existing contract.
    ///
    /// # Errors
    ///
    /// Rejects a repeated semantic type identity or primitive role.
    pub fn insert(
        &mut self,
        ty: impl Into<RuntimeDataId>,
        kind: RuntimeType,
    ) -> Result<(), RuntimeTypeTableBuildError> {
        let ty = ty.into();
        if self.entries.contains_key(&ty) {
            return Err(RuntimeTypeTableBuildError::DuplicateType(ty));
        }
        if let RuntimeType::Primitive(primitive) = kind
            && self.primitives.contains_key(&primitive)
        {
            return Err(RuntimeTypeTableBuildError::DuplicatePrimitive(primitive));
        }
        if let RuntimeType::Primitive(primitive) = kind {
            self.primitives.insert(primitive, ty);
        }
        self.entries.insert(ty, kind);
        Ok(())
    }

    fn validate_callable_alternatives(
        &self,
        owner: RuntimeDataId,
        cases: &[RuntimeDataId],
    ) -> Result<(), RuntimeTypeTableBuildError> {
        let mut unique = std::collections::BTreeSet::new();
        if cases.len() < 2 {
            return Err(RuntimeTypeTableBuildError::InvalidCallableAlternatives(
                owner,
            ));
        }
        for case in cases {
            match self.entries.get(case) {
                None => {
                    return Err(RuntimeTypeTableBuildError::UnknownReference {
                        owner,
                        referenced: *case,
                    });
                }
                Some(RuntimeType::StaticCallable { .. }) if unique.insert(*case) => {}
                _ => {
                    return Err(RuntimeTypeTableBuildError::InvalidCallableAlternatives(
                        owner,
                    ));
                }
            }
        }
        Ok(())
    }

    /// Freezes a closed table after validating every stored-payload reference. Empty-array
    /// element identities are established upstream and require no stored runtime entry.
    ///
    /// # Errors
    ///
    /// Rejects a type whose concrete stored payload refers to an absent runtime identity.
    pub fn finish(self) -> Result<RuntimeTypeTable, RuntimeTypeTableBuildError> {
        for (owner, kind) in &self.entries {
            let referenced = match kind {
                RuntimeType::Pointer(ty)
                | RuntimeType::Slice(ty)
                | RuntimeType::Optional(ty)
                | RuntimeType::Fallible(ty)
                | RuntimeType::Future(ty) => Some(*ty),
                RuntimeType::Borrow { referent, .. } => Some(*referent),
                RuntimeType::StaticCallable { environment } => *environment,
                RuntimeType::CallableAlternatives(cases) => {
                    self.validate_callable_alternatives(*owner, cases)?;
                    None
                }
                RuntimeType::FixedArray { element, .. } => Some(*element),
                RuntimeType::Tuple(elements) => {
                    if elements.len() < 2 {
                        return Err(RuntimeTypeTableBuildError::InvalidTupleArity {
                            owner: *owner,
                            actual: elements.len(),
                        });
                    }
                    for referenced in elements {
                        if !self.entries.contains_key(referenced) {
                            return Err(RuntimeTypeTableBuildError::UnknownReference {
                                owner: *owner,
                                referenced: *referenced,
                            });
                        }
                    }
                    None
                }
                RuntimeType::PackEntry { key, value } => {
                    if !self.entries.contains_key(key) {
                        return Err(RuntimeTypeTableBuildError::UnknownReference {
                            owner: *owner,
                            referenced: *key,
                        });
                    }
                    Some(*value)
                }
                RuntimeType::Primitive(_)
                | RuntimeType::Uninhabited
                | RuntimeType::EmptyArray { .. }
                | RuntimeType::Unit
                | RuntimeType::Aggregate
                | RuntimeType::Closure
                | RuntimeType::ErasedCallable
                | RuntimeType::Callable
                | RuntimeType::Storage(_)
                | RuntimeType::Opaque => None,
            };
            if let Some(referenced) = referenced
                && !self.entries.contains_key(&referenced)
            {
                return Err(RuntimeTypeTableBuildError::UnknownReference {
                    owner: *owner,
                    referenced,
                });
            }
        }
        Ok(RuntimeTypeTable {
            entries: self.entries,
            primitives: self.primitives,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeTypeTableBuildError {
    DuplicateType(RuntimeDataId),
    InvalidCallableAlternatives(RuntimeDataId),
    DuplicatePrimitive(RuntimePrimitive),
    InvalidTupleArity {
        owner: RuntimeDataId,
        actual: usize,
    },
    UnknownReference {
        owner: RuntimeDataId,
        referenced: RuntimeDataId,
    },
}

impl std::fmt::Display for RuntimeTypeTableBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid runtime type table: {self:?}")
    }
}

impl std::error::Error for RuntimeTypeTableBuildError {}

#[cfg(test)]
mod tests {
    use nocter_model::{BuiltinType, TypeStore};

    use super::{
        RuntimePrimitive, RuntimeType, RuntimeTypeTableBuildError, RuntimeTypeTableBuilder,
    };

    #[test]
    fn contextual_callable_shapes_have_independent_closed_storage_identities() {
        use crate::RuntimeDataId;
        let source = TypeStore::new();
        let boolean = RuntimeDataId::from(source.builtin(BuiltinType::Bool));
        let captured = RuntimeDataId::Instance(0);
        let empty = RuntimeDataId::Instance(1);
        let mut builder = RuntimeTypeTableBuilder::new();
        builder
            .insert(boolean, RuntimeType::Primitive(RuntimePrimitive::Bool))
            .unwrap();
        builder
            .insert(
                captured,
                RuntimeType::StaticCallable {
                    environment: Some(boolean),
                },
            )
            .unwrap();
        builder
            .insert(empty, RuntimeType::StaticCallable { environment: None })
            .unwrap();
        let table = builder.finish().unwrap();
        assert_eq!(
            table.get(captured),
            Some(&RuntimeType::StaticCallable {
                environment: Some(boolean)
            })
        );
        assert_eq!(
            table.get(empty),
            Some(&RuntimeType::StaticCallable { environment: None })
        );
        assert_eq!(boolean.source(), Some(source.builtin(BuiltinType::Bool)));
        assert!(captured.source().is_none());
        assert_eq!(table.primitive(RuntimePrimitive::Bool), Some(boolean));
    }

    #[test]
    fn builder_rejects_duplicate_primitive_authority() {
        let mut builder = RuntimeTypeTableBuilder::new();
        let semantic = TypeStore::new();
        let first: crate::RuntimeDataId = semantic.builtin(BuiltinType::Bool).into();
        let second: crate::RuntimeDataId = semantic.builtin(BuiltinType::I8).into();
        builder
            .insert(first, RuntimeType::Primitive(RuntimePrimitive::Bool))
            .unwrap();

        assert_eq!(
            builder
                .insert(second, RuntimeType::Primitive(RuntimePrimitive::Bool))
                .unwrap_err(),
            RuntimeTypeTableBuildError::DuplicatePrimitive(RuntimePrimitive::Bool)
        );
    }

    #[test]
    fn finish_rejects_dangling_concrete_type_references() {
        let mut builder = RuntimeTypeTableBuilder::new();
        let semantic = TypeStore::new();
        let owner: crate::RuntimeDataId = semantic.builtin(BuiltinType::Bool).into();
        let referenced: crate::RuntimeDataId = semantic.builtin(BuiltinType::I8).into();
        builder
            .insert(owner, RuntimeType::Pointer(referenced))
            .unwrap();

        assert_eq!(
            builder.finish().unwrap_err(),
            RuntimeTypeTableBuildError::UnknownReference { owner, referenced }
        );
    }

    #[test]
    fn empty_array_identity_is_not_a_stored_payload_reference() {
        let semantic = TypeStore::new();
        let owner: crate::RuntimeDataId = semantic.builtin(BuiltinType::Bool).into();
        let element: crate::RuntimeDataId = semantic.builtin(BuiltinType::I32).into();
        let mut builder = RuntimeTypeTableBuilder::new();
        builder
            .insert(owner, RuntimeType::EmptyArray { element })
            .unwrap();
        let table = builder.finish().unwrap();
        assert_eq!(table.get(owner).unwrap().array(), Some((element, 0)));
        assert!(table.get(element).is_none());
        let mut released = RuntimeTypeTableBuilder::new();
        released
            .insert(owner, RuntimeType::FixedArray { element, length: 0 })
            .unwrap();
        assert_eq!(
            released.finish().unwrap_err(),
            RuntimeTypeTableBuildError::UnknownReference {
                owner,
                referenced: element
            }
        );
    }

    #[test]
    fn static_callable_environment_requires_a_closed_type_reference() {
        let semantic = TypeStore::new();
        let owner: crate::RuntimeDataId = semantic.builtin(BuiltinType::Bool).into();
        let referenced: crate::RuntimeDataId = semantic.builtin(BuiltinType::I8).into();
        let mut builder = RuntimeTypeTableBuilder::new();
        builder
            .insert(
                owner,
                RuntimeType::StaticCallable {
                    environment: Some(referenced),
                },
            )
            .unwrap();
        assert_eq!(
            builder.finish().unwrap_err(),
            RuntimeTypeTableBuildError::UnknownReference { owner, referenced }
        );
    }

    #[test]
    fn finish_rejects_a_tuple_shape_outside_the_language_arity() {
        let mut builder = RuntimeTypeTableBuilder::new();
        let semantic = TypeStore::new();
        let owner: crate::RuntimeDataId = semantic.builtin(BuiltinType::Bool).into();
        builder
            .insert(owner, RuntimeType::Tuple(Box::new([])))
            .unwrap();

        assert_eq!(
            builder.finish().unwrap_err(),
            RuntimeTypeTableBuildError::InvalidTupleArity { owner, actual: 0 }
        );
    }
}
