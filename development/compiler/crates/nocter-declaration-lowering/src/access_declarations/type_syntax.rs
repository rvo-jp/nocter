//! One syntax projection for structural callable parameters and local annotations.
use super::{Failure, input_operation, text};
use nocter_model::{ValueInputOperation, ValueInvocation};
use nocter_syntax::{
    BoundSyntax, ContextualSpelling, Keyword, NodeId, NodeKind, SyntaxToken, TokenKind,
    child_node_iter, direct_node, first_direct_token,
};

#[derive(Clone, Debug)]
pub struct FunctionTypeSyntax {
    pub invocation: ValueInvocation,
    pub inputs: Box<[(ValueInputOperation, SyntaxToken)]>,
    pub result: SyntaxToken,
}
#[derive(Clone, Debug)]
pub enum ParameterTypeSyntax {
    Data(SyntaxToken),
    Function(FunctionTypeSyntax),
}
impl ParameterTypeSyntax {
    pub(super) fn tokens(&self) -> Vec<SyntaxToken> {
        match self {
            Self::Data(token) => vec![*token],
            Self::Function(contract) => contract
                .inputs
                .iter()
                .map(|(_, token)| *token)
                .chain(std::iter::once(contract.result))
                .collect(),
        }
    }
}
/// Project complete private type syntax without resolving a name or interpreting body use.
/// # Errors
/// Rejects a product inconsistent with the bounded grammar.
pub fn parameter_type(syntax: BoundSyntax<'_>, ty: NodeId) -> Result<ParameterTypeSyntax, Failure> {
    let Some(contract) = direct_node(syntax.tree(), ty, NodeKind::CallableType) else {
        return data_token(syntax, ty).map(ParameterTypeSyntax::Data);
    };
    let token = first_direct_token(syntax.tree(), contract).ok_or(Failure::InconsistentSyntax)?;
    let invocation = match (
        token.kind(),
        ContextualSpelling::from_spelling(text(syntax, token)),
    ) {
        (TokenKind::Keyword(Keyword::Func), _) => ValueInvocation::Read,
        (_, Some(ContextualSpelling::Mut)) => ValueInvocation::Edit,
        (_, Some(ContextualSpelling::Once)) => ValueInvocation::Take,
        _ => return Err(Failure::InconsistentSyntax),
    };
    let parameters = direct_node(syntax.tree(), contract, NodeKind::CallableParameters)
        .ok_or(Failure::InconsistentSyntax)?;
    let inputs = child_node_iter(syntax.tree(), parameters)
        .map(|parameter| {
            let ty = direct_node(syntax.tree(), parameter, NodeKind::Type)
                .ok_or(Failure::InconsistentSyntax)?;
            Ok((input_operation(syntax, parameter)?, data_token(syntax, ty)?))
        })
        .collect::<Result<_, Failure>>()?;
    let result =
        direct_node(syntax.tree(), contract, NodeKind::Type).ok_or(Failure::InconsistentSyntax)?;
    Ok(ParameterTypeSyntax::Function(FunctionTypeSyntax {
        invocation,
        inputs,
        result: data_token(syntax, result)?,
    }))
}
pub(super) fn data_token(syntax: BoundSyntax<'_>, ty: NodeId) -> Result<SyntaxToken, Failure> {
    let named =
        direct_node(syntax.tree(), ty, NodeKind::NamedType).ok_or(Failure::InconsistentSyntax)?;
    first_direct_token(syntax.tree(), named).ok_or(Failure::InconsistentSyntax)
}
