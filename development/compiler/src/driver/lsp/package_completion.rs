//! Completion for source-native package directives.

use super::documents::OpenDocument;
use super::protocol::byte_offset_to_lsp_position;
use serde_json::{Value, json};

const DIRECTIVES: [(&str, &str); 6] = [
    ("name", "#name: \"${1:package}\""),
    ("version", "#version: \"${1:0.1.0}\""),
    (
        "dependencies",
        "#dependencies: {\n    ${1:name}: { path: \"${2:./path}\" },\n}",
    ),
    ("lock", "#lock: {\n    version: 1,\n}"),
    (
        "executable",
        "#executable: {\n    name: \"${1:app}\",\n    module: \"${2:./src/app}\",\n}",
    ),
    (
        "test",
        "#test: {\n    name: \"${1:unit}\",\n    module: \"${2:./tests/unit}\",\n}",
    ),
];

pub(super) fn package_manifest_completion_items(
    document: &OpenDocument,
    offset: usize,
) -> Option<Vec<Value>> {
    if document
        .absolute_path
        .as_deref()
        .and_then(|path| path.file_name())
        .is_none_or(|name| name != "nocter.nct")
    {
        return None;
    }
    let offset = offset.min(document.text.len());
    let line_start = document.text[..offset]
        .rfind('\n')
        .map_or(0, |newline| newline + 1);
    let indentation = document.text[line_start..offset].len()
        - document.text[line_start..offset].trim_start().len();
    let directive_start = line_start + indentation;
    let prefix = document.text.get(directive_start..offset)?;
    let typed = prefix.strip_prefix('#')?;
    if typed.contains(':')
        || !typed
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        return None;
    }

    let start = byte_offset_to_lsp_position(&document.text, directive_start);
    let end = byte_offset_to_lsp_position(&document.text, offset);
    Some(
        DIRECTIVES
            .iter()
            .filter(|(name, _)| name.starts_with(typed))
            .map(|(name, insertion)| {
                json!({
                    "label": format!("#{name}"),
                    "kind": 10,
                    "detail": "package directive",
                    "insertTextFormat": 2,
                    "textEdit": {
                        "range": {
                            "start": { "line": start.line, "character": start.character },
                            "end": { "line": end.line, "character": end.character }
                        },
                        "newText": insertion
                    }
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn completes_test_directive_with_an_explicit_target_shape() {
        let document = OpenDocument {
            uri: "file:///tmp/nocter.nct".to_string(),
            version: Some(1),
            display_path: "/tmp/nocter.nct".to_string(),
            absolute_path: Some(PathBuf::from("/tmp/nocter.nct")),
            text: "  #te".to_string(),
        };

        let items = package_manifest_completion_items(&document, document.text.len()).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["label"], "#test");
        assert_eq!(items[0]["textEdit"]["range"]["start"]["character"], 2);
        assert_eq!(items[0]["textEdit"]["range"]["end"]["character"], 5);
        assert_eq!(
            items[0]["textEdit"]["newText"],
            "#test: {\n    name: \"${1:unit}\",\n    module: \"${2:./tests/unit}\",\n}"
        );
    }

    #[test]
    fn leaves_source_completion_in_control_after_a_directive_colon() {
        let document = OpenDocument {
            uri: "file:///tmp/nocter.nct".to_string(),
            version: Some(1),
            display_path: "/tmp/nocter.nct".to_string(),
            absolute_path: Some(PathBuf::from("/tmp/nocter.nct")),
            text: "#test:".to_string(),
        };

        assert!(package_manifest_completion_items(&document, document.text.len()).is_none());
    }
}
