use nocter_json::Value;
use nocter_lsp::{
    DefinitionParams, HoverParams, ReferencesParams, RequestId, ResponseErrorCode,
    SemanticTokensParams, render_error_response, render_success_response,
};

use crate::hover::query_hover;
use crate::navigation::{NavigationQueryError, query_definition, query_references};
use crate::semantic_tokens::{SemanticTokensQueryError, query_semantic_tokens};

use super::{LanguageServer, ServerIssue, ServerStep};

impl LanguageServer {
    pub(super) fn hover(&self, id: &RequestId, params: Option<Value>) -> ServerStep {
        let params = match HoverParams::decode(params) {
            Ok(params) => params,
            Err(error) => {
                let detail = Value::String(error.to_string().into_boxed_str());
                return ServerStep {
                    response: Some(render_error_response(
                        Some(id),
                        ResponseErrorCode::InvalidParams,
                        Some(&detail),
                    )),
                    ..ServerStep::default()
                };
            }
        };
        let result = query_hover(
            &self.documents,
            self.analyses
                .as_ref()
                .expect("initialized server owns workspace analyses"),
            &params,
        );
        match result {
            Ok(result) => ServerStep {
                response: Some(render_success_response(id, &result)),
                ..ServerStep::default()
            },
            Err(error) => {
                let detail = Value::String(error.to_string().into_boxed_str());
                ServerStep {
                    response: Some(render_error_response(
                        Some(id),
                        ResponseErrorCode::InvalidParams,
                        Some(&detail),
                    )),
                    issues: vec![ServerIssue::Hover(error)].into_boxed_slice(),
                    ..ServerStep::default()
                }
            }
        }
    }

    pub(super) fn semantic_tokens(&self, id: &RequestId, params: Option<Value>) -> ServerStep {
        let params = match SemanticTokensParams::decode(params) {
            Ok(params) => params,
            Err(error) => {
                let detail = Value::String(error.to_string().into_boxed_str());
                return ServerStep {
                    response: Some(render_error_response(
                        Some(id),
                        ResponseErrorCode::InvalidParams,
                        Some(&detail),
                    )),
                    ..ServerStep::default()
                };
            }
        };
        let result = query_semantic_tokens(
            &self.documents,
            self.analyses
                .as_ref()
                .expect("initialized server owns workspace analyses"),
            &params,
        );
        match result {
            Ok(result) => ServerStep {
                response: Some(render_success_response(id, &result)),
                ..ServerStep::default()
            },
            Err(error) => {
                let detail = Value::String(error.to_string().into_boxed_str());
                let code = match error {
                    SemanticTokensQueryError::Document(_) => ResponseErrorCode::InvalidParams,
                    SemanticTokensQueryError::Coordinate(_)
                    | SemanticTokensQueryError::Multiline
                    | SemanticTokensQueryError::Encoding(_) => ResponseErrorCode::InternalError,
                };
                ServerStep {
                    response: Some(render_error_response(Some(id), code, Some(&detail))),
                    issues: vec![ServerIssue::SemanticTokens(error)].into_boxed_slice(),
                    ..ServerStep::default()
                }
            }
        }
    }

    pub(super) fn definition(&self, id: &RequestId, params: Option<Value>) -> ServerStep {
        let params = match DefinitionParams::decode(params) {
            Ok(params) => params,
            Err(error) => return invalid_params(id, error.to_string()),
        };
        Self::navigation_result(
            id,
            query_definition(
                &self.documents,
                self.analyses
                    .as_ref()
                    .expect("initialized server owns workspace analyses"),
                &params,
            ),
        )
    }

    pub(super) fn references(&self, id: &RequestId, params: Option<Value>) -> ServerStep {
        let params = match ReferencesParams::decode(params) {
            Ok(params) => params,
            Err(error) => return invalid_params(id, error.to_string()),
        };
        Self::navigation_result(
            id,
            query_references(
                &self.documents,
                self.analyses
                    .as_ref()
                    .expect("initialized server owns workspace analyses"),
                &params,
            ),
        )
    }

