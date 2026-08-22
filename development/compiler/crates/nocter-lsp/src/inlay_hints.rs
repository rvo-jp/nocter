use nocter_json::{Member, Value};

use crate::coordinates::{decode_position, position_value};
use crate::decode::{Object, required, string};
use crate::{DocumentUri, ParameterError, ParameterErrorKind, Position, Range};

/// Validated full-range request for compiler-owned inlay facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InlayHintParams {
    uri: DocumentUri,
    range: Range,
}

impl InlayHintParams {
    /// Decodes one inlay-hint request without interpreting its source contents.
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
        let mut range = Object::new(root.take("range")?, "params.range")?;
        let start = decode_position(range.take("start")?, "params.range.start")?;
        let end = decode_position(range.take("end")?, "params.range.end")?;
        Ok(Self {
            uri,
            range: Range::new(start, end),
        })
    }

    #[must_use]
    pub const fn uri(&self) -> &DocumentUri {
        &self.uri
    }

    #[must_use]
    pub const fn range(&self) -> Range {
        self.range
    }
}

/// One protocol inlay hint projected from a compiler-owned source fact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InlayHint<'a> {
    position: Position,
    label: &'a str,
    kind: Option<InlayHintKind>,
}

impl<'a> InlayHint<'a> {
    #[must_use]
    pub const fn new(position: Position, label: &'a str, kind: Option<InlayHintKind>) -> Self {
        Self {
            position,
            label,
            kind,
        }
    }
}

/// Stable LSP `InlayHintKind` values used by Nocter analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InlayHintKind {
    Type,
}

impl InlayHintKind {
    const fn protocol_value(self) -> u32 {
        match self {
            Self::Type => 1,
        }
    }
}

/// Renders compiler-ordered inlay hints without protocol-side inference.
#[must_use]
pub fn inlay_hints_result(hints: &[InlayHint<'_>]) -> Value {
    Value::Array(
        hints
            .iter()
            .map(|hint| {
                let mut members = vec![
                    Member {
                        name: "position".into(),
                        value: position_value(hint.position),
                    },
                    Member {
                        name: "label".into(),
                        value: Value::String(hint.label.into()),
                    },
                ];
                if let Some(kind) = hint.kind {
                    members.push(Member {
                        name: "kind".into(),
                        value: Value::Number(kind.protocol_value().to_string().into()),
                    });
                }
                Value::Object(members)
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use nocter_json::{parse, write_value};

    use super::*;

    #[test]
    fn decodes_the_document_and_requested_range() {
        let params = InlayHintParams::decode(Some(
            parse(concat!(
                "{\"textDocument\":{\"uri\":\"file:///workspace/main.nct\"},",
                "\"range\":{\"start\":{\"line\":1,\"character\":2},",
                "\"end\":{\"line\":3,\"character\":4}}}"
            ))
            .unwrap(),
        ))
        .unwrap();
        assert_eq!(params.range().start(), Position::new(1, 2));
        assert_eq!(params.range().end(), Position::new(3, 4));
    }

    #[test]
    fn renders_compiler_ordered_type_hints() {
        let hints = [
            InlayHint::new(Position::new(2, 9), ": i32", Some(InlayHintKind::Type)),
            InlayHint::new(Position::new(4, 27), " from text", None),
        ];
        let mut rendered = String::new();
        write_value(&mut rendered, &inlay_hints_result(&hints));
        assert_eq!(
            rendered,
            concat!(
                "[{\"position\":{\"line\":2,\"character\":9},",
                "\"label\":\": i32\",\"kind\":1},",
                "{\"position\":{\"line\":4,\"character\":27},",
                "\"label\":\" from text\"}]"
            )
        );
    }
}
