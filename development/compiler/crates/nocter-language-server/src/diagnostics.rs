use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use nocter_diagnostics::SourceDiagnostic;
use nocter_json::{Member, Value};
use nocter_lsp::{DocumentUri, DocumentUriError, render_notification};
use nocter_package::{PackageGraphError, PackageResolutionError};
use nocter_source::{CoordinateError, SourceMap, Utf16Range};

use crate::{AnalysisScope, WorkspaceAnalysisGeneration};

/// Stateful projection that publishes complete diagnostic sets and clears superseded documents.
#[derive(Debug, Default)]
pub struct DiagnosticPublisher {
    published: BTreeMap<AnalysisScope, BTreeSet<DocumentUri>>,
}

impl DiagnosticPublisher {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Projects one analysis generation into complete `textDocument/publishDiagnostics`
    /// notifications. Previously published documents absent from this generation receive an empty
    /// set before the new scope becomes visible.
    ///
    /// # Errors
    ///
    /// Returns a source identity, UTF-16 coordinate, or local URI projection failure.
    pub fn publish(
        &mut self,
        analysis: &WorkspaceAnalysisGeneration,
    ) -> Result<Box<[String]>, DiagnosticPublicationError> {
        let current = project_analysis(analysis)?;
        let has_source_diagnostics = !current.is_empty();
        let mut clear = BTreeSet::new();
        for scope in analysis.invalidated_scopes() {
            if let Some(uris) = self.published.remove(scope) {
                clear.extend(uris);
            }
        }
        let current_uris = current.keys().cloned().collect::<BTreeSet<_>>();
        if let Some(scope) = analysis.scope() {
            if let Some(previous) = self.published.remove(scope) {
                clear.extend(previous.difference(&current_uris).cloned());
            }
            self.published.insert(scope.clone(), current_uris.clone());
        }
        for uri in &current_uris {
            clear.remove(uri);
        }

        let mut notifications = Vec::with_capacity(clear.len() + current.len());
        for uri in clear {
            let path = uri.file_path().map_err(DiagnosticPublicationError::Uri)?;
            let version = analysis
                .source_overlay()
                .document(&path)
                .map(|document| document.version().get());
            notifications.push(publication(&uri, version, Vec::new()));
        }
        for (uri, document) in current {
            notifications.push(publication(&uri, document.version, document.diagnostics));
        }
        if let Some(error) = analysis.preparation_failure()
            && !has_source_diagnostics
        {
            notifications.push(render_notification(
                "window/showMessage",
                &object([
                    ("type", Value::Number("1".into())),
                    (
                        "message",
                        Value::String(
                            format!("error[{}]: {error}", error.diagnostic_code()).into_boxed_str(),
                        ),
                    ),
                ]),
            ));
        }
        Ok(notifications.into_boxed_slice())
    }
}

struct ProjectedDocument {
    version: Option<i32>,
    diagnostics: Vec<Value>,
}

fn project_analysis(
    analysis: &WorkspaceAnalysisGeneration,
) -> Result<BTreeMap<DocumentUri, ProjectedDocument>, DiagnosticPublicationError> {
    let mut projected = BTreeMap::new();
    if let Some(snapshot) = analysis.snapshot() {
        for diagnostic in snapshot.diagnostics() {
            let (uri, path, value) = project_diagnostic(diagnostic, snapshot.sources())?;
            let version = snapshot
                .source_overlay()
                .document(&path)
                .map(|document| document.version().get());
            let document = projected.entry(uri).or_insert_with(|| ProjectedDocument {
                version,
                diagnostics: Vec::new(),
            });
            if document.version != version {
                return Err(DiagnosticPublicationError::InconsistentDocumentVersion(
                    path,
                ));
            }
            document.diagnostics.push(value);
        }
    } else if let Some(crate::WorkspaceAnalysisError::Package(failure)) =
        analysis.preparation_failure()
        && let PackageResolutionError::Graph(PackageGraphError::Declaration(error)) =
            failure.error()
    {
        let subject = error.subject();
        let sources = failure.reached().sources();
        let source =
            sources
                .get(subject.source())
                .ok_or(DiagnosticPublicationError::MissingSource(
                    subject.source().index(),
                ))?;
        let tree = failure
            .reached()
            .syntax_trees()
            .iter()
            .find(|tree| tree.node(subject).is_some())
            .ok_or(DiagnosticPublicationError::MissingSyntaxSubject {
                source: subject.source().index(),
                node: subject.index(),
            })?;
        let node = tree
            .node(subject)
            .expect("selected syntax tree contains package declaration subject");
        let diagnostic = SourceDiagnostic::new(
            "E0800",
            error.to_string(),
            source.span(node.range()),
            [],
            None::<Box<str>>,
        );
        let (uri, path, value) = project_diagnostic(&diagnostic, sources)?;
        let version = failure
            .reached()
            .source_overlay()
            .document(&path)
            .map(|document| document.version().get());
        projected.insert(
            uri,
            ProjectedDocument {
                version,
                diagnostics: vec![value],
            },
        );
    }
    Ok(projected)
}

