use nocter_json::{Member, Value};

use crate::coordinates::range_value;
use crate::{Range, TextDocumentPositionParams};

pub type HoverParams = TextDocumentPositionParams;

/// Builds a hover result whose contents and range came from one semantic snapshot.
#[must_use]
pub fn hover_result(code: &str, documentation: Option<&str>, range: Range) -> Value {
    let mut markdown = format!("```nocter\n{code}\n```");
    if let Some(documentation) = documentation {
        markdown.push_str("\n\n");
        markdown.push_str(documentation);
    }
    object([
        (
            "contents",
            object([
                ("kind", Value::String("markdown".into())),
                ("value", Value::String(markdown.into_boxed_str())),
            ]),
        ),
        ("range", range_value(range)),
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
    use crate::{ParameterErrorKind, Position};

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
                Some("A growable sequence."),
                Range::new(Position::new(1, 11), Position::new(1, 14)),
            ),
        );
        assert!(rendered.contains("```nocter\\npub struct Vec<T>\\n```"));
        assert!(rendered.contains("A growable sequence."));
        assert!(rendered.contains("\"start\":{\"line\":1,\"character\":11}"));
    }
}
