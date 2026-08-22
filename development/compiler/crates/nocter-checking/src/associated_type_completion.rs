use std::collections::BTreeSet;
use std::fmt;

use nocter_declarations::DeclarationGraph;
use nocter_model::{AssociatedTypeId, Symbol};
use nocter_source_index::SourceOrigin;

use crate::{CheckedProgram, PreparedSemanticProgram};

/// One checked type-position context and its exact normalized requirement candidates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssociatedTypeCompletionContext {
    origin: SourceOrigin,
    candidates: Box<[AssociatedTypeId]>,
}

impl AssociatedTypeCompletionContext {
    #[must_use]
    pub(crate) fn new(
        origin: SourceOrigin,
        candidates: impl Into<Box<[AssociatedTypeId]>>,
    ) -> Self {
        Self {
            origin,
            candidates: candidates.into(),
        }
    }

    #[must_use]
    pub const fn origin(&self) -> SourceOrigin {
        self.origin
    }

    #[must_use]
    pub const fn candidates(&self) -> &[AssociatedTypeId] {
        &self.candidates
    }
}

/// One associated type proven available for an exact type-position base.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssociatedTypeCompletionCandidate {
    associated: AssociatedTypeId,
    name: Symbol,
}

impl AssociatedTypeCompletionCandidate {
    #[must_use]
    pub const fn associated(self) -> AssociatedTypeId {
        self.associated
    }

    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }
}

/// Inconsistency between retained type-position evidence and the declaration graph.
#[derive(Debug)]
pub enum AssociatedTypeCompletionError {
    MissingAssociatedType(AssociatedTypeId),
    DuplicateAssociatedType(AssociatedTypeId),
}

impl fmt::Display for AssociatedTypeCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingAssociatedType(associated) => {
                write!(
                    formatter,
                    "associated completion type {associated:?} is absent"
                )
            }
            Self::DuplicateAssociatedType(associated) => write!(
                formatter,
                "associated completion type {associated:?} appears more than once"
            ),
        }
    }
}

impl std::error::Error for AssociatedTypeCompletionError {}

impl PreparedSemanticProgram {
    /// Validates and renders the associated identities retained by body checking.
    ///
    /// # Errors
    ///
    /// Returns an error when retained identities are absent or duplicated.
    pub fn associated_type_completions(
        &self,
        associated: &[AssociatedTypeId],
    ) -> Result<Box<[AssociatedTypeCompletionCandidate]>, AssociatedTypeCompletionError> {
        select_associated_type_completions(self.graph(), associated)
    }
}

impl CheckedProgram {
    /// Validates associated candidates retained in a checked type-position context.
    ///
    /// # Errors
    ///
    /// Returns an error when retained identities are absent or duplicated.
    pub fn associated_type_completions(
        &self,
        associated: &[AssociatedTypeId],
    ) -> Result<Box<[AssociatedTypeCompletionCandidate]>, AssociatedTypeCompletionError> {
        select_associated_type_completions(self.graph(), associated)
    }
}

fn select_associated_type_completions(
    graph: &DeclarationGraph,
    associated: &[AssociatedTypeId],
) -> Result<Box<[AssociatedTypeCompletionCandidate]>, AssociatedTypeCompletionError> {
    let mut seen = BTreeSet::new();
    associated
        .iter()
        .copied()
        .map(|associated| {
            if !seen.insert(associated) {
                return Err(AssociatedTypeCompletionError::DuplicateAssociatedType(
                    associated,
                ));
            }
            let declaration = graph
                .declarations()
                .associated_types()
                .get(associated)
                .ok_or(AssociatedTypeCompletionError::MissingAssociatedType(
                    associated,
                ))?;
            Ok(AssociatedTypeCompletionCandidate {
                associated,
                name: declaration.name(),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}