fn project_diagnostic(
    diagnostic: &SourceDiagnostic,
    sources: &SourceMap,
) -> Result<(DocumentUri, PathBuf, Value), DiagnosticPublicationError> {
    let primary = diagnostic.primary();
    let source = sources
        .get(primary.source())
        .ok_or(DiagnosticPublicationError::MissingSource(
            primary.source().index(),
        ))?;
    let path = PathBuf::from(source.name().as_str());
    let uri = DocumentUri::from_file_path(&path).map_err(DiagnosticPublicationError::Uri)?;
    let range = source
        .utf16_range(primary.span().range())
        .map_err(DiagnosticPublicationError::Coordinate)?;
    let mut members = vec![
        member("range", range_value(range)),
        member("severity", Value::Number("1".into())),
        member("code", Value::String(diagnostic.code().into())),
        member("source", Value::String("nocter".into())),
        member("message", Value::String(diagnostic.message().into())),
    ];
    if !diagnostic.notes().is_empty() {
        let mut related = Vec::with_capacity(diagnostic.notes().len());
        for note in diagnostic.notes() {
            let origin = note.origin();
            let related_source =
                sources
                    .get(origin.source())
                    .ok_or(DiagnosticPublicationError::MissingSource(
                        origin.source().index(),
                    ))?;
            let related_path = Path::new(related_source.name().as_str());
            let related_uri = DocumentUri::from_file_path(related_path)
                .map_err(DiagnosticPublicationError::Uri)?;
            let related_range = related_source
                .utf16_range(origin.span().range())
                .map_err(DiagnosticPublicationError::Coordinate)?;
            related.push(object([
                (
                    "location",
                    object([
                        ("uri", Value::String(related_uri.as_str().into())),
                        ("range", range_value(related_range)),
                    ]),
                ),
                ("message", Value::String(note.message().into())),
            ]));
        }
        members.push(member("relatedInformation", Value::Array(related)));
    }
    if let Some(help) = diagnostic.help() {
        members.push(member(
            "data",
            object([("help", Value::String(help.into()))]),
        ));
    }
    Ok((uri, path, Value::Object(members)))
}

fn publication(uri: &DocumentUri, version: Option<i32>, diagnostics: Vec<Value>) -> String {
    let mut members = vec![member("uri", Value::String(uri.as_str().into()))];
    if let Some(version) = version {
        members.push(member(
            "version",
            Value::Number(version.to_string().into_boxed_str()),
        ));
    }
    members.push(member("diagnostics", Value::Array(diagnostics)));
    render_notification("textDocument/publishDiagnostics", &Value::Object(members))
}

fn range_value(range: Utf16Range) -> Value {
    object([
        (
            "start",
            position(range.start().line(), range.start().character()),
        ),
        ("end", position(range.end().line(), range.end().character())),
    ])
}

fn position(line: u32, character: u32) -> Value {
    object([
        ("line", Value::Number(line.to_string().into_boxed_str())),
        (
            "character",
            Value::Number(character.to_string().into_boxed_str()),
        ),
    ])
}

fn object<const N: usize>(members: [(&str, Value); N]) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(name, value)| member(name, value))
            .collect(),
    )
}

fn member(name: &str, value: Value) -> Member {
    Member {
        name: name.into(),
        value,
    }
}

#[derive(Debug)]
pub enum DiagnosticPublicationError {
    MissingSource(u32),
    Coordinate(CoordinateError),
    Uri(DocumentUriError),
    InconsistentDocumentVersion(PathBuf),
    MissingSyntaxSubject { source: u32, node: usize },
}

impl fmt::Display for DiagnosticPublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSource(source) => {
                write!(formatter, "diagnostic refers to missing source {source}")
            }
            Self::Coordinate(error) => error.fmt(formatter),
            Self::Uri(error) => error.fmt(formatter),
            Self::InconsistentDocumentVersion(path) => write!(
                formatter,
                "diagnostics disagree on open-document version for {}",
                path.display()
            ),
            Self::MissingSyntaxSubject { source, node } => write!(
                formatter,
                "package diagnostic refers to missing syntax node {source}:{node}"
            ),
        }
    }
}

