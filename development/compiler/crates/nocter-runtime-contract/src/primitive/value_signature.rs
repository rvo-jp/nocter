//! Data and input operations for the qualified value-model primitive signature fragment.
use super::PrimitiveRole;
use crate::{RuntimeDataId, RuntimePrimitive, RuntimeType, RuntimeTypeTable};
use nocter_model::{BuiltinType, TypeId, TypeKind, ValueInputOperation};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrimitiveValueInput {
    data: BuiltinType,
    operation: ValueInputOperation,
}
impl PrimitiveValueInput {
    const fn new(data: BuiltinType, operation: ValueInputOperation) -> Self {
        Self { data, operation }
    }
    #[must_use]
    pub const fn data(self) -> BuiltinType {
        self.data
    }
    #[must_use]
    pub const fn operation(self) -> ValueInputOperation {
        self.operation
    }
}

/// One role-owned shape, without source identity, physical ABI, effects or origin evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrimitiveValueSignature {
    inputs: &'static [PrimitiveValueInput],
    result: PrimitiveValueResult,
}
impl PrimitiveValueSignature {
    #[must_use]
    pub const fn inputs(&self) -> &'static [PrimitiveValueInput] {
        self.inputs
    }
    #[must_use]
    pub const fn result(&self) -> PrimitiveValueResult {
        self.result
    }
}

/// Exact result shape, including the order of any outcome layers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrimitiveValueResult {
    Builtin(BuiltinType),
    Optional(&'static Self),
}

impl PrimitiveValueResult {
    #[must_use]
    pub fn matches_type<'a>(
        self,
        mut data: TypeId,
        mut kind: impl FnMut(TypeId) -> Option<&'a TypeKind>,
    ) -> bool {
        let mut shape = self;
        loop {
            (shape, data) = match (shape, kind(data)) {
                (Self::Builtin(expected), Some(TypeKind::Builtin(actual))) => {
                    return *actual == expected;
                }
                (Self::Optional(expected), Some(TypeKind::Optional(actual))) => {
                    (*expected, *actual)
                }
                _ => return false,
            };
        }
    }

    #[must_use]
    pub fn matches_runtime(self, types: &RuntimeTypeTable, mut data: RuntimeDataId) -> bool {
        let mut shape = self;
        loop {
            (shape, data) = match (shape, types.get(data)) {
                (Self::Builtin(expected), Some(RuntimeType::Primitive(actual))) => {
                    return *actual == RuntimePrimitive::for_builtin(expected);
                }
                (Self::Optional(expected), Some(RuntimeType::Optional(actual))) => {
                    (*expected, *actual)
                }
                _ => return false,
            };
        }
    }
}

impl fmt::Display for PrimitiveValueResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Builtin(data) => formatter.write_str(data.as_str()),
            Self::Optional(payload) => write!(formatter, "{payload}?"),
        }
    }
}

const NEW_ERROR: PrimitiveValueSignature = PrimitiveValueSignature {
    inputs: &[
        PrimitiveValueInput::new(BuiltinType::Str, ValueInputOperation::Read),
        PrimitiveValueInput::new(BuiltinType::Str, ValueInputOperation::Read),
    ],
    result: PrimitiveValueResult::Builtin(BuiltinType::Error),
};
const ERROR_CONTEXT: PrimitiveValueSignature = PrimitiveValueSignature {
    inputs: &[
        PrimitiveValueInput::new(BuiltinType::Error, ValueInputOperation::Take),
        PrimitiveValueInput::new(BuiltinType::Str, ValueInputOperation::Read),
    ],
    result: PrimitiveValueResult::Builtin(BuiltinType::Error),
};
const ALLOCATION_FAILURE_ERROR: PrimitiveValueSignature = PrimitiveValueSignature {
    inputs: &[],
    result: PrimitiveValueResult::Builtin(BuiltinType::Error),
};
const PROCESS_COUNT: PrimitiveValueSignature = PrimitiveValueSignature {
    inputs: &[],
    result: PrimitiveValueResult::Builtin(BuiltinType::Usize),
};
const PROCESS_ARGUMENT: PrimitiveValueSignature = PrimitiveValueSignature {
    inputs: &[PrimitiveValueInput::new(
        BuiltinType::Usize,
        ValueInputOperation::Read,
    )],
    // Outer absence means no argument; inner absence means invalid encoding.
    result: PrimitiveValueResult::Optional(&PrimitiveValueResult::Optional(
        &PrimitiveValueResult::Builtin(BuiltinType::Str),
    )),
};
impl PrimitiveRole {
    /// The qualified value-model signatures shared with source signature validation.
    /// A shape grants neither primitive authority nor executable/constant-domain admission.
    #[must_use]
    pub const fn value_signature(self) -> Option<&'static PrimitiveValueSignature> {
        match self {
            Self::NewError => Some(&NEW_ERROR),
            Self::ErrorContext => Some(&ERROR_CONTEXT),
            Self::AllocationFailureError => Some(&ALLOCATION_FAILURE_ERROR),
            Self::ProcessArgumentCount | Self::ProcessEnvironmentCount => Some(&PROCESS_COUNT),
            Self::ProcessArgument => Some(&PROCESS_ARGUMENT),
            _ => None,
        }
    }
}
