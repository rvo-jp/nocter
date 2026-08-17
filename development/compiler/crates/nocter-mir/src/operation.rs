use nocter_model::{
    BorrowCapability, CaptureId, ExecutableItemId, FieldId, MirDropFlagId, MirLocalId, MirPlaceId,
    MirValueId, NominalTypeId, TypeId, VariantId,
};
use nocter_target_program::PrimitiveRole;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirConstant {
    Bool(bool),
    Integer(i128),
    Text(Box<str>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirReadMode {
    Copy,
    Move,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirUnaryOperation {
    LogicalNot,
    Negate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirBinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    ShiftLeft,
    ShiftRightSigned,
    ShiftRightUnsigned,
    Equal,
    Less,
}

/// One binding-preserving value supplied to a concrete closure environment field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirClosureCapture {
    binding: CaptureId,
    value: MirValueId,
}

impl MirClosureCapture {
    #[must_use]
    pub const fn new(binding: CaptureId, value: MirValueId) -> Self {
        Self { binding, value }
    }

    #[must_use]
    pub const fn binding(self) -> CaptureId {
        self.binding
    }

    #[must_use]
    pub const fn value(self) -> MirValueId {
        self.value
    }
}

/// One concrete aggregate value assembled in source-defined member order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirAggregate {
    Struct {
        definition: NominalTypeId,
        fields: Box<[(FieldId, MirValueId)]>,
    },
    Enum {
        variant: VariantId,
        payload: Box<[MirValueId]>,
    },
    FixedArray(Box<[MirValueId]>),
    Optional(Option<MirValueId>),
    FallibleSuccess(MirValueId),
    FallibleFailure(MirValueId),
    Closure {
        body: ExecutableItemId,
        captures: Box<[MirClosureCapture]>,
    },
    Opaque {
        witness: MirValueId,
    },
}

/// A compiler-owned structural call after all interface and coercion selection is complete.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirStructuralCall {
    Equality {
        subject: TypeId,
        operand: TypeId,
    },
    Ordering {
        subject: TypeId,
        operand: TypeId,
    },
    Index {
        capability: BorrowCapability,
        container: TypeId,
        receiver: TypeId,
        index: TypeId,
        result: TypeId,
    },
    BorrowWeakening {
        source: TypeId,
        target: TypeId,
    },
}

/// Concrete runtime signature retained by a call target without source binding metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirCallSignature {
    parameters: Box<[TypeId]>,
    result: TypeId,
}

impl MirCallSignature {
    #[must_use]
    pub fn new(parameters: impl Into<Box<[TypeId]>>, result: TypeId) -> Self {
        Self {
            parameters: parameters.into(),
            result,
        }
    }

    #[must_use]
    pub const fn parameters(&self) -> &[TypeId] {
        &self.parameters
    }

    #[must_use]
    pub const fn result(&self) -> TypeId {
        self.result
    }
}

/// The exact runtime target of one call instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirCallTarget {
    Direct(ExecutableItemId),
    StandardPrimitive {
        role: PrimitiveRole,
        type_arguments: Box<[TypeId]>,
        signature: MirCallSignature,
    },
    Structural(MirStructuralCall),
}

/// Allocation context visible only while one call executes.
///
/// Lexical regions already change the inherited context through their create/release lifetime.
/// Typed literal `using` overrides instead name an existing allocator or region place and must be
/// restored as part of the call boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirCallAllocation {
    Inherit,
    Explicit(MirPlaceId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirCall {
    target: MirCallTarget,
    arguments: Box<[MirValueId]>,
    allocation: MirCallAllocation,
}

impl MirCall {
    #[must_use]
    pub fn new(target: MirCallTarget, arguments: impl Into<Box<[MirValueId]>>) -> Self {
        Self {
            target,
            arguments: arguments.into(),
            allocation: MirCallAllocation::Inherit,
        }
    }

    #[must_use]
    pub fn with_allocation(
        target: MirCallTarget,
        arguments: impl Into<Box<[MirValueId]>>,
        allocation: MirCallAllocation,
    ) -> Self {
        Self {
            target,
            arguments: arguments.into(),
            allocation,
        }
    }

    #[must_use]
    pub const fn target(&self) -> &MirCallTarget {
        &self.target
    }

    #[must_use]
    pub const fn arguments(&self) -> &[MirValueId] {
        &self.arguments
    }

    #[must_use]
    pub const fn allocation(&self) -> MirCallAllocation {
        self.allocation
    }
}

/// One linear MIR instruction. Control transfer lives exclusively in [`crate::MirTerminator`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirOperationKind {
    Constant(MirConstant),
    Read {
        place: MirPlaceId,
        mode: MirReadMode,
    },
    Borrow {
        place: MirPlaceId,
        capability: BorrowCapability,
    },
    Store {
        destination: MirPlaceId,
        value: MirValueId,
    },
    Initialize {
        destination: MirPlaceId,
        value: MirValueId,
    },
    SetDropFlag {
        flag: MirDropFlagId,
        initialized: bool,
    },
    Unary {
        operation: MirUnaryOperation,
        operand: MirValueId,
    },
    Binary {
        operation: MirBinaryOperation,
        left: MirValueId,
        right: MirValueId,
    },
    IntegerConversion {
        operand: MirValueId,
    },
    Aggregate(MirAggregate),
    Call(MirCall),
    InvokeDrop {
        body: ExecutableItemId,
        place: MirPlaceId,
    },
    CreateRegion {
        parent: MirValueId,
    },
    ReleaseRegion {
        region: MirLocalId,
    },
}

impl MirOperationKind {
    #[must_use]
    pub const fn produces_value(&self) -> bool {
        matches!(
            self,
            Self::Constant(_)
                | Self::Read { .. }
                | Self::Borrow { .. }
                | Self::Unary { .. }
                | Self::Binary { .. }
                | Self::IntegerConversion { .. }
                | Self::Aggregate(_)
                | Self::Call(_)
                | Self::CreateRegion { .. }
        )
    }
}

/// One instruction and the optional SSA value it defines.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOperation {
    pub(crate) kind: MirOperationKind,
    pub(crate) result: Option<MirValueId>,
}

impl MirOperation {
    pub(crate) const fn new(kind: MirOperationKind, result: Option<MirValueId>) -> Self {
        Self { kind, result }
    }

    #[must_use]
    pub const fn kind(&self) -> &MirOperationKind {
        &self.kind
    }

    #[must_use]
    pub const fn result(&self) -> Option<MirValueId> {
        self.result
    }
}
