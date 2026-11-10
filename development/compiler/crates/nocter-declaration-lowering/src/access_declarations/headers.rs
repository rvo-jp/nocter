//! Syntax-only header extraction. Type/name meaning is resolved by the declaration owner.
use super::{
    Failure, ValueTypeSyntax, text,
    type_syntax::{DataTypeSyntax, data_syntax},
    value_type_syntax,
};
use nocter_declarations::value_access::CallableKind;
use nocter_model::{CompileTimeGuarantee, SymbolTable, ValueInputOperation};
use nocter_syntax::{
    BoundSyntax, ContextualSpelling, NodeId, NodeKind, SyntaxToken, child_node_iter,
    declaration_name_token, direct_identifier, direct_node, direct_node_iter, first_direct_token,
};

#[derive(Clone, Debug)]
pub(super) struct ParameterHeader {
    pub name: SyntaxToken,
    pub data: ValueTypeSyntax,
    pub ty: NodeId,
    pub operation: ValueInputOperation,
}
#[derive(Clone, Debug)]
pub(super) struct Header {
    pub name: SyntaxToken,
    pub kind: CallableKind,
    pub parameters: Vec<ParameterHeader>,
    pub result_syntax: NodeId,
    pub result_data: ValueTypeSyntax,
    pub body: NodeId,
}
#[derive(Clone, Debug)]
pub(super) struct RecordHeader {
    pub name: SyntaxToken,
    pub copy: bool,
    pub fields: Vec<(SyntaxToken, DataTypeSyntax, NodeId)>,
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
                data: value_type_syntax(syntax, ty)?,
                ty,
                operation,
            })
        })
        .collect::<Result<_, _>>()?;
    let result_syntax =
        direct_node(tree, tail, NodeKind::Type).ok_or(Failure::InconsistentSyntax)?;
    let compile_time = if direct_node(tree, declaration, NodeKind::CompileTimeModifier).is_some() {
        CompileTimeGuarantee::Evaluatable
    } else {
        CompileTimeGuarantee::RuntimeOnly
    };
    Ok(Header {
        name,
        kind: CallableKind::Function(compile_time),
        parameters,
        result_syntax,
        result_data: value_type_syntax(syntax, result_syntax)?,
        body: direct_node(tree, declaration, NodeKind::Block).ok_or(Failure::InconsistentSyntax)?,
    })
}
pub(super) fn initializer(syntax: BoundSyntax<'_>, declaration: NodeId) -> Result<Header, Failure> {
    let tree = syntax.tree();
    let name = declaration_name_token(tree, declaration).ok_or(Failure::InconsistentSyntax)?;
    let result_syntax =
        direct_node(tree, declaration, NodeKind::Type).ok_or(Failure::InconsistentSyntax)?;
    Ok(Header {
        name,
        kind: if tree
            .node(declaration)
            .is_some_and(|node| node.kind() == NodeKind::StaticDeclaration)
        {
            CallableKind::StaticInitializer
        } else {
            CallableKind::ConstantInitializer
        },
        parameters: Vec::new(),
        result_syntax,
        result_data: value_type_syntax(syntax, result_syntax)?,
        body: direct_node(tree, declaration, NodeKind::Expression)
            .ok_or(Failure::InconsistentSyntax)?,
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
            Ok((name, data_syntax(syntax, ty)?, ty))
        })
        .collect::<Result<_, _>>()?;
    Ok(RecordHeader { name, copy, fields })
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
            Some(NodeKind::ConstantDeclaration | NodeKind::StaticDeclaration) => {
                functions.push(initializer(syntax, *declaration)?);
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

/// Header spellings only; initializer and callable bodies never participate in this symbol prefix.
pub(super) fn symbols(
    syntax: BoundSyntax<'_>,
    functions: &[Header],
    records: &[RecordHeader],
) -> SymbolTable {
    SymbolTable::from_spellings(
        functions
            .iter()
            .flat_map(|h| {
                std::iter::once(h.name).chain(h.result_data.tokens()).chain(
                    h.parameters
                        .iter()
                        .flat_map(|p| std::iter::once(p.name).chain(p.data.tokens())),
                )
            })
            .chain(records.iter().flat_map(|r| {
                std::iter::once(r.name).chain(
                    r.fields
                        .iter()
                        .flat_map(|(name, data, _)| std::iter::once(*name).chain(data.tokens())),
                )
            }))
            .map(|token| text(syntax, token)),
    )
}
