use nocter_json::{Member, Value};

use crate::decode::{Object, integer, required, string};
use crate::{DocumentUri, ParameterError, ParameterErrorKind};

/// One zero-based editor position measured in the negotiated UTF-16 encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Position {
    line: u32,
    character: u32,
}

impl Position {
    #[must_use]
    pub const fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }

    #[must_use]
    pub const fn line(self) -> u32 {
        self.line
    }

    #[must_use]
    pub const fn character(self) -> u32 {
        self.character
    }
}

/// One half-open editor range measured in the negotiated UTF-16 encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Range {
    start: Position,
    end: Position,
}

impl Range {
    #[must_use]
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub const fn start(self) -> Position {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> Position {
        self.end
    }
}

/// Validated `textDocument/hover` request parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HoverParams {
    uri: DocumentUri,
    position: Position,
}

impl HoverParams {
    /// Decodes one hover request without accepting negative or non-integral positions.
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
        let mut position = Object::new(root.take("position")?, "params.position")?;
        let line = nonnegative(position.take("line")?, "params.position.line")?;
        let character = nonnegative(position.take("character")?, "params.position.character")?;
        Ok(Self {
            uri,
            position: Position::new(line, character),
        })
    }

    #[must_use]
    pub const fn uri(&self) -> &DocumentUri {
        &self.uri
    }

    #[must_use]
    pub const fn position(&self) -> Position {
        self.position
    }
}

/// Builds a hover result whose contents and range came from one semantic snapshot.
#[must_use]
pub fn hover_result(code: &str, range: Range) -> Value {
    object([
        (
            "contents",
            object([
                ("kind", Value::String("markdown".into())),
                (
                    "value",
                    Value::String(format!("```nocter\n{code}\n```").into_boxed_str()),
                ),
            ]),
        ),
        (
            "range",
            object([
                ("start", position(range.start())),
                ("end", position(range.end())),
            ]),
        ),
    ])
}

fn nonnegative(value: Value, path: &str) -> Result<u32, ParameterError> {
    let value = integer(value, path)?;
    u32::try_from(value)
        .map_err(|_| ParameterError::new(ParameterErrorKind::ExpectedNonnegativeInteger, path))
}

fn position(position: Position) -> Value {
    object([
        ("line", Value::Number(position.line().to_string().into())),
        (
            "character",
            Value::Number(position.character().to_string().into()),
        ),
    ])
}

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
    fn decodes_nonnegative_utf16_positions() {
        let params = HoverParams::decode(Some(
            parse(
                r#"{"textDocument":{"uri":"file:///workspace/main.nct"},"position":{"line":2,"character":7}}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        assert_eq!(params.position(), Position::new(2, 7));
    }

    #[test]
    fn rejects_negative_positions() {
        let error = HoverParams::decode(Some(
            parse(
                r#"{"textDocument":{"uri":"file:///workspace/main.nct"},"position":{"line":-1,"character":0}}"#,
            )
            .unwrap(),
        ))
        .unwrap_err();
        assert_eq!(error.kind(), ParameterErrorKind::ExpectedNonnegativeInteger);
        assert_eq!(error.path(), "params.position.line");
    }

    #[test]
    fn renders_semantic_code_and_its_exact_range() {
        let mut rendered = String::new();
        write_value(
            &mut rendered,
            &hover_result(
                "pub struct Vec<T>",
                Range::new(Position::new(1, 11), Position::new(1, 14)),
            ),
        );
        assert!(rendered.contains("```nocter\\npub struct Vec<T>\\n```"));
        assert!(rendered.contains("\"start\":{\"line\":1,\"character\":11}"));
    }
}
