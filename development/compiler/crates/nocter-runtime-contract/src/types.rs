use std::collections::BTreeMap;

use nocter_model::{BorrowCapability, TypeId};

/// Closed primitive identities retained past semantic lowering.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimePrimitive {
    Bool,
    Signed(u16),
    Unsigned(u16),
    Isize,
    Usize,
    Error,
    Text,
    Void,
    Never,
}

/// One fully concrete runtime type. Symbolic semantic forms are intentionally unrepresentable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeType {
    Primitive(RuntimePrimitive),
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
    Aggregate,
    Closure,
    Callable,
    Optional(TypeId),
    Fallible(TypeId),
    Opaque,
}

/// Closed semantic IDs paired only with concrete runtime shapes.
#[derive(Clone, Debug, Default)]
pub struct RuntimeTypeTable {
    entries: BTreeMap<TypeId, RuntimeType>,
    primitives: BTreeMap<RuntimePrimitive, TypeId>,
}

impl RuntimeTypeTable {
    #[must_use]
    pub fn get(&self, ty: TypeId) -> Option<&RuntimeType> {
        self.entries.get(&ty)
    }

    #[must_use]
    pub fn primitive(&self, primitive: RuntimePrimitive) -> Option<TypeId> {
        self.primitives.get(&primitive).copied()
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (TypeId, &RuntimeType)> {
        self.entries.iter().map(|(ty, kind)| (*ty, kind))
    }
}

#[derive(Debug, Default)]
pub struct RuntimeTypeTableBuilder {
    entries: BTreeMap<TypeId, RuntimeType>,
    primitives: BTreeMap<RuntimePrimitive, TypeId>,
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
        ty: TypeId,
        kind: RuntimeType,
    ) -> Result<(), RuntimeTypeTableBuildError> {
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

    /// Freezes a closed table after validating every referenced runtime type.
    ///
    /// # Errors
    ///
    /// Rejects a type whose concrete runtime shape refers to an absent identity.
    pub fn finish(self) -> Result<RuntimeTypeTable, RuntimeTypeTableBuildError> {
        for (owner, kind) in &self.entries {
            let referenced = match kind {
                RuntimeType::Pointer(ty)
                | RuntimeType::Slice(ty)
                | RuntimeType::Optional(ty)
                | RuntimeType::Fallible(ty) => Some(*ty),
                RuntimeType::Borrow { referent, .. } => Some(*referent),
                RuntimeType::FixedArray { element, .. } => Some(*element),
                RuntimeType::Primitive(_)
                | RuntimeType::Aggregate
                | RuntimeType::Closure
                | RuntimeType::Callable
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
    DuplicateType(TypeId),
    DuplicatePrimitive(RuntimePrimitive),
    UnknownReference { owner: TypeId, referenced: TypeId },
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
    fn builder_rejects_duplicate_primitive_authority() {
        let mut builder = RuntimeTypeTableBuilder::new();
        let semantic = TypeStore::new();
        let first = semantic.builtin(BuiltinType::Bool);
        let second = semantic.builtin(BuiltinType::I8);
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
        let owner = semantic.builtin(BuiltinType::Bool);
        let referenced = semantic.builtin(BuiltinType::I8);
        builder
            .insert(owner, RuntimeType::Pointer(referenced))
            .unwrap();

        assert_eq!(
            builder.finish().unwrap_err(),
            RuntimeTypeTableBuildError::UnknownReference { owner, referenced }
        );
    }
}
