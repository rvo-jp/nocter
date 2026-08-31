use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use nocter_diagnostics::SourceDiagnostic;
use nocter_json::{Member, Value};
use nocter_lsp::{DocumentUri, DocumentUriError, render_notification};
use nocter_source::{CoordinateError, SourceMap, Utf16Range};

use crate::{WorkspaceAnalysisBatch, WorkspaceAnalysisGeneration};

/// Stateful projection of one complete workspace diagnostic snapshot into URI-global LSP state.
#[derive(Debug, Default)]
pub struct DiagnosticPublisher {
    published: BTreeMap<DocumentUri, ProjectedDocument>,
}

impl DiagnosticPublisher {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Atomically projects the complete active state after one workspace transition.
    ///
    /// A physical source may contribute diagnostics to several package scopes. Equal diagnostics
    /// are deduplicated and distinct diagnostics are merged in canonical order before the URI is
    /// compared with its previously published state. A scope leaving the workspace therefore
    /// cannot clear a diagnostic still owned by another active scope.
    ///
    /// # Errors
    ///
    /// Returns a source identity, UTF-16 coordinate, or local URI projection failure.
    pub fn publish(
        &mut self,
        batch: &WorkspaceAnalysisBatch,
    ) -> Result<Box<[String]>, DiagnosticPublicationError> {
        let current = project_workspace(batch.current_generations())?;
        let mut notifications = Vec::new();
        for uri in self.published.keys() {
            if current.contains_key(uri) {
                continue;
            }
            let path = uri.file_path().map_err(DiagnosticPublicationError::Uri)?;
            let version = batch
                .primary()
                .source_overlay()
                .document(&path)
                .map(|document| document.version().get());
            notifications.push(publication(uri, version, Vec::new()));
        }
        for (uri, document) in &current {
            if self.published.get(uri) != Some(document) {
                notifications.push(publication(
                    uri,
                    document.version,
                    document.diagnostics.clone(),
                ));
            }
        }

        let mut shown = BTreeSet::new();
        for analysis in batch.updated_generations() {
            if let Some(error) = analysis.preparation_failure()
                && analysis
                    .diagnostics()
                    .map_err(DiagnosticPublicationError::Workspace)?
                    .is_empty()
            {
                let message = format!("error[{}]: {error}", error.diagnostic_code());
                if shown.insert(message.clone()) {
                    notifications.push(render_notification(
                        "window/showMessage",
                        &object([
                            ("type", Value::Number("1".into())),
                            ("message", Value::String(message.into_boxed_str())),
                        ]),
                    ));
                }
            }
        }
        self.published = current;
        Ok(notifications.into_boxed_slice())
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ProjectedDocument {
    version: Option<i32>,
    diagnostics: Vec<Value>,
}

fn project_workspace<'a>(
    analyses: impl IntoIterator<Item = &'a WorkspaceAnalysisGeneration>,
) -> Result<BTreeMap<DocumentUri, ProjectedDocument>, DiagnosticPublicationError> {
    let mut workspace = BTreeMap::<DocumentUri, ProjectedDocument>::new();
    for analysis in analyses {
        for (uri, incoming) in project_analysis(analysis)? {
            let path = uri.file_path().map_err(DiagnosticPublicationError::Uri)?;
            let document = workspace.entry(uri).or_insert_with(|| ProjectedDocument {
                version: incoming.version,
                diagnostics: Vec::new(),
            });
            if document.version != incoming.version {
                return Err(DiagnosticPublicationError::InconsistentDocumentVersion(
                    path,
                ));
            }
            for diagnostic in incoming.diagnostics {
                if !document.diagnostics.contains(&diagnostic) {
                    document.diagnostics.push(diagnostic);
                }
            }
        }
    }
    for document in workspace.values_mut() {
        document.diagnostics.sort_by_cached_key(rendered_value);
    }
    Ok(workspace)
}

fn rendered_value(value: &Value) -> String {
    let mut rendered = String::new();
    nocter_json::write_value(&mut rendered, value);
    rendered
}

fn project_analysis(
    analysis: &WorkspaceAnalysisGeneration,
) -> Result<BTreeMap<DocumentUri, ProjectedDocument>, DiagnosticPublicationError> {
    let mut projected = BTreeMap::new();
    if let Some(sources) = analysis.reached_sources() {
        for diagnostic in analysis
            .diagnostics()
            .map_err(DiagnosticPublicationError::Workspace)?
        {
            let (uri, path, value) = project_diagnostic(diagnostic, sources)?;
            let version = analysis
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
    Workspace(crate::WorkspaceDiagnosticError),
    InconsistentDocumentVersion(PathBuf),
}

impl fmt::Display for DiagnosticPublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSource(source) => {
                write!(formatter, "diagnostic refers to missing source {source}")
            }
            Self::Coordinate(error) => error.fmt(formatter),
            Self::Uri(error) => error.fmt(formatter),
            Self::Workspace(error) => error.fmt(formatter),
            Self::InconsistentDocumentVersion(path) => write!(
                formatter,
                "diagnostics disagree on open-document version for {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for DiagnosticPublicationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Coordinate(error) => Some(error),
            Self::Uri(error) => Some(error),
            Self::Workspace(error) => Some(error),
            Self::MissingSource(_) | Self::InconsistentDocumentVersion(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use nocter_json::parse;
    use nocter_lsp::{DidChangeParams, DidCloseParams, DidOpenParams, InitializeParams};
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
        let failed = analyses.analyze(accepted).unwrap();
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
        let complete = analyses.analyze(changed).unwrap();
        let cleared = publisher.publish(&complete).unwrap();

        assert_eq!(cleared.len(), 1);
        assert!(cleared[0].contains("\"diagnostics\":[]"));
        assert!(cleared[0].contains("\"version\":2"));
    }

    #[test]
    fn shared_diagnostics_survive_one_scope_leaving_and_publish_once_per_uri() {
        let temporary = TemporaryDirectory::new();
        let first_root = temporary.path().join("first");
        let second_root = temporary.path().join("second");
        fs::create_dir_all(&first_root).unwrap();
        fs::create_dir_all(&second_root).unwrap();
        let first = first_root.join("index.nct");
        let second = second_root.join("index.nct");
        let first_text = "#package: { name: \"first\", version: \"0.0.0\", }\nuse std/fs\n";
        let second_text = "#package: { name: \"second\", version: \"0.0.0\", }\nuse std/fs\n";
        fs::write(&first, first_text).unwrap();
        fs::write(&second, second_text).unwrap();

        let standard_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std");
        let shared = fs::canonicalize(standard_root.join("fs/index.nct")).unwrap();
        let shared_uri = DocumentUri::from_file_path(&shared).unwrap();
        let mut documents = DocumentWorkspace::new();
        let mut analyses = WorkspaceAnalyses::new(configuration(temporary.path()));
        let mut publisher = DiagnosticPublisher::new();

        for (source, text) in [(&first, first_text), (&second, second_text)] {
            let revision = documents.open(&open_params(source, 1, text)).unwrap();
            let batch = analyses.analyze(revision).unwrap();
            publisher.publish(&batch).unwrap();
        }
        let revision = documents
            .open(&open_params(&shared, 1, "func broken(: void\n"))
            .unwrap();
        let batch = analyses.analyze(revision).unwrap();
        let diagnostic_messages = publisher.publish(&batch).unwrap();
        assert_eq!(
            publications_for(&diagnostic_messages, &shared_uri, false),
            1,
            "one URI-global diagnostic set must merge every active scope"
        );

        let revision = documents.close(&close_params(&first)).unwrap();
        let batch = analyses.analyze(revision).unwrap();
        let after_first_close = publisher.publish(&batch).unwrap();
        assert_eq!(publications_for(&after_first_close, &shared_uri, false), 0);
        assert_eq!(
            publications_for(&after_first_close, &shared_uri, true),
            0,
            "a leaving scope cannot clear another active scope's diagnostic"
        );

        let revision = documents.close(&close_params(&shared)).unwrap();
        let batch = analyses.analyze(revision).unwrap();
        let after_shared_close = publisher.publish(&batch).unwrap();
        assert_eq!(
            publications_for(&after_shared_close, &shared_uri, true),
            1,
            "the URI is cleared once after the final diagnostic contribution disappears"
        );
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
        let failed = analyses.analyze(accepted).unwrap();

        let emitted = DiagnosticPublisher::new().publish(&failed).unwrap();

        assert_eq!(emitted.len(), 1);
        assert!(emitted[0].contains("textDocument/publishDiagnostics"));
        assert!(emitted[0].contains("\"code\":\"E0800\""));
        assert!(emitted[0].contains("\"version\":1"));
        assert!(!emitted[0].contains("window/showMessage"));
        assert_eq!(failed.primary().reached_sources().unwrap().len(), 2);
    }

    fn configuration(root: &Path) -> WorkspaceConfiguration {
        let standard_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../std");
        let environment = LanguageServerEnvironment::new(
            root,
            LanguageServerToolchain::new(
                CompilationTarget::Arm64Darwin,
                root,
                StandardPackage::new(
                    PackageIdentity::new("toolchain:std"),
                    standard_root,
                    "0.22.0",
                ),
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
        crate::workspace::resolve_workspace_configuration(&environment, &params).unwrap()
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

    fn close_params(path: &Path) -> DidCloseParams {
        DidCloseParams::decode(Some(
            parse(&format!(
                "{{\"textDocument\":{{\"uri\":\"file://{}\"}}}}",
                path.display()
            ))
            .unwrap(),
        ))
        .unwrap()
    }

    fn publications_for(messages: &[String], uri: &DocumentUri, empty: bool) -> usize {
        let diagnostic_shape = if empty {
            "\"diagnostics\":[]"
        } else {
            "\"diagnostics\":[{"
        };
        messages
            .iter()
            .filter(|message| message.contains(uri.as_str()) && message.contains(diagnostic_shape))
            .count()
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
