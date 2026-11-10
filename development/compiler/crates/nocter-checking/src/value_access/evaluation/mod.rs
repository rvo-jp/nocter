//! Dependency-scheduled ordinary checking and required closed-plan execution.
mod preparation;
mod projection;
use super::accepted::Failure;
use super::{Failure as BodyFailure, flow, source_origin};
use nocter_constant_evaluation::state;
use nocter_model::FrozenData;
struct Completed {
    _execution: state::Value,
    frozen: FrozenData,
}
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_diagnostics::{DiagnosticCode, DiagnosticNote, SourceDiagnostic};
use nocter_model::AccessCallableId;
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rule {
    Execution(state::Rule),
    DependencyCycle,
    FinalValue,
}
impl Rule {
    fn diagnostic(self) -> Result<(DiagnosticCode, &'static str), BodyFailure> {
        Ok(match self {
            Self::Execution(state::Rule::ArithmeticFailure) => (
                DiagnosticCode::E0325,
                "required constant evaluation encountered invalid checked arithmetic",
            ),
            Self::Execution(state::Rule::StepLimit) => (
                DiagnosticCode::E0322,
                "required constant evaluation exhausted its operation budget",
            ),
            Self::Execution(state::Rule::StorageLimit) => (
                DiagnosticCode::E0322,
                "required constant evaluation exhausted its live-storage budget",
            ),
            Self::Execution(state::Rule::CallDepthLimit) => (
                DiagnosticCode::E0322,
                "required constant evaluation exhausted its call-depth budget",
            ),
            Self::Execution(state::Rule::InvalidPlan | state::Rule::ReachedUnreachable) => {
                return Err(BodyFailure::InconsistentSyntax);
            }
            Self::DependencyCycle => (
                DiagnosticCode::E0324,
                "required constant evaluation contains an initializer dependency cycle",
            ),
            Self::FinalValue => (
                DiagnosticCode::E0323,
                "this data type is outside the private constant-publication domain",
            ),
        })
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    rule: Rule,
    source: SourceDiagnostic,
}
impl Diagnostic {
    #[must_use]
    pub const fn rule(&self) -> Rule {
        self.rule
    }
    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}
fn origin(
    context: &DeclarationContext<'_>,
    bodies: &BTreeMap<AccessCallableId, flow::Query>,
    site: state::Site,
) -> Result<nocter_syntax::SyntaxOrigin, BodyFailure> {
    let bound = bodies
        .get(&site.code)
        .ok_or(BodyFailure::InconsistentSyntax)?
        .bind(context)?;
    bound.origin(flow::Location {
        point: site.point,
        action: site.action,
    })
}
fn project(
    context: &DeclarationContext<'_>,
    bodies: &BTreeMap<AccessCallableId, flow::Query>,
    error: &state::Error,
) -> Result<Diagnostic, BodyFailure> {
    let root = context
        .callable_source(error.code)
        .ok_or(BodyFailure::InconsistentSyntax)?;
    let mut notes = vec![DiagnosticNote::new(
        "this initializer requires a compile-time value",
        source_origin(
            context.syntax(),
            nocter_syntax::SyntaxOrigin::Token(root.name()),
        )?,
    )];
    for &site in &error.trace {
        notes.push(DiagnosticNote::new(
            "constant evaluation depends on this operation",
            source_origin(context.syntax(), origin(context, bodies, site)?)?,
        ));
    }
    let origin = if let Some(site) = error.site {
        origin(context, bodies, site)?
    } else {
        nocter_syntax::SyntaxOrigin::Node(root.body())
    };
    let (code, message) = Rule::Execution(error.rule).diagnostic()?;
    Ok(Diagnostic {
        rule: Rule::Execution(error.rule),
        source: SourceDiagnostic::new(
            code,
            message,
            source_origin(context.syntax(), origin)?,
            notes,
            None::<&str>,
        ),
    })
}
fn final_diagnostic(
    context: &DeclarationContext<'_>,
    id: AccessCallableId,
) -> Result<Diagnostic, BodyFailure> {
    let source = context
        .callable_source(id)
        .ok_or(BodyFailure::InconsistentSyntax)?;
    let primary = source_origin(
        context.syntax(),
        nocter_syntax::SyntaxOrigin::Node(source.result()),
    )?;
    let (code, message) = Rule::FinalValue.diagnostic()?;
    Ok(Diagnostic {
        rule: Rule::FinalValue,
        source: SourceDiagnostic::new(code, message, primary, [], None::<&str>),
    })
}
pub(super) trait Values {
    fn value(&self, id: AccessCallableId) -> Option<&FrozenData>;
}
impl Values for () {
    fn value(&self, _: AccessCallableId) -> Option<&FrozenData> {
        None
    }
}
impl Values for nocter_constant_evaluation::DependencyQuery<AccessCallableId, Completed> {
    fn value(&self, id: AccessCallableId) -> Option<&FrozenData> {
        self.completed(&id).map(|value| &value.frozen)
    }
}
pub(super) struct Output {
    pub(super) queries: BTreeMap<AccessCallableId, super::Query>,
    pub(super) values: BTreeMap<AccessCallableId, Arc<FrozenData>>,
    pub(super) dependencies: Arc<super::source_dependencies::SourceDependencies>,
}
pub(super) use preparation::prepare;
fn failure_at(
    context: &DeclarationContext<'_>,
    code: AccessCallableId,
    rule: Rule,
    origin: nocter_syntax::SyntaxOrigin,
) -> Failure {
    let (diagnostic_code, message) = match rule.diagnostic() {
        Ok(details) => details,
        Err(failure) => {
            return Failure::Body {
                callable: code,
                failure,
            };
        }
    };
    let source = match source_origin(context.syntax(), origin) {
        Ok(primary) => SourceDiagnostic::new(diagnostic_code, message, primary, [], None::<&str>),
        Err(failure) => {
            return Failure::Body {
                callable: code,
                failure,
            };
        }
    };
    Failure::Evaluation {
        callable: code,
        diagnostic: Box::new(Diagnostic { rule, source }),
    }
}
fn add_dependency_note(
    failure: &mut Failure,
    context: &DeclarationContext<'_>,
    origin: nocter_syntax::SyntaxOrigin,
) -> Result<(), BodyFailure> {
    let Failure::Evaluation { diagnostic, .. } = failure else {
        return Ok(());
    };
    let origin = source_origin(context.syntax(), origin)?;
    if diagnostic.source.primary() == nocter_diagnostics::DiagnosticOrigin::from(origin) {
        return Ok(());
    }
    diagnostic.source.push_note(DiagnosticNote::new(
        "constant evaluation depends on this operation",
        origin,
    ));
    Ok(())
}
