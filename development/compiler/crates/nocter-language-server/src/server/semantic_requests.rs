use nocter_json::Value;
use nocter_lsp::{
    CodeActionParams, CompletionParams, DefinitionParams, HoverParams, ImplementationParams,
    InlayHintParams, ReferencesParams, RenameParams, RequestId, ResponseErrorCode,
    SemanticTokensParams, SignatureHelpParams, render_error_response, render_success_response,
};

use crate::code_actions::query_code_actions;
use crate::completion::query_completion;
use crate::hover::query_hover;
use crate::inlay_hints::query_inlay_hints;
use crate::navigation::{
    NavigationQueryError, query_definition, query_implementation, query_references,
};
use crate::rename::query_rename;
use crate::semantic_tokens::{SemanticTokensQueryError, query_semantic_tokens};
use crate::signature::query_signature_help;

use super::{LanguageServer, ServerIssue, ServerStep};

impl LanguageServer {
    pub(super) fn code_actions(&self, id: &RequestId, params: Option<Value>) -> ServerStep {
        let params = match CodeActionParams::decode(params) {
            Ok(params) => params,
            Err(error) => return invalid_params(id, error.to_string()),
        };
        match query_code_actions(
            &self.documents,
            self.analyses
                .as_ref()
                .expect("initialized server owns workspace analyses"),
            &params,
        ) {
            Ok(result) => ServerStep {
                response: Some(render_success_response(id, &result)),
                ..ServerStep::default()
            },
            Err(error) => {
                let code = if error.is_request_error() {
                    ResponseErrorCode::InvalidParams
                } else {
                    ResponseErrorCode::InternalError
                };
                let detail = Value::String(error.to_string().into_boxed_str());
                ServerStep {
                    response: Some(render_error_response(Some(id), code, Some(&detail))),
                    issues: vec![ServerIssue::CodeActions(error)].into_boxed_slice(),
                    ..ServerStep::default()
                }
            }
        }
    }

