use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use nocter_syntax::{NodeKind, Punctuation, SyntaxElement, SyntaxTree, TokenKind};

use crate::{ExactDependencyLock, ExactDependencyLockKind, PackageDeclaration};

pub(crate) fn render_effective_selections(
    path: &Path,
    original: &[u8],
    syntax: &SyntaxTree,
    declaration: &PackageDeclaration,
    selections: &BTreeMap<Box<str>, ExactDependencyLock>,
) -> Result<PackageExactSelectionSourceUpdate, PackageExactSelectionSourceError> {
    let mut insertions = Vec::new();
    for (alias, dependency) in declaration.dependencies() {
        let expected = dependency.source().exact_lock_kind();
        let selected = selections.get(alias);
        match (expected, selected) {
            (None, None) => {}
            (None, Some(_)) => {
                return Err(PackageExactSelectionSourceError::UnexpectedSelection(
                    alias.clone(),
                ));
            }
            (Some(_), None) => {
                return Err(PackageExactSelectionSourceError::MissingSelection(
                    alias.clone(),
                ));
            }
            (Some(expected), Some(selected)) if selected.kind() != expected => {
                return Err(PackageExactSelectionSourceError::SelectionKindMismatch(
                    alias.clone(),
                ));
            }
            (Some(_), Some(selected)) => {
                if let Some(authored) = dependency.selection() {
                    if authored.exact() != *selected {
                        return Err(PackageExactSelectionSourceError::AuthoredSelectionMismatch(
                            alias.clone(),
                        ));
                    }
                    continue;
                }
                insertions.extend(selection_insertions(
                    original,
                    syntax,
                    dependency.record(),
                    selected,
                )?);
            }
        }
    }
    if let Some(alias) = selections
        .keys()
        .find(|alias| !declaration.dependencies().contains_key(*alias))
    {
        return Err(PackageExactSelectionSourceError::UnexpectedSelection(
            alias.clone(),
        ));
    }

    insertions.sort_unstable_by_key(|insertion| Reverse(insertion.offset));
    let mut replacement = original.to_vec();
    for insertion in insertions {
        if insertion.offset > replacement.len() {
            return Err(PackageExactSelectionSourceError::InvalidSourceRange);
        }
        replacement.splice(insertion.offset..insertion.offset, insertion.text);
    }
    Ok(PackageExactSelectionSourceUpdate::new(
        path,
        original,
        replacement.into_boxed_slice(),
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageExactSelectionSourceUpdate {
    path: PathBuf,
    original: Box<[u8]>,
    replacement: Box<[u8]>,
}

impl PackageExactSelectionSourceUpdate {
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

struct SourceInsertion {
    offset: usize,
    text: Vec<u8>,
}

fn selection_insertions(
    original: &[u8],
    syntax: &SyntaxTree,
    record: nocter_syntax::NodeId,
    selection: &ExactDependencyLock,
) -> Result<Vec<SourceInsertion>, PackageExactSelectionSourceError> {
    let node = syntax
        .node(record)
        .filter(|node| node.kind() == NodeKind::DirectiveRecord)
        .ok_or(PackageExactSelectionSourceError::MissingDependencyRecord)?;
    let closing = node
        .range()
        .end()
        .get()
        .checked_sub(1)
        .ok_or(PackageExactSelectionSourceError::InvalidSourceRange)?;
    let closing = raw_offset(original, closing)?;
    if original.get(closing) != Some(&b'}') {
        return Err(PackageExactSelectionSourceError::InvalidSourceRange);
    }

    let last_field = syntax
        .children(record)
        .iter()
        .filter_map(|element| {
            let SyntaxElement::Node(node) = element else {
                return None;
            };
            syntax
                .node(*node)
                .filter(|node| node.kind() == NodeKind::DirectiveField)
        })
        .max_by_key(|node| node.range().end().get())
        .ok_or(PackageExactSelectionSourceError::MissingDependencyField)?;
    let last_field_end = last_field.range().end().get();
    let separator_present = syntax.children(record).iter().any(|element| {
        let SyntaxElement::Token(token) = element else {
            return false;
        };
        token.range().start().get() >= last_field_end
            && token.range().end().get() <= node.range().end().get()
            && token.kind() == TokenKind::Punctuation(Punctuation::Comma)
    });

    let whitespace_start = original
        .get(..closing)
        .ok_or(PackageExactSelectionSourceError::InvalidSourceRange)?
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(0, |offset| offset + 1);
    let closing_whitespace = original
        .get(whitespace_start..closing)
        .ok_or(PackageExactSelectionSourceError::InvalidSourceRange)?;
    let field = selection_field(selection);
    let mut field_insertion = Vec::new();
    if closing_whitespace.contains(&b'\n') {
        let indentation = closing_whitespace
            .iter()
            .rposition(|byte| *byte == b'\n')
            .and_then(|newline| closing_whitespace.get(newline + 1..))
            .ok_or(PackageExactSelectionSourceError::InvalidSourceRange)?;
        field_insertion.extend_from_slice(preferred_newline(original).as_bytes());
        field_insertion.extend_from_slice(indentation);
        field_insertion.extend_from_slice(b"    ");
        render_field(&mut field_insertion, field, selection.value());
    } else if closing_whitespace.is_empty() {
        field_insertion.push(b' ');
        render_field(&mut field_insertion, field, selection.value());
        field_insertion.push(b' ');
    } else {
        field_insertion.extend_from_slice(closing_whitespace);
        render_field(&mut field_insertion, field, selection.value());
    }

    let mut insertions = vec![SourceInsertion {
        offset: whitespace_start,
        text: field_insertion,
    }];
    if !separator_present {
        insertions.push(SourceInsertion {
            offset: raw_offset(original, last_field_end)?,
            text: vec![b','],
        });
    }
    Ok(insertions)
}

fn selection_field(selection: &ExactDependencyLock) -> &'static str {
    match selection.kind() {
        ExactDependencyLockKind::Git => "commit",
        ExactDependencyLockKind::Sha256 => "sha256",
    }
}

fn render_field(output: &mut Vec<u8>, field: &str, value: &str) {
    output.extend_from_slice(field.as_bytes());
    output.extend_from_slice(b": \"");
    output.extend_from_slice(value.as_bytes());
    output.extend_from_slice(b"\",");
}

fn preferred_newline(source: &[u8]) -> &'static str {
    if source.windows(2).any(|pair| pair == b"\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn raw_offset(source: &[u8], normalized: u32) -> Result<usize, PackageExactSelectionSourceError> {
    let mut raw = 0_usize;
    let mut current = 0_u32;
    while current < normalized {
        let byte = source
            .get(raw)
            .ok_or(PackageExactSelectionSourceError::InvalidSourceRange)?;
        if *byte == b'\r' && source.get(raw + 1) == Some(&b'\n') {
            raw += 2;
        } else {
            raw += 1;
        }
        current = current
            .checked_add(1)
            .ok_or(PackageExactSelectionSourceError::InvalidSourceRange)?;
    }
    Ok(raw)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackageExactSelectionSourceError {
    UnknownPackage,
    MissingPackageSyntax,
    MissingPackageSource,
    MissingPackageDeclaration,
    MissingDependencyRecord,
    MissingDependencyField,
    MissingSelection(Box<str>),
    UnexpectedSelection(Box<str>),
    SelectionKindMismatch(Box<str>),
    AuthoredSelectionMismatch(Box<str>),
    InvalidSourceRange,
}

impl fmt::Display for PackageExactSelectionSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPackage => formatter.write_str("resolved package is unknown"),
            Self::MissingPackageSyntax => formatter.write_str("resolved package syntax is missing"),
            Self::MissingPackageSource => formatter.write_str("resolved package source is missing"),
            Self::MissingPackageDeclaration => {
                formatter.write_str("resolved package declaration is unavailable")
            }
            Self::MissingDependencyRecord => {
                formatter.write_str("dependency record is missing from its syntax tree")
            }
            Self::MissingDependencyField => {
                formatter.write_str("dependency record has no source field")
            }
            Self::MissingSelection(alias) => {
                write!(
                    formatter,
                    "dependency {alias} has no effective exact selection"
                )
            }
            Self::UnexpectedSelection(alias) => {
                write!(
                    formatter,
                    "dependency {alias} has an unexpected exact selection"
                )
            }
            Self::SelectionKindMismatch(alias) => {
                write!(
                    formatter,
                    "dependency {alias} has the wrong exact selection kind"
                )
            }
            Self::AuthoredSelectionMismatch(alias) => {
                write!(
                    formatter,
                    "dependency {alias} differs from its authored exact selection"
                )
            }
            Self::InvalidSourceRange => {
                formatter.write_str("dependency declaration has an invalid source range")
            }
        }
    }
}

impl std::error::Error for PackageExactSelectionSourceError {}

#[cfg(test)]
mod tests {
    use nocter_source::{SourceMap, SourceName};
    use nocter_syntax::{ParseGoal, parse};

    use super::*;
    use crate::decode_package_declaration;

    fn render(
        source: &str,
        selections: &[(&str, ExactDependencyLock)],
    ) -> PackageExactSelectionSourceUpdate {
        render_result(source, selections).unwrap()
    }

    fn render_result(
        source: &str,
        selections: &[(&str, ExactDependencyLock)],
    ) -> Result<PackageExactSelectionSourceUpdate, PackageExactSelectionSourceError> {
        let mut sources = SourceMap::new();
        let source_id = sources
            .add_bytes(SourceName::new("index.nct"), source.as_bytes())
            .unwrap();
        let normalized = sources.get(source_id).unwrap();
        let syntax = parse(normalized, ParseGoal::SourceFile);
        let declaration = decode_package_declaration(normalized, &syntax).unwrap();
        let selections = selections
            .iter()
            .map(|(alias, selection)| (Box::<str>::from(*alias), selection.clone()))
            .collect();
        render_effective_selections(
            Path::new("index.nct"),
            source.as_bytes(),
            &syntax,
            &declaration,
            &selections,
        )
    }

    #[test]
    fn adds_source_specific_exact_fields_without_rewriting_dependencies() {
        let source = "#package: { name: \"app\", version: \"0.0.0\", }\n#dependencies: {\n    json: {\n        git: \"https://example.test/json.git\",\n        revision: \"main\",\n    },\n    http: { archive: \"https://example.test/http.tar.gz\", },\n}\n";
        let rendered = render(
            source,
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
        let replacement = std::str::from_utf8(rendered.replacement()).unwrap();

        assert!(replacement.contains(
            "revision: \"main\",\n        commit: \"7db21c1000000000000000000000000000000000\",\n"
        ));
        assert!(replacement.contains(
            "http: { archive: \"https://example.test/http.tar.gz\", sha256: \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\", },"
        ));
        assert!(!replacement.contains("#lock"));
        assert_eq!(replacement.matches("#dependencies").count(), 1);
    }

    #[test]
    fn adds_a_separator_when_the_authored_record_has_no_trailing_comma() {
        let rendered = render(
            "#package: { name: \"app\", version: \"0.0.0\" }\n#dependencies: { json: { git: \"u\", revision: \"main\" } }\n",
            &[(
                "json",
                ExactDependencyLock::git("7db21c1000000000000000000000000000000000").unwrap(),
            )],
        );

        assert_eq!(
            std::str::from_utf8(rendered.replacement()).unwrap(),
            "#package: { name: \"app\", version: \"0.0.0\" }\n#dependencies: { json: { git: \"u\", revision: \"main\", commit: \"7db21c1000000000000000000000000000000000\", } }\n"
        );
    }

    #[test]
    fn preserves_crlf_and_is_idempotent_for_authored_selections() {
        let source = "#package: { name: \"app\", version: \"0.0.0\", }\r\n#dependencies: {\r\n    json: {\r\n        git: \"u\",\r\n        revision: \"main\",\r\n    },\r\n}\r\n";
        let selection =
            ExactDependencyLock::git("7db21c1000000000000000000000000000000000").unwrap();
        let first = render(source, &[("json", selection.clone())]);
        let first_text = std::str::from_utf8(first.replacement()).unwrap();
        assert!(first_text.contains("\r\n        commit:"));
        let second = render(first_text, &[("json", selection)]);
        assert_eq!(second.replacement(), first.replacement());
    }

    #[test]
    fn preserves_comments_while_adding_only_the_exact_field() {
        let source = "#package: { name: \"app\", version: \"0.0.0\", }\n#dependencies: {\n    json: {\n        git: \"u\",\n        revision: \"main\", // Keep the requested branch.\n        // Generated selections stay below authored intent.\n    },\n}\n";
        let rendered = render(
            source,
            &[(
                "json",
                ExactDependencyLock::git("7db21c1000000000000000000000000000000000").unwrap(),
            )],
        );
        let replacement = std::str::from_utf8(rendered.replacement()).unwrap();

        assert!(replacement.contains("// Keep the requested branch."));
        assert!(replacement.contains("// Generated selections stay below authored intent."));
        assert!(replacement.contains(
            "// Generated selections stay below authored intent.\n        commit: \"7db21c1000000000000000000000000000000000\","
        ));
    }

    #[test]
    fn rejects_incomplete_or_inconsistent_effective_selection_maps() {
        let remote = "#package: { name: \"app\", version: \"0.0.0\", }\n#dependencies: { json: { git: \"u\", revision: \"main\", } }\n";
        assert!(matches!(
            render_result(remote, &[]),
            Err(PackageExactSelectionSourceError::MissingSelection(alias))
                if alias.as_ref() == "json"
        ));
        assert!(matches!(
            render_result(
                remote,
                &[(
                    "json",
                    ExactDependencyLock::sha256(
                        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                    )
                    .unwrap(),
                )],
            ),
            Err(PackageExactSelectionSourceError::SelectionKindMismatch(alias))
                if alias.as_ref() == "json"
        ));

        let local = "#package: { name: \"app\", version: \"0.0.0\", }\n#dependencies: { local: { path: \"../local\", } }\n";
        assert!(matches!(
            render_result(
                local,
                &[(
                    "local",
                    ExactDependencyLock::git(
                        "7db21c1000000000000000000000000000000000"
                    )
                    .unwrap(),
                )],
            ),
            Err(PackageExactSelectionSourceError::UnexpectedSelection(alias))
                if alias.as_ref() == "local"
        ));
    }
}
