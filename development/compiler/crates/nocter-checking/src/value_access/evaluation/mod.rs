//! Dependency-scheduled ordinary checking and required closed-plan execution.
pub(in crate::value_access) mod call_support;
mod dependencies;
mod discovery;
mod preparation;
mod projection;
mod types;
use super::accepted::Failure;
use super::{Failure as BodyFailure, flow};
use nocter_constant_evaluation::state;
use nocter_model::FrozenData;
pub(super) use types::Types;
#[cfg(test)]
pub(in crate::value_access) use types::received_test_frames;
#[derive(Clone, Debug)]
struct Completed {
    execution: state::Value,
    frozen: FrozenData,
}
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_diagnostics::{DiagnosticCode, DiagnosticNote, SourceDiagnostic};
use nocter_model::AccessCodeId;
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rule {
    Execution(state::Rule),
    DependencyCycle,
    FinalValue,
    ConditionWorkLimit,
    UnresolvedContext,
}
impl Rule {
    fn diagnostic(self) -> Result<(DiagnosticCode, &'static str), BodyFailure> {
        Ok(match self {
            Self::Execution(state::Rule::ArithmeticFailure) => (
                DiagnosticCode::E0325,
                "required constant evaluation encountered invalid checked arithmetic",
            ),
            Self::Execution(state::Rule::InactiveVariant) => (
                DiagnosticCode::E0325,
                "required constant evaluation asserted an inactive outcome variant",
            ),
            Self::Execution(state::Rule::IndexOutOfBounds) => (
                DiagnosticCode::E0325,
                "required constant evaluation selected an out-of-range array element",
            ),
            Self::Execution(state::Rule::UnavailablePlan) => (
                DiagnosticCode::E0323,
                "required constant evaluation has no matched implementation for this call",
            ),
            Self::Execution(state::Rule::InputConditionFailed) => (
                DiagnosticCode::E0323,
                "required constant evaluation did not satisfy an input-state condition",
            ),
            Self::Execution(state::Rule::PlanWorkLimit) => (
                DiagnosticCode::E0322,
                "required constant preparation exhausted its plan-work budget",
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
            Self::ConditionWorkLimit => (
                DiagnosticCode::E0322,
                "required constant analysis exhausted its condition-work budget",
            ),
            Self::UnresolvedContext | Self::Execution(state::Rule::UnresolvedContext) => (
                DiagnosticCode::E0323,
                "required constant analysis cannot establish one concrete value context",
            ),
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
    checked: &super::accepted::Checked,
    site: state::Site,
) -> Result<nocter_syntax::SyntaxOrigin, BodyFailure> {
    let bound = checked
        .code(site.code)
        .ok_or(BodyFailure::InconsistentSyntax)?
        .bind(context)?;
    bound.origin(flow::Location {
        point: site.point,
        action: site.action,
    })
}
fn source_origin(
    context: &DeclarationContext<'_>,
    origin: nocter_syntax::SyntaxOrigin,
) -> Result<nocter_source_index::SourceOrigin, BodyFailure> {
    context.source_origin(origin).map_err(Into::into)
}
fn project(
    context: &DeclarationContext<'_>,
    checked: &super::accepted::Checked,
    error: &state::Error,
) -> Result<Diagnostic, BodyFailure> {
    let root = checked
        .bodies
        .get(&error.code)
        .ok_or(BodyFailure::InconsistentSyntax)?
        .typed()
        .source
        .bind(context)?;
    let mut notes = vec![DiagnosticNote::new(
        "this expression requires a compile-time value",
        source_origin(context, root.origin())?,
    )];
    for &site in &error.trace {
        notes.push(DiagnosticNote::new(
            "constant evaluation depends on this operation",
            source_origin(context, origin(context, checked, site)?)?,
        ));
    }
    let origin = if let Some(site) = error.site {
        origin(context, checked, site)?
    } else {
        nocter_syntax::SyntaxOrigin::Node(root.body().ok_or(BodyFailure::InconsistentSyntax)?)
    };
    let rule = if error.rule == state::Rule::UnresolvedContext {
        Rule::UnresolvedContext
    } else {
        Rule::Execution(error.rule)
    };
    let (code, message) = rule.diagnostic()?;
    Ok(Diagnostic {
        rule,
        source: SourceDiagnostic::new(
            code,
            message,
            source_origin(context, origin)?,
            notes,
            None::<&str>,
        ),
    })
}
fn project_input_condition(
    context: &DeclarationContext<'_>,
    checked: &super::accepted::Checked,
    frame: &nocter_declarations::value_access::CodeInstanceKey,
    violation: super::constant::Violation,
    error: &state::Error,
) -> Result<super::constant::Diagnostic, BodyFailure> {
    let mut diagnostic = checked.project_const_violation(context, frame, violation)?;
    for site in &error.trace {
        diagnostic.note_call(source_origin(context, origin(context, checked, *site)?)?);
    }
    Ok(diagnostic)
}
fn final_diagnostic(
    context: &DeclarationContext<'_>,
    source: &nocter_declaration_lowering::access_declarations::BodySource,
) -> Result<Diagnostic, BodyFailure> {
    let source = source.bind(context)?;
    let primary = source_origin(
        context,
        nocter_syntax::SyntaxOrigin::Node(source.result_origin()),
    )?;
    let (code, message) = Rule::FinalValue.diagnostic()?;
    Ok(Diagnostic {
        rule: Rule::FinalValue,
        source: SourceDiagnostic::new(code, message, primary, [], None::<&str>),
    })
}
fn plan_failure(
    context: &DeclarationContext<'_>,
    checked: &super::accepted::Checked,
    code: nocter_model::ValueCodeRoot,
    failure: projection::Failure,
) -> Failure {
    if matches!(failure, projection::Failure::Integrity) {
        return Failure::Integrity(flow::IntegrityError::InconsistentTypedBody);
    }
    let failure = failure.into_plan();
    let error = state::Error {
        instance: None,
        rule: failure.rule,
        site: failure.site,
        code,
        trace: Box::new([]),
    };
    match project(context, checked, &error) {
        Ok(diagnostic) => Failure::Evaluation {
            callable: code.source_owner(),
            diagnostic: Box::new(diagnostic),
        },
        Err(failure) => Failure::Body {
            callable: code.source_owner(),
            failure,
        },
    }
}
pub(super) trait Values {
    fn value(&self, id: nocter_model::ValueCodeRoot) -> Option<&FrozenData>;
}
impl Values for () {
    fn value(&self, _: nocter_model::ValueCodeRoot) -> Option<&FrozenData> {
        None
    }
}
impl Values for dependencies::Dependencies {
    fn value(&self, id: nocter_model::ValueCodeRoot) -> Option<&FrozenData> {
        dependencies::value(self, id).map(|value| &value.frozen)
    }
}
pub(super) struct Output {
    pub(super) queries: BTreeMap<nocter_model::ValueCodeRoot, super::Query>,
    pub(super) values: BTreeMap<nocter_model::ValueCodeRoot, Arc<FrozenData>>,
    pub(super) header_sources: nocter_declaration_lowering::access_declarations::BodySources,
}
pub(super) use preparation::{prepare, prepare_module_source, prepare_source};
fn failure_at(
    context: &DeclarationContext<'_>,
    code: AccessCodeId,
    rule: Rule,
    origin: nocter_syntax::SyntaxOrigin,
) -> Failure {
    let primary = match source_origin(context, origin) {
        Ok(primary) => primary,
        Err(failure) => {
            return Failure::Body {
                callable: code,
                failure,
            };
        }
    };
    failure_at_source(code, rule, primary)
}
pub(super) fn condition_step(
    work: &mut state::WorkBudget,
    code: AccessCodeId,
    primary: nocter_source_index::SourceOrigin,
) -> Result<(), Failure> {
    work.step()
        .map_err(|_| failure_at_source(code, Rule::ConditionWorkLimit, primary))
}
pub(super) fn condition_failure(
    failure: super::InstantiationFailure,
    code: AccessCodeId,
    primary: nocter_source_index::SourceOrigin,
) -> Failure {
    match failure {
        super::InstantiationFailure::WorkLimit => {
            failure_at_source(code, Rule::ConditionWorkLimit, primary)
        }
        super::InstantiationFailure::Unresolved => {
            failure_at_source(code, Rule::UnresolvedContext, primary)
        }
        _ => Failure::Integrity(flow::IntegrityError::InconsistentTypedBody),
    }
}
fn failure_at_source(
    code: AccessCodeId,
    rule: Rule,
    primary: nocter_source_index::SourceOrigin,
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
    let source = SourceDiagnostic::new(diagnostic_code, message, primary, [], None::<&str>);
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
    let origin = source_origin(context, origin)?;
    if diagnostic.source.primary() == nocter_diagnostics::DiagnosticOrigin::from(origin) {
        return Ok(());
    }
    diagnostic.source.push_note(DiagnosticNote::new(
        "constant evaluation depends on this operation",
        origin,
    ));
    Ok(())
}