impl std::error::Error for DiagnosticPublicationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Coordinate(error) => Some(error),
            Self::Uri(error) => Some(error),
            Self::MissingSource(_)
            | Self::InconsistentDocumentVersion(_)
            | Self::MissingSyntaxSubject { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use nocter_json::parse;
    use nocter_lsp::{DidChangeParams, DidOpenParams, InitializeParams};
    use nocter_model::{CompilationTarget, PackageIdentity};
    use nocter_package::StandardPackage;

    use super::*;
    use crate::{
        DocumentWorkspace, DocumentWorkspaceChange, LanguageServerEnvironment,
        LanguageServerToolchain, WorkspaceAnalyses, WorkspaceConfiguration,
    };

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn publishes_versioned_diagnostics_and_clears_them_from_the_next_snapshot() {
        let temporary = TemporaryDirectory::new();
        let source = temporary.path().join("a b.nct");
        let mut documents = DocumentWorkspace::new();
        let accepted = documents
            .open(&open_params(
                &source,
                1,
                "func main(): void { let text = \"😀\" unknown() return }\n",
            ))
            .unwrap();
        let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));
        let failed = analyses.analyze(accepted);
        let mut publisher = DiagnosticPublisher::new();

        let emitted = publisher.publish(&failed).unwrap();

        assert_eq!(emitted.len(), 1);
        assert!(emitted[0].contains("file:///"));
        assert!(emitted[0].contains("a%20b.nct"));
        assert!(emitted[0].contains("\"version\":1"));
        assert!(!emitted[0].contains("\"diagnostics\":[]"));

        let changed = documents
            .change(&change_params(&source, 2, "func main(): void { return }\n"))
            .unwrap();
        let DocumentWorkspaceChange::Accepted(changed) = changed else {
            panic!("newer document version must be accepted")
        };
        let complete = analyses.analyze(changed);
        let cleared = publisher.publish(&complete).unwrap();

        assert_eq!(cleared.len(), 1);
        assert!(cleared[0].contains("\"diagnostics\":[]"));
        assert!(cleared[0].contains("\"version\":2"));
    }

    #[test]
    fn package_declaration_failure_uses_its_retained_syntax_subject() {
        let temporary = TemporaryDirectory::new();
        let package_source = temporary.path().join("index.nct");
        let mut documents = DocumentWorkspace::new();
        let accepted = documents
            .open(&open_params(
                &package_source,
                1,
                concat!(
                    "#package: { name: \"app\", version: \"0.0.0\", }\n",
                    "#dependencies: { remote: { unknown: \"value\", }, }\n",
                ),
            ))
            .unwrap();
        let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));
        let failed = analyses.analyze(accepted);

        let emitted = DiagnosticPublisher::new().publish(&failed).unwrap();

        assert_eq!(emitted.len(), 1);
        assert!(emitted[0].contains("textDocument/publishDiagnostics"));
        assert!(emitted[0].contains("\"code\":\"E0800\""));
        assert!(emitted[0].contains("\"version\":1"));
        assert!(!emitted[0].contains("window/showMessage"));
        assert_eq!(failed.reached_sources().unwrap().len(), 2);
        assert_eq!(failed.reached_syntax_trees().len(), 2);
    }

    fn configuration(root: &Path) -> WorkspaceConfiguration {
        let standard_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std");
        let environment = LanguageServerEnvironment::new(
            root,
            LanguageServerToolchain::new(
                CompilationTarget::Arm64Darwin,
                root,
                StandardPackage::new(PackageIdentity::new("toolchain:std"), standard_root),
            ),
        );
        let params = InitializeParams::decode(Some(
            parse(&format!(
                "{{\"rootUri\":\"file://{}\",\"capabilities\":{{}}}}",
                root.display()
            ))
            .unwrap(),
        ))
        .unwrap();
        WorkspaceConfiguration::resolve(&environment, &params).unwrap()
    }

    fn open_params(path: &Path, version: i32, text: &str) -> DidOpenParams {
        DidOpenParams::decode(Some(
            parse(&document_json(path, version, text, true)).unwrap(),
        ))
        .unwrap()
    }

    fn change_params(path: &Path, version: i32, text: &str) -> DidChangeParams {
        DidChangeParams::decode(Some(
            parse(&document_json(path, version, text, false)).unwrap(),
        ))
        .unwrap()
    }

    fn document_json(path: &Path, version: i32, text: &str, open: bool) -> String {
        let escaped = text
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n");
        if open {
            format!(
                concat!(
                    "{{\"textDocument\":{{\"uri\":\"file://{}\",",
                    "\"languageId\":\"nocter\",\"version\":{},\"text\":\"{}\"}}}}"
                ),
                path.display(),
                version,
                escaped
            )
        } else {
            format!(
                concat!(
                    "{{\"textDocument\":{{\"uri\":\"file://{}\",\"version\":{}}},",
                    "\"contentChanges\":[{{\"text\":\"{}\"}}]}}"
                ),
                path.display(),
                version,
                escaped
            )
        }
    }

    struct TemporaryDirectory(PathBuf);

    impl TemporaryDirectory {
        fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "nocter-language-server-diagnostics-{}-{id}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
