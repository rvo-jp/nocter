//! Closure capture and body-scope resolution.

use super::body::Scope;
use super::diagnostics::unresolved_identifier_diagnostic;
use super::{LocalSymbolKind, Resolver};
use crate::ast::ClosureExpr;

impl Resolver<'_> {
    pub(super) fn resolve_closure(&mut self, closure: &ClosureExpr, outer: &Scope) {
        let mut body_scope = outer.without_locals();
        for capture in &closure.captures {
            if let Some(source) = outer.resolve(&capture.name) {
                self.output
                    .local_identifier_targets
                    .insert(capture.name_span, source);
            } else {
                self.output
                    .diagnostics
                    .push(unresolved_identifier_diagnostic(
                        self.sources,
                        &capture.name,
                        capture.name_span,
                    ));
            }
            body_scope.unblock_local(&capture.name);
            self.define_local_name(
                capture.name.clone(),
                capture.name_span,
                LocalSymbolKind::ClosureCapture(capture.mode),
                &mut body_scope,
            );
        }
        for parameter in &closure.parameters {
            self.define_local_name(
                parameter.name.clone(),
                parameter.name_span,
                LocalSymbolKind::Parameter,
                &mut body_scope,
            );
        }
        self.resolve_block(&closure.body, &mut body_scope);
    }
}
