use nocter_compile_input::is_valid_module_segment;
use nocter_declarations::PackageTargetKind;
use nocter_source::SourceFile;
use nocter_syntax::{Keyword, NodeId, NodeKind, SyntaxElement, SyntaxTree, TokenKind};

use crate::{DiscoveryError, PackageTargetFailure};

pub(crate) struct AuthoredPackageTarget {
    declaration: NodeId,
    module: Box<[Box<str>]>,
}

impl AuthoredPackageTarget {
    pub(crate) const fn declaration(&self) -> NodeId {
        self.declaration
    }

    pub(crate) const fn module(&self) -> &[Box<str>] {
        &self.module
    }
}

pub(crate) fn authored_package_targets(
    source: &SourceFile,
    tree: &SyntaxTree,
) -> Result<Vec<AuthoredPackageTarget>, DiscoveryError> {
    let mut targets = Vec::new();
    for declaration in child_nodes(tree, tree.root_id()) {
        let Some(kind) = package_target_kind(source, tree, declaration) else {
            continue;
        };
        let module = package_target_module(source, tree, declaration, kind)?;
        targets.push(AuthoredPackageTarget {
            declaration,
            module,
        });
    }
    Ok(targets)
}

fn package_target_kind(
    source: &SourceFile,
    tree: &SyntaxTree,
    declaration: NodeId,
) -> Option<PackageTargetKind> {
    if tree.node(declaration)?.kind() != NodeKind::PackageDirective {
        return None;
    }
    tree.children(declaration).iter().find_map(|element| {
        let SyntaxElement::Token(token) = element else {
            return None;
        };
        if token.kind() == TokenKind::Keyword(Keyword::Test) {
            Some(PackageTargetKind::Test)
        } else if token.kind() == TokenKind::Identifier
            && source.text_at(token.range()) == Some("executable")
        {
            Some(PackageTargetKind::Executable)
        } else {
            None
        }
    })
}

fn package_target_module(
    source: &SourceFile,
    tree: &SyntaxTree,
    declaration: NodeId,
    kind: PackageTargetKind,
) -> Result<Box<[Box<str>]>, DiscoveryError> {
    let mut module = None;
    for field in descendants(tree, declaration, NodeKind::DirectiveField) {
        if !tree.children(field).iter().any(|element| {
            matches!(element, SyntaxElement::Token(token) if token.kind() == TokenKind::Identifier && source.text_at(token.range()) == Some("module"))
        }) {
            continue;
        }
        let literal = descendants(tree, field, NodeKind::StringLiteral)
            .into_iter()
            .next()
            .ok_or_else(|| {
                package_target_error(declaration, PackageTargetFailure::InvalidModule)
            })?;
        let authored =
            nocter_syntax::decode_string_literal(source, tree, literal).ok_or_else(|| {
                package_target_error(declaration, PackageTargetFailure::InvalidModule)
            })?;
        let parsed = parse_module_path(&authored).ok_or_else(|| {
            package_target_error(declaration, PackageTargetFailure::InvalidModule)
        })?;
        if module.replace(parsed).is_some() {
            return Err(package_target_error(
                declaration,
                PackageTargetFailure::DuplicateModule,
            ));
        }
    }
    match (module, kind) {
        (Some(module), _) => Ok(module),
        (None, PackageTargetKind::Executable) => Ok(Box::new([])),
        (None, PackageTargetKind::Test) => Err(package_target_error(
            declaration,
            PackageTargetFailure::MissingModule,
        )),
    }
}

fn parse_module_path(authored: &str) -> Option<Box<[Box<str>]>> {
    if authored == "." {
        return Some(Box::new([]));
    }
    let relative = authored.strip_prefix("./")?;
    if relative.is_empty() {
        return None;
    }
    relative
        .split('/')
        .map(|segment| is_valid_module_segment(segment).then(|| Box::<str>::from(segment)))
        .collect::<Option<Vec<_>>>()
        .map(Vec::into_boxed_slice)
}

fn package_target_error(declaration: NodeId, failure: PackageTargetFailure) -> DiscoveryError {
    DiscoveryError::PackageTarget {
        declaration,
        failure,
    }
}

fn child_nodes(tree: &SyntaxTree, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    tree.children(node)
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Node(node) => Some(*node),
            SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
        })
}

fn descendants(tree: &SyntaxTree, root: NodeId, kind: NodeKind) -> Vec<NodeId> {
    let mut result = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if tree.node(node).is_some_and(|node| node.kind() == kind) {
            result.push(node);
            continue;
        }
        pending.extend(child_nodes(tree, node));
    }
    result
}
