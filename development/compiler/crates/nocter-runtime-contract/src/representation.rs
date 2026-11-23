use crate::RuntimeDataId;
use std::collections::BTreeMap;

use nocter_model::{AccessFieldId, CaptureId, FieldId, ParameterId, ValueField, VariantId};

/// A value-model aggregate component, preserving its logical selector independently of layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeValueFieldRepresentation {
    field: ValueField,
    ty: RuntimeDataId,
}

impl RuntimeValueFieldRepresentation {
    #[must_use]
    pub fn new(field: ValueField, ty: impl Into<RuntimeDataId>) -> Self {
        Self {
            field,
            ty: ty.into(),
        }
    }

    #[must_use]
    pub const fn field(self) -> ValueField {
        self.field
    }

    #[must_use]
    pub const fn ty(self) -> RuntimeDataId {
        self.ty
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCaptureRepresentation {
    capture: CaptureId,
    ty: RuntimeDataId,
}

impl RuntimeCaptureRepresentation {
    #[must_use]
    pub fn new(capture: CaptureId, ty: impl Into<RuntimeDataId>) -> Self {
        Self {
            capture,
            ty: ty.into(),
        }
    }

    #[must_use]
    pub const fn capture(self) -> CaptureId {
        self.capture
    }

    #[must_use]
    pub const fn ty(self) -> RuntimeDataId {
        self.ty
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeFieldRepresentation {
    field: FieldId,
    ty: RuntimeDataId,
}

impl RuntimeFieldRepresentation {
    #[must_use]
    pub fn new(field: FieldId, ty: impl Into<RuntimeDataId>) -> Self {
        Self {
            field,
            ty: ty.into(),
        }
    }

    #[must_use]
    pub const fn field(self) -> FieldId {
        self.field
    }

    #[must_use]
    pub const fn ty(self) -> RuntimeDataId {
        self.ty
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePayloadRepresentation {
    parameter: ParameterId,
    ty: RuntimeDataId,
}

impl RuntimePayloadRepresentation {
    #[must_use]
    pub fn new(parameter: ParameterId, ty: impl Into<RuntimeDataId>) -> Self {
        Self {
            parameter,
            ty: ty.into(),
        }
    }

    #[must_use]
    pub const fn parameter(self) -> ParameterId {
        self.parameter
    }

    #[must_use]
    pub const fn ty(self) -> RuntimeDataId {
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

/// A v2 enum variant and its stored fields in declaration order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeValueVariantRepresentation {
    variant: VariantId,
    fields: Box<[(AccessFieldId, RuntimeDataId)]>,
}
impl RuntimeValueVariantRepresentation {
    #[must_use]
    pub fn new(
        variant: VariantId,
        fields: impl Into<Box<[(AccessFieldId, RuntimeDataId)]>>,
    ) -> Self {
        Self {
            variant,
            fields: fields.into(),
        }
    }
    #[must_use]
    pub const fn variant(&self) -> VariantId {
        self.variant
    }
    #[must_use]
    pub const fn fields(&self) -> &[(AccessFieldId, RuntimeDataId)] {
        &self.fields
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeTypeRepresentation {
    /// A storable v2 immutable UTF-8 descriptor. Backing extent belongs to its producer.
    ValueText,
    ValueAggregate {
        fields: Box<[RuntimeValueFieldRepresentation]>,
    },
    /// A v2 payloadless enum in declaration order, retaining nominal variant identities.
    ScalarEnum {
        variants: Box<[VariantId]>,
    },
    ValueEnum {
        variants: Box<[RuntimeValueVariantRepresentation]>,
    },
    Struct {
        fields: Box<[RuntimeFieldRepresentation]>,
    },
    Enum {
        variants: Box<[RuntimeVariantRepresentation]>,
    },
    Opaque {
        witness: RuntimeDataId,
    },
    Closure {
        captures: Box<[RuntimeCaptureRepresentation]>,
    },
}

/// Complete specialized runtime members and opaque witnesses, keyed by concrete type.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeTypeRepresentationTable {
    entries: BTreeMap<RuntimeDataId, RuntimeTypeRepresentation>,
}

impl RuntimeTypeRepresentationTable {
    #[must_use]
    pub fn new(entries: BTreeMap<RuntimeDataId, RuntimeTypeRepresentation>) -> Self {
        Self { entries }
    }

    /// Adapt an established source-identity map at the semantic-to-runtime boundary.
    #[must_use]
    pub fn from_source_entries(
        entries: BTreeMap<nocter_model::TypeId, RuntimeTypeRepresentation>,
    ) -> Self {
        Self::new(
            entries
                .into_iter()
                .map(|(source, representation)| (source.into(), representation))
                .collect(),
        )
    }

    #[must_use]
    pub fn get(&self, ty: RuntimeDataId) -> Option<&RuntimeTypeRepresentation> {
        self.entries.get(&ty)
    }

    #[must_use]
    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (RuntimeDataId, &RuntimeTypeRepresentation)> {
        self.entries
            .iter()
            .map(|(ty, representation)| (*ty, representation))
    }
}
