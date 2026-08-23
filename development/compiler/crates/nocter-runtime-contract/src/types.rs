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

/// Dense semantic IDs paired only with concrete runtime shapes.
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

    pub fn insert(&mut self, ty: TypeId, kind: RuntimeType) -> Option<RuntimeType> {
        if let RuntimeType::Primitive(primitive) = kind {
            self.primitives.insert(primitive, ty);
        }
        self.entries.insert(ty, kind)
    }

    #[must_use]
    pub fn finish(self) -> RuntimeTypeTable {
        RuntimeTypeTable {
            entries: self.entries,
            primitives: self.primitives,
        }
    }
}
