//! Syntax-only header extraction. Type/name meaning is resolved by the declaration owner.
use super::{
    Failure, TypeSyntax, project_type_syntax,
    source::{FieldSource, RecordSource},
    text,
};
use nocter_declarations::value_access::CodeKind;
use nocter_model::{CompileTimeGuarantee, SymbolTable, ValueInputOperation};
use nocter_syntax::{
    BoundSyntax, ContextualSpelling, NodeId, NodeKind, SyntaxToken, child_node_iter,
    declaration_name_token, direct_identifier, direct_node, direct_node_iter, first_direct_token,
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(super) struct ParameterHeader {
    pub name: SyntaxToken,
    pub data: TypeSyntax,
    pub ty: NodeId,
    pub operation: ValueInputOperation,
}
#[derive(Clone, Debug)]
pub(super) struct Header {
    pub name: SyntaxToken,
    pub kind: CodeKind,
    pub deferred_modifier: Option<NodeId>,
    pub parameters: Vec<ParameterHeader>,
    pub result_syntax: NodeId,
    pub result_data: TypeSyntax,
    pub body: Option<NodeId>,
}
#[derive(Clone, Debug)]
pub(super) struct RecordHeader {
    pub source: Arc<RecordSource>,
    pub copy: bool,
    pub fields: Box<[TypeSyntax]>,
}

pub(super) fn function(syntax: BoundSyntax<'_>, declaration: NodeId) -> Result<Header, Failure> {
    let tree = syntax.tree();
    let deferred_modifier = direct_node(tree, declaration, NodeKind::AsyncModifier);
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
                data: project_type_syntax(syntax, ty)?,
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
        kind: if is_primitive(syntax, declaration) {
            CodeKind::Primitive
        } else if deferred_modifier.is_some() {
            CodeKind::DeferredFunction
        } else {
            CodeKind::Function(compile_time)
        },
        deferred_modifier,
        parameters,
        result_syntax,
        result_data: project_type_syntax(syntax, result_syntax)?,
        body: direct_node(tree, declaration, NodeKind::Block),
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
            CodeKind::StaticInitializer
        } else {
            CodeKind::ConstantInitializer
        },
        deferred_modifier: None,
        parameters: Vec::new(),
        result_syntax,
        result_data: project_type_syntax(syntax, result_syntax)?,
        body: direct_node(tree, declaration, NodeKind::Expression),
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
            Ok((FieldSource { name, ty }, project_type_syntax(syntax, ty)?))
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    let (fields, types): (Vec<_>, Vec<_>) = fields.into_iter().unzip();
    Ok(RecordHeader {
        source: Arc::new(RecordSource {
            name,
            fields: fields.into(),
        }),
        copy,
        fields: types.into(),
    })
}

pub(super) fn collect(
    syntax: BoundSyntax<'_>,
) -> Result<(Vec<Header>, Vec<RecordHeader>), Failure> {
    let tree = syntax.tree();
    let mut functions = Vec::new();
    let mut records = Vec::new();
    for declaration in declarations(syntax)? {
        match tree.node(declaration).map(nocter_syntax::SyntaxNode::kind) {
            Some(NodeKind::FunctionDeclaration) => {
                functions.push(function(syntax, declaration)?);
            }
            Some(NodeKind::ConstantDeclaration | NodeKind::StaticDeclaration) => {
                functions.push(initializer(syntax, declaration)?);
            }
            Some(NodeKind::StructDeclaration) => {
                records.push(record(syntax, declaration)?);
            }
            // Test roots have a compiler-owned result and no authored type to reconcile.
            Some(NodeKind::TestDeclaration) => {}
            _ => return Err(Failure::InconsistentSyntax),
        }
    }
    functions.sort_by_key(|h| text(syntax, h.name));
    records.sort_by_key(|h| text(syntax, h.source.name));
    Ok((functions, records))
}

pub(super) fn is_primitive(syntax: BoundSyntax<'_>, declaration: NodeId) -> bool {
    nocter_syntax::direct_token_iter(syntax.tree(), declaration).any(|token| {
        token.kind() == nocter_syntax::TokenKind::Keyword(nocter_syntax::Keyword::Primitive)
    })
}

pub(super) fn declarations(syntax: BoundSyntax<'_>) -> Result<Vec<NodeId>, Failure> {
    let tree = syntax.tree();
    child_node_iter(tree, tree.root_id())
        .filter(|item| {
            !tree.node(*item).is_some_and(|node| {
                matches!(
                    node.kind(),
                    NodeKind::SourceVisibilityDeclaration
                        | NodeKind::UseDeclaration
                        | NodeKind::PackageDirective
                )
            })
        })
        .map(|item| {
            if tree.node(item).is_none_or(|n| n.kind() != NodeKind::Item) {
                return Err(Failure::InconsistentSyntax);
            }
            nocter_syntax::item_declaration(tree, item).ok_or(Failure::InconsistentSyntax)
        })
        .collect()
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
                std::iter::once(r.source.name).chain(
                    r.source
                        .fields
                        .iter()
                        .zip(&r.fields)
                        .flat_map(|(source, data)| {
                            std::iter::once(source.name).chain(data.tokens())
                        }),
                )
            }))
            .map(|token| text(syntax, token)),
    )
}
