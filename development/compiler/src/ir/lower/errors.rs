//! Projection of compiler-known static error helpers from checked MIR.

use crate::ir::{Instruction, StrLocation, StrValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ErrorPayload {
    code: StrValue,
    message: StrValue,
}

impl ErrorPayload {
    pub(super) fn into_store_instructions(
        self,
        code_destination: StrLocation,
        message_destination: StrLocation,
    ) -> Vec<Instruction> {
        vec![
            Instruction::SetStr {
                destination: code_destination,
                value: self.code,
            },
            Instruction::SetStr {
                destination: message_destination,
                value: self.message,
            },
        ]
    }
}

impl From<crate::mir::StaticErrorPayload> for ErrorPayload {
    fn from(payload: crate::mir::StaticErrorPayload) -> Self {
        Self {
            code: StrValue::StaticBytes(payload.code),
            message: StrValue::StaticBytes(payload.message),
        }
    }
}
