use nocter_syntax::{NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxTree, TokenKind};
pub(super) use nocter_syntax::{direct_identifier, direct_node, direct_nodes, direct_tokens};

pub(super) fn descendant(tree: &SyntaxTree, root: NodeId, kind: NodeKind) -> Option<NodeId> {
    nocter_syntax::outermost_descendant_node_iter(tree, root, kind).next()
}

pub(super) fn has_punctuation(tree: &SyntaxTree, node: NodeId, punctuation: Punctuation) -> bool {
    direct_tokens(tree, node)
        .into_iter()
        .any(|token| token.kind() == TokenKind::Punctuation(punctuation))
}

pub(super) fn requirement_origins(tree: &SyntaxTree, root: NodeId) -> Vec<NodeId> {
    let mut result = Vec::new();
    let mut pending: Vec<_> = tree.children(root).iter().rev().copied().collect();
    while let Some(element) = pending.pop() {
        let SyntaxElement::Node(node) = element else {
            continue;
        };
        let Some(kind) = tree.node(node).map(nocter_syntax::SyntaxNode::kind) else {
            continue;
        };
        match kind {
            NodeKind::WhereClause => {
                for predicate in tree
                    .children(node)
                    .iter()
                    .filter_map(|element| match element {
                        SyntaxElement::Node(child) => Some(*child),
                        _ => None,
                    })
                {
                    if tree
                        .node(predicate)
                        .is_some_and(|syntax| syntax.kind() == NodeKind::InterfacePredicate)
                    {
                        for application in
                            direct_nodes(tree, predicate, NodeKind::InterfaceApplication)
                        {
                            result.push(application);
                        }
                    } else {
                        result.push(predicate);
                    }
                }
            }
            NodeKind::InterfaceBounds => {
                result.extend(direct_nodes(tree, node, NodeKind::InterfaceApplication));
            }
            NodeKind::Block => {}
            kind if declaration_node(kind) => {}
            _ => pending.extend(tree.children(node).iter().rev().copied()),
        }
    }
    result
}

const fn declaration_node(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::FunctionDeclaration
            | NodeKind::TypeAliasDeclaration
            | NodeKind::StructDeclaration
            | NodeKind::StructField
            | NodeKind::EnumDeclaration
            | NodeKind::EnumVariant
            | NodeKind::InterfaceDeclaration
            | NodeKind::AssociatedTypeDeclaration
            | NodeKind::InterfaceMethod
            | NodeKind::ConstructDeclaration
            | NodeKind::ConstructionFunction
            | NodeKind::LiteralDeclaration
            | NodeKind::InstanceDeclaration
            | NodeKind::InherentMethod
            | NodeKind::CoercionDeclaration
            | NodeKind::EqualityOperator
            | NodeKind::OrderingOperator
            | NodeKind::IndexOperator
            | NodeKind::ExpansionOperator
            | NodeKind::InterfaceImplementation
            | NodeKind::AssociatedTypeBinding
            | NodeKind::DropDeclaration
            | NodeKind::TestDeclaration
    )
}
