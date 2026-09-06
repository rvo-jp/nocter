use nocter_diagnostics::DiagnosticNote;
use nocter_model::{BodyId, BodyNodeId};

use crate::{
    BodyCheckError, BodyCheckInternalError, BodyRule, body_relations::BodyRelationCatalog,
};

/// Source-neutral failure produced by whole-program body-relation analysis.
///
/// Relation passes retain semantic body and node identities instead of current-generation source
/// spans. Projection joins those identities with the exact current [`BodyRelationCatalog`] only
/// after analysis has finished, so a reusable relation result can never retain stale editor
/// coordinates.
#[derive(Clone, Debug)]
pub(crate) enum BodyRelationError {
    Rule {
        body: BodyId,
        rule: BodyRule,
        primary: BodyNodeId,
        notes: Box<[BodyRelationNote]>,
    },
    Internal(BodyCheckInternalError),
}

#[derive(Clone, Debug)]
pub(crate) struct BodyRelationNote {
    message: &'static str,
    node: BodyNodeId,
}

impl BodyRelationNote {
    #[must_use]
    pub(crate) const fn new(message: &'static str, node: BodyNodeId) -> Self {
        Self { message, node }
    }
}

impl BodyRelationError {
    #[must_use]
    pub(crate) fn rule(
        body: BodyId,
        rule: BodyRule,
        primary: BodyNodeId,
        notes: impl Into<Box<[BodyRelationNote]>>,
    ) -> Self {
        Self::Rule {
            body,
            rule,
            primary,
            notes: notes.into(),
        }
    }

    pub(crate) fn project(self, catalog: &BodyRelationCatalog<'_, '_>) -> BodyCheckError {
        let (body, rule, primary, notes) = match self {
            Self::Rule {
                body,
                rule,
                primary,
                notes,
            } => (body, rule, primary, notes),
            Self::Internal(error) => return error.into(),
        };
        let projected = (|| {
            let input = catalog.get(body)?;
            let primary = input.origin(primary)?;
            let notes = notes
                .into_vec()
                .into_iter()
                .map(|note| {
                    input
                        .origin(note.node)
                        .map(|origin| DiagnosticNote::new(note.message, origin))
                })
                .collect::<Result<Vec<_>, BodyCheckInternalError>>()?;
            Ok::<_, BodyCheckInternalError>(BodyCheckError::from_rule(
                rule,
                rule.diagnostic_with_notes(primary, notes),
            ))
        })();
        projected.unwrap_or_else(BodyCheckError::from)
    }
}

impl From<BodyCheckInternalError> for BodyRelationError {
    fn from(error: BodyCheckInternalError) -> Self {
        Self::Internal(error)
    }
}
