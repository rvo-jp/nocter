//! Filesystem discovery for module-path completion.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub(crate) fn module_segment_candidates(directory: &Path, prefix: &str) -> Vec<String> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut candidates = BTreeSet::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let segment = if path.is_dir() {
            name
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("nct") {
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            if stem == "index" {
                continue;
            }
            stem.to_string()
        } else {
            continue;
        };
        if segment.starts_with(prefix) && valid_module_segment(&segment) {
            candidates.insert(segment);
        }
    }
    candidates.into_iter().collect()
}

fn valid_module_segment(segment: &str) -> bool {
    let mut bytes = segment.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    matches!(first, b'a'..=b'z' | b'_')
        && segment != "_"
        && bytes.all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_segments_follow_source_layout_and_prefix() {
        let root = std::env::temp_dir().join(format!(
            "nocter-module-discovery-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("string_builder")).unwrap();
        fs::write(root.join("string.nct"), "").unwrap();
        fs::write(root.join("index.nct"), "").unwrap();
        fs::write(root.join("String.nct"), "").unwrap();

        assert_eq!(
            module_segment_candidates(&root, "string"),
            vec!["string".to_string(), "string_builder".to_string()]
        );

        fs::remove_dir_all(root).unwrap();
    }
}
