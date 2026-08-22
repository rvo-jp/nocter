use nocter_json::{Member, Value};

use crate::Range;
use crate::coordinates::range_value;

/// One protocol text replacement shared by completion and workspace edits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextEdit {
    range: Range,
    new_text: Box<str>,
}

impl TextEdit {
    #[must_use]
    pub fn new(range: Range, new_text: impl Into<Box<str>>) -> Self {
        Self {
            range,
            new_text: new_text.into(),
        }
    }
}

pub(crate) fn text_edit_value(edit: &TextEdit) -> Value {
    Value::Object(vec![
        Member {
            name: "range".into(),
            value: range_value(edit.range),
        },
        Member {
            name: "newText".into(),
            value: Value::String(edit.new_text.clone()),
        },
    ])
}
