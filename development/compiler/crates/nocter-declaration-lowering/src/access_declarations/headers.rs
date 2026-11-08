//! Syntax-only header extraction. Type/name meaning is resolved by the declaration owner.
use super::{Failure, text};
use nocter_model::ValueInputOperation;
use nocter_syntax::{
    BoundSyntax, ContextualSpelling, NodeId, NodeKind, SyntaxToken, child_node_iter,
    declaration_name_token, direct_identifier, direct_node, direct_node_iter, first_direct_token,
};

#[derive(Clone, Debug)]
pub(super) struct ParameterHeader {
    pub name: SyntaxToken,
    pub data: SyntaxToken,
    pub ty: NodeId,
    pub operation: ValueInputOperation,
}
#[derive(Clone, Debug)]
pub(super) struct Header {
    pub name: SyntaxToken,
    pub parameters: Vec<ParameterHeader>,
    pub result_syntax: NodeId,
    pub result_data: SyntaxToken,
    pub body: NodeId,
}
#[derive(Clone, Debug)]
pub(super) struct RecordHeader {
    pub name: SyntaxToken,
    pub copy: bool,
    pub fields: Vec<(SyntaxToken, SyntaxToken, NodeId)>,
}

pub(super) fn function(syntax: BoundSyntax<'_>, declaration: NodeId) -> Result<Header, Failure> {
    let tree = syntax.tree();
    let name = declaration_name_token(tree, declaration).ok_or(Failure::InconsistentSyntax)?;
    let tail = direct_node(tree, declaration, NodeKind::CallableTail)
        .ok_or(Failure::InconsistentSyntax)?;
    let parameters_node =
        direct_node(tree, tail, NodeKind::Parameters).ok_or(Failure::InconsistentSyntax)?;
    let parameters = direct_node_iter(tree, parameters_node, NodeKind::Parameter)
        .map(|parameter| {
            let name = direct_identifier(tree, parameter).ok_or(Failure::InconsistentSyntax)?;
            let ty =
                direct_node(tree, parameter, NodeKind::Type).ok_or(Failure::InconsistentSyntax)?;
            let operation = super::input_operation(syntax, parameter)?;
            Ok(ParameterHeader {
                name,
                data: type_token(syntax, ty)?,
                ty,
                operation,
            })
        })
        .collect::<Result<_, _>>()?;
    let result_syntax =
        direct_node(tree, tail, NodeKind::Type).ok_or(Failure::InconsistentSyntax)?;
    Ok(Header {
        name,
        parameters,
        result_syntax,
        result_data: type_token(syntax, result_syntax)?,
        body: direct_node(tree, declaration, NodeKind::Block).ok_or(Failure::InconsistentSyntax)?,
    })
}
pub(super) fn record(
    syntax: BoundSyntax<'_>,
    declaration: NodeId,
) -> Result<RecordHeader, Failure> {
    let tree = syntax.tree();
    let name = declaration_name_token(tree, declaration).ok_or(Failure::InconsistentSyntax)?;
    let first = first_direct_token(tree, declaration).ok_or(Failure::InconsistentSyntax)?;
    let copy =
        ContextualSpelling::from_spelling(text(syntax, first)) == Some(ContextualSpelling::Copy);
    let fields = direct_node_iter(tree, declaration, NodeKind::StructField)
        .map(|field| {
            let name = direct_identifier(tree, field).ok_or(Failure::InconsistentSyntax)?;
            let ty = direct_node(tree, field, NodeKind::Type).ok_or(Failure::InconsistentSyntax)?;
            Ok((name, type_token(syntax, ty)?, ty))
        })
        .collect::<Result<_, _>>()?;
    Ok(RecordHeader { name, copy, fields })
}
fn type_token(syntax: BoundSyntax<'_>, ty: NodeId) -> Result<SyntaxToken, Failure> {
    let named =
        direct_node(syntax.tree(), ty, NodeKind::NamedType).ok_or(Failure::InconsistentSyntax)?;
    first_direct_token(syntax.tree(), named).ok_or(Failure::InconsistentSyntax)
}

pub(super) fn collect(
    syntax: BoundSyntax<'_>,
) -> Result<(Vec<Header>, Vec<RecordHeader>), Failure> {
    let tree = syntax.tree();
    let mut functions = Vec::new();
    let mut records = Vec::new();
    for item in child_node_iter(tree, tree.root_id()) {
        if tree.node(item).is_none_or(|n| n.kind() != NodeKind::Item) {
            return Err(Failure::InconsistentSyntax);
        }
        let children = child_node_iter(tree, item).collect::<Vec<_>>();
        let [declaration] = children.as_slice() else {
            return Err(Failure::InconsistentSyntax);
        };
        match tree.node(*declaration).map(nocter_syntax::SyntaxNode::kind) {
            Some(NodeKind::FunctionDeclaration) => {
                functions.push(function(syntax, *declaration)?);
            }
            Some(NodeKind::StructDeclaration) => {
                records.push(record(syntax, *declaration)?);
            }
            _ => return Err(Failure::InconsistentSyntax),
        }
    }
    functions.sort_by_key(|h| text(syntax, h.name));
    records.sort_by_key(|h| text(syntax, h.name));
    Ok((functions, records))
}
