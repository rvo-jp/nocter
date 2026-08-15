//! Semantic queries over checked MIR values represented by the builtin
//! `error` type.
//!
//! Native code generation currently inlines zero-argument error helpers. The
//! helper contract is derived here from checked MIR so buildability and
//! projection never maintain a second AST-level model of the function body.

use super::{Body, Operand, Place, Rvalue, Statement, Terminator};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StaticErrorPayload {
    pub(crate) code: Vec<u8>,
    pub(crate) message: Vec<u8>,
}

/// Returns the payload of the deliberately narrow native error-helper form.
///
/// A static helper has one checked block, assigns two static string operands
/// directly to its return place, and returns. Keeping the query strict makes
/// every additional supported error-producing form an explicit MIR/backend
/// capability instead of an accidental source-pattern exception.
pub(crate) fn static_error_payload(body: &Body) -> Option<StaticErrorPayload> {
    let [block] = body.blocks.as_slice() else {
        return None;
    };
    if block.terminator != Terminator::Return {
        return None;
    }
    let [
        Statement::Assign {
            destination,
            value: Rvalue::Error { code, message },
            ..
        },
    ] = block.statements.as_slice()
    else {
        return None;
    };
    if *destination != Place::local(body.return_local) {
        return None;
    }
    let (
        Operand::StaticStr {
            bytes: code_bytes, ..
        },
        Operand::StaticStr {
            bytes: message_bytes,
            ..
        },
    ) = (code, message)
    else {
        return None;
    };
    Some(StaticErrorPayload {
        code: code_bytes.clone(),
        message: message_bytes.clone(),
    })
}
