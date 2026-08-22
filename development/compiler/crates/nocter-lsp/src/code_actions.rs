use nocter_json::{Member, Value};

use crate::coordinates::decode_position;
use crate::decode::{Object, array, required, string};
use crate::{DocumentUri, ParameterError, ParameterErrorKind, Range};

/// Validated range and document identity for `textDocument/codeAction`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeActionParams {
    uri: DocumentUri,
    range: Range,
}

impl CodeActionParams {
    /// Decodes the request envelope while leaving diagnostic authority with compiler analysis.
    ///
    /// # Errors
    ///
    /// Returns the exact missing, duplicate, or incorrectly typed protocol field.
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
        let mut context = Object::new(root.take("context")?, "params.context")?;
        // Clients report diagnostics so they can filter UI, but stale or fabricated diagnostic
        // contents must never select a compiler repair. Validate only the required container.
        array(context.take("diagnostics")?, "params.context.diagnostics")?;
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

/// One protocol code action whose title and edit were already selected and validated upstream.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeAction<'a> {
    title: &'a str,
    edit: &'a Value,
    preferred: bool,
}

impl<'a> CodeAction<'a> {
    #[must_use]
    pub const fn new(title: &'a str, edit: &'a Value, preferred: bool) -> Self {
        Self {
            title,
            edit,
            preferred,
        }
    }
}

/// Renders a deterministic array of eager quick fixes.
#[must_use]
pub fn code_actions_result(actions: &[CodeAction<'_>]) -> Value {
    Value::Array(
        actions
            .iter()
            .map(|action| {
                Value::Object(vec![
                    Member {
                        name: "title".into(),
                        value: Value::String(action.title.into()),
                    },
                    Member {
                        name: "kind".into(),
                        value: Value::String("quickfix".into()),
                    },
                    Member {
                        name: "isPreferred".into(),
                        value: Value::Bool(action.preferred),
                    },
                    Member {
                        name: "edit".into(),
                        value: action.edit.clone(),
                    },
                ])
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use nocter_json::{parse, write_value};

    use super::*;

    #[test]
    fn decodes_range_and_requires_a_diagnostic_context() {
        let params = CodeActionParams::decode(Some(
            parse(concat!(
                "{\"textDocument\":{\"uri\":\"file:///workspace/main.nct\"},",
                "\"range\":{\"start\":{\"line\":1,\"character\":2},",
                "\"end\":{\"line\":1,\"character\":7}},",
                "\"context\":{\"diagnostics\":[]}}"
            ))
            .unwrap(),
        ))
        .unwrap();
        assert_eq!(params.range().start().line(), 1);
        assert_eq!(params.range().end().character(), 7);
    }

    #[test]
    fn renders_only_validated_quick_fix_fields() {
        let edit = parse("{\"documentChanges\":[]}").unwrap();
        let actions = [CodeAction::new(
            "Import `print` from `std/io.print`",
            &edit,
            true,
        )];
        let mut rendered = String::new();
        write_value(&mut rendered, &code_actions_result(&actions));
        assert_eq!(
            rendered,
            concat!(
                "[{\"title\":\"Import `print` from `std/io.print`\",",
                "\"kind\":\"quickfix\",\"isPreferred\":true,",
                "\"edit\":{\"documentChanges\":[]}}]"
            )
        );
    }
}
