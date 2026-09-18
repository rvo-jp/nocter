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
                    SemanticTokensQueryError::Evidence(_)
                    | SemanticTokensQueryError::Coordinate(_)
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
                    NavigationQueryError::Evidence(_)
                    | NavigationQueryError::MissingSource(_)
                    | NavigationQueryError::Uri(_) => ResponseErrorCode::InternalError,
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
mod tests;
