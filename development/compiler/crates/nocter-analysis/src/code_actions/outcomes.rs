use std::fmt;

use nocter_declarations::BodyOwner;
use nocter_source::{SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceRole};

use super::SemanticCodeAction;
use crate::callable_source::project_callable_source;
use crate::presentation::recovery_type_presentation;
use crate::{AnalysisSnapshot, SemanticSourceEdit};

#[derive(Debug)]
pub enum OutcomeActionError {
    Evidence(crate::EvidenceIntegrityError),
    MissingBody(nocter_model::BodyId),
    MissingCallable(nocter_model::CallableId),
    MissingDeclarationSite(nocter_model::DeclarationSiteId),
    MissingSourceBinding(nocter_model::CallableId),
    MissingSyntax(SourceId),
    InvalidCallableSource,
    MissingResultType(nocter_model::CallableId),
    UnrenderableResult(nocter_model::TypeId),
}

impl fmt::Display for OutcomeActionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Evidence(error) => error.fmt(formatter),
            Self::MissingBody(id) => write!(formatter, "missing interrupted body {id:?}"),
            Self::MissingCallable(id) => write!(formatter, "missing interrupted callable {id:?}"),
            Self::MissingDeclarationSite(id) => {
                write!(formatter, "missing callable declaration site {id:?}")
            }
            Self::MissingSourceBinding(id) => {
                write!(formatter, "missing source binding for callable {id:?}")
            }
            Self::MissingSyntax(source) => {
                write!(
                    formatter,
                    "missing syntax tree for callable source {source}"
                )
            }
            Self::InvalidCallableSource => {
                formatter.write_str("callable source binding does not identify its declaration")
            }
            Self::MissingResultType(id) => {
                write!(formatter, "callable {id:?} has no result type syntax")
            }
            Self::UnrenderableResult(ty) => {
                write!(formatter, "cannot render proposed callable result {ty:?}")
            }
        }
    }
}

impl std::error::Error for OutcomeActionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Evidence(error) => Some(error),
            Self::MissingBody(_)
            | Self::MissingCallable(_)
            | Self::MissingDeclarationSite(_)
            | Self::MissingSourceBinding(_)
            | Self::MissingSyntax(_)
            | Self::InvalidCallableSource
            | Self::MissingResultType(_)
            | Self::UnrenderableResult(_) => None,
        }
    }
}

impl From<crate::EvidenceIntegrityError> for OutcomeActionError {
    fn from(error: crate::EvidenceIntegrityError) -> Self {
        Self::Evidence(error)
    }
}

pub(super) fn callable_contract_action(
    snapshot: &AnalysisSnapshot,
    requested_source: SourceId,
    diagnostic_code: &str,
    diagnostic_range: TextRange,
) -> Result<Option<SemanticCodeAction>, OutcomeActionError> {
    let Some(query) = snapshot.semantic_query()? else {
        return Ok(None);
    };
    let Some(recovery) = query.body_recovery() else {
        return Ok(None);
    };
    let Some(interruption) = recovery.interruption_overlapping(requested_source, diagnostic_range)
    else {
        return Ok(None);
    };
    let nocter_checking::TypedBodyInterruptionKind::OutcomeContract {
        layer,
        proposed_result,
    } = interruption.kind()
    else {
        return Ok(None);
    };
    let graph = recovery.prepared().graph();
    let body = graph
        .declarations()
        .bodies()
        .get(interruption.body())
        .ok_or(OutcomeActionError::MissingBody(interruption.body()))?;
    let BodyOwner::Callable(callable_id) = body.owner() else {
        return Ok(None);
    };
    let callable = graph
        .declarations()
        .callables()
        .get(callable_id)
        .ok_or(OutcomeActionError::MissingCallable(callable_id))?;
    let module = graph
        .declaration_sites()
        .get(callable.site())
        .map(|site| site.module())
        .ok_or(OutcomeActionError::MissingDeclarationSite(callable.site()))?;
    let binding = recovery
        .source_index()
        .bindings_for(SemanticEntity::Callable(callable_id))
        .iter()
        .find(|binding| {
            binding.role() == SourceRole::Declaration
                && binding.origin().source() == requested_source
        })
        .ok_or(OutcomeActionError::MissingSourceBinding(callable_id))?;
    let syntax = snapshot
        .syntax_trees()
        .iter()
        .find(|tree| tree.source() == requested_source)
        .ok_or(OutcomeActionError::MissingSyntax(requested_source))?;
    let projection = project_callable_source(syntax, binding.origin().syntax(), callable.kind())
        .ok_or(OutcomeActionError::InvalidCallableSource)?;
    let Some(result) = projection.editable_result() else {
        return Ok(None);
    };
    let result_range = syntax
        .node(result)
        .map(nocter_syntax::SyntaxNode::range)
        .ok_or(OutcomeActionError::MissingResultType(callable_id))?;
    let projection = recovery
        .interrupted_outcome_type(interruption)
        .transpose()
        .map_err(|_| OutcomeActionError::UnrenderableResult(*proposed_result))?
        .ok_or(OutcomeActionError::UnrenderableResult(*proposed_result))?;
    let spellings = snapshot.queries.module_spellings(graph, module);
    let presentation = recovery_type_presentation(projection, graph, &spellings)
        .ok_or(OutcomeActionError::UnrenderableResult(*proposed_result))?;
    let layer_name = match layer {
        nocter_checking::OutcomeLayer::Optional => "optional",
        nocter_checking::OutcomeLayer::Fallible => "fallible",
    };
    Ok(Some(SemanticCodeAction {
        title: format!(
            "Make callable result {layer_name}: `{}`",
            presentation.code()
        )
        .into(),
        diagnostic_code: diagnostic_code.into(),
        diagnostic_range,
        edits: Box::new([SemanticSourceEdit::new(
            requested_source,
            result_range,
            presentation.code(),
        )]),
    }))
}
