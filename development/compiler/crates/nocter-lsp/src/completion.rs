use nocter_json::{Member, Value};

use crate::TextDocumentPositionParams;

pub type CompletionParams = TextDocumentPositionParams;

/// One protocol-level completion item after compiler classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompletionItem<'a> {
    label: &'a str,
    kind: CompletionItemKind,
    detail: Option<&'a str>,
}

impl<'a> CompletionItem<'a> {
    #[must_use]
    pub const fn new(label: &'a str, kind: CompletionItemKind, detail: Option<&'a str>) -> Self {
        Self {
            label,
            kind,
            detail,
        }
    }
}

/// Stable LSP `CompletionItemKind` values used by Nocter semantic completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CompletionItemKind {
    Function = 3,
    Variable = 6,
    Class = 7,
    Interface = 8,
    Module = 9,
    Enum = 13,
    Struct = 22,
    TypeParameter = 25,
}

/// Renders a complete, deterministically ordered completion array.
#[must_use]
pub fn completion_result(items: &[CompletionItem<'_>]) -> Value {
    Value::Array(
        items
            .iter()
            .map(|item| {
                let mut members = vec![
                    Member {
                        name: "label".into(),
                        value: Value::String(item.label.into()),
                    },
                    Member {
                        name: "kind".into(),
                        value: Value::Number((item.kind as u8).to_string().into()),
                    },
                ];
                if let Some(detail) = item.detail {
                    members.push(Member {
                        name: "detail".into(),
                        value: Value::String(detail.into()),
                    });
                }
                Value::Object(members)
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use nocter_json::write_value;

    use super::{CompletionItem, CompletionItemKind, completion_result};

    #[test]
    fn renders_compiler_ordered_items_without_protocol_side_inference() {
        let mut rendered = String::new();
        write_value(
            &mut rendered,
            &completion_result(&[
                CompletionItem::new(
                    "read",
                    CompletionItemKind::Function,
                    Some("func read(): i32"),
                ),
                CompletionItem::new("value", CompletionItemKind::Variable, None),
            ]),
        );
        assert_eq!(
            rendered,
            "[{\"label\":\"read\",\"kind\":3,\"detail\":\"func read(): i32\"},{\"label\":\"value\",\"kind\":6}]"
        );
    }
}