    pub(super) fn completion(&self, id: &RequestId, params: Option<Value>) -> ServerStep {
        let params = match CompletionParams::decode(params) {
            Ok(params) => params,
            Err(error) => return invalid_params(id, error.to_string()),
        };
        match query_completion(
            &self.documents,
            self.analyses
                .as_ref()
                .expect("initialized server owns workspace analyses"),
            &params,
        ) {
            Ok(result) => ServerStep {
                response: Some(render_success_response(id, &result)),
                ..ServerStep::default()
            },
            Err(error) => {
                let detail = Value::String(error.to_string().into_boxed_str());
                let code = match &error {
                    crate::completion::CompletionQueryError::Document(_)
                    | crate::completion::CompletionQueryError::Coordinate(_) => {
                        ResponseErrorCode::InvalidParams
                    }
                    crate::completion::CompletionQueryError::Semantic(_) => {
                        ResponseErrorCode::InternalError
                    }
                };
                ServerStep {
                    response: Some(render_error_response(Some(id), code, Some(&detail))),
                    issues: vec![ServerIssue::Completion(error)].into_boxed_slice(),
                    ..ServerStep::default()
                }
            }
        }
    }

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
                let code = match &error {
                    crate::hover::HoverQueryError::Document(_)
                    | crate::hover::HoverQueryError::Coordinate(_) => {
                        ResponseErrorCode::InvalidParams
                    }
                    crate::hover::HoverQueryError::Semantic(_) => ResponseErrorCode::InternalError,
                };
                ServerStep {
                    response: Some(render_error_response(Some(id), code, Some(&detail))),
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

    pub(super) fn inlay_hints(&self, id: &RequestId, params: Option<Value>) -> ServerStep {
        let params = match InlayHintParams::decode(params) {
            Ok(params) => params,
            Err(error) => return invalid_params(id, error.to_string()),
        };
        match query_inlay_hints(
            &self.documents,
            self.analyses
                .as_ref()
                .expect("initialized server owns workspace analyses"),
            &params,
        ) {
            Ok(result) => ServerStep {
                response: Some(render_success_response(id, &result)),
                ..ServerStep::default()
            },
            Err(error) => {
                let detail = Value::String(error.to_string().into_boxed_str());
                let code = match &error {
                    crate::inlay_hints::InlayHintQueryError::Document(_)
                    | crate::inlay_hints::InlayHintQueryError::RequestCoordinate(_) => {
                        ResponseErrorCode::InvalidParams
                    }
                    crate::inlay_hints::InlayHintQueryError::ResultCoordinate(_)
                    | crate::inlay_hints::InlayHintQueryError::Semantic(_) => {
                        ResponseErrorCode::InternalError
                    }
                };
                ServerStep {
                    response: Some(render_error_response(Some(id), code, Some(&detail))),
                    issues: vec![ServerIssue::InlayHints(error)].into_boxed_slice(),
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

    pub(super) fn implementation(&self, id: &RequestId, params: Option<Value>) -> ServerStep {
        let params = match ImplementationParams::decode(params) {
            Ok(params) => params,
            Err(error) => return invalid_params(id, error.to_string()),
        };
        Self::navigation_result(
            id,
            query_implementation(
                &self.documents,
                self.analyses
                    .as_ref()
                    .expect("initialized server owns workspace analyses"),
                &params,
            ),
        )
    }

    pub(super) fn rename(&self, id: &RequestId, params: Option<Value>) -> ServerStep {
        let params = match RenameParams::decode(params) {
            Ok(params) => params,
            Err(error) => return invalid_params(id, error.to_string()),
        };
        match query_rename(
            &self.documents,
            self.analyses
                .as_ref()
                .expect("initialized server owns workspace analyses"),
            &params,
        ) {
            Ok(result) => ServerStep {
                response: Some(render_success_response(id, &result)),
                ..ServerStep::default()
            },
            Err(error) => {
                let detail = Value::String(error.to_string().into_boxed_str());
                let code = if error.is_request_error() {
                    ResponseErrorCode::InvalidParams
                } else {
                    ResponseErrorCode::InternalError
                };
                ServerStep {
                    response: Some(render_error_response(Some(id), code, Some(&detail))),
                    issues: vec![ServerIssue::Rename(error)].into_boxed_slice(),
                    ..ServerStep::default()
                }
            }
        }
    }

    pub(super) fn signature_help(&self, id: &RequestId, params: Option<Value>) -> ServerStep {
        let params = match SignatureHelpParams::decode(params) {
            Ok(params) => params,
            Err(error) => return invalid_params(id, error.to_string()),
        };
        match query_signature_help(
            &self.documents,
            self.analyses
                .as_ref()
                .expect("initialized server owns workspace analyses"),
            &params,
        ) {
            Ok(result) => ServerStep {
                response: Some(render_success_response(id, &result)),
                ..ServerStep::default()
            },
            Err(error) => {
                let detail = Value::String(error.to_string().into_boxed_str());
                let code = match &error {
                    crate::signature::SignatureQueryError::Document(_)
                    | crate::signature::SignatureQueryError::Coordinate(_) => {
                        ResponseErrorCode::InvalidParams
                    }
                    crate::signature::SignatureQueryError::Semantic(_)
                    | crate::signature::SignatureQueryError::InvalidLabelRange(_) => {
                        ResponseErrorCode::InternalError
                    }
                };
                ServerStep {
                    response: Some(render_error_response(Some(id), code, Some(&detail))),
                    issues: vec![ServerIssue::Signature(error)].into_boxed_slice(),
                    ..ServerStep::default()
                }
            }
        }
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
    use crate::{LanguageServer, ServerStep};

    fn construction_completion_server(temporary: &TemporaryDirectory) -> (String, LanguageServer) {
        let uri = format!("file://{}", temporary.path().join("main.nct").display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        (uri, server)
    }

    fn set_completion_document(
        server: &mut LanguageServer,
        uri: &str,
        text: &str,
        version: i32,
    ) -> ServerStep {
        let mut text_json = String::new();
        nocter_json::write_string(&mut text_json, text);
        if version == 1 {
            server.receive(&format!(
                "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":{text_json}}}}}}}"
            ))
        } else {
            server.receive(&format!(
                "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didChange\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"version\":{version}}},\"contentChanges\":[{{\"text\":{text_json}}}]}}}}"
            ))
        }
    }

    fn request_completion(
        server: &mut LanguageServer,
        uri: &str,
        id: i32,
        line: usize,
        character: usize,
    ) -> ServerStep {
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"textDocument/completion\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":{line},\"character\":{character}}}}}}}"
        ))
    }

    fn request_outcome_code_action(
        source: &str,
        start: (usize, usize),
        end: (usize, usize),
    ) -> ServerStep {
        let temporary = TemporaryDirectory::new();
        let source_path = temporary.path().join("index.nct");
        let full_source = format!("#package: {{ name: \"app\", version: \"0.0.0\", }}\n{source}");
        std::fs::write(&source_path, &full_source).unwrap();
        let uri = format!("file://{}", source_path.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let opened = set_completion_document(&mut server, &uri, &full_source, 1);
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::CompilationFailed
        );
        assert_eq!(snapshot.diagnostics()[0].code(), "E0392", "{source}");

        request_code_action(
            &mut server,
            &uri,
            2,
            (start.0 + 1, start.1),
            (end.0 + 1, end.1),
        )
    }

    fn request_code_action(
        server: &mut LanguageServer,
        uri: &str,
        id: usize,
        start: (usize, usize),
        end: (usize, usize),
    ) -> ServerStep {
        server.receive(&format!(
            concat!(
                "{{\"jsonrpc\":\"2.0\",\"id\":{id},",
                "\"method\":\"textDocument/codeAction\",",
                "\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},",
                "\"range\":{{\"start\":{{\"line\":{start_line},\"character\":{start_character}}},",
                "\"end\":{{\"line\":{end_line},\"character\":{end_character}}}}},",
                "\"context\":{{\"diagnostics\":[]}}}}}}"
            ),
            id = id,
            uri = uri,
            start_line = start.0,
            start_character = start.1,
            end_line = end.0,
            end_character = end.1,
        ))
    }

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
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.compilation_failure()
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
    fn constants_share_one_editor_identity_and_canonical_value_presentation() {
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
        let text = "pub const base: i32 = 40\npub const answer: i32 = base + 2\nfunc main(): i32 {\n    answer\n}\n";
        let mut text_json = String::new();
        nocter_json::write_string(&mut text_json, text);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":{text_json}}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.compilation_failure()
        );

        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":3,\"character\":7}}}}}}"
        ));
        let hover_response = hover.response().unwrap();
        assert!(
            hover_response.contains("```nocter\\npub const answer: i32 = 42\\n```"),
            "{hover_response}"
        );

        let definition = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/definition\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":3,\"character\":7}}}}}}"
        ));
        assert_eq!(
            definition.response(),
            Some(
                format!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":[{{\"uri\":\"{result_uri}\",\"range\":{{\"start\":{{\"line\":1,\"character\":10}},\"end\":{{\"line\":1,\"character\":16}}}}}}]}}"
                )
                .as_str()
            )
        );

        let completion = request_completion(&mut server, &uri, 4, 3, 4);
        let response = completion.response().unwrap();
        assert!(response.contains("\"label\":\"answer\""));
        assert!(response.contains("\"kind\":21"));

        let rename = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"textDocument/rename\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":3,\"character\":7}},\"newName\":\"result\"}}}}"
        ));
        let response = rename.response().unwrap();
        assert!(response.contains("\"newText\":\"result\""), "{response}");
        assert!(response.matches("newText").count() >= 2);

        let initializer_reference = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"textDocument/references\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":0,\"character\":11}},\"context\":{{\"includeDeclaration\":false}}}}}}"
        ));
        let response = initializer_reference.response().unwrap();
        assert!(
            response.contains("\"line\":1,\"character\":24"),
            "{response}"
        );
        assert!(hover.issue().is_none());
        assert!(definition.issue().is_none());
        assert!(completion.issue().is_none());
        assert!(rename.issue().is_none());
        assert!(initializer_reference.issue().is_none());
    }

    #[test]
    fn separated_constant_selects_its_contract_and_initializer() {
        let temporary = TemporaryDirectory::new();
        std::fs::write(
            temporary.path().join("index.nct"),
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
        )
        .unwrap();
        let contract = temporary.path().join("index.nct");
        let implementation = temporary.path().join("limits.nct");
        let contract_body = concat!(
            "see ./limits.nct\n",
            "\n",
            "pub const answer: i32\n",
            "func main(): i32 {\n",
            "    answer\n",
            "}\n",
        );
        let contract_text =
            format!("#package: {{ name: \"app\", version: \"0.0.0\", }}\n{contract_body}");
        std::fs::write(&contract, &contract_text).unwrap();
        std::fs::write(
            &implementation,
            concat!("see ./index.nct\n", "\n", "const answer: i32 = 42\n",),
        )
        .unwrap();
        let contract_uri = format!("file://{}", contract.display());
        let canonical_contract_uri = format!(
            "file://{}/index.nct",
            std::fs::canonicalize(temporary.path()).unwrap().display()
        );
        let canonical_implementation_uri = format!(
            "file://{}/limits.nct",
            std::fs::canonicalize(temporary.path()).unwrap().display()
        );
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let mut source_json = String::new();
        nocter_json::write_string(&mut source_json, &contract_text);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{contract_uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":{source_json}}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.compilation_failure()
        );

        let definition = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/definition\",\"params\":{{\"textDocument\":{{\"uri\":\"{contract_uri}\"}},\"position\":{{\"line\":5,\"character\":7}}}}}}"
        ));
        assert_eq!(
            definition.response(),
            Some(
                format!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":[{{\"uri\":\"{canonical_contract_uri}\",\"range\":{{\"start\":{{\"line\":3,\"character\":10}},\"end\":{{\"line\":3,\"character\":16}}}}}}]}}"
                )
                .as_str()
            )
        );

        let initializer = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/implementation\",\"params\":{{\"textDocument\":{{\"uri\":\"{contract_uri}\"}},\"position\":{{\"line\":5,\"character\":7}}}}}}"
        ));
        assert_eq!(
            initializer.response(),
            Some(
                format!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":[{{\"uri\":\"{canonical_implementation_uri}\",\"range\":{{\"start\":{{\"line\":2,\"character\":6}},\"end\":{{\"line\":2,\"character\":12}}}}}}]}}"
                )
                .as_str()
            )
        );
        assert!(definition.issue().is_none());
        assert!(initializer.issue().is_none());
    }

    #[test]
    fn definition_selects_the_contract_and_implementation_selects_the_body() {
        let temporary = TemporaryDirectory::new();
        std::fs::write(
            temporary.path().join("index.nct"),
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
        )
        .unwrap();
        let contract = temporary.path().join("index.nct");
        let implementation = temporary.path().join("value.nct");
        let contract_body = concat!(
            "see ./value.nct\n",
            "\n",
            "pub func helper(): i32\n",
            "func main(): i32 {\n",
            "    return helper()\n",
            "}\n",
        );
        let contract_text =
            format!("#package: {{ name: \"app\", version: \"0.0.0\", }}\n{contract_body}");
        std::fs::write(&contract, &contract_text).unwrap();
        std::fs::write(
            &implementation,
            concat!(
                "see ./index.nct\n",
                "\n",
                "func helper(): i32 { return 1 }\n",
            ),
        )
        .unwrap();
        let contract_uri = format!("file://{}", contract.display());
        let canonical_contract_uri = format!(
            "file://{}/index.nct",
            std::fs::canonicalize(temporary.path()).unwrap().display()
        );
        let canonical_implementation_uri = format!(
            "file://{}/value.nct",
            std::fs::canonicalize(temporary.path()).unwrap().display()
        );
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let mut source_json = String::new();
        nocter_json::write_string(&mut source_json, &contract_text);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{contract_uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":{source_json}}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.compilation_failure()
        );

        let definition = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/definition\",\"params\":{{\"textDocument\":{{\"uri\":\"{contract_uri}\"}},\"position\":{{\"line\":5,\"character\":12}}}}}}"
        ));
        assert_eq!(
            definition.response(),
            Some(
                format!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":[{{\"uri\":\"{canonical_contract_uri}\",\"range\":{{\"start\":{{\"line\":3,\"character\":9}},\"end\":{{\"line\":3,\"character\":15}}}}}}]}}"
                )
                .as_str()
            )
        );

        let body = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/implementation\",\"params\":{{\"textDocument\":{{\"uri\":\"{contract_uri}\"}},\"position\":{{\"line\":5,\"character\":12}}}}}}"
        ));
        assert_eq!(
            body.response(),
            Some(
                format!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":[{{\"uri\":\"{canonical_implementation_uri}\",\"range\":{{\"start\":{{\"line\":2,\"character\":5}},\"end\":{{\"line\":2,\"character\":11}}}}}}]}}"
                )
                .as_str()
            )
        );
    }

    #[test]
    fn separated_interface_default_keeps_contract_and_body_navigation() {
        let temporary = TemporaryDirectory::new();
        std::fs::write(
            temporary.path().join("index.nct"),
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
        )
        .unwrap();
        let contract = temporary.path().join("index.nct");
        let implementation = temporary.path().join("defaults.nct");
        let contract_body = concat!(
            "see ./defaults.nct\n",
            "\n",
            "pub interface Answer {\n",
            "    pub default method &self.answer(): i32\n",
            "}\n",
            "struct Value {}\n",
            "conform Answer for Value {}\n",
            "func main(): i32 {\n",
            "    let value = Value {}\n",
            "    return value.answer()\n",
            "}\n",
        );
        let contract_text =
            format!("#package: {{ name: \"app\", version: \"0.0.0\", }}\n{contract_body}");
        std::fs::write(&contract, &contract_text).unwrap();
        std::fs::write(
            &implementation,
            concat!(
                "see ./index.nct\n",
                "\n",
                "interface Answer {\n",
                "    default method &self.answer(): i32 { return 1 }\n",
                "}\n",
            ),
        )
        .unwrap();
        let contract_uri = format!("file://{}", contract.display());
        let canonical_contract_uri = format!(
            "file://{}/index.nct",
            std::fs::canonicalize(temporary.path()).unwrap().display()
        );
        let canonical_implementation_uri = format!(
            "file://{}/defaults.nct",
            std::fs::canonicalize(temporary.path()).unwrap().display()
        );
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let mut source_json = String::new();
        nocter_json::write_string(&mut source_json, &contract_text);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{contract_uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":{source_json}}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "discovery={:?}, compilation={:?}, diagnostics={:?}",
            snapshot.discovery_failure(),
            snapshot.compilation_failure(),
            snapshot.diagnostics()
        );

        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{contract_uri}\"}},\"position\":{{\"line\":4,\"character\":31}}}}}}"
        ));
        assert!(
            hover.response().is_some_and(
                |response| response.contains("pub default method &Answer.answer(): i32")
            ),
            "{:?}",
            hover.response()
        );

        let definition = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/definition\",\"params\":{{\"textDocument\":{{\"uri\":\"{contract_uri}\"}},\"position\":{{\"line\":10,\"character\":18}}}}}}"
        ));
        assert_eq!(
            definition.response(),
            Some(
                format!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":[{{\"uri\":\"{canonical_contract_uri}\",\"range\":{{\"start\":{{\"line\":4,\"character\":29}},\"end\":{{\"line\":4,\"character\":35}}}}}}]}}"
                )
                .as_str()
            )
        );

        let body = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/implementation\",\"params\":{{\"textDocument\":{{\"uri\":\"{contract_uri}\"}},\"position\":{{\"line\":10,\"character\":18}}}}}}"
        ));
        assert_eq!(
            body.response(),
            Some(
                format!(
                    "{{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":[{{\"uri\":\"{canonical_implementation_uri}\",\"range\":{{\"start\":{{\"line\":3,\"character\":25}},\"end\":{{\"line\":3,\"character\":31}}}}}}]}}"
                )
                .as_str()
            )
        );
    }

    #[test]
    fn rename_recompiles_the_candidate_and_returns_one_versioned_workspace_edit() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let text = concat!(
            "func helper(): i32 { return 1 }\n",
            "func main(): void {\n",
            "    let value = helper()\n",
            "    return\n",
            "}\n"
        );
        let mut text_json = String::new();
        nocter_json::write_string(&mut text_json, text);
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":7,\"text\":{text_json}}}}}}}"
        ));

        let renamed = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/rename\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":2,\"character\":18}},\"newName\":\"calculate\"}}}}"
        ));
        let response = renamed.response().unwrap();
        assert!(response.contains("\"documentChanges\""));
        assert!(response.contains("\"version\":7"));
        assert_eq!(response.matches("\"newText\":\"calculate\"").count(), 2);
        assert!(response.contains("\"line\":0,\"character\":5"));
        assert!(response.contains("\"line\":2,\"character\":16"));
        assert!(renamed.issue().is_none());

        let collision = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/rename\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":0,\"character\":7}},\"newName\":\"main\"}}}}"
        ));
        assert!(collision.response().unwrap().contains("\"code\":-32602"));
        assert!(
            collision
                .response()
                .unwrap()
                .contains("would collide with or rebind")
        );

        let invalid = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"textDocument/rename\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":0,\"character\":7}},\"newName\":\"two words\"}}}}"
        ));
        assert!(invalid.response().unwrap().contains("\"code\":-32602"));
    }

    #[test]
    fn rename_versions_open_sources_and_leaves_closed_sources_unversioned() {
        let temporary = TemporaryDirectory::new();
        std::fs::write(
            temporary.path().join("index.nct"),
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
        )
        .unwrap();
        let source = temporary.path().join("program.nct");
        let helper = temporary.path().join("helper.nct");
        std::fs::write(
            &source,
            concat!(
                "see ./helper.nct\n",
                "func main(): void {\n",
                "    let value = answer()\n",
                "    return\n",
                "}\n"
            ),
        )
        .unwrap();
        std::fs::write(&helper, "func answer(): i32 { return 1 }\n").unwrap();
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let text = std::fs::read_to_string(&source).unwrap();
        let mut text_json = String::new();
        nocter_json::write_string(&mut text_json, &text);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":3,\"text\":{text_json}}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.compilation_failure()
        );

        let renamed = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/rename\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":2,\"character\":18}},\"newName\":\"result\"}}}}"
        ));
        let response = renamed.response().unwrap();
        assert!(response.contains("\"version\":3"));
        assert!(response.contains("\"version\":null"));
        assert!(response.contains("helper.nct"));
        assert_eq!(response.matches("\"newText\":\"result\"").count(), 2);
        assert!(renamed.issue().is_none(), "{:?}", renamed.issue());
    }

    #[test]
    fn rename_rejects_standard_library_occurrences_as_one_readonly_plan() {
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
            .find(|(_, line)| line.contains("pub method &self.len(): usize"))
            .unwrap();
        let character = source_line.find("len").unwrap();
        let rejected = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/rename\",\"params\":{{\"textDocument\":{{\"uri\":\"file://{}\"}},\"position\":{{\"line\":{line},\"character\":{character}}},\"newName\":\"size\"}}}}",
            standard.display()
        ));
        assert!(rejected.response().unwrap().contains("\"code\":-32602"));
        assert!(
            rejected
                .response()
                .unwrap()
                .contains("dependency or standard source")
        );
    }

    #[test]
    fn rename_preserves_the_binding_family_across_explicit_closure_captures() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let text = concat!(
            "struct Box { value: i32 }\n",
            "func main(): void {\n",
            "    let value = Box { value: 1 }\n",
            "    let closure = (move value;): i32 { value.value }\n",
            "    let result = closure()\n",
            "    return\n",
            "}\n"
        );
        let mut text_json = String::new();
        nocter_json::write_string(&mut text_json, text);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":2,\"text\":{text_json}}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.compilation_failure()
        );

        let renamed = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/rename\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":2,\"character\":9}},\"newName\":\"number\"}}}}"
        ));
        let response = renamed.response().unwrap();
        assert_eq!(response.matches("\"newText\":\"number\"").count(), 3);
        assert!(renamed.issue().is_none(), "{:?}", renamed.issue());
    }

    #[test]
    fn signature_help_uses_the_checked_specialization_and_active_argument() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let text = concat!(
            "func choose<T>(left: T, right: T): T { move left }\n",
            "func main(): void {\n",
            "    let value = choose(1, 2)\n",
            "    let double = (number: i32): i32 { number * 2 }\n",
            "    let doubled = double(4)\n",
            "    return\n",
            "}\n"
        );
        let mut text_json = String::new();
        nocter_json::write_string(&mut text_json, text);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":{text_json}}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.compilation_failure()
        );

        let help = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/signatureHelp\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":2,\"character\":26}}}}}}"
        ));
        let response = help.response().unwrap();
        assert!(response.contains("func choose<i32>(left: i32, right: i32): i32"));
        assert!(response.contains("\"parameters\":[{\"label\":[17,26]"));
        assert!(response.contains("\"activeParameter\":1"));
        assert!(help.issue().is_none(), "{:?}", help.issue());

        let closure = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/signatureHelp\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":4,\"character\":26}}}}}}"
        ));
        let response = closure.response().unwrap();
        assert!(response.contains("&func(i32): i32"));
        assert!(response.contains("\"activeParameter\":0"));
        assert!(closure.issue().is_none(), "{:?}", closure.issue());
    }

    #[test]
    fn signature_help_renders_and_selects_the_final_argument_pack() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let text = concat!(
            "func total(seed: i32, ...items: i32): i32 {\n",
            "    var result = seed\n",
            "    for item in items { result += item }\n",
            "    return result\n",
            "}\n",
            "func main(): void {\n",
            "    let value = total(1, 2, 3)\n",
            "    return\n",
            "}\n"
        );
        let mut text_json = String::new();
        nocter_json::write_string(&mut text_json, text);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":{text_json}}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.compilation_failure()
        );

        let help = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/signatureHelp\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":6,\"character\":29}}}}}}"
        ));
        let response = help.response().unwrap();
        assert!(response.contains("func total(seed: i32, ...items: i32): i32"));
        assert!(response.contains("\"activeParameter\":1"));
        assert!(help.issue().is_none(), "{:?}", help.issue());
    }

    #[test]
    fn semantic_presentations_share_source_local_type_aliases() {
        let temporary = TemporaryDirectory::new();
        let widgets = temporary.path().join("widgets");
        std::fs::create_dir(&widgets).unwrap();
        std::fs::write(
            temporary.path().join("index.nct"),
            "#package: { name: \"presentation-alias\", version: \"0.1.0\", }\n#executable: {\n    name: \"presentation-alias\",\n}\n",
        )
        .unwrap();
        std::fs::write(
            widgets.join("index.nct"),
            concat!(
                "pub struct Widget {\n    pub value: i32\n}\n\n",
                "construct Widget {\n",
                "    pub default func new(): Self { return Widget { value: 1 } }\n",
                "}\n",
            ),
        )
        .unwrap();
        let source_path = temporary.path().join("app.nct");
        let source = concat!(
            "use ./widgets.Widget as LocalWidget\n",
            "\n",
            "func passthrough(value: LocalWidget): LocalWidget { move value }\n",
            "func main(): void {\n",
            "    let input = LocalWidget.new()\n",
            "    let inferred = passthrough(move input)\n",
            "    return\n",
            "}\n",
        );
        std::fs::write(&source_path, source).unwrap();
        let uri = format!("file://{}", source_path.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let opened = set_completion_document(&mut server, &uri, source, 1);
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.compilation_failure()
        );

        let signature = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/signatureHelp\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":5,\"character\":42}}}}}}"
        ));
        let response = signature.response().unwrap();
        assert!(
            response.contains("func passthrough(value: LocalWidget): LocalWidget"),
            "{response}"
        );
        assert!(signature.issue().is_none(), "{:?}", signature.issue());

        let hints = server.receive(&format!(
            concat!(
                "{{\"jsonrpc\":\"2.0\",\"id\":3,",
                "\"method\":\"textDocument/inlayHint\",",
                "\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},",
                "\"range\":{{\"start\":{{\"line\":0,\"character\":0}},",
                "\"end\":{{\"line\":7,\"character\":1}}}}}}}}"
            ),
            uri = uri,
        ));
        let response = hints.response().unwrap();
        assert!(
            response.contains("\"label\":\": LocalWidget\""),
            "{response}"
        );
        assert!(hints.issue().is_none(), "{:?}", hints.issue());

        let completion = request_completion(&mut server, &uri, 4, 6, 4);
        let response = completion.response().unwrap();
        assert!(response.contains("LocalWidget"), "{response}");
        assert!(completion.issue().is_none(), "{:?}", completion.issue());
    }

    #[test]
    fn completion_uses_checked_module_and_lexical_scope_identity() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let text = concat!(
            "func helper(): i32 { 1 }\n",
            "func main(input: i32): void {\n",
            "    let before = input\n",
            "\n",
            "    let after = input\n",
            "    let closure = (&before; inner: i32): i32 {\n",
            "        inner\n",
            "    }\n",
            "    return\n",
            "}\n"
        );
        let mut text_json = String::new();
        nocter_json::write_string(&mut text_json, text);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":{text_json}}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.compilation_failure()
        );

        let body = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/completion\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":3,\"character\":0}}}}}}"
        ));
        let response = body.response().unwrap();
        assert!(response.contains("\"label\":\"helper\""), "{response}");
        assert!(response.contains("\"label\":\"input\""));
        assert!(response.contains("\"label\":\"before\""));
        assert!(!response.contains("\"label\":\"after\""));
        assert!(body.issue().is_none(), "{:?}", body.issue());

        let closure = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/completion\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":6,\"character\":10}}}}}}"
        ));
        let response = closure.response().unwrap();
        assert!(response.contains("\"label\":\"before\""));
        assert!(response.contains("\"label\":\"inner\""));
        assert!(!response.contains("\"label\":\"input\""));
        assert!(!response.contains("\"label\":\"after\""));
        assert!(closure.issue().is_none(), "{:?}", closure.issue());

        let failed_text = concat!(
            "func replacement(): i32 { 1 }\n",
            "func main(current: i32): void {\n",
            "    let local = current\n",
            "    local.missing()\n",
            "    return\n",
            "}\n"
        );
        let mut failed_json = String::new();
        nocter_json::write_string(&mut failed_json, failed_text);
        let changed = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didChange\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"version\":2}},\"contentChanges\":[{{\"text\":{failed_json}}}]}}}}"
        ));
        let snapshot = changed.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::CompilationFailed
        );

        let failed = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"textDocument/completion\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":4,\"character\":4}}}}}}"
        ));
        let response = failed.response().unwrap();
        assert!(response.contains("\"label\":\"replacement\""));
        assert!(response.contains("\"label\":\"current\""));
        assert!(response.contains("\"label\":\"local\""));
        assert!(!response.contains("\"label\":\"helper\""));
        assert!(!response.contains("\"label\":\"input\""));
        assert!(!response.contains("\"label\":\"before\""));
        assert!(failed.issue().is_none(), "{:?}", failed.issue());
    }

    #[test]
    fn completion_retains_only_current_scopes_before_a_name_error() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let text = concat!(
            "func latest(): i32 { 1 }\n",
            "func main(subject: i32): void {\n",
            "    let visible = subject\n",
            "    unresolved\n",
            "    let hidden = subject\n",
            "    return\n",
            "}\n"
        );
        let mut text_json = String::new();
        nocter_json::write_string(&mut text_json, text);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":{text_json}}}}}}}"
        ));
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::CompilationFailed
        );

        let completion = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/completion\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":3,\"character\":8}}}}}}"
        ));
        let response = completion.response().unwrap();
        assert!(response.contains("\"label\":\"latest\""), "{response}");
        assert!(response.contains("\"label\":\"subject\""), "{response}");
        assert!(response.contains("\"label\":\"visible\""), "{response}");
        assert!(!response.contains("\"label\":\"hidden\""), "{response}");
        assert!(completion.issue().is_none(), "{:?}", completion.issue());
    }

    #[test]
    fn completion_supplies_contextual_test_and_copy_keywords() {
        let temporary = TemporaryDirectory::new();
        let uri = format!("file://{}", temporary.path().join("main.nct").display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);

        let top_level = "te\n";
        let opened = set_completion_document(&mut server, &uri, top_level, 1);
        assert_eq!(
            opened.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::SyntaxFailed
        );
        let completion = request_completion(&mut server, &uri, 2, 0, 2);
        let response = completion.response().unwrap();
        assert!(
            response.contains("\"label\":\"test\",\"kind\":14"),
            "{response}"
        );
        assert!(response.contains("test name { ... }"), "{response}");
        assert!(completion.issue().is_none(), "{:?}", completion.issue());

        let generic = "func clone<T>(value: T): T where co\n";
        let changed = set_completion_document(&mut server, &uri, generic, 2);
        assert_eq!(
            changed.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::SyntaxFailed
        );
        let completion = request_completion(&mut server, &uri, 3, 0, generic.trim_end().len());
        let response = completion.response().unwrap();
        assert!(
            response.contains("\"label\":\"copy\",\"kind\":14"),
            "{response}"
        );
        assert!(completion.issue().is_none(), "{:?}", completion.issue());

        for (version, source) in [
            (3, "func clone<T>(value: T): T where copy T, \n"),
            (4, "func clone(value: i32): i32 where co\n"),
            (5, "struct Value {\n    te\n}\n"),
        ] {
            set_completion_document(&mut server, &uri, source, version);
            let (line, character) = if version == 5 {
                (1, 6)
            } else {
                (0, source.trim_end().len())
            };
            let completion = request_completion(&mut server, &uri, version + 1, line, character);
            let response = completion.response().unwrap();
            assert!(!response.contains("\"label\":\"copy\""), "{response}");
            assert!(!response.contains("\"label\":\"test\""), "{response}");
            assert!(completion.issue().is_none(), "{:?}", completion.issue());
        }
    }

    #[test]
    fn completion_uses_checked_receiver_selection_for_methods() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("main.nct");
        let uri = format!("file://{}", source.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let text = concat!(
            "struct Text { value: i32 }\n",
            "struct Wrapper { text: Text }\n",
            "instance Text {\n",
            "    pub method &self.len(): usize { 0 }\n",
            "    pub method &+self.clear(): void { return }\n",
            "}\n",
            "instance Wrapper { pub coerce &self as &Text { &self.text } }\n",
            "func inspect(value: &Wrapper): usize { value.len() }\n",
        );
        let mut text_json = String::new();
        nocter_json::write_string(&mut text_json, text);
        let opened = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"languageId\":\"nocter\",\"version\":1,\"text\":{text_json}}}}}}}"
        ));
        assert_eq!(
            opened.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::Complete
        );

        let completion = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/completion\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":7,\"character\":47}}}}}}"
        ));
        let response = completion.response().unwrap();
        assert!(
            response.contains("\"label\":\"len\",\"kind\":2"),
            "{response}"
        );
        assert!(
            response.contains("\"label\":\"text\",\"kind\":5"),
            "{response}"
        );
        assert!(!response.contains("\"label\":\"clear\""), "{response}");
        assert!(!response.contains("\"label\":\"value\""), "{response}");
        assert!(completion.issue().is_none(), "{:?}", completion.issue());

        let failed_text = text.replace("value.len()", "value.missing()");
        let mut failed_json = String::new();
        nocter_json::write_string(&mut failed_json, &failed_text);
        let changed = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didChange\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"version\":2}},\"contentChanges\":[{{\"text\":{failed_json}}}]}}}}"
        ));
        assert_eq!(
            changed.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::CompilationFailed
        );
        let failed = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/completion\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":7,\"character\":49}}}}}}"
        ));
        let response = failed.response().unwrap();
        assert!(
            response.contains("\"label\":\"len\",\"kind\":2"),
            "{response}"
        );
        assert!(
            response.contains("\"label\":\"text\",\"kind\":5"),
            "{response}"
        );
        assert!(!response.contains("\"label\":\"clear\""), "{response}");
        assert!(!response.contains("\"label\":\"value\""), "{response}");
        assert!(failed.issue().is_none(), "{:?}", failed.issue());

        let syntax_text = text.replace("value.len()", "value.");
        let mut syntax_json = String::new();
        nocter_json::write_string(&mut syntax_json, &syntax_text);
        let changed = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didChange\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\",\"version\":3}},\"contentChanges\":[{{\"text\":{syntax_json}}}]}}}}"
        ));
        assert_eq!(
            changed.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::SyntaxFailed
        );
        let incomplete = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"textDocument/completion\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":7,\"character\":45}}}}}}"
        ));
        let response = incomplete.response().unwrap();
        assert!(
            response.contains("\"label\":\"len\",\"kind\":2"),
            "{response}"
        );
        assert!(
            response.contains("\"label\":\"text\",\"kind\":5"),
            "{response}"
        );
        assert!(!response.contains("\"label\":\"clear\""), "{response}");
        assert!(incomplete.issue().is_none(), "{:?}", incomplete.issue());
    }

    #[test]
    fn completion_uses_the_use_site_construction_surface_in_every_generation_state() {
        let temporary = TemporaryDirectory::new();
        let (uri, mut server) = construction_completion_server(&temporary);

        let source_with = |selection: &str| {
            concat!(
                "pub enum Choice {\n",
                "    first\n",
                "    second(value: i32)\n",
                "}\n",
                "construct Choice {\n",
                "    pub func new(): Self { loop {} }\n",
                "}\n",
                "func main(): Choice { Choice.$selection }\n",
            )
            .replace("$selection", selection)
        };
        let assert_surface = |step: &ServerStep| {
            let response = step.response().unwrap();
            assert!(
                response.contains("\"label\":\"first\",\"kind\":20"),
                "{response}"
            );
            assert!(
                response.contains("\"label\":\"second\",\"kind\":20"),
                "{response}"
            );
            assert!(
                response.contains("\"label\":\"new\",\"kind\":4"),
                "{response}"
            );
            assert!(!response.contains("\"label\":\"main\""), "{response}");
            assert!(step.issue().is_none(), "{:?}", step.issue());
        };

        let checked_text = source_with("first");
        let opened = set_completion_document(&mut server, &uri, &checked_text, 1);
        assert_eq!(
            opened.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::Complete
        );
        assert_surface(&request_completion(&mut server, &uri, 2, 7, 31));

        let failed_text = source_with("missing");
        let changed = set_completion_document(&mut server, &uri, &failed_text, 2);
        assert_eq!(
            changed.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::CompilationFailed
        );
        assert_surface(&request_completion(&mut server, &uri, 3, 7, 32));

        let incomplete_text = source_with("");
        let changed = set_completion_document(&mut server, &uri, &incomplete_text, 3);
        assert_eq!(
            changed.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::SyntaxFailed
        );
        assert_surface(&request_completion(&mut server, &uri, 4, 7, 29));
    }

    #[test]
    fn completion_supports_builtin_construction_surfaces() {
        let temporary = TemporaryDirectory::new();
        let (uri, mut server) = construction_completion_server(&temporary);
        let changed =
            set_completion_document(&mut server, &uri, "func main(): error { error. }\n", 1);
        assert_eq!(
            changed.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::SyntaxFailed
        );
        let builtin = request_completion(&mut server, &uri, 2, 0, 27);
        let response = builtin.response().unwrap();
        assert!(
            response.contains("\"label\":\"new\",\"kind\":4"),
            "{response}"
        );
        assert!(builtin.issue().is_none(), "{:?}", builtin.issue());

        let changed = set_completion_document(&mut server, &uri, "func main(): void { i32. }\n", 2);
        assert_eq!(
            changed.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::SyntaxFailed
        );
        let empty = request_completion(&mut server, &uri, 3, 0, 24);
        assert!(empty.response().unwrap().contains("\"result\":[]"));
        assert!(empty.issue().is_none(), "{:?}", empty.issue());
    }

    #[test]
    fn builtin_error_members_are_ordinary_source_backed_methods() {
        let temporary = TemporaryDirectory::new();
        let (uri, mut server) = construction_completion_server(&temporary);
        let incomplete = concat!(
            "func main(): void {\n",
            "    let failure = error.new(\"app.failure\", \"failed\")\n",
            "    failure.\n",
            "}\n",
        );
        let changed = set_completion_document(&mut server, &uri, incomplete, 1);
        assert_eq!(
            changed.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::SyntaxFailed
        );
        let completion = request_completion(&mut server, &uri, 2, 2, 12);
        let response = completion.response().unwrap();
        for method in ["context", "code", "message", "has_code"] {
            assert!(
                response.contains(&format!("\"label\":\"{method}\",\"kind\":2")),
                "{response}"
            );
        }
        assert!(!response.contains("\"kind\":5"), "{response}");
        assert!(completion.issue().is_none(), "{:?}", completion.issue());

        let complete = concat!(
            "func main(): usize {\n",
            "    let failure = error.new(\"app.failure\", \"failed\")\n",
            "    let enriched = failure.context(\"outer\")\n",
            "    let first = enriched.message()[0]\n",
            "    return first as usize\n",
            "}\n",
        );
        let changed = set_completion_document(&mut server, &uri, complete, 2);
        let snapshot = changed.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.compilation_failure()
        );
        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":3,\"character\":26}}}}}}"
        ));
        let response = hover.response().unwrap();
        assert!(
            response.contains("pub method &error.message(): &str from self"),
            "{response}"
        );
        assert!(hover.issue().is_none(), "{:?}", hover.issue());

        let definition = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"textDocument/definition\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":3,\"character\":26}}}}}}"
        ));
        let response = definition.response().unwrap();
        assert!(response.contains("/std/error/index.nct"), "{response}");
        assert!(definition.issue().is_none(), "{:?}", definition.issue());

        let references = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"textDocument/references\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":3,\"character\":26}},\"context\":{{\"includeDeclaration\":true}}}}}}"
        ));
        let response = references.response().unwrap();
        assert!(response.contains("/std/error/index.nct"), "{response}");
        assert!(
            response.contains("/std/error/construction.nct"),
            "{response}"
        );
        assert!(response.contains("/main.nct"), "{response}");
        assert!(references.issue().is_none(), "{:?}", references.issue());
    }

    #[test]
    fn completion_recovers_a_generic_construction_owner_before_the_missing_member() {
        let temporary = TemporaryDirectory::new();
        let (uri, mut server) = construction_completion_server(&temporary);
        let generic_text = concat!(
            "pub enum GenericChoice<T> {\n",
            "    empty\n",
            "    value(item: T)\n",
            "}\n",
            "construct GenericChoice<T> {\n",
            "    pub func new(item: T): Self { GenericChoice.value(move item) }\n",
            "}\n",
            "func main(): GenericChoice<i32> { GenericChoice<i32>. }\n",
        );
        let changed = set_completion_document(&mut server, &uri, generic_text, 1);
        assert_eq!(
            changed.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::SyntaxFailed
        );
        let generic = request_completion(&mut server, &uri, 2, 7, 53);
        let response = generic.response().unwrap();
        assert!(
            response.contains("\"label\":\"empty\",\"kind\":20"),
            "{response}"
        );
        assert!(
            response.contains("\"label\":\"value\",\"kind\":20"),
            "{response}"
        );
        assert!(
            response.contains("\"label\":\"new\",\"kind\":4"),
            "{response}"
        );
        assert!(generic.issue().is_none(), "{:?}", generic.issue());
    }

    #[test]
    fn completion_offers_only_uninitialized_structural_fields() {
        let temporary = TemporaryDirectory::new();
        let (uri, mut server) = construction_completion_server(&temporary);
        let source_with = |fields: &str| {
            concat!(
                "struct Record {\n",
                "    first: i32\n",
                "    second: i32\n",
                "    third: i32\n",
                "}\n",
                "func main(): Record {\n",
                "    Record {\n",
                "$fields",
                "    }\n",
                "}\n",
            )
            .replace("$fields", fields)
        };

        let incomplete = source_with("        first: 1,\n\n");
        let opened = set_completion_document(&mut server, &uri, &incomplete, 1);
        assert_eq!(
            opened.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::CompilationFailed
        );
        let completion = request_completion(&mut server, &uri, 2, 8, 0);
        let response = completion.response().unwrap();
        assert!(!response.contains("\"label\":\"first\""), "{response}");
        assert!(
            response.contains("\"label\":\"second\",\"kind\":5"),
            "{response}"
        );
        assert!(
            response.contains("\"label\":\"third\",\"kind\":5"),
            "{response}"
        );
        assert!(completion.issue().is_none(), "{:?}", completion.issue());

        let syntax_incomplete = source_with("        first: 1,\n        second:\n");
        let changed = set_completion_document(&mut server, &uri, &syntax_incomplete, 2);
        assert_eq!(
            changed.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::SyntaxFailed
        );
        let completion = request_completion(&mut server, &uri, 3, 8, 15);
        let response = completion.response().unwrap();
        assert!(!response.contains("\"label\":\"first\""), "{response}");
        assert!(!response.contains("\"label\":\"second\""), "{response}");
        assert!(
            response.contains("\"label\":\"third\",\"kind\":5"),
            "{response}"
        );
        assert!(completion.issue().is_none(), "{:?}", completion.issue());

        let complete = source_with(concat!(
            "        first: 1,\n",
            "        second: 2,\n",
            "        third: 3,\n",
        ));
        let changed = set_completion_document(&mut server, &uri, &complete, 3);
        assert_eq!(
            changed.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::Complete
        );
        let completion = request_completion(&mut server, &uri, 4, 8, 10);
        assert!(
            completion.response().unwrap().contains("\"result\":[]"),
            "{}",
            completion.response().unwrap()
        );
        assert!(completion.issue().is_none(), "{:?}", completion.issue());
    }

    #[test]
    fn completion_in_enum_patterns_excludes_non_variant_construction_entries() {
        let temporary = TemporaryDirectory::new();
        let (uri, mut server) = construction_completion_server(&temporary);
        let source_with = |selection: &str| {
            concat!(
                "enum Choice {\n",
                "    first\n",
                "    second(value: i32)\n",
                "}\n",
                "construct Choice {\n",
                "    pub func new(): Self { loop {} }\n",
                "}\n",
                "func inspect(value: &Choice): void {\n",
                "    match value {\n",
                "        Choice.$selection { return }\n",
                "        _ { return }\n",
                "    }\n",
                "}\n",
            )
            .replace("$selection", selection)
        };
        let assert_variants = |step: &ServerStep| {
            let response = step.response().unwrap();
            assert!(
                response.contains("\"label\":\"first\",\"kind\":20"),
                "{response}"
            );
            assert!(
                response.contains("\"label\":\"second\",\"kind\":20"),
                "{response}"
            );
            assert!(!response.contains("\"label\":\"new\""), "{response}");
            assert!(step.issue().is_none(), "{:?}", step.issue());
        };

        let complete = source_with("first");
        let opened = set_completion_document(&mut server, &uri, &complete, 1);
        assert_eq!(
            opened.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::Complete
        );
        assert_variants(&request_completion(&mut server, &uri, 2, 9, 17));

        let invalid = source_with("missing");
        let changed = set_completion_document(&mut server, &uri, &invalid, 2);
        assert_eq!(
            changed.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::CompilationFailed
        );
        assert_variants(&request_completion(&mut server, &uri, 3, 9, 18));

        let incomplete = source_with("");
        let changed = set_completion_document(&mut server, &uri, &incomplete, 3);
        assert_eq!(
            changed.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::SyntaxFailed
        );
        assert_variants(&request_completion(&mut server, &uri, 4, 9, 15));
    }

    #[test]
    fn completion_uses_resolved_generic_bounds_for_associated_types() {
        let temporary = TemporaryDirectory::new();
        let (uri, mut server) = construction_completion_server(&temporary);
        let source_with = |selection: &str| {
            concat!(
                "interface Source {\n",
                "    pub type Item\n",
                "    pub type Failure\n",
                "    pub method &self.read(): Self.Item\n",
                "}\n",
                "func inspect<T>(value: &T): void where T: Source {\n",
                "    let item: T.$selection = value.read()\n",
                "    return\n",
                "}\n",
            )
            .replace("$selection", selection)
        };
        let assert_associated = |step: &ServerStep| {
            let response = step.response().unwrap();
            assert!(
                response.contains("\"label\":\"Item\",\"kind\":7"),
                "{response}"
            );
            assert!(
                response.contains("\"label\":\"Failure\",\"kind\":7"),
                "{response}"
            );
            assert!(!response.contains("\"label\":\"Source\""), "{response}");
            assert!(step.issue().is_none(), "{:?}", step.issue());
        };

        let complete = source_with("Item");
        let opened = set_completion_document(&mut server, &uri, &complete, 1);
        assert_eq!(
            opened.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::Complete
        );
        assert_associated(&request_completion(&mut server, &uri, 2, 6, 18));

        let invalid = source_with("Missing");
        let changed = set_completion_document(&mut server, &uri, &invalid, 2);
        assert_eq!(
            changed.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::CompilationFailed
        );
        assert_associated(&request_completion(&mut server, &uri, 3, 6, 19));

        let incomplete = source_with("");
        let changed = set_completion_document(&mut server, &uri, &incomplete, 3);
        assert_eq!(
            changed.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::SyntaxFailed
        );
        assert_associated(&request_completion(&mut server, &uri, 4, 6, 16));

        let self_source = |selection: &str| {
            concat!(
                "interface Source {\n",
                "    pub type Item\n",
                "    pub type Failure\n",
                "    pub default method &self.inspect(): void {\n",
                "        let value: Self.$selection = 0\n",
                "        return\n",
                "    }\n",
                "}\n",
            )
            .replace("$selection", selection)
        };
        let changed = set_completion_document(&mut server, &uri, &self_source("Missing"), 4);
        assert_eq!(
            changed.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::CompilationFailed
        );
        assert_associated(&request_completion(&mut server, &uri, 5, 4, 27));

        let changed = set_completion_document(&mut server, &uri, &self_source(""), 5);
        assert_eq!(
            changed.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::SyntaxFailed
        );
        assert_associated(&request_completion(&mut server, &uri, 6, 4, 24));
    }

    #[test]
    fn completion_supplies_a_top_level_edit_for_a_reached_export() {
        let temporary = TemporaryDirectory::new();
        std::fs::write(
            temporary.path().join("index.nct"),
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
        )
        .unwrap();
        std::fs::create_dir(temporary.path().join("tools")).unwrap();
        std::fs::write(
            temporary.path().join("tools/index.nct"),
            "pub func helper(): i32 { return 1 }\n",
        )
        .unwrap();
        let source = concat!(
            "use ./tools\n",
            "\n",
            "func main(): void {\n",
            "    return\n",
            "}\n",
        );
        let source_path = temporary.path().join("app.nct");
        std::fs::write(&source_path, source).unwrap();
        let uri = format!("file://{}", source_path.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let opened = set_completion_document(&mut server, &uri, source, 1);
        assert_eq!(
            opened.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::Complete
        );

        let completion = request_completion(&mut server, &uri, 2, 3, 4);
        let response = completion.response().unwrap();
        assert!(
            response.contains("\"label\":\"helper\",\"kind\":3"),
            "{response}"
        );
        assert!(
            response.contains(concat!(
                "\"additionalTextEdits\":[{\"range\":{",
                "\"start\":{\"line\":0,\"character\":11},",
                "\"end\":{\"line\":0,\"character\":11}},",
                "\"newText\":\"\\nuse ./tools.helper\"}]"
            )),
            "{response}"
        );
        assert!(completion.issue().is_none(), "{:?}", completion.issue());

        let imported = concat!(
            "use ./tools\n",
            "use ./tools.helper\n",
            "\n",
            "func main(): void {\n",
            "    return\n",
            "}\n",
        );
        let changed = set_completion_document(&mut server, &uri, imported, 2);
        let snapshot = changed.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.compilation_failure()
        );
    }

    #[test]
    fn inlay_hints_project_only_unannotated_checked_binding_types_in_the_requested_range() {
        let temporary = TemporaryDirectory::new();
        let source_path = temporary.path().join("main.nct");
        let source = concat!(
            "func main(): void {\n",
            "    let inferred = 1\n",
            "    let explicit: i32 = 2\n",
            "    var mutable = 3\n",
            "    return\n",
            "}\n",
        );
        std::fs::write(&source_path, source).unwrap();
        let uri = format!("file://{}", source_path.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let opened = set_completion_document(&mut server, &uri, source, 1);
        assert_eq!(
            opened.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::Complete
        );

        let hints = server.receive(&format!(
            concat!(
                "{{\"jsonrpc\":\"2.0\",\"id\":2,",
                "\"method\":\"textDocument/inlayHint\",",
                "\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},",
                "\"range\":{{\"start\":{{\"line\":0,\"character\":0}},",
                "\"end\":{{\"line\":5,\"character\":1}}}}}}}}"
            ),
            uri = uri,
        ));
        let response = hints.response().unwrap();
        assert!(
            response.contains(concat!(
                "\"position\":{\"line\":1,\"character\":16},",
                "\"label\":\": i32\",\"kind\":1"
            )),
            "{response}"
        );
        assert!(
            response.contains(concat!(
                "\"position\":{\"line\":3,\"character\":15},",
                "\"label\":\": i32\",\"kind\":1"
            )),
            "{response}"
        );
        assert!(!response.contains("\"line\":2,\"character\":16"));
        assert_eq!(response.matches("\"kind\":1").count(), 2, "{response}");
        assert!(hints.issue().is_none(), "{:?}", hints.issue());

        let narrowed = server.receive(&format!(
            concat!(
                "{{\"jsonrpc\":\"2.0\",\"id\":3,",
                "\"method\":\"textDocument/inlayHint\",",
                "\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},",
                "\"range\":{{\"start\":{{\"line\":3,\"character\":0}},",
                "\"end\":{{\"line\":3,\"character\":19}}}}}}}}"
            ),
            uri = uri,
        ));
        let response = narrowed.response().unwrap();
        assert_eq!(response.matches("\"kind\":1").count(), 1, "{response}");
        assert!(response.contains("\"line\":3,\"character\":15"));
        assert!(narrowed.issue().is_none(), "{:?}", narrowed.issue());

        let ending_at_hint = server.receive(&format!(
            concat!(
                "{{\"jsonrpc\":\"2.0\",\"id\":4,",
                "\"method\":\"textDocument/inlayHint\",",
                "\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},",
                "\"range\":{{\"start\":{{\"line\":1,\"character\":0}},",
                "\"end\":{{\"line\":1,\"character\":16}}}}}}}}"
            ),
            uri = uri,
        ));
        let response = ending_at_hint.response().unwrap();
        assert!(response.contains("\"result\":[]"), "{response}");
        assert!(
            ending_at_hint.issue().is_none(),
            "{:?}",
            ending_at_hint.issue()
        );

        let extending_past_hint = server.receive(&format!(
            concat!(
                "{{\"jsonrpc\":\"2.0\",\"id\":5,",
                "\"method\":\"textDocument/inlayHint\",",
                "\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},",
                "\"range\":{{\"start\":{{\"line\":1,\"character\":0}},",
                "\"end\":{{\"line\":1,\"character\":17}}}}}}}}"
            ),
            uri = uri,
        ));
        let response = extending_past_hint.response().unwrap();
        assert_eq!(response.matches("\"kind\":1").count(), 1, "{response}");
        assert!(response.contains("\"line\":1,\"character\":16"));
        assert!(
            extending_past_hint.issue().is_none(),
            "{:?}",
            extending_past_hint.issue()
        );
    }

    #[test]
    fn inlay_hints_show_only_elided_external_result_provenance() {
        let temporary = TemporaryDirectory::new();
        let source_path = temporary.path().join("main.nct");
        let source = concat!(
            "func view(text: &str): &str {\n",
            "    let closure = (): bool { true }\n",
            "    text\n",
            "}\n",
            "func explicit(text: &str): &str from text { return text }\n",
            "func main(): void { return }\n",
            "struct Text { value: &str }\n",
            "instance Text {\n",
            "    pub coerce &self as &str { self.value }\n",
            "}\n",
        );
        std::fs::write(&source_path, source).unwrap();
        let uri = format!("file://{}", source_path.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let opened = set_completion_document(&mut server, &uri, source, 1);
        assert_eq!(
            opened.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::Complete
        );

        let hints = server.receive(&format!(
            concat!(
                "{{\"jsonrpc\":\"2.0\",\"id\":2,",
                "\"method\":\"textDocument/inlayHint\",",
                "\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},",
                "\"range\":{{\"start\":{{\"line\":0,\"character\":0}},",
                "\"end\":{{\"line\":9,\"character\":1}}}}}}}}"
            ),
            uri = uri,
        ));
        let response = hints.response().unwrap();
        assert!(
            response.contains(concat!(
                "\"position\":{\"line\":0,\"character\":27},",
                "\"label\":\" from text\""
            )),
            "{response}"
        );
        assert_eq!(response.matches(" from text").count(), 1, "{response}");
        assert!(
            response.contains(concat!(
                "\"position\":{\"line\":8,\"character\":28},",
                "\"label\":\" from self\""
            )),
            "{response}"
        );
        assert!(!response.contains("\"line\":4"), "{response}");
        assert!(hints.issue().is_none(), "{:?}", hints.issue());
    }

    #[test]
    fn automatic_imports_respect_direct_dependency_visibility() {
        let temporary = TemporaryDirectory::new();
        let application = temporary.path().join("app");
        let dependency = temporary.path().join("dependency");
        std::fs::create_dir(&application).unwrap();
        std::fs::create_dir(&dependency).unwrap();
        std::fs::create_dir(dependency.join("api")).unwrap();
        std::fs::write(
            application.join("index.nct"),
            concat!(
                "#package: { name: \"app\", version: \"0.0.0\", }\n",
                "#dependencies: { dep: { path: \"../dependency\" } }\n",
            ),
        )
        .unwrap();
        std::fs::write(
            dependency.join("index.nct"),
            "#package: { name: \"dependency\", version: \"0.0.0\", }\n",
        )
        .unwrap();
        std::fs::write(
            dependency.join("api/index.nct"),
            concat!(
                "pub func public_helper(): i32 { return 1 }\n",
                "func private_helper(): i32 { return 2 }\n",
            ),
        )
        .unwrap();
        let source = concat!(
            "use dep/api\n",
            "\n",
            "func main(): void {\n",
            "    return\n",
            "}\n",
        );
        let source_path = application.join("app.nct");
        std::fs::write(&source_path, source).unwrap();
        let uri = format!("file://{}", source_path.display());
        let mut server = semantic_server(&application);
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            application.display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let opened = set_completion_document(&mut server, &uri, source, 1);
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "compilation={:?}, discovery={:?}",
            snapshot.compilation_failure(),
            snapshot.discovery_failure()
        );

        let completion = request_completion(&mut server, &uri, 2, 3, 4);
        let response = completion.response().unwrap();
        assert!(
            response.contains("\"label\":\"public_helper\""),
            "{response}"
        );
        assert!(response.contains("use dep/api.public_helper"), "{response}");
        assert!(!response.contains("private_helper"), "{response}");
        assert!(completion.issue().is_none(), "{:?}", completion.issue());
    }

    #[test]
    fn automatic_imports_do_not_create_a_module_cycle() {
        let temporary = TemporaryDirectory::new();
        std::fs::write(
            temporary.path().join("index.nct"),
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
        )
        .unwrap();
        std::fs::create_dir(temporary.path().join("child")).unwrap();
        std::fs::write(
            temporary.path().join("index.nct"),
            concat!(
                "#package: { name: \"app\", version: \"0.0.0\", }\n",
                "pub func root_value(): i32 {\n",
                "    use ./child\n",
                "\n",
                "    return 1\n",
                "}\n",
            ),
        )
        .unwrap();
        let child_source = "func inspect(): void { return }\n";
        let child_path = temporary.path().join("child/index.nct");
        std::fs::write(&child_path, child_source).unwrap();
        let uri = format!("file://{}", child_path.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let opened = set_completion_document(&mut server, &uri, child_source, 1);
        assert_eq!(
            opened.analysis().unwrap().snapshot().unwrap().status(),
            nocter_analysis::AnalysisStatus::Complete
        );

        let completion = request_completion(&mut server, &uri, 2, 0, 5);
        let response = completion.response().unwrap();
        assert!(!response.contains("\"label\":\"root_value\""), "{response}");
        assert!(completion.issue().is_none(), "{:?}", completion.issue());
    }

    #[test]
    fn code_actions_publish_only_recompiled_compiler_owned_import_edits() {
        let temporary = TemporaryDirectory::new();
        std::fs::write(
            temporary.path().join("index.nct"),
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
        )
        .unwrap();
        std::fs::create_dir(temporary.path().join("api")).unwrap();
        std::fs::create_dir(temporary.path().join("child")).unwrap();
        std::fs::write(
            temporary.path().join("index.nct"),
            concat!(
                "#package: { name: \"app\", version: \"0.0.0\", }\n",
                "use ./api\n",
                "use ./child\n",
                "pub func root_marker(): i32 { return 1 }\n",
            ),
        )
        .unwrap();
        std::fs::write(
            temporary.path().join("api/index.nct"),
            "pub func public_helper(): i32 { return 7 }\n",
        )
        .unwrap();
        let source = "func inspect(): i32 { return public_helper() }\n";
        let source_path = temporary.path().join("child/index.nct");
        std::fs::write(&source_path, source).unwrap();
        let uri = format!("file://{}", source_path.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let opened = set_completion_document(&mut server, &uri, source, 1);
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::CompilationFailed
        );
        assert_eq!(snapshot.diagnostics()[0].code(), "E0340");

        let completion = request_completion(&mut server, &uri, 4, 0, 30);
        assert!(
            completion
                .response()
                .unwrap()
                .contains("\"label\":\"public_helper\""),
            "{}",
            completion.response().unwrap()
        );

        let action = request_code_action(&mut server, &uri, 2, (0, 29), (0, 42));
        let response = action.response().unwrap();
        assert!(
            response.contains("Import `public_helper` from `../api.public_helper`"),
            "{response}"
        );
        assert!(response.contains("\"version\":1"), "{response}");
        assert!(response.contains("use ../api.public_helper"), "{response}");
        assert!(response.contains("\"isPreferred\":true"), "{response}");
        assert!(action.issue().is_none(), "{:?}", action.issue());

        let outside = request_code_action(&mut server, &uri, 3, (0, 0), (0, 3));
        assert!(outside.response().unwrap().contains("\"result\":[]"));
        assert!(outside.issue().is_none(), "{:?}", outside.issue());

        let ending_at_diagnostic = request_code_action(&mut server, &uri, 5, (0, 0), (0, 29));
        assert!(
            ending_at_diagnostic
                .response()
                .unwrap()
                .contains("\"result\":[]")
        );
        assert!(
            ending_at_diagnostic.issue().is_none(),
            "{:?}",
            ending_at_diagnostic.issue()
        );

        let cursor_at_diagnostic = request_code_action(&mut server, &uri, 6, (0, 29), (0, 29));
        assert!(
            cursor_at_diagnostic
                .response()
                .unwrap()
                .contains("Import `public_helper` from `../api.public_helper`")
        );
        assert!(
            cursor_at_diagnostic.issue().is_none(),
            "{:?}",
            cursor_at_diagnostic.issue()
        );

        let source_only = server.receive(&format!(
            concat!(
                "{{\"jsonrpc\":\"2.0\",\"id\":7,",
                "\"method\":\"textDocument/codeAction\",",
                "\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},",
                "\"range\":{{\"start\":{{\"line\":0,\"character\":29}},",
                "\"end\":{{\"line\":0,\"character\":42}}}},",
                "\"context\":{{\"diagnostics\":[],\"only\":[\"source\"]}}}}}}"
            ),
            uri = uri,
        ));
        assert!(source_only.response().unwrap().contains("\"result\":[]"));
        assert!(source_only.issue().is_none(), "{:?}", source_only.issue());
    }

    #[test]
    fn code_actions_implement_required_conformance_methods_with_abort() {
        let temporary = TemporaryDirectory::new();
        std::fs::write(
            temporary.path().join("index.nct"),
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
        )
        .unwrap();
        let source = concat!(
            "pub interface Readable {\n",
            "    pub type Item\n",
            "    pub method &self.read<T>(fallback: T): Self.Item from self where copy T\n",
            "    pub method &self.ready(): bool\n",
            "}\n",
            "\n",
            "struct Value {}\n",
            "conform Readable for Value {\n",
            "    type Item = i32\n",
            "}\n",
        );
        let source_path = temporary.path().join("index.nct");
        let full_source = format!("#package: {{ name: \"app\", version: \"0.0.0\", }}\n{source}");
        std::fs::write(&source_path, &full_source).unwrap();
        let uri = format!("file://{}", source_path.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let opened = set_completion_document(&mut server, &uri, &full_source, 1);
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::CompilationFailed
        );
        assert_eq!(snapshot.diagnostics()[0].code(), "E0350");

        let action = server.receive(&format!(
            concat!(
                "{{\"jsonrpc\":\"2.0\",\"id\":2,",
                "\"method\":\"textDocument/codeAction\",",
                "\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},",
                "\"range\":{{\"start\":{{\"line\":8,\"character\":0}},",
                "\"end\":{{\"line\":10,\"character\":1}}}},",
                "\"context\":{{\"diagnostics\":[]}}}}}}"
            ),
            uri = uri,
        ));
        let response = action.response().unwrap();
        assert!(
            response.contains("Implement 2 required methods"),
            "{response}"
        );
        assert!(response.contains("use std/process.abort"), "{response}");
        assert!(
            response.contains("method &self.read<T>(fallback: T): i32 where copy T"),
            "{response}"
        );
        assert!(
            response.contains("method &self.ready(): bool"),
            "{response}"
        );
        assert!(response.contains("abort()"), "{response}");
        assert!(response.contains("\"version\":1"), "{response}");
        assert!(response.contains("\"isPreferred\":true"), "{response}");
        assert!(action.issue().is_none(), "{:?}", action.issue());
    }

    #[test]
    fn separated_conformance_code_action_edits_the_private_definition() {
        let temporary = TemporaryDirectory::new();
        std::fs::write(
            temporary.path().join("index.nct"),
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
        )
        .unwrap();
        let contract = concat!(
            "see ./value.nct\n",
            "pub interface Readable {\n",
            "    pub method &self.read(): i32\n",
            "}\n",
            "pub struct Value {}\n",
            "conform Readable for Value {}\n",
        );
        let contract_path = temporary.path().join("index.nct");
        let implementation_path = temporary.path().join("value.nct");
        let full_contract =
            format!("#package: {{ name: \"app\", version: \"0.0.0\", }}\n{contract}");
        std::fs::write(&contract_path, &full_contract).unwrap();
        std::fs::write(
            &implementation_path,
            "see ./index.nct\nconform Readable for Value {}\n",
        )
        .unwrap();
        let contract_uri = format!("file://{}", contract_path.display());
        let implementation_uri = format!(
            "file://{}",
            std::fs::canonicalize(&implementation_path)
                .unwrap()
                .display()
        );
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let opened = set_completion_document(&mut server, &contract_uri, &full_contract, 1);
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::CompilationFailed
        );
        assert_eq!(snapshot.diagnostics()[0].code(), "E0350");

        let action = server.receive(&format!(
            concat!(
                "{{\"jsonrpc\":\"2.0\",\"id\":2,",
                "\"method\":\"textDocument/codeAction\",",
                "\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},",
                "\"range\":{{\"start\":{{\"line\":6,\"character\":0}},",
                "\"end\":{{\"line\":6,\"character\":29}}}},",
                "\"context\":{{\"diagnostics\":[]}}}}}}"
            ),
            uri = contract_uri,
        ));
        let response = action.response().unwrap();
        assert!(
            response.contains("Implement required method `read`"),
            "{response}"
        );
        assert!(response.contains(&implementation_uri), "{response}");
        assert!(response.contains("method &self.read(): i32"), "{response}");
        assert!(response.contains("use std/process.abort"), "{response}");
        assert!(response.contains("\"version\":null"), "{response}");
        assert!(action.issue().is_none(), "{:?}", action.issue());
    }

    #[test]
    fn code_actions_add_missing_callable_outcome_contracts() {
        for (source, expected) in [
            (
                "func load(value: i32!): i32 { move value? }\n",
                "Make callable result fallible: `i32!`",
            ),
            (
                "func load(value: i32?): i32 { value? }\n",
                "Make callable result optional: `i32?`",
            ),
            (
                "func load(value: i32?): i32! { value? }\n",
                "Make callable result optional: `i32?!`",
            ),
        ] {
            let line_length = source.trim_end().len();
            let action = request_outcome_code_action(source, (0, 0), (0, line_length));
            let response = action.response().unwrap();
            assert!(response.contains(expected), "{source}: {response}");
            assert!(response.contains("\"version\":1"), "{response}");
            assert!(response.contains("\"isPreferred\":true"), "{response}");
            assert!(action.issue().is_none(), "{:?}", action.issue());
        }
    }

    #[test]
    fn callable_outcome_action_ignores_nested_closure_results() {
        let source = concat!(
            "func load(value: i32!): i32 {\n",
            "    let closure = (): bool { true }\n",
            "    move value?\n",
            "}\n",
        );
        let action = request_outcome_code_action(source, (2, 4), (2, 15));
        let response = action.response().unwrap();
        assert!(
            response.contains("Make callable result fallible: `i32!`"),
            "{response}"
        );
        assert!(action.issue().is_none(), "{:?}", action.issue());
    }

    #[test]
    fn callable_outcome_action_rewrites_method_results() {
        let source = concat!(
            "struct Loader {}\n",
            "instance Loader {\n",
            "    pub method &self.load(value: i32!): i32 { move value? }\n",
            "}\n",
        );
        let action = request_outcome_code_action(source, (2, 48), (2, 59));
        let response = action.response().unwrap();
        assert!(
            response.contains("Make callable result fallible: `i32!`"),
            "{response}"
        );
        assert!(action.issue().is_none(), "{:?}", action.issue());
    }

    #[test]
    fn code_actions_do_not_rewrite_fixed_operator_results() {
        let source = concat!(
            "struct Value { status: bool! }\n",
            "instance Value {\n",
            "    pub operator (&self == other: &Self): bool { self.status? }\n",
            "}\n",
        );
        let action = request_outcome_code_action(source, (2, 0), (2, 63));
        let response = action.response().unwrap();
        assert!(response.contains("\"result\":[]"), "{response}");
        assert!(action.issue().is_none(), "{:?}", action.issue());
    }

    #[test]
    fn package_root_selection_compiles_from_a_child_target() {
        let temporary = TemporaryDirectory::new();
        std::fs::write(
            temporary.path().join("index.nct"),
            concat!(
                "#package: { name: \"app\", version: \"0.0.0\", }\n",
                "#executable: { name: \"app\", module: \"./child\" }\n",
            ),
        )
        .unwrap();
        std::fs::create_dir(temporary.path().join("child")).unwrap();
        std::fs::write(
            temporary.path().join("index.nct"),
            concat!(
                "#package: { name: \"app\", version: \"0.0.0\", }\n",
                "#executable: { name: \"app\", module: \"./child\" }\n",
                "pub func root_value(): i32 { return 1 }\n",
            ),
        )
        .unwrap();
        let source = concat!(
            "use /.root_value\n",
            "\n",
            "func main(): i32 { return root_value() }\n",
        );
        let source_path = temporary.path().join("child/index.nct");
        std::fs::write(&source_path, source).unwrap();
        let uri = format!("file://{}", source_path.display());
        let mut server = semantic_server(temporary.path());
        server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}}}",
            temporary.path().display()
        ));
        server.receive(r#"{"jsonrpc":"2.0","method":"initialized"}"#);
        let opened = set_completion_document(&mut server, &uri, source, 1);
        let snapshot = opened.analysis().unwrap().snapshot().unwrap();
        assert_eq!(
            snapshot.status(),
            nocter_analysis::AnalysisStatus::Complete,
            "{:?}",
            snapshot.compilation_failure()
        );
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

        let hover = server.receive(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/hover\",\"params\":{{\"textDocument\":{{\"uri\":\"{document_uri}\"}},\"position\":{{\"line\":{line},\"character\":{}}}}}}}",
            start + 1
        ));
        let response = hover.response().unwrap();
        assert!(response.contains("module std/iter"), "{response}");
        assert!(
            response.contains("Core iteration contracts, source iterators, and lazy adapters."),
            "{response}"
        );
        assert!(hover.issue().is_none(), "{:?}", hover.issue());
    }
}
