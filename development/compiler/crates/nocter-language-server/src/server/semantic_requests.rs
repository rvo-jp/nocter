use nocter_json::Value;
use nocter_lsp::{
    HoverParams, RequestId, ResponseErrorCode, SemanticTokensParams, render_error_response,
    render_success_response,
};

use crate::hover::query_hover;
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
}
