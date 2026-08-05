//! Navigation for package metadata whose targets are filesystem modules.

use super::documents::OpenDocument;
use super::protocol::{file_uri_for_path, lsp_position_to_byte_offset, position_from_params};
use crate::lexer::lex;
use crate::parser::parse_package_file;
use crate::source::SourceMap;
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn package_entry_definition(
    document: &OpenDocument,
    package_root: Option<&Path>,
    params: Option<&Value>,
) -> Option<Value> {
    let position = position_from_params(params)?;
    let offset = lsp_position_to_byte_offset(&document.text, position.line, position.character);
    let root = package_root?;
    if document.absolute_path.as_deref()? != root.join("nocter.nct") {
        return None;
    }

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
    let (entry, origin) =
        crate::package::executable_entry_at_offset(&package_file.manifest, offset)?;

    let target = crate::package::resolve_explicit_module_path(root, entry).ok()?;
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
