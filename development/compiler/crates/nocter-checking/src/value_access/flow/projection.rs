use super::{
    InitializationRequirement, InitializationState, InitializationViolation, Location, Origin,
    Query,
};
use crate::value_access::{Context as TypedContext, Failure, source_origin};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_diagnostics::{DiagnosticCode, DiagnosticNote, SourceDiagnostic};
use nocter_syntax::SyntaxOrigin;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitializationDiagnostic {
    violation: InitializationViolation,
    source: SourceDiagnostic,
}
impl InitializationDiagnostic {
    #[must_use]
    pub const fn violation(&self) -> &InitializationViolation {
        &self.violation
    }
    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}

/// Exact-current source binding beside the source-neutral flow/initialization authority.
#[derive(Debug)]
pub struct Context<'query, 'source> {
    query: &'query Query,
    typed: TypedContext<'query, 'source>,
}
impl Query {
    /// Rebinds through this query's own typed authority, without rebuilding flow or solving again.
    /// # Errors
    /// Rejects changed declaration authority/body or an inconsistent source projection.
    pub fn bind<'query, 'source>(
        &'query self,
        declarations: &DeclarationContext<'source>,
    ) -> Result<Context<'query, 'source>, Failure> {
        Ok(Context {
            query: self,
            typed: self.typed.bind(declarations)?,
        })
    }
}
impl Context<'_, '_> {
    #[must_use]
    pub const fn query(&self) -> &Query {
        self.query
    }
    #[must_use]
    pub const fn typed(&self) -> &TypedContext<'_, '_> {
        &self.typed
    }
    pub(super) fn origin(&self, location: Location) -> Result<SyntaxOrigin, Failure> {
        let origin = self
            .query
            .graph
            .points()
            .get(location.point)
            .and_then(|point| point.actions.get(location.action))
            .ok_or(Failure::InconsistentSyntax)?
            .origin;
        match origin {
            Origin::Expression(id) => self.typed.expression_origin(id),
            Origin::Statement(id) => self.typed.statement_origin(id),
            Origin::Block(id) => self.typed.block_origin(id),
        }
        .ok_or(Failure::InconsistentSyntax)
    }
    /// Projects fixed-point initialization failures with declarations and reaching invalidations.
    /// # Errors
    /// Returns an integrity failure if a semantic/source origin is inconsistent.
    pub fn diagnostics(&self) -> Result<Vec<InitializationDiagnostic>, Failure> {
        let syntax = self.typed.syntax();
        let mut diagnostics = Vec::new();
        for violation in self.query.initialization.violations() {
            let mut notes = vec![DiagnosticNote::new(
                "this slot is declared here",
                source_origin(
                    syntax,
                    self.typed
                        .local_origin(violation.local)
                        .ok_or(Failure::InconsistentSyntax)?,
                )?,
            )];
            for &cause in &violation.causes {
                notes.push(DiagnosticNote::new("this action leaves the slot uninitialized on a reaching path, possibly through a loop backedge",
                    source_origin(syntax, self.origin(cause)?)?));
            }
            let message = match (violation.requirement, violation.state) {
                (InitializationRequirement::Value, InitializationState::Uninitialized) => {
                    "this operation requires a complete initialized value, but part or all of it is unavailable"
                }
                (InitializationRequirement::Value, InitializationState::Maybe) => {
                    "the selected value must be fully initialized on every reaching path"
                }
                (InitializationRequirement::Container, InitializationState::Uninitialized) => {
                    "this operation requires a present containing place, but that container is unavailable"
                }
                (InitializationRequirement::Container, InitializationState::Maybe) => {
                    "the containing place must be present on every reaching path"
                }
                (_, InitializationState::Initialized) => return Err(Failure::InconsistentSyntax),
            };
            diagnostics.push(InitializationDiagnostic {
                violation: violation.clone(),
                source: SourceDiagnostic::new(
                    DiagnosticCode::E0378,
                    message,
                    source_origin(syntax, self.origin(violation.location)?)?,
                    notes,
                    None::<&str>,
                ),
            });
        }
        diagnostics.sort_by_key(|diagnostic| diagnostic.source.primary().span().range().start());
        Ok(diagnostics)
    }
}
