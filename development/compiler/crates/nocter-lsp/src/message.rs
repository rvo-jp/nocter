use std::fmt;

use nocter_json::{JsonError, Member, Value, parse};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RequestId {
    Integer(i32),
    String(Box<str>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResponseResult {
    Success(Value),
    Error(ResponseError),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResponseError {
    code: i32,
    message: Box<str>,
    data: Option<Value>,
}

impl ResponseError {
    #[must_use]
    pub const fn code(&self) -> i32 {
        self.code
    }

    #[must_use]
    pub const fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub const fn data(&self) -> Option<&Value> {
        self.data.as_ref()
    }
}

impl fmt::Display for ResponseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "client response error {}: {}",
            self.code, self.message
        )
    }
}

impl std::error::Error for ResponseError {}

#[derive(Clone, Debug, PartialEq)]
pub enum IncomingMessage {
    Request {
        id: RequestId,
        method: Box<str>,
        params: Option<Value>,
    },
    Notification {
        method: Box<str>,
        params: Option<Value>,
    },
    Response {
        id: RequestId,
        result: ResponseResult,
    },
}

impl IncomingMessage {
    /// Decodes one LSP JSON-RPC request, notification, or response.
    ///
    /// # Errors
    ///
    /// Returns a parse or exact envelope-shape failure. JSON-RPC batch messages are not part of
    /// LSP and are rejected.
    pub fn decode(input: &str) -> Result<Self, MessageDecodeError> {
        let value = parse(input).map_err(MessageDecodeError::from_json)?;
        let Value::Object(members) = value else {
            return Err(MessageDecodeError::new(
                MessageDecodeErrorKind::ExpectedObject,
            ));
        };
        decode_object(members)
    }

    #[must_use]
    pub const fn method(&self) -> Option<&str> {
        match self {
            Self::Request { method, .. } | Self::Notification { method, .. } => Some(method),
            Self::Response { .. } => None,
        }
    }

    #[must_use]
    pub const fn request_id(&self) -> Option<&RequestId> {
        match self {
            Self::Request { id, .. } | Self::Response { id, .. } => Some(id),
            Self::Notification { .. } => None,
        }
    }

    #[must_use]
    pub const fn is_request(&self) -> bool {
        matches!(self, Self::Request { .. })
    }

    #[must_use]
    pub const fn is_response(&self) -> bool {
        matches!(self, Self::Response { .. })
    }
}

fn decode_object(members: Vec<Member>) -> Result<IncomingMessage, MessageDecodeError> {
    let mut version = None;
    let mut id = None;
    let mut method = None;
    let mut params = None;
    let mut result = None;
    let mut error = None;
    for Member { name, value } in members {
        match name.as_ref() {
            "jsonrpc" => set_once(&mut version, value, "jsonrpc")?,
            "id" => set_once(&mut id, value, "id")?,
            "method" => set_once(&mut method, value, "method")?,
            "params" => set_once(&mut params, value, "params")?,
            "result" => set_once(&mut result, value, "result")?,
            "error" => set_once(&mut error, value, "error")?,
            _ => {}
        }
    }

    match version {
        Some(Value::String(value)) if value.as_ref() == "2.0" => {}
        Some(_) => {
            return Err(MessageDecodeError::new(
                MessageDecodeErrorKind::UnsupportedVersion,
            ));
        }
        None => {
            return Err(MessageDecodeError::new(
                MessageDecodeErrorKind::MissingVersion,
            ));
        }
    }
    if let Some(method) = method {
        let Value::String(method) = method else {
            return Err(MessageDecodeError::new(
                MessageDecodeErrorKind::InvalidMethod,
            ));
        };
        if result.is_some() || error.is_some() {
            return Err(MessageDecodeError::new(
                MessageDecodeErrorKind::InvalidResponse,
            ));
        }
        if !matches!(params, None | Some(Value::Array(_) | Value::Object(_))) {
            return Err(MessageDecodeError::new(
                MessageDecodeErrorKind::InvalidParams,
            ));
        }

        return match id {
            Some(value) => Ok(IncomingMessage::Request {
                id: decode_id(value)?,
                method,
                params,
            }),
            None => Ok(IncomingMessage::Notification { method, params }),
        };
    }

    if params.is_some() {
        return Err(MessageDecodeError::new(
            MessageDecodeErrorKind::InvalidResponse,
        ));
    }
    let id =
        id.ok_or_else(|| MessageDecodeError::new(MessageDecodeErrorKind::MissingResponseId))?;
    let result = match (result, error) {
        (Some(result), None) => ResponseResult::Success(result),
        (None, Some(error)) => ResponseResult::Error(decode_response_error(error)?),
        (None, None) | (Some(_), Some(_)) => {
            return Err(MessageDecodeError::new(
                MessageDecodeErrorKind::InvalidResponse,
            ));
        }
    };
    Ok(IncomingMessage::Response {
        id: decode_id(id)?,
        result,
    })
}

