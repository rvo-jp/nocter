//! One ordinary body preparation path over progressively closed callable headers.
use super::{Failure as BodyFailure, accepted, checker, evaluation};
use nocter_constant_evaluation::state;
use nocter_declaration_lowering::access_declarations as declarations;
use nocter_syntax::BoundSyntax;
use std::collections::BTreeMap;

#[derive(Debug)]
pub enum SourceFailure {
    Declarations(declarations::Failure),
    Checking(accepted::Failure),
}

/// Complete source acceptance and its exact reusable header query. No public parts constructor.
#[derive(Debug)]
pub struct SourceUnit {
    declarations: declarations::Query,
    unit: accepted::Unit,
}
impl SourceUnit {
    /// Prepare ordinary bodies as their selected contracts become ready, then close the whole unit.
    /// # Errors
    /// Reports header, ordinary typing/safety or required-evaluation rejection.
    pub fn prepare(syntax: BoundSyntax<'_>) -> Result<Self, SourceFailure> {
        Self::prepare_with_limits(syntax, state::Limits::default())
    }
    /// # Errors
    /// Uses explicit required-evaluation limits with the same staged source preparation.
    pub fn prepare_with_limits(
        syntax: BoundSyntax<'_>,
        limits: impl Into<state::Limits>,
    ) -> Result<Self, SourceFailure> {
        let mut headers =
            declarations::Preparation::prepare(syntax).map_err(SourceFailure::Declarations)?;
        let ids: Vec<_> = headers.callable_ids().collect();
        let mut queries = BTreeMap::new();
        for id in ids {
            let mut context = headers.context(id).map_err(SourceFailure::Declarations)?;
            loop {
                match checker::prepare(&context, id, &()) {
                    Ok(query) => {
                        queries.insert(id, query);
                        break;
                    }
                    Err(BodyFailure::RequiredHeader { callable, .. }) => {
                        context = headers
                            .context(callable)
                            .map_err(SourceFailure::Declarations)?;
                    }
                    // Values still use the existing shared dependency/evaluator authority below.
                    // No successful body is reopened when a later constant finishes.
                    Err(BodyFailure::RequiredConstant { .. }) => break,
                    Err(failure) => {
                        return Err(SourceFailure::Checking(accepted::Failure::Body {
                            callable: id,
                            failure,
                        }));
                    }
                }
            }
        }
        let declarations = headers.finish();
        let context = declarations
            .bind(syntax)
            .map_err(SourceFailure::Declarations)?;
        let evaluation = evaluation::prepare(&context, limits.into(), queries)
            .map_err(SourceFailure::Checking)?;
        let unit = accepted::Unit::close_evaluation(&context, evaluation)
            .map_err(SourceFailure::Checking)?;
        Ok(Self { declarations, unit })
    }
    #[must_use]
    pub const fn declarations(&self) -> &declarations::Query {
        &self.declarations
    }
    #[must_use]
    pub const fn unit(&self) -> &accepted::Unit {
        &self.unit
    }
    #[must_use]
    pub fn into_unit(self) -> accepted::Unit {
        self.unit
    }
}
