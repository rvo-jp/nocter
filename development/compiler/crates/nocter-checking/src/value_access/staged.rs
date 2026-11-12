//! One ordinary body preparation path over progressively closed declaration contracts.
use super::{accepted, evaluation};
use nocter_constant_evaluation::state;
use nocter_declaration_lowering::access_declarations as declarations;
use nocter_syntax::BoundSyntax;

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
impl From<accepted::Failure> for SourceFailure {
    fn from(failure: accepted::Failure) -> Self {
        Self::Checking(failure)
    }
}
impl From<declarations::Failure> for SourceFailure {
    fn from(failure: declarations::Failure) -> Self {
        Self::Declarations(failure)
    }
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
        let headers =
            declarations::Preparation::prepare(syntax).map_err(SourceFailure::Declarations)?;
        Self::close(syntax, headers, limits.into())
    }
    /// Admit only primitive declarations selected by this host-provided module authority.
    /// # Errors
    /// Reports the same whole-source failures as ordinary preparation; authority never supplies a body.
    pub fn prepare_with_primitive_authority(
        syntax: BoundSyntax<'_>,
        authority: &declarations::PrimitiveAuthority,
        limits: impl Into<state::Limits>,
    ) -> Result<Self, SourceFailure> {
        let headers =
            declarations::Preparation::prepare_with_primitive_authority(syntax, authority)?;
        Self::close(syntax, headers, limits.into())
    }
    fn close(
        syntax: BoundSyntax<'_>,
        headers: declarations::Preparation<'_>,
        limits: state::Limits,
    ) -> Result<Self, SourceFailure> {
        let (declarations, evaluation) = evaluation::prepare_source(headers, limits)?;
        let context = declarations
            .bind(syntax)
            .map_err(SourceFailure::Declarations)?;
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
