//! Data and input operations for the qualified value-model primitive signature fragment.
use super::PrimitiveRole;
use nocter_model::{BuiltinType, ValueInputOperation};

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
    result: BuiltinType,
}
impl PrimitiveValueSignature {
    #[must_use]
    pub const fn inputs(&self) -> &'static [PrimitiveValueInput] {
        self.inputs
    }
    #[must_use]
    pub const fn result(&self) -> BuiltinType {
        self.result
    }
}
const NEW_ERROR: PrimitiveValueSignature = PrimitiveValueSignature {
    inputs: &[
        PrimitiveValueInput::new(BuiltinType::Str, ValueInputOperation::Read),
        PrimitiveValueInput::new(BuiltinType::Str, ValueInputOperation::Read),
    ],
    result: BuiltinType::Error,
};
const ERROR_CONTEXT: PrimitiveValueSignature = PrimitiveValueSignature {
    inputs: &[
        PrimitiveValueInput::new(BuiltinType::Error, ValueInputOperation::Take),
        PrimitiveValueInput::new(BuiltinType::Str, ValueInputOperation::Read),
    ],
    result: BuiltinType::Error,
};
const PROCESS_COUNT: PrimitiveValueSignature = PrimitiveValueSignature {
    inputs: &[],
    result: BuiltinType::Usize,
};
impl PrimitiveRole {
    /// The qualified value-model signatures shared with source signature validation.
    /// A shape grants neither primitive authority nor executable/constant-domain admission.
    #[must_use]
    pub const fn value_signature(self) -> Option<&'static PrimitiveValueSignature> {
        match self {
            Self::NewError => Some(&NEW_ERROR),
            Self::ErrorContext => Some(&ERROR_CONTEXT),
            Self::ProcessArgumentCount | Self::ProcessEnvironmentCount => Some(&PROCESS_COUNT),
            _ => None,
        }
    }
}
