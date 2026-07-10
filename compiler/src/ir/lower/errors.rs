use crate::ast::{CallExpr, Expr};
use crate::diagnostics::Diagnostic;
use crate::literals::decode_string_literal_bytes;

pub(super) fn lower_make_error_message(expression: &Expr) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let Expr::Call(call) = expression else {
        return Err(unsupported_fail_payload_diagnostic());
    };

    if !is_make_error_call(call) || call.arguments.len() != 2 {
        return Err(unsupported_fail_payload_diagnostic());
    }

    let Expr::StringLiteral(message) = &call.arguments[1] else {
        return Err(unsupported_fail_payload_diagnostic());
    };

    decode_string_literal_bytes(&message.value).map_err(|message| {
        vec![Diagnostic::error(
            "E8005",
            format!("IR v0 cannot decode failure message literal: {message}"),
        )]
    })
}

pub(super) fn with_trailing_newline(mut bytes: Vec<u8>) -> Vec<u8> {
    if !bytes.ends_with(b"\n") {
        bytes.push(b'\n');
    }
    bytes
}

fn is_make_error_call(call: &CallExpr) -> bool {
    matches!(call.callee.as_ref(), Expr::Identifier(identifier) if identifier.name == "make_error")
}

fn unsupported_fail_payload_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8004",
        "IR v0 can only lower `return make_error(<string code>, <string message>)` as fallible failure",
    )]
}
