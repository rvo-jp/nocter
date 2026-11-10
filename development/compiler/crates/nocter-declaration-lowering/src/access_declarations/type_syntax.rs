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
    pub inputs: Box<[(ValueInputOperation, DataTypeSyntax)]>,
    pub result: DataTypeSyntax,
}
#[derive(Clone, Debug)]
pub enum ValueTypeSyntax {
    Data(DataTypeSyntax),
    Function(FunctionTypeSyntax),
}
#[derive(Clone, Debug)]
pub enum DataTypeSyntax {
    Named(SyntaxToken),
    Tuple(Box<[DataTypeSyntax]>),
    Array {
        element: Box<DataTypeSyntax>,
        length: NodeId,
    },
}
impl DataTypeSyntax {
    pub(super) fn tokens(&self) -> Vec<SyntaxToken> {
        match self {
            Self::Named(token) => vec![*token],
            Self::Tuple(fields) => fields.iter().flat_map(Self::tokens).collect(),
            Self::Array { element, .. } => element.tokens(),
        }
    }
}
impl ValueTypeSyntax {
    pub(super) fn tokens(&self) -> Vec<SyntaxToken> {
        match self {
            Self::Data(data) => data.tokens(),
            Self::Function(contract) => contract
                .inputs
                .iter()
                .flat_map(|(_, data)| data.tokens())
                .chain(contract.result.tokens())
                .collect(),
        }
    }
}
/// Project complete private type syntax without resolving a name or interpreting body use.
/// # Errors
/// Rejects a product inconsistent with the bounded grammar.
pub fn value_type_syntax(syntax: BoundSyntax<'_>, ty: NodeId) -> Result<ValueTypeSyntax, Failure> {
    let Some(contract) = direct_node(syntax.tree(), ty, NodeKind::CallableType) else {
        return data_syntax(syntax, ty).map(ValueTypeSyntax::Data);
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
            Ok((
                input_operation(syntax, parameter)?,
                data_syntax(syntax, ty)?,
            ))
        })
        .collect::<Result<_, Failure>>()?;
    let result =
        direct_node(syntax.tree(), contract, NodeKind::Type).ok_or(Failure::InconsistentSyntax)?;
    Ok(ValueTypeSyntax::Function(FunctionTypeSyntax {
        invocation,
        inputs,
        result: data_syntax(syntax, result)?,
    }))
}
pub(super) fn data_syntax(syntax: BoundSyntax<'_>, ty: NodeId) -> Result<DataTypeSyntax, Failure> {
    let shape = child_node_iter(syntax.tree(), ty)
        .next()
        .ok_or(Failure::InconsistentSyntax)?;
    match syntax
        .tree()
        .node(shape)
        .ok_or(Failure::InconsistentSyntax)?
        .kind()
    {
        NodeKind::NamedType => Ok(DataTypeSyntax::Named(
            first_direct_token(syntax.tree(), shape).ok_or(Failure::InconsistentSyntax)?,
        )),
        NodeKind::GroupedType => data_syntax(
            syntax,
            direct_node(syntax.tree(), shape, NodeKind::Type).ok_or(Failure::InconsistentSyntax)?,
        ),
        NodeKind::FixedArrayType => Ok(DataTypeSyntax::Array {
            element: Box::new(data_syntax(
                syntax,
                direct_node(syntax.tree(), shape, NodeKind::Type)
                    .ok_or(Failure::InconsistentSyntax)?,
            )?),
            length: direct_node(syntax.tree(), shape, NodeKind::Expression)
                .ok_or(Failure::InconsistentSyntax)?,
        }),
        NodeKind::TupleType => {
            let fields = child_node_iter(syntax.tree(), shape)
                .map(|field| data_syntax(syntax, field))
                .collect::<Result<Box<[_]>, _>>()?;
            if fields.len() < 2 {
                return Err(Failure::InconsistentSyntax);
            }
            Ok(DataTypeSyntax::Tuple(fields))
        }
        _ => Err(Failure::InconsistentSyntax),
    }
}
