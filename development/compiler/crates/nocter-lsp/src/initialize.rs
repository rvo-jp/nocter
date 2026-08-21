use nocter_json::{Member, Value};

use crate::decode::{Object, array, boolean, required, string};
use crate::{DocumentUri, ParameterError, ParameterErrorKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceFolder {
    uri: DocumentUri,
    name: Box<str>,
}

impl WorkspaceFolder {
    #[must_use]
    pub const fn uri(&self) -> &DocumentUri {
        &self.uri
    }

    #[must_use]
    pub const fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitializeParams {
    root_uri: Option<DocumentUri>,
    workspace_folders: Box<[WorkspaceFolder]>,
    dynamic_watched_files: bool,
}

impl InitializeParams {
    /// Decodes the initialization fields that affect server roots and capability registration.
    /// Unknown client capabilities remain forward-compatible and are ignored.
    ///
    /// # Errors
    ///
    /// Returns the exact recognized field whose shape or duplicate occurrence is invalid.
    pub fn decode(params: Option<Value>) -> Result<Self, ParameterError> {
        let mut root = Object::new(required(params, "params")?, "params")?;
        let root_uri = optional_uri(root.take_optional("rootUri")?, "params.rootUri")?;
        let workspace_folders = decode_workspace_folders(root.take_optional("workspaceFolders")?)?;
        let capabilities = Object::new(root.take("capabilities")?, "params.capabilities")?;
        let dynamic_watched_files = decode_dynamic_watched_files(capabilities)?;
        Ok(Self {
            root_uri,
            workspace_folders: workspace_folders.into_boxed_slice(),
            dynamic_watched_files,
        })
    }

    #[must_use]
    pub const fn root_uri(&self) -> Option<&DocumentUri> {
        self.root_uri.as_ref()
    }

    #[must_use]
    pub const fn workspace_folders(&self) -> &[WorkspaceFolder] {
        &self.workspace_folders
    }

    #[must_use]
    pub const fn supports_dynamic_watched_files(&self) -> bool {
        self.dynamic_watched_files
    }
}

/// Builds the exact capabilities advertised by the currently implemented server surface.
#[must_use]
pub fn initialize_result(server_version: &str) -> Value {
    object([
        (
            "capabilities",
            object([
                ("positionEncoding", Value::String("utf-16".into())),
                (
                    "textDocumentSync",
                    object([
                        ("openClose", Value::Bool(true)),
                        ("change", Value::Number("1".into())),
                        ("save", object([("includeText", Value::Bool(true))])),
                    ]),
                ),
            ]),
        ),
        (
            "serverInfo",
            object([
                ("name", Value::String("Nocter".into())),
                ("version", Value::String(server_version.into())),
            ]),
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

fn optional_uri(value: Option<Value>, path: &str) -> Result<Option<DocumentUri>, ParameterError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            let value = string(value, path)?;
            DocumentUri::new(value)
                .map(Some)
                .map_err(|_| ParameterError::new(ParameterErrorKind::EmptyUri, path))
        }
    }
}

fn decode_workspace_folders(value: Option<Value>) -> Result<Vec<WorkspaceFolder>, ParameterError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if value == Value::Null {
        return Ok(Vec::new());
    }
    let values = array(value, "params.workspaceFolders")?;
    let mut folders = Vec::with_capacity(values.len());
    for (index, value) in values.into_iter().enumerate() {
        let base = format!("params.workspaceFolders[{index}]");
        let mut object = Object::new(value, base.clone())?;
        let uri_path = format!("{base}.uri");
        let uri = DocumentUri::new(string(object.take("uri")?, &uri_path)?)
            .map_err(|_| ParameterError::new(ParameterErrorKind::EmptyUri, uri_path))?;
        let name_path = format!("{base}.name");
        let name = string(object.take("name")?, &name_path)?;
        folders.push(WorkspaceFolder { uri, name });
    }
    Ok(folders)
}

fn decode_dynamic_watched_files(mut capabilities: Object) -> Result<bool, ParameterError> {
    let Some(workspace) = capabilities.take_optional("workspace")? else {
        return Ok(false);
    };
    let mut workspace = Object::new(workspace, "params.capabilities.workspace")?;
    let Some(watched) = workspace.take_optional("didChangeWatchedFiles")? else {
        return Ok(false);
    };
    let mut watched = Object::new(
        watched,
        "params.capabilities.workspace.didChangeWatchedFiles",
    )?;
    match watched.take_optional("dynamicRegistration")? {
        Some(value) => boolean(
            &value,
            "params.capabilities.workspace.didChangeWatchedFiles.dynamicRegistration",
        ),
        None => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use nocter_json::{parse, write_value};

    use super::*;

    #[test]
    fn decodes_roots_and_dynamic_watcher_support() {
        let params = InitializeParams::decode(Some(
            parse(
                r#"{
                    "rootUri":"file:///fallback",
                    "workspaceFolders":[{"uri":"file:///one","name":"One"}],
                    "capabilities":{"workspace":{"didChangeWatchedFiles":{"dynamicRegistration":true}}},
                    "clientInfo":{"name":"future-compatible"}
                }"#,
            )
            .unwrap(),
        ))
        .unwrap();

        assert_eq!(params.root_uri().unwrap().as_str(), "file:///fallback");
        assert_eq!(params.workspace_folders().len(), 1);
        assert_eq!(params.workspace_folders()[0].name(), "One");
        assert!(params.supports_dynamic_watched_files());
    }

    #[test]
    fn defaults_optional_roots_and_capabilities_without_inventing_support() {
        let params = InitializeParams::decode(Some(
            parse(r#"{"rootUri":null,"workspaceFolders":null,"capabilities":{}}"#).unwrap(),
        ))
        .unwrap();
        assert!(params.root_uri().is_none());
        assert!(params.workspace_folders().is_empty());
        assert!(!params.supports_dynamic_watched_files());
    }

    #[test]
    fn rejects_wrong_recognized_capability_shapes() {
        let error = InitializeParams::decode(Some(
            parse(r#"{"capabilities":{"workspace":{"didChangeWatchedFiles":{"dynamicRegistration":"yes"}}}}"#)
                .unwrap(),
        ))
        .unwrap_err();
        assert_eq!(error.kind(), ParameterErrorKind::ExpectedBoolean);
        assert_eq!(
            error.path(),
            "params.capabilities.workspace.didChangeWatchedFiles.dynamicRegistration"
        );
    }

    #[test]
    fn advertises_only_the_implemented_full_sync_surface() {
        let mut rendered = String::new();
        write_value(&mut rendered, &initialize_result("0.14.0-dev"));
        assert_eq!(
            rendered,
            concat!(
                "{\"capabilities\":{\"positionEncoding\":\"utf-16\",",
                "\"textDocumentSync\":{\"openClose\":true,\"change\":1,",
                "\"save\":{\"includeText\":true}}},",
                "\"serverInfo\":{\"name\":\"Nocter\",\"version\":\"0.14.0-dev\"}}"
            )
        );
        assert!(!rendered.contains("hoverProvider"));
        assert!(!rendered.contains("semanticTokensProvider"));
    }
}
