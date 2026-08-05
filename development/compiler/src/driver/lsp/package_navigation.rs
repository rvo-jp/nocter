//! Navigation for package metadata whose targets are filesystem modules.

use super::documents::{OpenDocument, WorkspaceRoot};
use super::protocol::{file_uri_for_path, lsp_position_to_byte_offset, position_from_params};
use crate::ast::DirectiveValue;
use crate::lexer::lex;
use crate::parser::parse_package_file;
use crate::source::SourceMap;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(super) fn package_module_definition(
    document: &OpenDocument,
    workspace_roots: &[WorkspaceRoot],
    params: Option<&Value>,
) -> Option<Value> {
    let position = position_from_params(params)?;
    let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
    let root = package_root_for_manifest(document, workspace_roots)?;

    let mut sources = SourceMap::new();
    let source = sources.add_source(
        document.display_path.clone(),
        document.absolute_path.clone(),
        document.text.clone(),
    );
    let lexed = lex(&sources, source);
    if !lexed.diagnostics.is_empty() {
        return None;
    }
    let package_file = parse_package_file(&sources, source, &lexed.tokens).package_file?;
    let (logical, origin) = package_file
        .manifest
        .directives
        .iter()
        .filter(|directive| directive.name == "executable")
        .filter_map(|directive| match &directive.value {
            DirectiveValue::Record { fields, .. } => Some(fields),
            _ => None,
        })
        .flatten()
        .filter(|field| field.name == "module")
        .filter_map(|field| field.value.string_value())
        .find(|(_, span)| span.start <= offset && offset <= span.end)?;

    let target =
        crate::package::resolve_package_module(&root, document.absolute_path.as_deref()?, logical)
            .ok()?;
    Some(json!([{
        "originSelectionRange": super::protocol::range_for_byte_span(&document.text, origin),
        "targetUri": file_uri_for_path(&target),
        "targetRange": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 }
        },
        "targetSelectionRange": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 0 }
        }
    }]))
}

fn package_root_for_manifest(
    document: &OpenDocument,
    workspace_roots: &[WorkspaceRoot],
) -> Option<PathBuf> {
    let manifest_path = document.absolute_path.as_deref()?;
    if manifest_path.file_name()? != "nocter.nct" {
        return None;
    }
    let parent = canonical_or_owned(manifest_path.parent()?);
    let inside_workspace = workspace_roots
        .iter()
        .filter_map(|root| root.path.as_deref())
        .map(canonical_or_owned)
        .any(|root| parent.starts_with(root));
    inside_workspace.then_some(parent)
}

fn canonical_or_owned(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