fn decode_response_error(value: Value) -> Result<ResponseError, MessageDecodeError> {
    let Value::Object(members) = value else {
        return Err(MessageDecodeError::new(
            MessageDecodeErrorKind::InvalidResponseError,
        ));
    };
    let mut code = None;
    let mut message = None;
    let mut data = None;
    for Member { name, value } in members {
        match name.as_ref() {
            "code" => set_once(&mut code, value, "error.code")?,
            "message" => set_once(&mut message, value, "error.message")?,
            "data" => set_once(&mut data, value, "error.data")?,
            _ => {}
        }
    }
    let code = match code {
        Some(Value::Number(value)) => value
            .parse::<i32>()
            .map_err(|_| MessageDecodeError::new(MessageDecodeErrorKind::InvalidResponseError))?,
        None | Some(_) => {
            return Err(MessageDecodeError::new(
                MessageDecodeErrorKind::InvalidResponseError,
            ));
        }
    };
    let Some(Value::String(message)) = message else {
        return Err(MessageDecodeError::new(
            MessageDecodeErrorKind::InvalidResponseError,
        ));
    };
    Ok(ResponseError {
        code,
        message,
        data,
    })
}

fn set_once(
    slot: &mut Option<Value>,
    value: Value,
    field: &'static str,
) -> Result<(), MessageDecodeError> {
    if slot.replace(value).is_some() {
        Err(MessageDecodeError::duplicate(field))
    } else {
        Ok(())
    }
}

