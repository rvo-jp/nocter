use std::fmt;

use nocter_json::{Member, Value};

use crate::DocumentUri;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DidOpenParams {
    uri: DocumentUri,
    version: i32,
    text: Box<str>,
}

impl DidOpenParams {
    /// Decodes one full `textDocument/didOpen` parameter object.
    ///
    /// # Errors
    ///
    /// Returns the exact missing, duplicate, or incorrectly typed field.
    pub fn decode(params: Option<Value>) -> Result<Self, ParameterError> {
        let mut root = Object::new(required(params, "params")?, "params")?;
        let mut document = Object::new(root.take("textDocument")?, "params.textDocument")?;
        let uri = uri(document.take("uri")?, "params.textDocument.uri")?;
        let version = integer(document.take("version")?, "params.textDocument.version")?;
        string(
            document.take("languageId")?,
            "params.textDocument.languageId",
        )?;
        let text = string(document.take("text")?, "params.textDocument.text")?;
        Ok(Self { uri, version, text })
    }

    #[must_use]
    pub const fn uri(&self) -> &DocumentUri {
        &self.uri
    }

    #[must_use]
    pub const fn version(&self) -> i32 {
        self.version
    }

    #[must_use]
    pub const fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DidChangeParams {
    uri: DocumentUri,
    version: i32,
    text: Box<str>,
}

impl DidChangeParams {
    /// Decodes one full-document `textDocument/didChange` parameter object.
    ///
    /// # Errors
    ///
    /// Returns the exact structural failure, including incremental or multiple content changes.
    pub fn decode(params: Option<Value>) -> Result<Self, ParameterError> {
        let mut root = Object::new(required(params, "params")?, "params")?;
        let mut document = Object::new(root.take("textDocument")?, "params.textDocument")?;
        let uri = uri(document.take("uri")?, "params.textDocument.uri")?;
        let version = integer(document.take("version")?, "params.textDocument.version")?;
        let mut changes = array(root.take("contentChanges")?, "params.contentChanges")?;
        if changes.len() != 1 {
            return Err(ParameterError::new(
                ParameterErrorKind::ExpectedOneFullChange,
                "params.contentChanges",
            ));
        }
        let mut change = Object::new(
            changes.pop().ok_or_else(|| {
                ParameterError::new(
                    ParameterErrorKind::ExpectedOneFullChange,
                    "params.contentChanges",
                )
            })?,
            "params.contentChanges[0]",
        )?;
        if change.contains("range") || change.contains("rangeLength") {
            return Err(ParameterError::new(
                ParameterErrorKind::IncrementalChangeUnsupported,
                "params.contentChanges[0]",
            ));
        }
        let text = string(change.take("text")?, "params.contentChanges[0].text")?;
        Ok(Self { uri, version, text })
    }

    #[must_use]
    pub const fn uri(&self) -> &DocumentUri {
        &self.uri
    }

    #[must_use]
    pub const fn version(&self) -> i32 {
        self.version
    }

    #[must_use]
    pub const fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DidSaveParams {
    uri: DocumentUri,
    text: Option<Box<str>>,
}

impl DidSaveParams {
    /// Decodes one `textDocument/didSave` parameter object with optional included text.
    ///
    /// # Errors
    ///
    /// Returns the exact missing, duplicate, or incorrectly typed field.
    pub fn decode(params: Option<Value>) -> Result<Self, ParameterError> {
        let mut root = Object::new(required(params, "params")?, "params")?;
        let mut document = Object::new(root.take("textDocument")?, "params.textDocument")?;
        let uri = uri(document.take("uri")?, "params.textDocument.uri")?;
        let text = match root.take_optional("text")? {
            Some(Value::String(text)) => Some(text),
            Some(Value::Null) | None => None,
            Some(_) => {
                return Err(ParameterError::new(
                    ParameterErrorKind::ExpectedString,
                    "params.text",
                ));
            }
        };
        Ok(Self { uri, text })
    }

    #[must_use]
    pub const fn uri(&self) -> &DocumentUri {
        &self.uri
    }

    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DidCloseParams {
    uri: DocumentUri,
}

impl DidCloseParams {
    /// Decodes one `textDocument/didClose` parameter object.
    ///
    /// # Errors
    ///
    /// Returns the exact missing, duplicate, or incorrectly typed field.
    pub fn decode(params: Option<Value>) -> Result<Self, ParameterError> {
        let mut root = Object::new(required(params, "params")?, "params")?;
        let mut document = Object::new(root.take("textDocument")?, "params.textDocument")?;
        Ok(Self {
            uri: uri(document.take("uri")?, "params.textDocument.uri")?,
        })
    }

    #[must_use]
    pub const fn uri(&self) -> &DocumentUri {
        &self.uri
    }
}

fn required(value: Option<Value>, path: &str) -> Result<Value, ParameterError> {
    value.ok_or_else(|| ParameterError::new(ParameterErrorKind::MissingField, path))
}

fn uri(value: Value, path: &str) -> Result<DocumentUri, ParameterError> {
    let value = string(value, path)?;
    DocumentUri::new(value).map_err(|_| ParameterError::new(ParameterErrorKind::EmptyUri, path))
}

fn string(value: Value, path: &str) -> Result<Box<str>, ParameterError> {
    match value {
        Value::String(value) => Ok(value),
        _ => Err(ParameterError::new(
            ParameterErrorKind::ExpectedString,
            path,
        )),
    }
}

fn integer(value: Value, path: &str) -> Result<i32, ParameterError> {
    match value {
        Value::Number(value) => value
            .parse()
            .map_err(|_| ParameterError::new(ParameterErrorKind::ExpectedInteger, path)),
        _ => Err(ParameterError::new(
            ParameterErrorKind::ExpectedInteger,
            path,
        )),
    }
}

fn array(value: Value, path: &str) -> Result<Vec<Value>, ParameterError> {
    match value {
        Value::Array(value) => Ok(value),
        _ => Err(ParameterError::new(ParameterErrorKind::ExpectedArray, path)),
    }
}

struct Object {
    members: Vec<Member>,
    path: Box<str>,
}

impl Object {
    fn new(value: Value, path: impl Into<Box<str>>) -> Result<Self, ParameterError> {
        let path = path.into();
        match value {
            Value::Object(members) => Ok(Self { members, path }),
            _ => Err(ParameterError::new(
                ParameterErrorKind::ExpectedObject,
                path,
            )),
        }
    }

