use std::fmt;

use nocter_source::{ByteOffset, SourceFile, SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceRole};
use nocter_syntax::is_valid_name;

use crate::AnalysisSnapshot;

/// One exact source replacement belonging to a semantic rename transaction.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticRenameEdit {
    entity: SemanticEntity,
    source: SourceId,
    range: TextRange,
}

impl SemanticRenameEdit {
    #[must_use]
    pub const fn entity(self) -> SemanticEntity {
        self.entity
    }

    #[must_use]
    pub const fn source(self) -> SourceId {
        self.source
    }

    #[must_use]
    pub const fn range(self) -> TextRange {
        self.range
    }
}

/// Compiler-owned, all-or-nothing rename intent before protocol projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticRenamePlan {
    entities: Box<[SemanticEntity]>,
    replacement: Box<str>,
    edits: Box<[SemanticRenameEdit]>,
}

impl SemanticRenamePlan {
    #[must_use]
    pub const fn entities(&self) -> &[SemanticEntity] {
        &self.entities
    }

    #[must_use]
    pub const fn replacement(&self) -> &str {
        &self.replacement
    }

    #[must_use]
    pub const fn edits(&self) -> &[SemanticRenameEdit] {
        &self.edits
    }
}

impl AnalysisSnapshot {
    /// Plans replacement of every reached occurrence of one exact, workspace-owned identity.
    ///
    /// `Ok(None)` means the position has no renameable identifier. A plan is rejected in full if
    /// any occurrence belongs to a dependency or the standard package, or if source projections
    /// disagree on the authored spelling.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid replacement, a non-owned occurrence, or an inconsistent
    /// compiler source projection.
    pub fn semantic_rename(
        &self,
        source: SourceId,
        offset: ByteOffset,
        replacement: &str,
    ) -> Result<Option<SemanticRenamePlan>, SemanticRenameError> {
        if !is_valid_name(replacement) {
            return Err(SemanticRenameError::InvalidReplacement(replacement.into()));
        }
        let Some(query) = self.semantic_query()? else {
            return Ok(None);
        };
        let Some(authority) = query.complete() else {
            return Ok(None);
        };
        let Some(selection) =
            crate::query::semantic_selection_from(authority.source_index(), source, offset)
        else {
            return Ok(None);
        };
        if !renameable_entity(selection.entity()) {
            return Ok(None);
        }
        let selected_source = self.require_source(source)?;
        let Some(spelling) = selected_source.text_at(selection.range()) else {
            return Err(SemanticRenameError::InvalidRange {
                source,
                range: selection.range(),
            });
        };
        if !is_valid_name(spelling) {
            return Ok(None);
        }
        let index = authority.source_index();
        let entities = authority.rename_family(selection.entity());
        let mut edits = Vec::new();
        for entity in &entities {
            for binding in index.bindings_for(*entity) {
                if !matches!(
                    binding.role(),
                    SourceRole::Declaration | SourceRole::Implementation | SourceRole::Reference
                ) {
                    continue;
                }
                let occurrence_source = binding.origin().source();
                if !self.source_is_root_owned(occurrence_source) {
                    return Err(SemanticRenameError::ReadOnlyOccurrence(occurrence_source));
                }
                let occurrence = self.require_source(occurrence_source)?;
                let range = binding.origin().span().range();
                let Some(occurrence_spelling) = occurrence.text_at(range) else {
                    return Err(SemanticRenameError::InvalidRange {
                        source: occurrence_source,
                        range,
                    });
                };
                if occurrence_spelling != spelling {
                    return Err(SemanticRenameError::InconsistentSpelling {
                        source: occurrence_source,
                        range,
                    });
                }
                edits.push(SemanticRenameEdit {
                    entity: *entity,
                    source: occurrence_source,
                    range,
                });
            }
        }
        edits.sort_unstable();
        edits.dedup();
        if edits.is_empty() {
            return Ok(None);
        }
        Ok(Some(SemanticRenamePlan {
            entities: entities.into_iter().collect(),
            replacement: replacement.into(),
            edits: edits.into_boxed_slice(),
        }))
    }

