//! File-role placement checks over shared syntax, without resolving names or publishing headers.
use super::{Failure, headers, validate_input};
use nocter_compile_input::ModuleSourceKind;
use nocter_diagnostics::{DiagnosticCode, SourceDiagnostic};
use nocter_source_index::SourceOrigin;
use nocter_syntax::{
    BoundSyntax, NodeId, NodeKind, Punctuation, TokenKind, direct_node, direct_token_iter,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceRoleRule {
    BodyInContract,
    InitializerInContract,
    MissingBody,
    MissingInitializer,
    MissingRepresentation,
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
    for declaration in headers::declarations(syntax)? {
        let kind = syntax
            .tree()
            .node(declaration)
            .ok_or(Failure::InconsistentSyntax)?
            .kind();
        let violation = match kind {
            NodeKind::FunctionDeclaration => {
                let body = direct_node(syntax.tree(), declaration, NodeKind::Block);
                match (root, body) {
                    (true, Some(body)) => Some((SourceRoleRule::BodyInContract, body)),
                    (false, None) if !headers::is_primitive(syntax, declaration) => {
                        Some((SourceRoleRule::MissingBody, declaration))
                    }
                    _ => None,
                }
            }
            NodeKind::ConstantDeclaration | NodeKind::StaticDeclaration => {
                let value = direct_node(syntax.tree(), declaration, NodeKind::Expression);
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
    let origin =
        SourceOrigin::from_node(syntax.tree(), subject).map_err(|_| Failure::InconsistentSyntax)?;
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
        SourceRoleRule::MissingRepresentation => (
            DiagnosticCode::E0231,
            "a record outside a contract source requires its complete representation",
        ),
    };
    Err(Failure::SourceRole(Box::new(SourceRoleDiagnostic {
        rule,
        source: SourceDiagnostic::new(code, message, origin, Vec::new(), None::<&str>),
    })))
}