    fn contains(&self, name: &str) -> bool {
        self.members
            .iter()
            .any(|member| member.name.as_ref() == name)
    }

    fn take(&mut self, name: &str) -> Result<Value, ParameterError> {
        self.take_optional(name)?.ok_or_else(|| {
            ParameterError::new(ParameterErrorKind::MissingField, self.field_path(name))
        })
    }

    fn take_optional(&mut self, name: &str) -> Result<Option<Value>, ParameterError> {
        let Some(index) = self
            .members
            .iter()
            .position(|member| member.name.as_ref() == name)
        else {
            return Ok(None);
        };
        if self.members[index + 1..]
            .iter()
            .any(|member| member.name.as_ref() == name)
        {
            return Err(ParameterError::new(
                ParameterErrorKind::DuplicateField,
                self.field_path(name),
            ));
        }
        Ok(Some(self.members.remove(index).value))
    }

    fn field_path(&self, name: &str) -> Box<str> {
        format!("{}.{name}", self.path).into_boxed_str()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterErrorKind {
    MissingField,
    DuplicateField,
    ExpectedObject,
    ExpectedArray,
    ExpectedString,
    ExpectedInteger,
    EmptyUri,
    ExpectedOneFullChange,
    IncrementalChangeUnsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterError {
    kind: ParameterErrorKind,
    path: Box<str>,
}

impl ParameterError {
    fn new(kind: ParameterErrorKind, path: impl Into<Box<str>>) -> Self {
        Self {
            kind,
            path: path.into(),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> ParameterErrorKind {
        self.kind
    }

    #[must_use]
    pub const fn path(&self) -> &str {
        &self.path
    }
}

impl fmt::Display for ParameterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid {} at {}",
            self.kind.description(),
            self.path
        )
    }
}

impl ParameterErrorKind {
    const fn description(self) -> &'static str {
        match self {
            Self::MissingField => "missing field",
            Self::DuplicateField => "duplicate field",
            Self::ExpectedObject => "object",
            Self::ExpectedArray => "array",
            Self::ExpectedString => "string",
            Self::ExpectedInteger => "32-bit integer",
            Self::EmptyUri => "empty document URI",
            Self::ExpectedOneFullChange => "full-document change count",
            Self::IncrementalChangeUnsupported => "incremental document change",
        }
    }
}

impl std::error::Error for ParameterError {}

#[cfg(test)]
mod tests {
    use nocter_json::parse;

    use super::*;

    fn value(source: &str) -> Option<Value> {
        parse(source).ok()
    }

    #[test]
    fn decodes_full_document_open_change_save_and_close() {
        let open = DidOpenParams::decode(value(
            r#"{"textDocument":{"uri":"file:///tmp/app.nct","languageId":"nocter","version":1,"text":"first"}}"#,
        ))
        .unwrap();
        assert_eq!(open.uri().as_str(), "file:///tmp/app.nct");
        assert_eq!(open.version(), 1);
        assert_eq!(open.text(), "first");

        let change = DidChangeParams::decode(value(
            r#"{"textDocument":{"uri":"file:///tmp/app.nct","version":2},"contentChanges":[{"text":"second"}]}"#,
        ))
        .unwrap();
        assert_eq!(change.version(), 2);
        assert_eq!(change.text(), "second");

        let save = DidSaveParams::decode(value(
            r#"{"textDocument":{"uri":"file:///tmp/app.nct"},"text":"saved"}"#,
        ))
        .unwrap();
        assert_eq!(save.text(), Some("saved"));

        let close =
            DidCloseParams::decode(value(r#"{"textDocument":{"uri":"file:///tmp/app.nct"}}"#))
                .unwrap();
        assert_eq!(close.uri().as_str(), "file:///tmp/app.nct");
    }

    #[test]
    fn rejects_incremental_or_ambiguous_changes() {
        let incremental = DidChangeParams::decode(value(
            r#"{"textDocument":{"uri":"file:///tmp/app.nct","version":2},"contentChanges":[{"range":{"start":{},"end":{}},"text":"x"}]}"#,
        ))
        .unwrap_err();
        assert_eq!(
            incremental.kind(),
            ParameterErrorKind::IncrementalChangeUnsupported
        );

        let multiple = DidChangeParams::decode(value(
            r#"{"textDocument":{"uri":"file:///tmp/app.nct","version":2},"contentChanges":[{"text":"a"},{"text":"b"}]}"#,
        ))
        .unwrap_err();
        assert_eq!(multiple.kind(), ParameterErrorKind::ExpectedOneFullChange);

        let duplicate = DidOpenParams::decode(value(
            r#"{"textDocument":{"uri":"file:///a","uri":"file:///b","languageId":"nocter","version":1,"text":""}}"#,
        ))
        .unwrap_err();
        assert_eq!(duplicate.kind(), ParameterErrorKind::DuplicateField);
        assert_eq!(duplicate.path(), "params.textDocument.uri");
    }
}
