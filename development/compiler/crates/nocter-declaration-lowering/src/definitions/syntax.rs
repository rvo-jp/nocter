use nocter_syntax::{
    NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind,
};

pub(super) fn direct_nodes(tree: &SyntaxTree, node: NodeId, kind: NodeKind) -> Vec<NodeId> {
    tree.children(node)
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Node(child)
                if tree
                    .node(*child)
                    .is_some_and(|syntax| syntax.kind() == kind) =>
            {
                Some(*child)
            }
            _ => None,
        })
        .collect()
}

pub(super) fn direct_node(tree: &SyntaxTree, node: NodeId, kind: NodeKind) -> Option<NodeId> {
    direct_nodes(tree, node, kind).into_iter().next()
}

pub(super) fn descendants(tree: &SyntaxTree, root: NodeId, kind: NodeKind) -> Vec<NodeId> {
    let mut result = Vec::new();
    let mut pending: Vec<_> = tree.children(root).iter().rev().copied().collect();
    while let Some(element) = pending.pop() {
        let SyntaxElement::Node(node) = element else {
            continue;
        };
        if tree.node(node).is_some_and(|syntax| syntax.kind() == kind) {
            result.push(node);
            continue;
        }
        pending.extend(tree.children(node).iter().rev().copied());
    }
    result
}

pub(super) fn descendant(tree: &SyntaxTree, root: NodeId, kind: NodeKind) -> Option<NodeId> {
    descendants(tree, root, kind).into_iter().next()
}

pub(super) fn direct_tokens(tree: &SyntaxTree, node: NodeId) -> Vec<SyntaxToken> {
    tree.children(node)
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Token(token) => Some(*token),
            _ => None,
        })
        .collect()
}

pub(super) fn direct_identifier(tree: &SyntaxTree, node: NodeId) -> Option<SyntaxToken> {
    direct_tokens(tree, node)
        .into_iter()
        .find(|token| token.kind() == TokenKind::Identifier)
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
                        .is_some_and(|syntax| syntax.kind() == NodeKind::CapabilityPredicate)
                    {
                        result.extend(direct_nodes(tree, predicate, NodeKind::Capability));
                    } else {
                        result.push(predicate);
                    }
                }
            }
            NodeKind::InterfaceBounds => {
                result.extend(direct_nodes(tree, node, NodeKind::Capability));
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
            | NodeKind::PrimitiveDeclaration
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
            | NodeKind::ConformDeclaration
            | NodeKind::AssociatedTypeBinding
            | NodeKind::ConformMethod
            | NodeKind::DropDeclaration
            | NodeKind::TestDeclaration
    )
}
