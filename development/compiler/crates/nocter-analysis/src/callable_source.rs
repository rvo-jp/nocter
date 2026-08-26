use nocter_declarations::CallableKind;
use nocter_source::{ByteOffset, TextRange};
use nocter_syntax::SyntaxOrigin;
use nocter_syntax::{NodeId, NodeKind, SyntaxElement, SyntaxTree, direct_node};

/// Structural source projection for one semantic callable declaration.
///
/// Callers use the editable result only for contracts whose grammar admits replacement. The result
/// end remains available for presentation features even when the result is opaque or not editable.
pub(crate) struct CallableSourceProjection {
    editable_result: Option<NodeId>,
    result_end: Option<ByteOffset>,
}

impl CallableSourceProjection {
    pub(crate) const fn editable_result(&self) -> Option<NodeId> {
        self.editable_result
    }

    pub(crate) const fn result_end(&self) -> Option<ByteOffset> {
        self.result_end
    }
}

pub(crate) fn project_callable_source(
    syntax: &SyntaxTree,
    binding: SyntaxOrigin,
    kind: CallableKind,
) -> Option<CallableSourceProjection> {
    let binding_range = match binding {
        SyntaxOrigin::Node(node) => syntax.node(node)?.range(),
        SyntaxOrigin::Token(token) => token.range(),
    };
    let declaration = syntax
        .nodes()
        .filter(|(_, node)| {
            declaration_matches(kind, node.kind()) && node.range().contains_range(binding_range)
        })
        .min_by_key(|(_, node)| node.range().len())?
        .0;

    match kind {
        CallableKind::Function
        | CallableKind::Primitive
        | CallableKind::ConstructionFunction
        | CallableKind::Literal(_) => {
            let tail = direct_node(syntax, declaration, NodeKind::CallableTail)?;
            Some(CallableSourceProjection {
                editable_result: (kind != CallableKind::Primitive)
                    .then(|| direct_node(syntax, tail, NodeKind::Type))
                    .flatten(),
                result_end: callable_tail_result_end(syntax, tail),
            })
        }
        CallableKind::Method => {
            let signature = direct_node(syntax, declaration, NodeKind::MethodSignature)?;
            let tail = direct_node(syntax, signature, NodeKind::CallableTail)?;
            Some(CallableSourceProjection {
                editable_result: direct_node(syntax, tail, NodeKind::Type),
                result_end: callable_tail_result_end(syntax, tail),
            })
        }
        CallableKind::Coercion | CallableKind::Expansion => {
            let result = direct_node(syntax, declaration, NodeKind::Type)?;
            Some(CallableSourceProjection {
                editable_result: Some(result),
                result_end: syntax
                    .node(result)
                    .map(nocter_syntax::SyntaxNode::range)
                    .map(TextRange::end),
            })
        }
        CallableKind::Index => {
            let result = direct_node(syntax, declaration, NodeKind::BorrowType)?;
            Some(CallableSourceProjection {
                editable_result: None,
                result_end: syntax
                    .node(result)
                    .map(nocter_syntax::SyntaxNode::range)
                    .map(TextRange::end),
            })
        }
        CallableKind::Equality | CallableKind::Ordering => Some(CallableSourceProjection {
            editable_result: None,
            result_end: None,
        }),
    }
}

fn callable_tail_result_end(syntax: &SyntaxTree, tail: NodeId) -> Option<ByteOffset> {
    let mut end = None;
    for element in syntax.children(tail) {
        match element {
            SyntaxElement::Node(child)
                if matches!(
                    syntax.node(*child)?.kind(),
                    NodeKind::ProvenanceClause | NodeKind::WhereClause
                ) =>
            {
                break;
            }
            SyntaxElement::Node(child) => end = Some(syntax.node(*child)?.range().end()),
            SyntaxElement::Token(token) => end = Some(token.range().end()),
            SyntaxElement::Missing(missing) => end = Some(missing.span().range().end()),
        }
    }
    end
}

const fn declaration_matches(callable: CallableKind, syntax: NodeKind) -> bool {
    match callable {
        CallableKind::Function | CallableKind::Primitive => {
            matches!(syntax, NodeKind::FunctionDeclaration)
        }
        CallableKind::Method => {
            matches!(syntax, NodeKind::InterfaceMethod | NodeKind::InherentMethod)
        }
        CallableKind::ConstructionFunction => matches!(syntax, NodeKind::ConstructionFunction),
        CallableKind::Literal(_) => matches!(syntax, NodeKind::LiteralDeclaration),
        CallableKind::Coercion => matches!(syntax, NodeKind::CoercionDeclaration),
        CallableKind::Equality => matches!(syntax, NodeKind::EqualityOperator),
        CallableKind::Ordering => matches!(syntax, NodeKind::OrderingOperator),
        CallableKind::Index => matches!(syntax, NodeKind::IndexOperator),
        CallableKind::Expansion => matches!(syntax, NodeKind::ExpansionOperator),
    }
}

#[cfg(test)]
mod tests {
    use nocter_declarations::CallableKind;
    use nocter_source::{ByteOffset, SourceMap, SourceName};
    use nocter_syntax::SyntaxOrigin;
    use nocter_syntax::{NodeKind, ParseGoal, parse};

    use super::project_callable_source;

    #[test]
    fn function_projection_uses_its_direct_tail_instead_of_a_nested_closure() {
        let text = concat!(
            "func view(text: &str): &str {\n",
            "    let closure = (): bool { true }\n",
            "    text\n",
            "}\n",
        );
        let (sources, syntax) = parsed(text);
        let declaration = node(&syntax, NodeKind::FunctionDeclaration);
        let projection = project_callable_source(
            &syntax,
            SyntaxOrigin::Node(declaration),
            CallableKind::Function,
        )
        .unwrap();
        let expected = u32::try_from(text.find("&str {\n").unwrap() + 4).unwrap();

        assert_eq!(projection.result_end(), Some(ByteOffset::new(expected)));
        assert_eq!(
            syntax
                .node(projection.editable_result().unwrap())
                .and_then(|node| sources.get(syntax.source())?.text_at(node.range())),
            Some("&str")
        );
    }

    #[test]
    fn fixed_index_result_has_a_presentation_end_but_is_not_editable() {
        let text = concat!(
            "instance Box<T> {\n",
            "    operator (&self[index: usize]): &T { return &self.value }\n",
            "}\n",
        );
        let (_, syntax) = parsed(text);
        let declaration = node(&syntax, NodeKind::IndexOperator);
        let projection = project_callable_source(
            &syntax,
            SyntaxOrigin::Node(declaration),
            CallableKind::Index,
        )
        .unwrap();
        let expected = u32::try_from(text.find("&T {").unwrap() + 2).unwrap();

        assert_eq!(projection.result_end(), Some(ByteOffset::new(expected)));
        assert_eq!(projection.editable_result(), None);
    }

    fn parsed(text: &str) -> (SourceMap, nocter_syntax::SyntaxTree) {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(SourceName::new("index.nct"), text.as_bytes())
            .unwrap();
        let syntax = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
        assert!(!syntax.has_errors(), "{:?}", syntax.diagnostics());
        (sources, syntax)
    }

    fn node(syntax: &nocter_syntax::SyntaxTree, kind: NodeKind) -> nocter_syntax::NodeId {
        syntax
            .nodes()
            .find(|(_, node)| node.kind() == kind)
            .map(|(node, _)| node)
            .unwrap()
    }
}
