use nocter_json::{Value, write_string, write_value};

use crate::RequestId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseErrorCode {
    ParseError,
    InvalidRequest,
    MethodNotFound,
    InvalidParams,
    InternalError,
    ServerNotInitialized,
}

impl ResponseErrorCode {
    #[must_use]
    pub const fn value(self) -> i32 {
        match self {
            Self::ParseError => -32700,
            Self::InvalidRequest => -32600,
            Self::MethodNotFound => -32601,
            Self::InvalidParams => -32602,
            Self::InternalError => -32603,
            Self::ServerNotInitialized => -32002,
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::ParseError => "Parse error",
            Self::InvalidRequest => "Invalid Request",
            Self::MethodNotFound => "Method not found",
            Self::InvalidParams => "Invalid params",
            Self::InternalError => "Internal error",
            Self::ServerNotInitialized => "Server not initialized",
        }
    }
}

#[must_use]
pub fn render_success_response(id: &RequestId, result: &Value) -> String {
    let mut output = String::from("{\"jsonrpc\":\"2.0\",\"id\":");
    write_request_id(&mut output, id);
    output.push_str(",\"result\":");
    write_value(&mut output, result);
    output.push('}');
    output
}

/// Renders one JSON-RPC notification with an already typed method and parameter value.
#[must_use]
pub fn render_notification(method: &str, params: &Value) -> String {
    let mut output = String::from("{\"jsonrpc\":\"2.0\",\"method\":");
    write_string(&mut output, method);
    output.push_str(",\"params\":");
    write_value(&mut output, params);
    output.push('}');
    output
}

#[must_use]
pub fn render_error_response(
    id: Option<&RequestId>,
    code: ResponseErrorCode,
    data: Option<&Value>,
) -> String {
    let mut output = String::from("{\"jsonrpc\":\"2.0\",\"id\":");
    match id {
        Some(id) => write_request_id(&mut output, id),
        None => output.push_str("null"),
    }
    output.push_str(",\"error\":{\"code\":");
    output.push_str(&code.value().to_string());
    output.push_str(",\"message\":");
    write_string(&mut output, code.message());
    if let Some(data) = data {
        output.push_str(",\"data\":");
        write_value(&mut output, data);
    }
    output.push_str("}}");
    output
}

fn write_request_id(output: &mut String, id: &RequestId) {
    match id {
        RequestId::Integer(value) => output.push_str(&value.to_string()),
        RequestId::String(value) => write_string(output, value),
    }
}

#[cfg(test)]
mod tests {
    use nocter_json::Member;

    use super::*;

    #[test]
    fn renders_exact_success_with_the_original_request_identity() {
        let result = Value::Object(vec![Member {
            name: "positionEncoding".into(),
            value: Value::String("utf-16".into()),
        }]);
        assert_eq!(
            render_success_response(&RequestId::String("client\"1".into()), &result),
            r#"{"jsonrpc":"2.0","id":"client\"1","result":{"positionEncoding":"utf-16"}}"#
        );
    }

    #[test]
    fn renders_standard_error_with_null_id_and_optional_data() {
        assert_eq!(
            render_error_response(
                None,
                ResponseErrorCode::InvalidRequest,
                Some(&Value::String("duplicate id".into())),
            ),
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"Invalid Request","data":"duplicate id"}}"#
        );
    }

    #[test]
    fn renders_notifications_without_request_identity() {
        assert_eq!(
            render_notification("workspace/example", &Value::Bool(true)),
            r#"{"jsonrpc":"2.0","method":"workspace/example","params":true}"#
        );
    }
}
