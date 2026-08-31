use nocter_json::{Member, Value};

use crate::text_edit::text_edit_value;
use crate::{TextDocumentPositionParams, TextEdit};

pub type CompletionParams = TextDocumentPositionParams;

/// One protocol-level completion item after compiler classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompletionItem<'a> {
    label: &'a str,
    kind: CompletionItemKind,
    detail: Option<&'a str>,
    additional_text_edits: &'a [TextEdit],
}

impl<'a> CompletionItem<'a> {
    #[must_use]
    pub const fn new(label: &'a str, kind: CompletionItemKind, detail: Option<&'a str>) -> Self {
        Self {
            label,
            kind,
            detail,
            additional_text_edits: &[],
        }
    }

    #[must_use]
    pub const fn with_additional_text_edits(mut self, edits: &'a [TextEdit]) -> Self {
        self.additional_text_edits = edits;
        self
    }
}

/// Stable LSP `CompletionItemKind` values used by Nocter semantic completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CompletionItemKind {
    Method = 2,
    Function = 3,
    Constructor = 4,
    Field = 5,
    Variable = 6,
    Class = 7,
    Interface = 8,
    Module = 9,
    Enum = 13,
    Keyword = 14,
    EnumMember = 20,
    Constant = 21,
    Struct = 22,
    TypeParameter = 25,
}

/// Renders a complete, deterministically ordered completion array.
#[must_use]
pub fn completion_result(items: &[CompletionItem<'_>]) -> Value {
    Value::Array(items.iter().map(completion_item_value).collect())
}

/// Renders a completion list whose semantic coverage is incomplete.
#[must_use]
pub fn incomplete_completion_result(items: &[CompletionItem<'_>]) -> Value {
    Value::Object(vec![
        Member {
            name: "isIncomplete".into(),
            value: Value::Bool(true),
        },
        Member {
            name: "items".into(),
            value: Value::Array(items.iter().map(completion_item_value).collect()),
        },
    ])
}

fn completion_item_value(item: &CompletionItem<'_>) -> Value {
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
    if !item.additional_text_edits.is_empty() {
        members.push(Member {
            name: "additionalTextEdits".into(),
            value: Value::Array(
                item.additional_text_edits
                    .iter()
                    .map(text_edit_value)
                    .collect(),
            ),
        });
    }
    Value::Object(members)
}

#[cfg(test)]
mod tests {
    use nocter_json::write_value;

    use super::{
        CompletionItem, CompletionItemKind, completion_result, incomplete_completion_result,
    };
    use crate::{Position, Range, TextEdit};

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

    #[test]
    fn renders_compiler_supplied_additional_text_edits() {
        let edit = TextEdit::new(
            Range::new(Position::new(0, 0), Position::new(0, 0)),
            "use std/io\n\n",
        );
        let item = CompletionItem::new("io.print", CompletionItemKind::Function, None)
            .with_additional_text_edits(std::slice::from_ref(&edit));
        let mut rendered = String::new();
        write_value(&mut rendered, &completion_result(&[item]));
        assert_eq!(
            rendered,
            "[{\"label\":\"io.print\",\"kind\":3,\"additionalTextEdits\":[{\"range\":{\"start\":{\"line\":0,\"character\":0},\"end\":{\"line\":0,\"character\":0}},\"newText\":\"use std/io\\n\\n\"}]}]"
        );
    }

    #[test]
    fn incomplete_semantic_coverage_is_explicit_in_the_protocol_result() {
        let mut rendered = String::new();
        write_value(
            &mut rendered,
            &incomplete_completion_result(&[CompletionItem::new(
                "value",
                CompletionItemKind::Variable,
                None,
            )]),
        );
        assert_eq!(
            rendered,
            "{\"isIncomplete\":true,\"items\":[{\"label\":\"value\",\"kind\":6}]}"
        );
    }
}
