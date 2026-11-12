//! One ordinary body preparation path over progressively closed declaration contracts.
use super::{accepted, evaluation};
use nocter_constant_evaluation::state;
use nocter_declaration_lowering::access_declarations as declarations;
use nocter_syntax::BoundSyntax;

#[derive(Debug)]
pub enum SourceFailure {
    Declarations(declarations::Failure),
    Modules(declarations::ModuleSourcesFailure),
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
impl From<declarations::ModuleSourcesFailure> for SourceFailure {
    fn from(failure: declarations::ModuleSourcesFailure) -> Self {
        match failure {
            declarations::ModuleSourcesFailure::Access(failure) => Self::Declarations(failure),
            failure => Self::Modules(failure),
        }
    }
}
/// Whole selected-module source acceptance through the common header/body/value coordinator.
/// Current declarations own this inventory; their `ModuleQuery` captures guarded header reuse.
#[derive(Debug)]
pub struct ModuleSourceUnit<'syntax> {
    declarations: declarations::ModuleContext<'syntax>,
    unit: accepted::Unit,
}
impl<'syntax> ModuleSourceUnit<'syntax> {
    /// Prepare explicit v2 module input; source topology and header agreement retain their owners.
    /// # Errors
    /// Reports source selection, contract, typing/safety or actual required-evaluation failures.
    pub fn prepare(
        input: &nocter_compile_input::CompileUnitInput<'syntax>,
    ) -> Result<Self, SourceFailure> {
        Self::prepare_with_limits(input, state::Limits::default())
    }
    /// Use the ordinary coordinator with explicit deterministic required-evaluation limits.
    /// # Errors
    /// Reports the same selected-module source failures as default preparation.
    pub fn prepare_with_limits(
        input: &nocter_compile_input::CompileUnitInput<'syntax>,
        limits: impl Into<state::Limits>,
    ) -> Result<Self, SourceFailure> {
        let headers = declarations::ModuleSources::prepare(input)?
            .pair()?
            .reserve_names()?;
        headers.validate_primitive_bindings()?;
        let (declarations, evaluation) = evaluation::prepare_module_source(headers, limits.into())?;
        let context = declarations.current()?;
        let unit = accepted::Unit::close_evaluation(&context, evaluation)?;
        Ok(Self { declarations, unit })
    }
    #[must_use]
    pub const fn declarations(&self) -> &declarations::ModuleContext<'syntax> {
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
    ) -> Result<Self, SourceFailure> {
        Self::prepare_with_primitive_authority_and_limits(
            syntax,
            authority,
            state::Limits::default(),
        )
    }
    /// Prepare authorized source with explicit deterministic required-evaluation limits.
    /// # Errors
    /// Reports the same whole-source failures as ordinary authorized preparation.
    pub fn prepare_with_primitive_authority_and_limits(
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
