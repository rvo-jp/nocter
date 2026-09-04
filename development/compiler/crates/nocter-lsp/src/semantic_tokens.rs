use std::fmt;

use nocter_json::{Member, Value};

use crate::decode::{Object, required, string};
use crate::{DocumentUri, ParameterError, ParameterErrorKind};

pub const SEMANTIC_TOKEN_TYPES: &[&str] = &[
    "namespace",
    "type",
    "struct",
    "enum",
    "interface",
    "typeParameter",
    "parameter",
    "variable",
    "property",
    "enumMember",
    "function",
    "method",
    "keyword",
    "string",
];

pub const SEMANTIC_TOKEN_MODIFIERS: &[&str] = &["declaration", "readonly"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticTokenType {
    Namespace,
    Type,
    Struct,
    Enum,
    Interface,
    TypeParameter,
    Parameter,
    Variable,
    Property,
    EnumMember,
    Function,
    Method,
    Keyword,
    String,
}

impl SemanticTokenType {
    const fn index(self) -> u32 {
        match self {
            Self::Namespace => 0,
            Self::Type => 1,
            Self::Struct => 2,
            Self::Enum => 3,
            Self::Interface => 4,
            Self::TypeParameter => 5,
            Self::Parameter => 6,
            Self::Variable => 7,
            Self::Property => 8,
            Self::EnumMember => 9,
            Self::Function => 10,
            Self::Method => 11,
            Self::Keyword => 12,
            Self::String => 13,
        }
    }
}

/// One absolute single-line token before protocol delta encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticToken {
    line: u32,
    start: u32,
    length: u32,
    token_type: SemanticTokenType,
    declaration: bool,
    readonly: bool,
}

impl SemanticToken {
    #[must_use]
    pub const fn new(
        line: u32,
        start: u32,
        length: u32,
        token_type: SemanticTokenType,
        declaration: bool,
        readonly: bool,
    ) -> Self {
        Self {
            line,
            start,
            length,
            token_type,
            declaration,
            readonly,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticTokensParams {
    uri: DocumentUri,
}

impl SemanticTokensParams {
    /// Decodes one full-document semantic-token request.
    ///
    /// # Errors
    ///
    /// Returns the exact missing, duplicate, or incorrectly typed field.
    pub fn decode(params: Option<Value>) -> Result<Self, ParameterError> {
        let mut root = Object::new(required(params, "params")?, "params")?;
        let mut document = Object::new(root.take("textDocument")?, "params.textDocument")?;
        let uri = DocumentUri::new(string(document.take("uri")?, "params.textDocument.uri")?)
            .map_err(|_| {
                ParameterError::new(ParameterErrorKind::EmptyUri, "params.textDocument.uri")
            })?;
        Ok(Self { uri })
    }

    #[must_use]
    pub const fn uri(&self) -> &DocumentUri {
        &self.uri
    }
}

/// Delta-encodes one sorted, non-overlapping semantic-token sequence and its immutable generation
/// identity.
///
/// # Errors
///
/// Returns an error for empty, out-of-order, or overlapping tokens.
pub fn semantic_tokens_result(
    result_id: &str,
    tokens: &[SemanticToken],
) -> Result<Value, SemanticTokenEncodingError> {
    let mut data = Vec::with_capacity(tokens.len() * 5);
    let mut previous: Option<SemanticToken> = None;
    for token in tokens {
        if token.length == 0 {
            return Err(SemanticTokenEncodingError::Empty);
        }
        if let Some(previous) = previous
            && (token.line < previous.line
                || token.line == previous.line
                    && token.start < previous.start.saturating_add(previous.length))
        {
            return Err(SemanticTokenEncodingError::OutOfOrderOrOverlapping);
        }
        let delta_line = previous.map_or(token.line, |previous| token.line - previous.line);
        let delta_start = match previous {
            Some(previous) if previous.line == token.line => token.start - previous.start,
            _ => token.start,
        };
        data.extend([
            delta_line,
            delta_start,
            token.length,
            token.token_type.index(),
            u32::from(token.declaration) | (u32::from(token.readonly) << 1),
        ]);
        previous = Some(*token);
    }
    Ok(object([
        ("resultId", Value::String(result_id.into())),
        (
            "data",
            Value::Array(
                data.into_iter()
                    .map(|value| Value::Number(value.to_string().into()))
                    .collect(),
            ),
        ),
    ]))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticTokenEncodingError {
    Empty,
    OutOfOrderOrOverlapping,
}

impl fmt::Display for SemanticTokenEncodingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "semantic token has an empty range",
            Self::OutOfOrderOrOverlapping => {
                "semantic tokens are out of order or have overlapping ranges"
            }
        })
    }
}

impl std::error::Error for SemanticTokenEncodingError {}

fn object<const N: usize>(members: [(&str, Value); N]) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(name, value)| Member {
                name: name.into(),
                value,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use nocter_json::{parse, write_value};

    use super::*;

    #[test]
    fn decodes_document_identity() {
        let params = SemanticTokensParams::decode(Some(
            parse(r#"{"textDocument":{"uri":"file:///workspace/main.nct"}}"#).unwrap(),
        ))
        .unwrap();
        assert_eq!(params.uri().as_str(), "file:///workspace/main.nct");
    }

    #[test]
    fn delta_encodes_sorted_tokens_and_modifiers() {
        let tokens = [
            SemanticToken::new(2, 4, 3, SemanticTokenType::Struct, true, false),
            SemanticToken::new(2, 10, 1, SemanticTokenType::Variable, false, true),
            SemanticToken::new(4, 1, 5, SemanticTokenType::Function, false, false),
        ];
        let mut rendered = String::new();
        write_value(
            &mut rendered,
            &semantic_tokens_result("17", &tokens).unwrap(),
        );
        assert_eq!(
            rendered,
            r#"{"resultId":"17","data":[2,4,3,2,1,0,6,1,7,2,2,1,5,10,0]}"#
        );
    }

    #[test]
    fn rejects_overlapping_tokens() {
        let tokens = [
            SemanticToken::new(0, 2, 4, SemanticTokenType::Type, false, false),
            SemanticToken::new(0, 5, 2, SemanticTokenType::Type, false, false),
        ];
        assert_eq!(
            semantic_tokens_result("1", &tokens),
            Err(SemanticTokenEncodingError::OutOfOrderOrOverlapping)
        );
    }
}
