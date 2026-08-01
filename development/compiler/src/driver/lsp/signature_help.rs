//! LSP protocol view of compiler-owned call signature information.

use crate::analysis::signature_help::{SignatureHelpInfo, SignatureParameterInfo};
use serde_json::{Value, json};

pub(super) fn signature_help_value(signature: SignatureHelpInfo) -> Value {
    json!({
        "signatures": [{
            "label": signature.label,
            "documentation": signature.documentation.map(markdown),
            "parameters": signature
                .parameters
                .into_iter()
                .map(parameter_value)
                .collect::<Vec<_>>()
        }],
        "activeSignature": 0,
        "activeParameter": signature.active_parameter
    })
}

fn parameter_value(parameter: SignatureParameterInfo) -> Value {
    json!({
        "label": parameter.label,
        "documentation": parameter.documentation.map(markdown)
    })
}

fn markdown(value: String) -> Value {
    json!({
        "kind": "markdown",
        "value": value
    })
}