fn decode_id(value: Value) -> Result<RequestId, MessageDecodeError> {
    match value {
        Value::String(value) => Ok(RequestId::String(value)),
        Value::Number(value) => value
            .parse::<i32>()
            .map(RequestId::Integer)
            .map_err(|_| MessageDecodeError::new(MessageDecodeErrorKind::InvalidId)),
        Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => {
            Err(MessageDecodeError::new(MessageDecodeErrorKind::InvalidId))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageDecodeErrorKind {
    Json,
    ExpectedObject,
    DuplicateField,
    MissingVersion,
    UnsupportedVersion,
    MissingMethod,
    MissingResponseId,
    InvalidMethod,
    InvalidParams,
    InvalidId,
    InvalidResponse,
    InvalidResponseError,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageDecodeError {
    kind: MessageDecodeErrorKind,
    json: Option<JsonError>,
    field: Option<&'static str>,
}

impl MessageDecodeError {
    const fn new(kind: MessageDecodeErrorKind) -> Self {
        Self {
            kind,
            json: None,
            field: None,
        }
    }

    const fn duplicate(field: &'static str) -> Self {
        Self {
            kind: MessageDecodeErrorKind::DuplicateField,
            json: None,
            field: Some(field),
        }
    }

    const fn from_json(error: JsonError) -> Self {
        Self {
            kind: MessageDecodeErrorKind::Json,
            json: Some(error),
            field: None,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> MessageDecodeErrorKind {
        self.kind
    }

    #[must_use]
    pub const fn json_error(&self) -> Option<JsonError> {
        self.json
    }
}

impl fmt::Display for MessageDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            MessageDecodeErrorKind::Json => self
                .json
                .expect("JSON error kind retains its source")
                .fmt(formatter),
            MessageDecodeErrorKind::ExpectedObject => {
                formatter.write_str("LSP message must be a JSON object")
            }
            MessageDecodeErrorKind::DuplicateField => write!(
                formatter,
                "duplicate JSON-RPC field `{}`",
                self.field.expect("duplicate field error retains its field")
            ),
            MessageDecodeErrorKind::MissingVersion => {
                formatter.write_str("missing JSON-RPC version")
            }
            MessageDecodeErrorKind::UnsupportedVersion => {
                formatter.write_str("JSON-RPC version must be `2.0`")
            }
            MessageDecodeErrorKind::MissingMethod => formatter.write_str("missing JSON-RPC method"),
            MessageDecodeErrorKind::MissingResponseId => {
                formatter.write_str("missing JSON-RPC response id")
            }
            MessageDecodeErrorKind::InvalidMethod => {
                formatter.write_str("JSON-RPC method must be a string")
            }
            MessageDecodeErrorKind::InvalidParams => {
                formatter.write_str("JSON-RPC params must be an object or array")
            }
            MessageDecodeErrorKind::InvalidId => {
                formatter.write_str("LSP request id must be a 32-bit integer or string")
            }
            MessageDecodeErrorKind::InvalidResponse => formatter.write_str(
                "LSP response must contain an id and exactly one of `result` or `error`",
            ),
            MessageDecodeErrorKind::InvalidResponseError => {
                formatter.write_str("LSP response error must contain an integer code and message")
            }
        }
    }
}

impl std::error::Error for MessageDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.json.as_ref().map(|error| error as _)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_request_and_notification_without_collapsing_params() {
        let request = IncomingMessage::decode(
            r#"{"jsonrpc":"2.0","id":7,"method":"initialize","params":{"rootUri":null}}"#,
        )
        .unwrap();
        assert_eq!(request.method(), Some("initialize"));
        assert_eq!(request.request_id(), Some(&RequestId::Integer(7)));
        assert!(request.is_request());

        let notification =
            IncomingMessage::decode(r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#)
                .unwrap();
        assert_eq!(notification.method(), Some("initialized"));
        assert!(!notification.is_request());
    }

    #[test]
    fn decodes_success_and_error_responses_without_a_method() {
        let success = IncomingMessage::decode(r#"{"jsonrpc":"2.0","id":3,"result":null}"#).unwrap();
        assert_eq!(success.request_id(), Some(&RequestId::Integer(3)));
        assert!(success.is_response());
        assert!(matches!(
            success,
            IncomingMessage::Response {
                result: ResponseResult::Success(Value::Null),
                ..
            }
        ));

        let error = IncomingMessage::decode(
            r#"{"jsonrpc":"2.0","id":"watch","error":{"code":-32603,"message":"no"}}"#,
        )
        .unwrap();
        let IncomingMessage::Response {
            result: ResponseResult::Error(error),
            ..
        } = error
        else {
            panic!("expected error response")
        };
        assert_eq!(error.code(), -32603);
        assert_eq!(error.message(), "no");
    }

    #[test]
    fn rejects_invalid_envelopes_before_dispatch() {
        let cases = [
            ("[]", MessageDecodeErrorKind::ExpectedObject),
            (
                r#"{"method":"initialize"}"#,
                MessageDecodeErrorKind::MissingVersion,
            ),
            (
                r#"{"jsonrpc":"1.0","method":"initialize"}"#,
                MessageDecodeErrorKind::UnsupportedVersion,
            ),
            (
                r#"{"jsonrpc":"2.0","method":"x","params":true}"#,
                MessageDecodeErrorKind::InvalidParams,
            ),
            (
                r#"{"jsonrpc":"2.0","id":1.5,"method":"x"}"#,
                MessageDecodeErrorKind::InvalidId,
            ),
            (
                r#"{"jsonrpc":"2.0","method":"x","method":"y"}"#,
                MessageDecodeErrorKind::DuplicateField,
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(IncomingMessage::decode(input).unwrap_err().kind(), expected);
        }
    }
}
