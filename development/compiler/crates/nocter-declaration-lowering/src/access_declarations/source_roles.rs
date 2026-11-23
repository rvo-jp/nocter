//! File-role placement checks over shared syntax, without resolving names or publishing headers.
use super::{Failure, headers, validate_input};
use nocter_compile_input::ModuleSourceKind;
use nocter_diagnostics::{DiagnosticCode, SourceDiagnostic};
use nocter_source_index::SourceOrigin;
use nocter_syntax::{
    BoundSyntax, NodeId, NodeKind, Punctuation, TokenKind, declaration_body, direct_token_iter,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceRoleRule {
    BodyInContract,
    InitializerInContract,
    MissingBody,
    MissingInitializer,
    MissingRepresentation,
    TypeBehaviorOwner,
    ConstructionResult,
    VisibilityOutsideContract,
    PackageMetadataOutsideContract,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRoleDiagnostic {
    rule: SourceRoleRule,
    source: SourceDiagnostic,
}
impl SourceRoleDiagnostic {
    #[must_use]
    pub const fn rule(&self) -> SourceRoleRule {
        self.rule
    }
    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}

/// Validate placement for a host-selected source role. This performs no discovery, pairing,
/// name resolution, type normalization or body checking and grants no accepted-unit evidence.
/// # Errors
/// Rejects unsupported syntax and declarations whose implementation shape violates their role.
pub fn validate_source_role(
    syntax: BoundSyntax<'_>,
    role: ModuleSourceKind,
) -> Result<(), Failure> {
    validate_input(syntax)?;
    let root = role == ModuleSourceKind::Root;
    if !root
        && let Some((node, _)) = syntax
            .tree()
            .nodes()
            .filter(|(_, node)| node.kind() == NodeKind::Visibility)
            .min_by_key(|(_, node)| node.range().start())
    {
        return reject(syntax, SourceRoleRule::VisibilityOutsideContract, node);
    }
    if !root
        && let Some(node) = nocter_syntax::direct_node(
            syntax.tree(),
            syntax.tree().root_id(),
            NodeKind::PackageDirective,
        )
    {
        return reject(syntax, SourceRoleRule::PackageMetadataOutsideContract, node);
    }
    let tree = syntax.tree();
    if root {
        for item in nocter_syntax::direct_node_iter(tree, tree.root_id(), NodeKind::Item) {
            if nocter_syntax::direct_node(tree, item, NodeKind::TypeBehaviorDeclaration).is_some()
                && let Some(visibility) =
                    nocter_syntax::direct_node(tree, item, NodeKind::Visibility)
            {
                return reject(
                    syntax,
                    SourceRoleRule::VisibilityOutsideContract,
                    visibility,
                );
            }
        }
    }
    let mut pending = headers::declarations(syntax)?;
    pending.reverse();
    while let Some(declaration) = pending.pop() {
        let kind = syntax
            .tree()
            .node(declaration)
            .ok_or(Failure::InconsistentSyntax)?
            .kind();
        let violation = match kind {
            NodeKind::PrimitiveTypeDeclaration => None,
            NodeKind::TestDeclaration => declaration_body(syntax.tree(), declaration)
                .filter(|_| root)
                .map(|body| (SourceRoleRule::BodyInContract, body)),
            NodeKind::TypeBehaviorDeclaration => {
                pending.extend(
                    nocter_syntax::child_node_iter(tree, declaration)
                        .rev()
                        .filter(|&node| {
                            tree.node(node).is_some_and(|node| {
                                matches!(
                                    node.kind(),
                                    NodeKind::ConstructionFunction | NodeKind::InherentMethod
                                )
                            })
                        }),
                );
                None
            }
            NodeKind::FunctionDeclaration
            | NodeKind::ConstructionFunction
            | NodeKind::InherentMethod => {
                let body = declaration_body(syntax.tree(), declaration);
                match (root, body) {
                    (true, Some(body)) => Some((SourceRoleRule::BodyInContract, body)),
                    (false, None) if !headers::is_primitive(syntax, declaration) => {
                        Some((SourceRoleRule::MissingBody, declaration))
                    }
                    _ => None,
                }
            }
            NodeKind::ConstantDeclaration | NodeKind::StaticDeclaration => {
                let value = declaration_body(syntax.tree(), declaration);
                match (root, value) {
                    (true, Some(value)) => Some((SourceRoleRule::InitializerInContract, value)),
                    (false, None) => Some((SourceRoleRule::MissingInitializer, declaration)),
                    _ => None,
                }
            }
            NodeKind::StructDeclaration => {
                let braced = direct_token_iter(syntax.tree(), declaration)
                    .any(|token| token.kind() == TokenKind::Punctuation(Punctuation::LeftBrace));
                (!root && !braced).then_some((SourceRoleRule::MissingRepresentation, declaration))
            }
            _ => return Err(Failure::InconsistentSyntax),
        };
        if let Some((rule, subject)) = violation {
            return reject(syntax, rule, subject);
        }
    }
    Ok(())
}

fn reject(syntax: BoundSyntax<'_>, rule: SourceRoleRule, subject: NodeId) -> Result<(), Failure> {
    Err(role_failure(syntax, rule, subject))
}

pub(super) fn role_failure(
    syntax: BoundSyntax<'_>,
    rule: SourceRoleRule,
    subject: NodeId,
) -> Failure {
    let Ok(origin) = SourceOrigin::from_node(syntax.tree(), subject) else {
        return Failure::InconsistentSyntax;
    };
    let (code, message) = match rule {
        SourceRoleRule::BodyInContract => (
            DiagnosticCode::E0237,
            "executable bodies belong in implementation sources",
        ),
        SourceRoleRule::InitializerInContract => (
            DiagnosticCode::E0237,
            "value initializers belong in implementation sources",
        ),
        SourceRoleRule::MissingBody => (
            DiagnosticCode::E0253,
            "a callable outside a contract source requires its implementation body",
        ),
        SourceRoleRule::MissingInitializer => (
            DiagnosticCode::E0275,
            "a value outside a contract source requires its initializer",
        ),
        SourceRoleRule::VisibilityOutsideContract => (
            DiagnosticCode::E0238,
            "visibility directives belong to eligible index.nct contracts",
        ),
        SourceRoleRule::PackageMetadataOutsideContract => (
            DiagnosticCode::E0276,
            "package metadata belongs to the package root index.nct",
        ),
        SourceRoleRule::MissingRepresentation => (
            DiagnosticCode::E0231,
            "a record outside a contract source requires its complete representation",
        ),
        SourceRoleRule::TypeBehaviorOwner => (
            DiagnosticCode::E0231,
            "impl requires a nominal declared by this module or a built-in authorized for it",
        ),
        SourceRoleRule::ConstructionResult => (
            DiagnosticCode::E0370,
            "a type-owned func must deliver its Self type, optionally wrapped in an outcome",
        ),
    };
    Failure::SourceRole(Box::new(SourceRoleDiagnostic {
        rule,
        source: SourceDiagnostic::new(code, message, origin, Vec::new(), None::<&str>),
    }))
}
