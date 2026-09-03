use nocter_model::{
    CaptureId, ExecutableItemId, FieldId, OpaqueTypeId, ParameterId, TypeId, VariantId,
};

/// A concrete destruction recipe retained for storage whose cleanup is deferred to a MIR
/// operation rather than expanded into the caller's control-flow graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirDestructionPlan {
    ty: TypeId,
    kind: MirDestructionKind,
}

impl MirDestructionPlan {
    #[must_use]
    pub const fn new(ty: TypeId, kind: MirDestructionKind) -> Self {
        Self { ty, kind }
    }

    #[must_use]
    pub const fn ty(&self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn kind(&self) -> &MirDestructionKind {
        &self.kind
    }
}

/// One exact concrete destruction shape after all user drop selections have become executable
/// item identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirDestructionKind {
    Struct {
        drop: Option<ExecutableItemId>,
        fields: Box<[MirFieldDestruction]>,
    },
    Enum {
        drop: Option<ExecutableItemId>,
        variants: Box<[MirVariantDestruction]>,
    },
    FixedArray {
        length: u64,
        element: Box<MirDestructionPlan>,
    },
    Tuple(Box<[MirTupleElementDestruction]>),
    Optional(Box<MirDestructionPlan>),
    Fallible {
        success: Option<Box<MirDestructionPlan>>,
        failure: Box<MirDestructionPlan>,
    },
    Error,
    Closure(Box<[MirCaptureDestruction]>),
    Opaque {
        definition: OpaqueTypeId,
        plan: Box<MirDestructionPlan>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirTupleElementDestruction {
    index: usize,
    plan: MirDestructionPlan,
}

impl MirTupleElementDestruction {
    #[must_use]
    pub const fn new(index: usize, plan: MirDestructionPlan) -> Self {
        Self { index, plan }
    }

    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    #[must_use]
    pub const fn plan(&self) -> &MirDestructionPlan {
        &self.plan
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFieldDestruction {
    field: FieldId,
    plan: MirDestructionPlan,
}

impl MirFieldDestruction {
    #[must_use]
    pub const fn new(field: FieldId, plan: MirDestructionPlan) -> Self {
        Self { field, plan }
    }

    #[must_use]
    pub const fn field(&self) -> FieldId {
        self.field
    }

    #[must_use]
    pub const fn plan(&self) -> &MirDestructionPlan {
        &self.plan
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirVariantDestruction {
    variant: VariantId,
    payload: Box<[MirPayloadDestruction]>,
}

impl MirVariantDestruction {
    #[must_use]
    pub fn new(variant: VariantId, payload: impl Into<Box<[MirPayloadDestruction]>>) -> Self {
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
    pub const fn payload(&self) -> &[MirPayloadDestruction] {
        &self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirPayloadDestruction {
    parameter: ParameterId,
    plan: MirDestructionPlan,
}

impl MirPayloadDestruction {
    #[must_use]
    pub const fn new(parameter: ParameterId, plan: MirDestructionPlan) -> Self {
        Self { parameter, plan }
    }

    #[must_use]
    pub const fn parameter(&self) -> ParameterId {
        self.parameter
    }

    #[must_use]
    pub const fn plan(&self) -> &MirDestructionPlan {
        &self.plan
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirCaptureDestruction {
    capture: CaptureId,
    plan: MirDestructionPlan,
}

impl MirCaptureDestruction {
    #[must_use]
    pub const fn new(capture: CaptureId, plan: MirDestructionPlan) -> Self {
        Self { capture, plan }
    }

    #[must_use]
    pub const fn capture(&self) -> CaptureId {
        self.capture
    }

    #[must_use]
    pub const fn plan(&self) -> &MirDestructionPlan {
        &self.plan
    }
}
