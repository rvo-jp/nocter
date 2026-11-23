//! Syntax-only header extraction. Type/name meaning is resolved by the declaration owner.
use super::{
    Failure, TypeSyntax, project_type_syntax,
    source::{AliasSource, EnumSource, FieldSource, ParameterSource, RecordSource, VariantSource},
    text,
};
use nocter_declarations::value_access::{CodeKind, ParameterBinding};
use nocter_model::{CompileTimeGuarantee, SymbolTable, ValueInputOperation};
use nocter_syntax::{
    BoundSyntax, ContextualSpelling, NodeId, NodeKind, SyntaxOrigin, SyntaxToken, child_node_iter,
    declaration_name_token, direct_identifier, direct_node, direct_node_iter, first_direct_token,
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(super) struct ParameterHeader {
    pub source: ParameterSource<SyntaxOrigin>,
    pub data: TypeSyntax,
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
#[derive(Clone, Debug)]
pub(super) struct AliasHeader {
    pub source: AliasSource,
    pub target: TypeSyntax,
}

#[derive(Clone, Debug)]
pub(super) struct EnumHeader {
    pub source: Arc<EnumSource>,
    pub field_types: Box<[Box<[TypeSyntax]>]>,
    pub opaque: bool,
    pub copy: bool,
}

pub(super) fn enumeration(
    syntax: BoundSyntax<'_>,
    declaration: NodeId,
) -> Result<EnumHeader, Failure> {
    let tree = syntax.tree();
    let (name, copy) = nominal_name_and_copy(syntax, declaration)?;
    let opaque = nocter_syntax::direct_token(
        tree,
        declaration,
        nocter_syntax::TokenKind::Punctuation(nocter_syntax::Punctuation::LeftBrace),
    )
    .is_none();
    let variants = direct_node_iter(tree, declaration, NodeKind::EnumVariant)
        .map(|variant| {
            let name = direct_identifier(tree, variant).ok_or(Failure::InconsistentSyntax)?;
            let fields = direct_node(tree, variant, NodeKind::EnumPayload)
                .into_iter()
                .flat_map(|payload| direct_node_iter(tree, payload, NodeKind::Parameter));
            let fields = nominal_fields(syntax, fields)?;
            Ok((
                VariantSource {
                    name,
                    fields: fields.sources,
                },
                fields.types,
            ))
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    let (variants, field_types): (Vec<_>, Vec<_>) = variants.into_iter().unzip();
    Ok(EnumHeader {
        source: Arc::new(EnumSource {
            name,
            variants: variants.into(),
        }),
        field_types: field_types.into(),
        opaque,
        copy,
    })
}

pub(super) fn alias(syntax: BoundSyntax<'_>, declaration: NodeId) -> Result<AliasHeader, Failure> {
    let tree = syntax.tree();
    let name = declaration_name_token(tree, declaration).ok_or(Failure::InconsistentSyntax)?;
    let ty = direct_node(tree, declaration, NodeKind::Type).ok_or(Failure::InconsistentSyntax)?;
    Ok(AliasHeader {
        source: AliasSource { name, ty },
        target: project_type_syntax(syntax, ty)?,
    })
}

pub(super) fn parameter(syntax: BoundSyntax<'_>, node: NodeId) -> Result<ParameterHeader, Failure> {
    let ty = direct_node(syntax.tree(), node, NodeKind::Type).ok_or(Failure::InconsistentSyntax)?;
    let source = if let Some(pattern) = direct_node(syntax.tree(), node, NodeKind::BindingPattern) {
        ParameterSource {
            origin: SyntaxOrigin::Node(pattern),
            binding: parameter_binding(syntax, pattern)?,
            ty: SyntaxOrigin::Node(ty),
        }
    } else {
        let token = direct_identifier(syntax.tree(), node).ok_or(Failure::InconsistentSyntax)?;
        ParameterSource::named(token, ty)
    };
    Ok(ParameterHeader {
        source,
        data: project_type_syntax(syntax, ty)?,
        operation: super::input_operation(syntax, node)?,
    })
}

pub(super) fn parameter_binding(
    syntax: BoundSyntax<'_>,
    node: NodeId,
) -> Result<ParameterBinding<SyntaxOrigin>, Failure> {
    let tree = syntax.tree();
    let mut children = direct_node_iter(tree, node, NodeKind::BindingPattern).peekable();
    if children.peek().is_some() {
        return Ok(ParameterBinding::Tuple(
            children
                .map(|child| parameter_binding(syntax, child))
                .collect::<Result<_, _>>()?,
        ));
    }
    let token = direct_identifier(tree, node).ok_or(Failure::InconsistentSyntax)?;
    Ok(
        if text(syntax, token) == ContextualSpelling::Discard.as_str() {
            ParameterBinding::Discard
        } else {
            ParameterBinding::Name(SyntaxOrigin::Token(token))
        },
    )
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
        .map(|node| parameter(syntax, node))
        .collect::<Result<_, _>>()?;
    let result_syntax =
        direct_node(tree, tail, NodeKind::Type).ok_or(Failure::InconsistentSyntax)?;
    Ok(Header {
        name,
        kind: if is_primitive(syntax, declaration) {
            CodeKind::Primitive
        } else if deferred_modifier.is_some() {
            CodeKind::DeferredFunction
        } else {
            immediate_function(syntax, declaration)
        },
        deferred_modifier,
        parameters,
        result_syntax,
        result_data: project_type_syntax(syntax, result_syntax)?,
        body: direct_node(tree, declaration, NodeKind::Block),
    })
}

pub(super) fn immediate_function(syntax: BoundSyntax<'_>, declaration: NodeId) -> CodeKind {
    CodeKind::Function(
        if direct_node(syntax.tree(), declaration, NodeKind::CompileTimeModifier).is_some() {
            CompileTimeGuarantee::Evaluatable
        } else {
            CompileTimeGuarantee::RuntimeOnly
        },
    )
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
    let (name, copy) = nominal_name_and_copy(syntax, declaration)?;
    let fields = nominal_fields(
        syntax,
        direct_node_iter(tree, declaration, NodeKind::StructField),
    )?;
    Ok(RecordHeader {
        source: Arc::new(RecordSource {
            name,
            fields: fields.sources,
        }),
        copy,
        fields: fields.types,
    })
}

struct NominalFields {
    sources: Box<[FieldSource]>,
    types: Box<[TypeSyntax]>,
}

fn nominal_fields(
    syntax: BoundSyntax<'_>,
    fields: impl Iterator<Item = NodeId>,
) -> Result<NominalFields, Failure> {
    let tree = syntax.tree();
    let fields = fields
        .map(|field| {
            let name = direct_identifier(tree, field).ok_or(Failure::InconsistentSyntax)?;
            let ty = direct_node(tree, field, NodeKind::Type).ok_or(Failure::InconsistentSyntax)?;
            Ok((FieldSource { name, ty }, project_type_syntax(syntax, ty)?))
        })
        .collect::<Result<Vec<_>, Failure>>()?;
    let (fields, types): (Vec<_>, Vec<_>) = fields.into_iter().unzip();
    Ok(NominalFields {
        sources: fields.into(),
        types: types.into(),
    })
}

fn nominal_name_and_copy(
    syntax: BoundSyntax<'_>,
    declaration: NodeId,
) -> Result<(SyntaxToken, bool), Failure> {
    let tree = syntax.tree();
    let name = declaration_name_token(tree, declaration).ok_or(Failure::InconsistentSyntax)?;
    let first = first_direct_token(tree, declaration).ok_or(Failure::InconsistentSyntax)?;
    let copy =
        ContextualSpelling::from_spelling(text(syntax, first)) == Some(ContextualSpelling::Copy);
    Ok((name, copy))
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
                        .flat_map(|p| p.source.names().chain(p.data.tokens())),
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
