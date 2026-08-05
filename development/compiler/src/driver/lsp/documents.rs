use super::protocol::percent_decode;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WorkspaceRoot {
    pub(super) uri: String,
    pub(super) path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OpenDocument {
    pub(super) uri: String,
    pub(super) version: Option<i64>,
    pub(super) display_path: String,
    pub(super) absolute_path: Option<PathBuf>,
    pub(super) text: String,
}

impl OpenDocument {
    pub(super) fn change_is_stale(&self, version: Option<i64>) -> bool {
        matches!((self.version, version), (Some(current), Some(next)) if next < current)
    }
}

pub(super) fn workspace_roots_from_initialize_params(params: Option<&Value>) -> Vec<WorkspaceRoot> {
    let Some(params) = params else {
        return Vec::new();
    };

    if let Some(folders) = params.get("workspaceFolders").and_then(Value::as_array) {
        let roots = folders
            .iter()
            .filter_map(|folder| {
                folder
                    .get("uri")
                    .and_then(Value::as_str)
                    .map(workspace_root_from_uri)
            })
            .collect::<Vec<_>>();
        if !roots.is_empty() {
            return roots;
        }
    }

    params
        .get("rootUri")
        .and_then(Value::as_str)
        .map(workspace_root_from_uri)
        .into_iter()
        .collect()
}

pub(super) fn open_document_from_params(params: Option<&Value>) -> Option<OpenDocument> {
    let text_document = params?.get("textDocument")?;
    let uri = text_document.get("uri")?.as_str()?.to_string();
    let version = text_document.get("version").and_then(Value::as_i64);
    let text = text_document.get("text")?.as_str()?.to_string();
    Some(open_document(uri, version, text))
}

pub(super) fn changed_document_from_params(
    params: Option<&Value>,
) -> Option<(String, Option<i64>, String)> {
    let params = params?;
    let uri = params
        .get("textDocument")?
        .get("uri")?
        .as_str()?
        .to_string();
    let version = params
        .get("textDocument")?
        .get("version")
        .and_then(Value::as_i64);
    let text = params
        .get("contentChanges")?
        .as_array()?
        .last()?
        .get("text")?
        .as_str()?
        .to_string();
    Some((uri, version, text))
}

pub(super) fn document_uri_from_params(params: Option<&Value>) -> Option<String> {
    params?
        .get("textDocument")?
        .get("uri")?
        .as_str()
        .map(str::to_string)
}

pub(super) fn saved_document_from_params(
    params: Option<&Value>,
) -> Option<(String, Option<String>)> {
    let params = params?;
    let uri = params
        .get("textDocument")?
        .get("uri")?
        .as_str()?
        .to_string();
    let text = params
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_string);
    Some((uri, text))
}

pub(super) fn supports_dynamic_file_watching(params: Option<&Value>) -> bool {
    params
        .and_then(|params| params.get("capabilities"))
        .and_then(|capabilities| capabilities.get("workspace"))
        .and_then(|workspace| workspace.get("didChangeWatchedFiles"))
        .and_then(|watching| watching.get("dynamicRegistration"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(super) fn changed_file_paths_from_params(params: Option<&Value>) -> Vec<PathBuf> {
    params
        .and_then(|params| params.get("changes"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|change| change.get("uri").and_then(Value::as_str))
        .filter_map(file_uri_to_path)
        .map(|path| path.canonicalize().unwrap_or(path))
        .collect()
}

pub(super) fn open_document(uri: String, version: Option<i64>, text: String) -> OpenDocument {
    let path = file_uri_to_path(&uri);
    let display_path = path
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| uri.clone());
    let absolute_path = path
        .as_ref()
        .map(|path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf()));

    OpenDocument {
        uri,
        version,
        display_path,
        absolute_path,
        text,
    }
}

pub(super) fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    Some(PathBuf::from(percent_decode(rest)))
}

fn workspace_root_from_uri(uri: &str) -> WorkspaceRoot {
    WorkspaceRoot {
        uri: uri.to_string(),
        path: file_uri_to_path(uri).map(|path| path.canonicalize().unwrap_or(path)),
    }
}