    /// Confirms that an edited checked snapshot preserves the selected identity at every edit.
    ///
    /// This is the collision gate: a syntactically valid replacement is accepted only when normal
    /// discovery, lowering, name resolution, and checking rebuild the same semantic bindings.
    #[must_use]
    pub fn validates_rename_candidate(&self, plan: &SemanticRenamePlan, candidate: &Self) -> bool {
        if !candidate.has_checked_semantics() {
            return false;
        }
        let Ok(replacement_len_u32) = u32::try_from(plan.replacement.len()) else {
            return false;
        };
        let replacement_len = i64::from(replacement_len_u32);
        let mut previous_source = None;
        let mut displacement = 0_i64;
        let mut previous_end = ByteOffset::new(0);
        for edit in plan.edits() {
            let Some(original) = self.sources().get(edit.source()) else {
                return false;
            };
            if previous_source != Some(edit.source()) {
                previous_source = Some(edit.source());
                displacement = 0;
            } else if edit.range().start() < previous_end {
                return false;
            }
            let Some(candidate_source) = candidate.sources().find_by_name(original.name().as_str())
            else {
                return false;
            };
            let start = i64::from(edit.range().start().get()) + displacement;
            let Ok(start) = u32::try_from(start) else {
                return false;
            };
            let Some(end) = start.checked_add(replacement_len_u32) else {
                return false;
            };
            let candidate_range = TextRange::new(ByteOffset::new(start), ByteOffset::new(end));
            let Ok(Some(selection)) =
                candidate.semantic_selection(candidate_source.id(), candidate_range.start())
            else {
                return false;
            };
            if selection.entity() != edit.entity() || selection.range() != candidate_range {
                return false;
            }
            displacement +=
                replacement_len - i64::from(edit.range().end().get() - edit.range().start().get());
            previous_end = edit.range().end();
        }
        true
    }

    fn require_source(&self, source: SourceId) -> Result<&SourceFile, SemanticRenameError> {
        self.sources()
            .get(source)
            .ok_or(SemanticRenameError::MissingSource(source))
    }

    fn source_is_root_owned(&self, source: SourceId) -> bool {
        let Some(file) = self.sources().get(source) else {
            return false;
        };
        let Some(unit) = self.current_unit() else {
            return false;
        };
        unit.is_root_package_source(file.name().as_str())
    }
}

const fn renameable_entity(entity: SemanticEntity) -> bool {
    matches!(
        entity,
        SemanticEntity::NominalType(_)
            | SemanticEntity::TypeAlias(_)
            | SemanticEntity::Interface(_)
            | SemanticEntity::AssociatedType(_)
            | SemanticEntity::Callable(_)
            | SemanticEntity::Constant(_)
            | SemanticEntity::Field(_)
            | SemanticEntity::Variant(_)
            | SemanticEntity::GenericParameter(_)
            | SemanticEntity::Parameter(_)
            | SemanticEntity::Test(_)
            | SemanticEntity::LocalBinding(..)
            | SemanticEntity::Capture(..)
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticRenameError {
    Evidence(crate::EvidenceIntegrityError),
    InvalidReplacement(Box<str>),
    MissingSource(SourceId),
    InvalidRange { source: SourceId, range: TextRange },
    InconsistentSpelling { source: SourceId, range: TextRange },
    ReadOnlyOccurrence(SourceId),
}

impl fmt::Display for SemanticRenameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Evidence(error) => error.fmt(formatter),
            Self::InvalidReplacement(name) => {
                write!(formatter, "rename replacement is not a Nocter name: {name}")
            }
            Self::MissingSource(source) => write!(formatter, "rename source is missing: {source}"),
            Self::InvalidRange { source, range } => write!(
                formatter,
                "rename range is invalid in {source}: {}..{}",
                range.start().get(),
                range.end().get()
            ),
            Self::InconsistentSpelling { source, range } => write!(
                formatter,
                "semantic identity has an inconsistent rename spelling in {source} at {}..{}",
                range.start().get(),
                range.end().get()
            ),
            Self::ReadOnlyOccurrence(source) => {
                write!(
                    formatter,
                    "rename would edit a dependency or standard source: {source}"
                )
            }
        }
    }
}

impl std::error::Error for SemanticRenameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Evidence(error) => Some(error),
            Self::InvalidReplacement(_)
            | Self::MissingSource(_)
            | Self::InvalidRange { .. }
            | Self::InconsistentSpelling { .. }
            | Self::ReadOnlyOccurrence(_) => None,
        }
    }
}

impl From<crate::EvidenceIntegrityError> for SemanticRenameError {
    fn from(error: crate::EvidenceIntegrityError) -> Self {
        Self::Evidence(error)
    }
}
