use nocter_json::{Member, Value};

use crate::decode::{Object, required, string, unsigned};
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

/// Shared validated identity and position for semantic text-document requests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextDocumentPositionParams {
    uri: DocumentUri,
    position: Position,
}

impl TextDocumentPositionParams {
    /// Decodes one positioned document request without accepting negative or fractional values.
    ///
    /// # Errors
    ///
    /// Returns the exact missing, duplicate, or incorrectly typed field.
    pub fn decode(params: Option<Value>) -> Result<Self, ParameterError> {
        let mut root = Object::new(required(params, "params")?, "params")?;
        Self::decode_from(&mut root)
    }

    pub(crate) fn decode_from(root: &mut Object) -> Result<Self, ParameterError> {
        let mut document = Object::new(root.take("textDocument")?, "params.textDocument")?;
        let uri = DocumentUri::new(string(document.take("uri")?, "params.textDocument.uri")?)
            .map_err(|_| {
                ParameterError::new(ParameterErrorKind::EmptyUri, "params.textDocument.uri")
            })?;
        let mut position = Object::new(root.take("position")?, "params.position")?;
        let line = unsigned(position.take("line")?, "params.position.line")?;
        let character = unsigned(position.take("character")?, "params.position.character")?;
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

pub(crate) fn range_value(range: Range) -> Value {
    object([
        ("start", position_value(range.start())),
        ("end", position_value(range.end())),
    ])
}

fn position_value(position: Position) -> Value {
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
