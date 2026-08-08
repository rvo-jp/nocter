//! Parameter validation at the JSON-RPC request boundary.

use super::documents::document_uri_from_params;
use super::protocol::position_from_params;
use serde_json::Value;

pub(super) fn supported_text_document_params_are_valid(
    method: &str,
    params: Option<&Value>,
) -> Option<bool> {
    if matches!(method, "textDocument/codeAction" | "textDocument/inlayHint") {
        return Some(document_uri_from_params(params).is_some() && range_is_valid(params));
    }
    let requires_position = match method {
        "textDocument/semanticTokens/full" | "textDocument/documentSymbol" => false,
        "textDocument/hover"
        | "textDocument/definition"
        | "textDocument/implementation"
        | "textDocument/references"
        | "textDocument/prepareRename"
        | "textDocument/rename"
        | "textDocument/completion"
        | "textDocument/signatureHelp" => true,
        _ => return None,
    };

    let valid = document_uri_from_params(params).is_some()
        && (!requires_position || position_from_params(params).is_some());
    Some(
        valid
            && (method != "textDocument/rename"
                || params
                    .and_then(|params| params.get("newName"))
                    .and_then(Value::as_str)
                    .is_some()),
    )
}

fn range_is_valid(params: Option<&Value>) -> bool {
    let Some(range) = params.and_then(|params| params.get("range")) else {
        return false;
    };
    ["start", "end"].into_iter().all(|bound| {
        range.get(bound).is_some_and(|position| {
            position.get("line").and_then(Value::as_u64).is_some()
                && position.get("character").and_then(Value::as_u64).is_some()
        })
    })
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

    #[test]
    fn validates_edit_and_range_request_shapes() {
        let ranged = json!({
            "textDocument": { "uri": "file:///tmp/main.nct" },
            "range": {
                "start": { "line": 0, "character": 1 },
                "end": { "line": 2, "character": 3 }
            }
        });
        assert_eq!(
            supported_text_document_params_are_valid("textDocument/codeAction", Some(&ranged)),
            Some(true)
        );
        assert_eq!(
            supported_text_document_params_are_valid(
                "textDocument/inlayHint",
                Some(&json!({ "textDocument": { "uri": "file:///tmp/main.nct" } }))
            ),
            Some(false)
        );

        let mut rename = json!({
            "textDocument": { "uri": "file:///tmp/main.nct" },
            "position": { "line": 0, "character": 1 }
        });
        assert_eq!(
            supported_text_document_params_are_valid("textDocument/rename", Some(&rename)),
            Some(false)
        );
        rename["newName"] = json!("renamed");
        assert_eq!(
            supported_text_document_params_are_valid("textDocument/rename", Some(&rename)),
            Some(true)
        );
    }
}
