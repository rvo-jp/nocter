use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use nocter_syntax::{NodeKind, SyntaxElement, SyntaxTree};

use crate::{ExactDependencyLock, PackageDeclaration};

pub(crate) fn render_effective_locks(
    path: &Path,
    original: &[u8],
    syntax: &SyntaxTree,
    declaration: &PackageDeclaration,
    locks: &BTreeMap<Box<str>, ExactDependencyLock>,
) -> Result<PackageLockSourceUpdate, PackageLockSourceError> {
    if locks.is_empty() {
        return Ok(PackageLockSourceUpdate::new(
            path,
            original,
            original.into(),
        ));
    }
    let newline = preferred_newline(original);
    let block = render_block(locks, newline);
    let Some(directive) = declaration.lock_directive() else {
        return Ok(PackageLockSourceUpdate::new(
            path,
            original,
            insert_block(original, syntax, &block, newline)?.into_boxed_slice(),
        ));
    };
    let node = syntax
        .node(directive)
        .ok_or(PackageLockSourceError::MissingLockDirective)?;
    let start = raw_offset(original, node.range().start().get())?;
    let end = raw_offset(original, node.range().end().get())?;
    let prefix = original
        .get(..start)
        .ok_or(PackageLockSourceError::InvalidSourceRange)?;
    let suffix = original
        .get(end..)
        .ok_or(PackageLockSourceError::InvalidSourceRange)?;
    let mut output = Vec::with_capacity(prefix.len() + block.len() + suffix.len());
    output.extend_from_slice(prefix);
    output.extend_from_slice(block.as_bytes());
    output.extend_from_slice(suffix);
    Ok(PackageLockSourceUpdate::new(
        path,
        original,
        output.into_boxed_slice(),
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageLockSourceUpdate {
    path: PathBuf,
    original: Box<[u8]>,
    replacement: Box<[u8]>,
}

impl PackageLockSourceUpdate {
    pub(crate) fn new(path: &Path, original: &[u8], replacement: Box<[u8]>) -> Self {
        Self {
            path: path.into(),
            original: original.into(),
            replacement,
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn original(&self) -> &[u8] {
        &self.original
    }

    #[must_use]
    pub const fn replacement(&self) -> &[u8] {
        &self.replacement
    }
}

fn render_block(locks: &BTreeMap<Box<str>, ExactDependencyLock>, newline: &str) -> String {
    let mut output = String::from("#lock: {");
    output.push_str(newline);
    output.push_str("    format: 1,");
    output.push_str(newline);
    output.push_str("    dependencies: {");
    output.push_str(newline);
    for (alias, lock) in locks {
        output.push_str("        ");
        output.push_str(alias);
        output.push_str(": \"");
        output.push_str(&lock.literal());
        output.push_str("\",");
        output.push_str(newline);
    }
    output.push_str("    },");
    output.push_str(newline);
    output.push('}');
    output
}

fn insert_block(
    original: &[u8],
    syntax: &SyntaxTree,
    block: &str,
    newline: &str,
) -> Result<Vec<u8>, PackageLockSourceError> {
    let insertion = syntax
        .children(syntax.root_id())
        .iter()
        .filter_map(|element| {
            let SyntaxElement::Node(node) = element else {
                return None;
            };
            syntax
                .node(*node)
                .filter(|node| node.kind() == NodeKind::PackageDirective)
                .map(|node| node.range().end().get())
        })
        .max()
        .ok_or(PackageLockSourceError::MissingPackageDeclaration)?;
    let insertion = raw_offset(original, insertion)?;
    let prefix = original
        .get(..insertion)
        .ok_or(PackageLockSourceError::InvalidSourceRange)?;
    let suffix = original
        .get(insertion..)
        .ok_or(PackageLockSourceError::InvalidSourceRange)?;
    let mut output = Vec::with_capacity(original.len() + block.len() + newline.len());
    output.extend_from_slice(prefix);
    output.extend_from_slice(newline.as_bytes());
    output.extend_from_slice(block.as_bytes());
    output.extend_from_slice(suffix);
    Ok(output)
}

fn preferred_newline(source: &[u8]) -> &'static str {
    if source.windows(2).any(|pair| pair == b"\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn raw_offset(source: &[u8], normalized: u32) -> Result<usize, PackageLockSourceError> {
    let mut raw = 0_usize;
    let mut current = 0_u32;
    while current < normalized {
        let byte = source
            .get(raw)
            .ok_or(PackageLockSourceError::InvalidSourceRange)?;
        if *byte == b'\r' && source.get(raw + 1) == Some(&b'\n') {
            raw += 2;
        } else {
            raw += 1;
        }
        current = current
            .checked_add(1)
            .ok_or(PackageLockSourceError::InvalidSourceRange)?;
    }
    Ok(raw)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageLockSourceError {
    UnknownPackage,
    MissingPackageSyntax,
    MissingPackageSource,
    MissingPackageDeclaration,
    MissingLockDirective,
    InvalidSourceRange,
}

impl fmt::Display for PackageLockSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPackage => formatter.write_str("resolved package is unknown"),
            Self::MissingPackageSyntax => formatter.write_str("resolved package syntax is missing"),
            Self::MissingPackageSource => formatter.write_str("resolved package source is missing"),
            Self::MissingPackageDeclaration => {
                formatter.write_str("resolved package declaration is unavailable")
            }
            Self::MissingLockDirective => {
                formatter.write_str("package lock directive is missing from its syntax tree")
            }
            Self::InvalidSourceRange => {
                formatter.write_str("package lock directive has an invalid source range")
            }
        }
    }
}

impl std::error::Error for PackageLockSourceError {}

#[cfg(test)]
mod tests {
    use nocter_source::{SourceMap, SourceName};
    use nocter_syntax::{ParseGoal, parse};

    use super::*;
    use crate::decode_package_declaration;

    fn render(source: &str, locks: &[(&str, ExactDependencyLock)]) -> PackageLockSourceUpdate {
        let mut sources = SourceMap::new();
        let source_id = sources
            .add_bytes(SourceName::new("index.nct"), source.as_bytes())
            .unwrap();
        let source = sources.get(source_id).unwrap();
        let syntax = parse(source, ParseGoal::SourceFile);
        let declaration = decode_package_declaration(source, &syntax).unwrap();
        let locks = locks
            .iter()
            .map(|(alias, lock)| (Box::<str>::from(*alias), lock.clone()))
            .collect();
        render_effective_locks(
            Path::new("index.nct"),
            source.text().as_bytes(),
            &syntax,
            &declaration,
            &locks,
        )
        .unwrap()
    }

    #[test]
    fn inserts_one_sorted_generated_block_in_the_directive_prefix() {
        let rendered = render(
            "#package: { name: \"app\", version: \"0.0.0\", }\n",
            &[
                (
                    "json",
                    ExactDependencyLock::git("7db21c1000000000000000000000000000000000").unwrap(),
                ),
                (
                    "http",
                    ExactDependencyLock::sha256(
                        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                    )
                    .unwrap(),
                ),
            ],
        );

        assert_eq!(
            std::str::from_utf8(rendered.replacement()).unwrap(),
            "#package: { name: \"app\", version: \"0.0.0\", }\n#lock: {\n    format: 1,\n    dependencies: {\n        http: \"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\n        json: \"git:7db21c1000000000000000000000000000000000\",\n    },\n}\n"
        );
    }

    #[test]
    fn inserts_before_source_dependencies_and_code() {
        let rendered = render(
            "#package: { name: \"app\", version: \"0.0.0\", }\n#dependencies: { json: { git: \"https://example.test/json.git\", revision: \"main\", }, }\n\nsee ./body.nct\n\npub func run(): void\n",
            &[(
                "json",
                ExactDependencyLock::git("7db21c1000000000000000000000000000000000").unwrap(),
            )],
        );
        let replacement = std::str::from_utf8(rendered.replacement()).unwrap();

        assert!(
            replacement
                .starts_with("#package: { name: \"app\", version: \"0.0.0\", }\n#dependencies:")
        );
        assert!(replacement.contains("\n#lock: {\n"));
        assert!(replacement.ends_with("\n\nsee ./body.nct\n\npub func run(): void\n"));

        let mut sources = SourceMap::new();
        let source_id = sources
            .add_bytes(SourceName::new("index.nct"), rendered.replacement())
            .unwrap();
        assert!(!parse(sources.get(source_id).unwrap(), ParseGoal::SourceFile).has_errors());
    }

    #[test]
    fn replaces_the_complete_existing_block_without_touching_neighbors() {
        let rendered = render(
            "//! package\n#dependencies: { json: { git: \"https://example.test/json.git\", revision: \"main\", }, }\n#lock: { format: 1, dependencies: { json: \"git:7db21c1000000000000000000000000000000000\", }, }\n#package: { name: \"app\", version: \"0.0.0\", }\n",
            &[(
                "json",
                ExactDependencyLock::git("7db21c1000000000000000000000000000000000").unwrap(),
            )],
        );

        assert_eq!(
            std::str::from_utf8(rendered.replacement()).unwrap(),
            "//! package\n#dependencies: { json: { git: \"https://example.test/json.git\", revision: \"main\", }, }\n#lock: {\n    format: 1,\n    dependencies: {\n        json: \"git:7db21c1000000000000000000000000000000000\",\n    },\n}\n#package: { name: \"app\", version: \"0.0.0\", }\n"
        );
    }

    #[test]
    fn preserves_crlf_when_adding_generated_source() {
        let source = "#package: { name: \"app\", version: \"0.0.0\", }\r\n";
        let mut sources = SourceMap::new();
        let source_id = sources
            .add_bytes(SourceName::new("index.nct"), source.as_bytes())
            .unwrap();
        let normalized = sources.get(source_id).unwrap();
        let syntax = parse(normalized, ParseGoal::SourceFile);
        let declaration = decode_package_declaration(normalized, &syntax).unwrap();
        let locks = [(
            Box::<str>::from("json"),
            ExactDependencyLock::git("7db21c1000000000000000000000000000000000").unwrap(),
        )]
        .into_iter()
        .collect();

        let update = render_effective_locks(
            Path::new("index.nct"),
            source.as_bytes(),
            &syntax,
            &declaration,
            &locks,
        )
        .unwrap();

        assert_eq!(update.original(), source.as_bytes());
        let replacement = std::str::from_utf8(update.replacement()).unwrap();
        assert!(replacement.contains("\r\n#lock: {\r\n"));
        for (index, byte) in replacement.bytes().enumerate() {
            if byte == b'\n' {
                assert_eq!(
                    replacement.as_bytes().get(index.wrapping_sub(1)),
                    Some(&b'\r')
                );
            }
        }
    }

    #[test]
    fn maps_normalized_lock_ranges_back_to_existing_crlf_bytes() {
        let source = "#dependencies: { remote: { git: \"https://example.test/remote.git\", revision: \"main\", }, }\r\n#lock: { format: 1, dependencies: { remote: \"git:7db21c1000000000000000000000000000000000\", }, }\r\n#package: { name: \"app\", version: \"0.0.0\", }\r\n";
        let mut sources = SourceMap::new();
        let source_id = sources
            .add_bytes(SourceName::new("index.nct"), source.as_bytes())
            .unwrap();
        let normalized = sources.get(source_id).unwrap();
        let syntax = parse(normalized, ParseGoal::SourceFile);
        let declaration = decode_package_declaration(normalized, &syntax).unwrap();
        let locks = [(
            Box::<str>::from("remote"),
            ExactDependencyLock::git("7db21c1000000000000000000000000000000000").unwrap(),
        )]
        .into_iter()
        .collect();

        let update = render_effective_locks(
            Path::new("index.nct"),
            source.as_bytes(),
            &syntax,
            &declaration,
            &locks,
        )
        .unwrap();
        let replacement = std::str::from_utf8(update.replacement()).unwrap();

        assert!(replacement.contains("\r\n#lock: {\r\n"));
        assert!(replacement.ends_with("}\r\n#package: { name: \"app\", version: \"0.0.0\", }\r\n"));
        assert!(!replacement.contains("#lock: { format:"));
    }
}
