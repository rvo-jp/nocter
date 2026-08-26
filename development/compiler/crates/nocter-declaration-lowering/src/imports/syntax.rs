use nocter_syntax::{NodeId, NodeKind, SyntaxToken, SyntaxTree, direct_node, direct_node_iter};

use super::ImportError;

#[derive(Clone, Copy, Debug)]
pub(super) struct SelectedNameSyntax {
    pub(super) exported: SyntaxToken,
    pub(super) local: SyntaxToken,
}

#[derive(Debug)]
pub(super) struct ImportSyntax {
    pub(super) visibility: Option<NodeId>,
    pub(super) path: NodeId,
    pub(super) selected: Option<Vec<SelectedNameSyntax>>,
}

pub(super) fn read(tree: &SyntaxTree, declaration: NodeId) -> Result<ImportSyntax, ImportError> {
    if tree.node(declaration).map(nocter_syntax::SyntaxNode::kind) != Some(NodeKind::UseDeclaration)
    {
        return Err(ImportError::InvalidSyntax(declaration));
    }
    let visibility = direct_node(tree, declaration, NodeKind::Visibility);
    let path = direct_node(tree, declaration, NodeKind::ModulePath)
        .ok_or(ImportError::InvalidSyntax(declaration))?;
    let selected = direct_node(tree, declaration, NodeKind::ImportSelection)
        .map(|selection| selected_names(tree, declaration, selection))
        .transpose()?;
    Ok(ImportSyntax {
        visibility,
        path,
        selected,
    })
}

pub(super) fn final_path_name(
    tree: &SyntaxTree,
    declaration: NodeId,
    path: NodeId,
) -> Result<SyntaxToken, ImportError> {
    descendant_identifiers(tree, path)
        .into_iter()
        .last()
        .ok_or(ImportError::InvalidSyntax(declaration))
}

fn selected_names(
    tree: &SyntaxTree,
    declaration: NodeId,
    selection: NodeId,
) -> Result<Vec<SelectedNameSyntax>, ImportError> {
    let mut names = Vec::new();
    for selected in direct_node_iter(tree, selection, NodeKind::SelectedName) {
        let tokens = descendant_identifiers(tree, selected);
        let exported = *tokens
            .first()
            .ok_or(ImportError::InvalidSyntax(declaration))?;
        let local = *tokens
            .last()
            .ok_or(ImportError::InvalidSyntax(declaration))?;
        names.push(SelectedNameSyntax { exported, local });
    }
    if names.is_empty() {
        Err(ImportError::InvalidSyntax(declaration))
    } else {
        Ok(names)
    }
}

fn descendant_identifiers(tree: &SyntaxTree, node: NodeId) -> Vec<SyntaxToken> {
    nocter_syntax::descendant_identifier_iter(tree, node).collect()
}