    fn navigation_result(
        id: &RequestId,
        result: Result<Value, NavigationQueryError>,
    ) -> ServerStep {
        match result {
            Ok(result) => ServerStep {
                response: Some(render_success_response(id, &result)),
                ..ServerStep::default()
            },
            Err(error) => {
                let detail = Value::String(error.to_string().into_boxed_str());
                let code = match error {
                    NavigationQueryError::Document(_) | NavigationQueryError::Coordinate(_) => {
                        ResponseErrorCode::InvalidParams
                    }
                    NavigationQueryError::MissingSource(_) | NavigationQueryError::Uri(_) => {
                        ResponseErrorCode::InternalError
                    }
                };
                ServerStep {
                    response: Some(render_error_response(Some(id), code, Some(&detail))),
                    issues: vec![ServerIssue::Navigation(error)].into_boxed_slice(),
                    ..ServerStep::default()
                }
            }
        }
    }
}

fn invalid_params(id: &RequestId, detail: String) -> ServerStep {
    let detail = Value::String(detail.into_boxed_str());
    ServerStep {
        response: Some(render_error_response(
            Some(id),
            ResponseErrorCode::InvalidParams,
            Some(&detail),
        )),
        ..ServerStep::default()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::tests::{TemporaryDirectory, semantic_server};

    #[test]
    fn definition_and_references_follow_identity_and_exact_ranges() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let result_uri = format!(
            "file://{}/main.nct",
            std::fs::canonicalize(temporary.path()).unwrap().display()
        );
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let source_text = concat!(
            "func helper(value: i32): i32 { return value }\n",
            "func main(): void {\n",
            "    let result = helper(1)\n",
            "    return\n",
            "}\n"
        );
        let mut source_json = String::new();
        nocter_json::write_string(&mut source_json, source_text);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":{source_json}}}}}}}"
        ));
        assert_eq!(
            opened.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::Complete
        );

        let definition = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/definition\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":2,\"character\":18}}}}}}"
        ));
        assert_eq!(
            definition.response(),
            Some(
                format!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":[{{\"uri\":\"{result_uri}\",\"range\":{{\"start\":{{\"line\":0,\"character\":5}},\"end\":{{\"line\":0,\"character\":11}}}}}}]}}"
                )
                .as_str()
            )
        );

        let references = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/references\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":0,\"character\":6}},\"context\":{{\"includeDeclaration\":false}}}}}}"
        ));
        assert_eq!(
            references.response(),
            Some(
                format!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":[{{\"uri\":\"{result_uri}\",\"range\":{{\"start\":{{\"line\":2,\"character\":17}},\"end\":{{\"line\":2,\"character\":23}}}}}}]}}"
                )
                .as_str()
            )
        );

        let declarations = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"textDocument/references\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":0,\"character\":6}},\"context\":{{\"includeDeclaration\":true}}}}}}"
        ));
        let response = declarations.response().unwrap();
        assert!(response.contains("\"line\":0,\"character\":5"));
        assert!(response.contains("\"line\":2,\"character\":17"));
        assert!(declarations.issue().is_none());
    }

    #[test]
    fn module_path_segments_navigate_as_one_resolved_namespace() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":\"func main(): void {{ return }}\\n\"}}}}}}"
        ));

        let standard = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std/str/index.nct");
        let text = std::fs::read_to_string(&standard).unwrap();
        let (line, source_line) = text
            .lines()
            .enumerate()
            .find(|(_, line)| line.starts_with("use std/iter."))
            .unwrap();
        let start = source_line.find("std/iter").unwrap();
        let document_uri = format!("file://{}", standard.display());
        let mut responses = Vec::new();
        for character in [start + 1, start + 5] {
            let response = server.receive(&format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/definition\",\"params\":{{\"textDocument\":{{\"uri\":\"{document_uri}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}"
            ));
            assert!(response.issue().is_none());
            responses.push(response.response().unwrap().to_owned());
        }
        assert_eq!(responses[0], responses[1]);
        assert!(responses[0].contains("/std/iter/index.nct"));
        assert!(
            responses[0].contains(concat!(
                "\"start\":{\"line\":0,\"character\":0},",
                "\"end\":{\"line\":0,\"character\":0}"
            )),
            "{}",
            responses[0]
        );
    }
}
