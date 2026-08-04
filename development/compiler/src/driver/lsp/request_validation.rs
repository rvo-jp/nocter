//! Parameter validation at the JSON-RPC request boundary.

use super::documents::document_uri_from_params;
use super::protocol::position_from_params;
use serde_json::Value;

pub(super) fn supported_text_document_params_are_valid(
    method: &str,
    params: Option<&Value>,
) -> Option<bool> {
    let requires_position = match method {
        "textDocument/semanticTokens/full" | "textDocument/documentSymbol" => false,
        "textDocument/hover"
        | "textDocument/definition"
        | "textDocument/references"
        | "textDocument/completion"
        | "textDocument/signatureHelp" => true,
        _ => return None,
    };

    Some(
        document_uri_from_params(params).is_some()
            && (!requires_position || position_from_params(params).is_some()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn distinguishes_document_and_position_requests() {
        let document = json!({ "textDocument": { "uri": "file:///tmp/main.nct" } });

        assert_eq!(
            supported_text_document_params_are_valid(
                "textDocument/documentSymbol",
                Some(&document)
            ),
            Some(true)
        );
        assert_eq!(
            supported_text_document_params_are_valid("textDocument/hover", Some(&document)),
            Some(false)
        );
        assert_eq!(
            supported_text_document_params_are_valid("workspace/symbol", Some(&document)),
            None
        );
    }
}
