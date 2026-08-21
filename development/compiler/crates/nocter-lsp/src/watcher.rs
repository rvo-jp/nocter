use nocter_json::{Member, Value};

use crate::decode::{Object, array, integer, required, string};
use crate::{DocumentUri, ParameterError, ParameterErrorKind};

pub const WATCHED_FILES_REGISTRATION_ID: &str = "nocter-source-files";

/// Builds the exact dynamic registration for all Nocter source create/change/delete events.
#[must_use]
pub fn watched_files_registration() -> Value {
    object([(
        "registrations",
        Value::Array(vec![object([
            ("id", Value::String(WATCHED_FILES_REGISTRATION_ID.into())),
            (
                "method",
                Value::String("workspace/didChangeWatchedFiles".into()),
            ),
            (
                "registerOptions",
                object([(
                    "watchers",
                    Value::Array(vec![object([
                        ("globPattern", Value::String("**/*.nct".into())),
                        ("kind", Value::Number("7".into())),
                    ])]),
                )]),
            ),
        ])]),
    )])
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatchedFileChangeKind {
    Created,
    Changed,
    Deleted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchedFileChange {
    uri: DocumentUri,
    kind: WatchedFileChangeKind,
}

impl WatchedFileChange {
    #[must_use]
    pub const fn uri(&self) -> &DocumentUri {
        &self.uri
    }

    #[must_use]
    pub const fn kind(&self) -> WatchedFileChangeKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DidChangeWatchedFilesParams {
    changes: Box<[WatchedFileChange]>,
}

impl DidChangeWatchedFilesParams {
    /// Decodes one ordered watched-file batch without resolving a URI through the filesystem.
    ///
    /// # Errors
    ///
    /// Returns the exact duplicate, missing, type, URI, or change-kind failure.
    pub fn decode(params: Option<Value>) -> Result<Self, ParameterError> {
        let mut root = Object::new(required(params, "params")?, "params")?;
        let values = array(root.take("changes")?, "params.changes")?;
        let mut changes = Vec::with_capacity(values.len());
        for (index, value) in values.into_iter().enumerate() {
            let base = format!("params.changes[{index}]");
            let mut object = Object::new(value, base.clone())?;
            let uri_path = format!("{base}.uri");
            let uri = DocumentUri::new(string(object.take("uri")?, &uri_path)?)
                .map_err(|_| ParameterError::new(ParameterErrorKind::EmptyUri, uri_path))?;
            let kind_path = format!("{base}.type");
            let kind = match integer(object.take("type")?, &kind_path)? {
                1 => WatchedFileChangeKind::Created,
                2 => WatchedFileChangeKind::Changed,
                3 => WatchedFileChangeKind::Deleted,
                _ => {
                    return Err(ParameterError::new(
                        ParameterErrorKind::ExpectedWatchedFileChangeType,
                        kind_path,
                    ));
                }
            };
            changes.push(WatchedFileChange { uri, kind });
        }
        Ok(Self {
            changes: changes.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn changes(&self) -> &[WatchedFileChange] {
        &self.changes
    }
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
    fn registration_covers_create_change_and_delete_for_nocter_sources() {
        let mut rendered = String::new();
        write_value(&mut rendered, &watched_files_registration());
        assert_eq!(
            rendered,
            concat!(
                "{\"registrations\":[{\"id\":\"nocter-source-files\",",
                "\"method\":\"workspace/didChangeWatchedFiles\",",
                "\"registerOptions\":{\"watchers\":[{",
                "\"globPattern\":\"**/*.nct\",\"kind\":7}]}}]}"
            )
        );
    }

    #[test]
    fn decodes_ordered_watched_file_changes() {
        let params = DidChangeWatchedFilesParams::decode(Some(
            parse(
                r#"{"changes":[{"uri":"file:///a.nct","type":1},{"uri":"file:///b.nct","type":3}]}"#,
            )
            .unwrap(),
        ))
        .unwrap();
        assert_eq!(params.changes().len(), 2);
        assert_eq!(params.changes()[0].kind(), WatchedFileChangeKind::Created);
        assert_eq!(params.changes()[1].kind(), WatchedFileChangeKind::Deleted);

        let error = DidChangeWatchedFilesParams::decode(Some(
            parse(r#"{"changes":[{"uri":"file:///a.nct","type":4}]}"#).unwrap(),
        ))
        .unwrap_err();
        assert_eq!(
            error.kind(),
            ParameterErrorKind::ExpectedWatchedFileChangeType
        );
    }
}
