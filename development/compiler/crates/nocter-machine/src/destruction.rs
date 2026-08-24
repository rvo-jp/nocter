use nocter_model::TypeId;

use crate::MachineFunctionId;

/// One field cleanup at its frozen byte offset.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MachineDestructionField {
    offset: u64,
    plan: MachineDestructionPlan,
}

impl MachineDestructionField {
    pub(crate) const fn new(offset: u64, plan: MachineDestructionPlan) -> Self {
        Self { offset, plan }
    }

    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    #[must_use]
    pub const fn plan(&self) -> &MachineDestructionPlan {
        &self.plan
    }
}

/// One enum payload cleanup at its frozen byte offset.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MachineDestructionPayload {
    offset: u64,
    plan: MachineDestructionPlan,
}

impl MachineDestructionPayload {
    pub(crate) const fn new(offset: u64, plan: MachineDestructionPlan) -> Self {
        Self { offset, plan }
    }

    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    #[must_use]
    pub const fn plan(&self) -> &MachineDestructionPlan {
        &self.plan
    }
}

/// Destruction for one active enum tag.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MachineDestructionVariant {
    tag: u8,
    payload: Box<[MachineDestructionPayload]>,
}

impl MachineDestructionVariant {
    pub(crate) fn new(tag: u8, payload: impl Into<Box<[MachineDestructionPayload]>>) -> Self {
        Self {
            tag,
            payload: payload.into(),
        }
    }

    #[must_use]
    pub const fn tag(&self) -> u8 {
        self.tag
    }

    #[must_use]
    pub const fn payload(&self) -> &[MachineDestructionPayload] {
        &self.payload
    }
}

/// One closure-capture cleanup at its frozen byte offset.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MachineDestructionCapture {
    offset: u64,
    plan: MachineDestructionPlan,
}

impl MachineDestructionCapture {
    pub(crate) const fn new(offset: u64, plan: MachineDestructionPlan) -> Self {
        Self { offset, plan }
    }

    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    #[must_use]
    pub const fn plan(&self) -> &MachineDestructionPlan {
        &self.plan
    }
}

/// A destruction recipe expressed only in machine functions, tags, strides, and byte offsets.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MachineDestructionKind {
    Struct {
        drop: Option<MachineFunctionId>,
        fields: Box<[MachineDestructionField]>,
    },
    Enum {
        drop: Option<MachineFunctionId>,
        tag_offset: u64,
        variants: Box<[MachineDestructionVariant]>,
    },
    FixedArray {
        length: u64,
        stride: u64,
        element: Box<MachineDestructionPlan>,
    },
    Outcome {
        tag_offset: u64,
        payload_offset: u64,
        active_tag: u8,
        payload: Box<MachineDestructionPlan>,
    },
    Fallible {
        tag_offset: u64,
        payload_offset: u64,
        success: Option<Box<MachineDestructionPlan>>,
        failure: Box<MachineDestructionPlan>,
    },
    Error,
    Closure(Box<[MachineDestructionCapture]>),
    Opaque(Box<MachineDestructionPlan>),
}

/// One exact cleanup plan for bytes with a frozen stored layout.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MachineDestructionPlan {
    ty: TypeId,
    size: u64,
    alignment: u64,
    kind: MachineDestructionKind,
}

impl MachineDestructionPlan {
    pub(crate) const fn new(
        ty: TypeId,
        size: u64,
        alignment: u64,
        kind: MachineDestructionKind,
    ) -> Self {
        Self {
            ty,
            size,
            alignment,
            kind,
        }
    }

    #[must_use]
    pub const fn ty(&self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    #[must_use]
    pub const fn kind(&self) -> &MachineDestructionKind {
        &self.kind
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineDestructionError {
    InvalidLayout(TypeId),
    MissingMember(TypeId),
}
