use super::super::Context;
use super::{AccessRule, Creation, Root, Violation};
use crate::value_access::{Failure, source_origin};
use nocter_diagnostics::{DiagnosticCode, DiagnosticNote, SourceDiagnostic};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessDiagnostic {
    violation: Violation,
    source: SourceDiagnostic,
}
impl AccessDiagnostic {
    #[must_use]
    pub const fn violation(&self) -> &Violation {
        &self.violation
    }
    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}
impl Context<'_, '_> {
    fn root_origin(&self, root: Root) -> Result<nocter_syntax::SyntaxOrigin, Failure> {
        Ok(match root {
            Root::Temporary { call, index } => {
                let crate::value_access::ExpressionKind::Call { arguments, .. } = &self
                    .query()
                    .typed()
                    .body()
                    .expressions()
                    .get(call)
                    .ok_or(Failure::InconsistentSyntax)?
                    .kind
                else {
                    return Err(Failure::InconsistentSyntax);
                };
                self.typed()
                    .expression_origin(
                        arguments
                            .get(index)
                            .ok_or(Failure::InconsistentSyntax)?
                            .value,
                    )
                    .ok_or(Failure::InconsistentSyntax)?
            }
            root => {
                let local = match root {
                    Root::Owned(local) | Root::Input(local) => local,
                    Root::Static(id) => self
                        .query()
                        .typed()
                        .body()
                        .locals()
                        .iter()
                        .find_map(|(local, declaration)| {
                            (declaration.origin == crate::value_access::LocalOrigin::Static(id))
                                .then_some(local)
                        })
                        .ok_or(Failure::InconsistentSyntax)?,
                    Root::Temporary { .. } => return Err(Failure::InconsistentSyntax),
                };
                self.typed()
                    .local_origin(local)
                    .ok_or(Failure::InconsistentSyntax)?
            }
        })
    }
    /// Projects referent/access failures through the same exact-current typed source binding.
    /// # Errors
    /// Returns an integrity failure for an inconsistent semantic/source origin.
    pub fn access_diagnostics(&self) -> Result<Vec<AccessDiagnostic>, Failure> {
        let syntax = self.typed().syntax();
        let mut diagnostics = Vec::new();
        for violation in self.query().access().violations() {
            let (code, message) = match violation.rule {
                AccessRule::ConflictingRead => (
                    DiagnosticCode::E0396,
                    "this read overlaps an active exclusive access",
                ),
                AccessRule::ConflictingUpdate => (
                    DiagnosticCode::E0397,
                    "this update overlaps an access still required elsewhere",
                ),
                AccessRule::ConflictingTransfer => (
                    DiagnosticCode::E0397,
                    "this ownership removal overlaps a required access",
                ),
                AccessRule::ReservedModification => (
                    DiagnosticCode::E0396,
                    "this operation changes a place reserved by a pending call",
                ),
                AccessRule::ConflictingArguments => (
                    DiagnosticCode::E0396,
                    "the selected call requires incompatible simultaneous accesses to overlapping places",
                ),
                AccessRule::ExpiredRoot => (
                    DiagnosticCode::E0398,
                    "this access requires an owner that no longer survives",
                ),
            };
            let root_origin = self.root_origin(violation.root)?;
            let mut notes = vec![DiagnosticNote::new(
                "the referent root is declared here",
                source_origin(syntax, root_origin)?,
            )];
            if let Some(retained) = violation.retained {
                let origin = match retained.created {
                    Creation::Entry(local) => self
                        .typed()
                        .local_origin(local)
                        .ok_or(Failure::InconsistentSyntax)?,
                    Creation::Action(at) => self.origin(at)?,
                };
                notes.push(DiagnosticNote::new(
                    "the retained access or reservation starts here",
                    source_origin(syntax, origin)?,
                ));
                if let Some(at) = retained.required {
                    notes.push(DiagnosticNote::new(
                        "this operation still requires that access",
                        source_origin(syntax, self.origin(at)?)?,
                    ));
                }
            }
            diagnostics.push(AccessDiagnostic {
                violation: violation.clone(),
                source: SourceDiagnostic::new(
                    code,
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
