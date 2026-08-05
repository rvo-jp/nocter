use super::{DependencyLock, LockedDependency};
use crate::lexer::lex;
use crate::parser::parse_package_file;
use crate::source::SourceMap;
use std::fs;
use std::path::Path;

pub(super) fn write_generated_lock(
    package_file_path: &Path,
    locks: &[LockedDependency],
) -> Result<(), String> {
    let mut sources = SourceMap::new();
    let source = sources
        .load_file(package_file_path)
        .map_err(|diagnostic| diagnostic.message)?;
    let lexed = lex(&sources, source);
    if !lexed.diagnostics.is_empty() {
        return Err(lexed.diagnostics[0].message.clone());
    }
    let parsed = parse_package_file(&sources, source, &lexed.tokens);
    if !parsed.diagnostics.is_empty() {
        return Err(parsed.diagnostics[0].message.clone());
    }
    let package_file = parsed
        .package_file
        .ok_or_else(|| "package parser produced no package file".to_string())?;
    let source_file = sources
        .get(source)
        .ok_or_else(|| "loaded package source disappeared".to_string())?;
    let text = source_file.text();
    let generated = render_lock(locks);
    let existing = package_file
        .manifest
        .directives
        .iter()
        .find(|directive| directive.name == "lock");
    let rewritten = if let Some(existing) = existing {
        format!(
            "{}{}{}",
            &text[..existing.span.start],
            generated,
            &text[existing.span.end..]
        )
    } else {
        let offset = package_file.manifest.span.end;
        let separator = if offset == 0 || text[..offset].ends_with('\n') {
            ""
        } else {
            "\n"
        };
        let trailing = if text[offset..].starts_with('\n') || text[offset..].is_empty() {
            "\n"
        } else {
            "\n\n"
        };
        format!(
            "{}{}{}{}{}",
            &text[..offset],
            separator,
            generated,
            trailing,
            &text[offset..]
        )
    };
    let temporary = package_file_path.with_extension("nct.lock-write");
    fs::write(&temporary, rewritten)
        .map_err(|error| format!("failed to write generated lock: {error}"))?;
    fs::rename(&temporary, package_file_path)
        .map_err(|error| format!("failed to install generated lock: {error}"))
}

fn render_lock(locks: &[LockedDependency]) -> String {
    let mut locks = locks.to_vec();
    locks.sort_by(|left, right| left.name().cmp(right.name()));
    let mut output = String::from("#lock: {\n    format: 1,\n    dependencies: {");
    if locks.is_empty() {
        output.push_str("},\n}");
        return output;
    }
    output.push('\n');
    for lock in locks {
        output.push_str("        ");
        output.push_str(lock.name());
        output.push_str(": \"");
        match lock.resolution() {
            DependencyLock::GitCommit(commit) => {
                output.push_str("git:");
                output.push_str(commit);
            }
            DependencyLock::ArchiveSha256(digest) => {
                output.push_str("sha256:");
                output.push_str(digest);
            }
        }
        output.push_str("\",\n");
    }
    output.push_str("    },\n}");
    output
}

#[cfg(test)]
mod tests {
    use super::render_lock;
    use crate::package::{DependencyLock, LockedDependency};
    use crate::source::{ByteSpan, SourceId};

    #[test]
    fn renders_locks_in_dependency_name_order() {
        let span = ByteSpan {
            source: SourceId::new(0),
            start: 0,
            end: 0,
        };
        let locks = vec![
            LockedDependency::new(
                "zeta".to_string(),
                span,
                DependencyLock::GitCommit("abc".to_string()),
            ),
            LockedDependency::new(
                "alpha".to_string(),
                span,
                DependencyLock::ArchiveSha256("00".repeat(32)),
            ),
        ];
        let rendered = render_lock(&locks);
        assert!(rendered.find("alpha:").unwrap() < rendered.find("zeta:").unwrap());
    }
}
