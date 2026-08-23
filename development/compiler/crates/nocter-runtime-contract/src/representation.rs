use std::collections::BTreeMap;

use nocter_model::{CaptureId, FieldId, ParameterId, TypeId, VariantId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCaptureRepresentation {
    capture: CaptureId,
    ty: TypeId,
}

impl RuntimeCaptureRepresentation {
    #[must_use]
    pub const fn new(capture: CaptureId, ty: TypeId) -> Self {
        Self { capture, ty }
    }

    #[must_use]
    pub const fn capture(self) -> CaptureId {
        self.capture
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeFieldRepresentation {
    field: FieldId,
    ty: TypeId,
}

impl RuntimeFieldRepresentation {
    #[must_use]
    pub const fn new(field: FieldId, ty: TypeId) -> Self {
        Self { field, ty }
    }

    #[must_use]
    pub const fn field(self) -> FieldId {
        self.field
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePayloadRepresentation {
    parameter: ParameterId,
    ty: TypeId,
}

impl RuntimePayloadRepresentation {
    #[must_use]
    pub const fn new(parameter: ParameterId, ty: TypeId) -> Self {
        Self { parameter, ty }
    }

    #[must_use]
    pub const fn parameter(self) -> ParameterId {
        self.parameter
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeVariantRepresentation {
    variant: VariantId,
    payload: Box<[RuntimePayloadRepresentation]>,
}

impl RuntimeVariantRepresentation {
    #[must_use]
    pub fn new(
        variant: VariantId,
        payload: impl Into<Box<[RuntimePayloadRepresentation]>>,
    ) -> Self {
        Self {
            variant,
            payload: payload.into(),
        }
    }

    #[must_use]
    pub const fn variant(&self) -> VariantId {
        self.variant
    }

    #[must_use]
    pub const fn payload(&self) -> &[RuntimePayloadRepresentation] {
        &self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeTypeRepresentation {
    Struct {
        fields: Box<[RuntimeFieldRepresentation]>,
    },
    Enum {
        variants: Box<[RuntimeVariantRepresentation]>,
    },
    Opaque {
        witness: TypeId,
    },
    Closure {
        captures: Box<[RuntimeCaptureRepresentation]>,
    },
}

/// Complete specialized runtime members and opaque witnesses, keyed by concrete type.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeTypeRepresentationTable {
    entries: BTreeMap<TypeId, RuntimeTypeRepresentation>,
}

impl RuntimeTypeRepresentationTable {
    #[must_use]
    pub fn new(entries: BTreeMap<TypeId, RuntimeTypeRepresentation>) -> Self {
        Self { entries }
    }

    #[must_use]
    pub fn get(&self, ty: TypeId) -> Option<&RuntimeTypeRepresentation> {
        self.entries.get(&ty)
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (TypeId, &RuntimeTypeRepresentation)> {
        self.entries
            .iter()
            .map(|(ty, representation)| (*ty, representation))
    }
}
