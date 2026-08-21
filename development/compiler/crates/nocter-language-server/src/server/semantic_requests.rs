use nocter_json::Value;
use nocter_lsp::{
    CompletionParams, DefinitionParams, HoverParams, ReferencesParams, RenameParams, RequestId,
    ResponseErrorCode, SemanticTokensParams, SignatureHelpParams, render_error_response,
    render_success_response,
};

use crate::completion::query_completion;
use crate::hover::query_hover;
use crate::navigation::{NavigationQueryError, query_definition, query_references};
use crate::rename::query_rename;
use crate::semantic_tokens::{SemanticTokensQueryError, query_semantic_tokens};
use crate::signature::query_signature_help;

use super::{LanguageServer, ServerIssue, ServerStep};

impl LanguageServer {
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
                ServerStep {
                    response: Some(render_error_response(
                        Some(id),
                        ResponseErrorCode::InvalidParams,
                        Some(&detail),
                    )),
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
                ServerStep {
                    response: Some(render_error_response(
                        Some(id),
                        ResponseErrorCode::InvalidParams,
                        Some(&detail),
                    )),
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
        std::fs::write(temporary.path().join("nocter.nct"), "#name: \"app\"\n").unwrap();
        let source = temporary.path().join("index.nct");
        let helper = temporary.path().join("helper.nct");
        std::fs::write(
            &source,
            concat!(
                "use ./helper\n",
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
            "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"textDocument/completion\",\"params\":{{\"textDocument\":{{\"uri\":\"{uri}\"}},\"position\":{{\"line\":3,\"character\":10}}}}}}"
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
