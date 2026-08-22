use nocter_json::{Member, Value};

use crate::decode::{Object, required, string};
use crate::text_edit::text_edit_value;
use crate::{DocumentUri, ParameterError, TextDocumentPositionParams, TextEdit};

/// Validated `textDocument/rename` parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenameParams {
    document: TextDocumentPositionParams,
    new_name: Box<str>,
}

impl RenameParams {
    /// Decodes the shared document position and required replacement spelling.
    ///
    /// Lexical validity belongs to the language analysis boundary, not this protocol decoder.
    ///
    /// # Errors
    ///
    /// Returns the exact missing, duplicate, or incorrectly typed field.
    pub fn decode(params: Option<Value>) -> Result<Self, ParameterError> {
        let mut root = Object::new(required(params, "params")?, "params")?;
        let document = TextDocumentPositionParams::decode_from(&mut root)?;
        let new_name = string(root.take("newName")?, "params.newName")?;
        Ok(Self { document, new_name })
    }

    #[must_use]
    pub const fn uri(&self) -> &DocumentUri {
        self.document.uri()
    }

    #[must_use]
    pub const fn position(&self) -> crate::Position {
        self.document.position()
    }

    #[must_use]
    pub const fn new_name(&self) -> &str {
        &self.new_name
    }
}

/// All edits for one document, carrying its accepted version when it is open.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentEdit {
    uri: DocumentUri,
    version: Option<i32>,
    edits: Box<[TextEdit]>,
}

impl DocumentEdit {
    #[must_use]
    pub fn new(uri: DocumentUri, version: Option<i32>, edits: impl Into<Box<[TextEdit]>>) -> Self {
        Self {
            uri,
            version,
            edits: edits.into(),
        }
    }
}

/// Renders one atomic workspace edit using versioned document changes.
#[must_use]
pub fn workspace_edit_result(documents: &[DocumentEdit]) -> Value {
    object([(
        "documentChanges",
        Value::Array(documents.iter().map(document_edit_value).collect()),
    )])
}

fn document_edit_value(document: &DocumentEdit) -> Value {
    object([
        (
            "textDocument",
            object([
                ("uri", Value::String(document.uri.as_str().into())),
                (
                    "version",
                    document.version.map_or(Value::Null, |version| {
                        Value::Number(version.to_string().into())
                    }),
                ),
            ]),
        ),
        (
            "edits",
            Value::Array(document.edits.iter().map(text_edit_value).collect()),
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
    use crate::{Position, Range};

    #[test]
    fn decodes_rename_without_claiming_language_name_validity() {
        let params = RenameParams::decode(Some(
            parse(concat!(
                "{\"textDocument\":{\"uri\":\"file:///workspace/main.nct\"},",
                "\"position\":{\"line\":2,\"character\":7},\"newName\":\"new value\"}"
            ))
            .unwrap(),
        ))
        .unwrap();
        assert_eq!(params.position(), Position::new(2, 7));
        assert_eq!(params.new_name(), "new value");
    }

    #[test]
    fn renders_open_and_closed_documents_in_one_atomic_edit() {
        let edit = TextEdit::new(
            Range::new(Position::new(1, 2), Position::new(1, 5)),
            "after",
        );
        let result = workspace_edit_result(&[
            DocumentEdit::new(
                DocumentUri::new("file:///workspace/open.nct").unwrap(),
                Some(4),
                [edit.clone()],
            ),
            DocumentEdit::new(
                DocumentUri::new("file:///workspace/closed.nct").unwrap(),
                None,
                [edit],
            ),
        ]);
        let mut rendered = String::new();
        write_value(&mut rendered, &result);
        assert!(rendered.contains("\"version\":4"));
        assert!(rendered.contains("\"version\":null"));
        assert!(rendered.contains("\"newText\":\"after\""));
    }
}
