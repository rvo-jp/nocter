use std::fmt;

use nocter_declarations::{BodyOwner, CallableKind};
use nocter_source::{SourceId, TextRange};
use nocter_source_index::{SemanticEntity, SourceRole, SyntaxOrigin};
use nocter_syntax::{NodeId, NodeKind, SyntaxElement, SyntaxTree};

use super::SemanticCodeAction;
use crate::presentation::recovery_type_presentation;
use crate::{AnalysisSnapshot, SemanticSourceEdit};

#[derive(Debug)]
pub enum OutcomeActionError {
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

impl std::error::Error for OutcomeActionError {}

pub(super) fn callable_contract_action(
    snapshot: &AnalysisSnapshot,
    requested_source: SourceId,
    diagnostic_code: &str,
    diagnostic_range: TextRange,
) -> Result<Option<SemanticCodeAction>, OutcomeActionError> {
    let Some(recovery) = snapshot.body_recovery() else {
        return Ok(None);
    };
    let Some(interruption) = recovery.interruption() else {
        return Ok(None);
    };
    let nocter_checking::TypedBodyInterruptionKind::OutcomeContract {
        layer,
        proposed_result,
    } = interruption.kind()
    else {
        return Ok(None);
    };
    if interruption.origin().source() != requested_source {
        return Ok(None);
    }
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
        .prepared()
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
    let declaration = callable_declaration(
        syntax,
        binding.origin().syntax(),
        interruption.origin().span().range(),
    )
    .ok_or(OutcomeActionError::InvalidCallableSource)?;
    let Some(result) = callable_result(syntax, declaration, callable_id, callable.kind())? else {
        return Ok(None);
    };
    let result_range = syntax
        .node(result)
        .map(nocter_syntax::SyntaxNode::range)
        .ok_or(OutcomeActionError::MissingResultType(callable_id))?;
    let presentation = recovery_type_presentation(recovery, *proposed_result, module)
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

fn callable_declaration(
    syntax: &SyntaxTree,
    binding: SyntaxOrigin,
    failure: TextRange,
) -> Option<NodeId> {
    let binding = match binding {
        SyntaxOrigin::Node(node) => syntax.node(node)?.range(),
        SyntaxOrigin::Token(token) => token.range(),
    };
    syntax
        .nodes()
        .filter(|(_, node)| {
            callable_declaration_kind(node.kind())
                && contains(node.range(), binding)
                && contains(node.range(), failure)
        })
        .min_by_key(|(_, node)| range_length(node.range()))
        .map(|(id, _)| id)
}

fn callable_result(
    syntax: &SyntaxTree,
    declaration: NodeId,
    callable: nocter_model::CallableId,
    kind: CallableKind,
) -> Result<Option<NodeId>, OutcomeActionError> {
    match kind {
        CallableKind::Function | CallableKind::ConstructionFunction | CallableKind::Literal(_) => {
            let tail = required_direct_node(syntax, declaration, NodeKind::CallableTail, callable)?;
            required_direct_node(syntax, tail, NodeKind::Type, callable).map(Some)
        }
        CallableKind::Method => {
            let signature =
                required_direct_node(syntax, declaration, NodeKind::MethodSignature, callable)?;
            let tail = required_direct_node(syntax, signature, NodeKind::CallableTail, callable)?;
            required_direct_node(syntax, tail, NodeKind::Type, callable).map(Some)
        }
        CallableKind::Coercion | CallableKind::Expansion => {
            required_direct_node(syntax, declaration, NodeKind::Type, callable).map(Some)
        }
        // These operator contracts have fixed or grammar-restricted result shapes. Changing them
        // is not a callable-result repair even when their implementation contains postfix `?`.
        CallableKind::Primitive
        | CallableKind::Equality
        | CallableKind::Ordering
        | CallableKind::Index => Ok(None),
    }
}

fn required_direct_node(
    syntax: &SyntaxTree,
    parent: NodeId,
    kind: NodeKind,
    callable: nocter_model::CallableId,
) -> Result<NodeId, OutcomeActionError> {
    direct_node(syntax, parent, kind).ok_or(OutcomeActionError::MissingResultType(callable))
}

fn direct_node(syntax: &SyntaxTree, parent: NodeId, kind: NodeKind) -> Option<NodeId> {
    syntax
        .children(parent)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Node(node)
                if syntax
                    .node(*node)
                    .is_some_and(|candidate| candidate.kind() == kind) =>
            {
                Some(*node)
            }
            SyntaxElement::Node(_) | SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
        })
}

const fn callable_declaration_kind(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::FunctionDeclaration
            | NodeKind::InterfaceMethod
            | NodeKind::ConstructionFunction
            | NodeKind::LiteralDeclaration
            | NodeKind::InherentMethod
            | NodeKind::CoercionDeclaration
            | NodeKind::EqualityOperator
            | NodeKind::OrderingOperator
            | NodeKind::IndexOperator
            | NodeKind::ExpansionOperator
            | NodeKind::ConformMethod
    )
}

const fn contains(outer: TextRange, inner: TextRange) -> bool {
    outer.start().get() <= inner.start().get() && inner.end().get() <= outer.end().get()
}

const fn range_length(range: TextRange) -> u32 {
    range.end().get() - range.start().get()
}
