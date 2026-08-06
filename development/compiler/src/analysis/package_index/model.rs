use super::super::occurrences::{SemanticOccurrenceKind, SemanticOccurrenceRole};
use crate::lexer::is_valid_identifier_name;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct StableSourceIdentity {
    package: Option<String>,
    absolute_path: Option<PathBuf>,
    display_path: String,
}

impl StableSourceIdentity {
    pub(super) fn new(
        package: Option<String>,
        absolute_path: Option<PathBuf>,
        display_path: String,
    ) -> Self {
        Self {
            package,
            absolute_path,
            display_path,
        }
    }

    pub(crate) fn absolute_path(&self) -> Option<&Path> {
        self.absolute_path.as_deref()
    }

    pub(crate) fn display_path(&self) -> &str {
        &self.display_path
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct StableSourceSpan {
    pub(crate) source: StableSourceIdentity,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) enum StableIdentityKind {
    Declaration,
    Member,
    Local,
    GenericParameter,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct StableSemanticIdentity {
    pub(super) kind: StableIdentityKind,
    pub(super) declaration: StableSourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexedOccurrence {
    pub(crate) identity: StableSemanticIdentity,
    pub(crate) span: StableSourceSpan,
    pub(crate) role: SemanticOccurrenceRole,
    pub(crate) kind: SemanticOccurrenceKind,
}

#[derive(Debug, Clone)]
pub(crate) struct PackageSemanticIndex {
    generation: u64,
    sources: HashMap<StableSourceIdentity, Arc<str>>,
    occurrences: Vec<IndexedOccurrence>,
}

impl PackageSemanticIndex {
    pub(super) fn new(
        generation: u64,
        sources: HashMap<StableSourceIdentity, Arc<str>>,
        occurrences: Vec<IndexedOccurrence>,
    ) -> Self {
        Self {
            generation,
            sources,
            occurrences,
        }
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn source_text(&self, source: &StableSourceIdentity) -> Option<&str> {
        self.sources.get(source).map(AsRef::as_ref)
    }

    pub(crate) fn references(
        &self,
        identity: &StableSemanticIdentity,
        include_declaration: bool,
    ) -> impl Iterator<Item = &IndexedOccurrence> {
        self.occurrences.iter().filter(move |occurrence| {
            occurrence.identity == *identity
                && (include_declaration || occurrence.role != SemanticOccurrenceRole::Declaration)
        })
    }

    pub(crate) fn rename_plan(
        &self,
        identity: &StableSemanticIdentity,
        new_name: &str,
        editable_root: &Path,
    ) -> Option<RenamePlan> {
        if !is_valid_identifier_name(new_name) {
            return None;
        }
        if identity.kind == StableIdentityKind::Declaration
            && self.occurrences.iter().any(|occurrence| {
                occurrence.role == SemanticOccurrenceRole::Declaration
                    && occurrence.identity.kind == StableIdentityKind::Declaration
                    && occurrence.identity != *identity
                    && occurrence.span.source == identity.declaration.source
                    && self
                        .source_text(&occurrence.span.source)
                        .and_then(|text| text.get(occurrence.span.start..occurrence.span.end))
                        == Some(new_name)
            })
        {
            return None;
        }

        let mut edits = self
            .references(identity, true)
            .map(|occurrence| {
                let source = &occurrence.span.source;
                let absolute_path = source.absolute_path()?.to_path_buf();
                if !absolute_path.starts_with(editable_root) {
                    return None;
                }
                let text = self.source_text(source)?;
                let old_name = text
                    .get(occurrence.span.start..occurrence.span.end)?
                    .to_string();
                Some(RenameEdit {
                    absolute_path,
                    display_path: source.display_path().to_string(),
                    start: occurrence.span.start,
                    end: occurrence.span.end,
                    old_name,
                    new_name: new_name.to_string(),
                    source_text: self.sources.get(source)?.clone(),
                })
            })
            .collect::<Option<Vec<_>>>()?;
        edits.sort_by(|left, right| {
            (&left.absolute_path, left.start, left.end).cmp(&(
                &right.absolute_path,
                right.start,
                right.end,
            ))
        });
        edits.dedup_by(|left, right| {
            left.absolute_path == right.absolute_path
                && left.start == right.start
                && left.end == right.end
        });
        (!edits.is_empty()).then_some(RenamePlan { edits })
    }

    #[cfg(test)]
    pub(crate) fn occurrences(&self) -> &[IndexedOccurrence] {
        &self.occurrences
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenameEdit {
    pub(crate) absolute_path: PathBuf,
    pub(crate) display_path: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) old_name: String,
    pub(crate) new_name: String,
    pub(crate) source_text: Arc<str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenamePlan {
    pub(crate) edits: Vec<RenameEdit>,
}
