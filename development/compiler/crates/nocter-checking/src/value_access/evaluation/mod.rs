//! Required initializer evaluation after ordinary whole-unit and const-domain checks.
mod dependencies;
mod projection;
use super::accepted::Failure;
use super::{Failure as BodyFailure, flow, source_origin};
use nocter_constant_evaluation::{
    CompileTimeEvaluationLimits,
    state::{self, Value},
};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::{CallableKind, Program};
use nocter_diagnostics::{DiagnosticCode, DiagnosticNote, SourceDiagnostic};
use nocter_model::{AccessCallableId, BuiltinType};
use std::{collections::BTreeMap, sync::Arc};
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rule {
    Execution(state::Rule),
    FinalValue,
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
        .get(&site.code())
        .ok_or(BodyFailure::InconsistentSyntax)?
        .bind(context)?;
    match site {
        state::Site::Action { point, action, .. } => bound.origin(flow::Location { point, action }),
        state::Site::Expression { expression, .. } => bound
            .typed()
            .expression_origin(expression)
            .ok_or(BodyFailure::InconsistentSyntax),
    }
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
    let (code, message) = match error.rule {
        state::Rule::ArithmeticFailure => (
            DiagnosticCode::E0325,
            "required constant evaluation encountered invalid checked arithmetic",
        ),
        state::Rule::StepLimit => (
            DiagnosticCode::E0322,
            "required constant evaluation exhausted its operation budget",
        ),
        state::Rule::CallDepthLimit => (
            DiagnosticCode::E0322,
            "required constant evaluation exhausted its call-depth budget",
        ),
        state::Rule::DependencyCycle => (
            DiagnosticCode::E0324,
            "required constant evaluation contains an initializer dependency cycle",
        ),
        state::Rule::InvalidPlan | state::Rule::ReachedUnreachable => {
            return Err(BodyFailure::InconsistentSyntax);
        }
    };
    let message = if error.rule == state::Rule::DependencyCycle {
        let names = error
            .dependency_cycle
            .iter()
            .map(|id| {
                context
                    .program()
                    .callables()
                    .get(*id)
                    .and_then(|c| context.program().symbols().spelling(c.name()))
                    .ok_or(BodyFailure::InconsistentSyntax)
            })
            .collect::<Result<Vec<_>, _>>()?;
        format!("{message}: {}", names.join(" -> "))
    } else {
        message.to_owned()
    };
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
    Ok(Diagnostic {
        rule: Rule::FinalValue,
        source: SourceDiagnostic::new(
            DiagnosticCode::E0323,
            "private constant publication currently admits only bool, i32 and void values",
            primary,
            [],
            None::<&str>,
        ),
    })
}
pub(super) struct Output {
    pub(super) values: BTreeMap<AccessCallableId, Arc<Value>>,
    pub(super) dependencies: Arc<super::source_dependencies::SourceDependencies>,
}
pub(super) fn evaluate(
    context: &DeclarationContext<'_>,
    program: &Program,
    bodies: &BTreeMap<AccessCallableId, flow::Query>,
    limits: CompileTimeEvaluationLimits,
) -> Result<Output, Failure> {
    for (id, declaration) in program.callables().iter() {
        if declaration.kind() == CallableKind::ConstantInitializer
            && !matches!(
                program.data_kind(declaration.result()),
                Some(BuiltinType::I32 | BuiltinType::Bool | BuiltinType::Void)
            )
        {
            let diagnostic = final_diagnostic(context, id).map_err(|failure| Failure::Body {
                callable: id,
                failure,
            })?;
            return Err(Failure::Evaluation {
                callable: id,
                diagnostic: Box::new(diagnostic),
            });
        }
    }
    let plans = projection::prepare(program, bodies).map_err(Failure::Integrity)?;
    let mut dependencies = super::source_dependencies::SourceDependencies::default();
    for id in &plans.sources {
        dependencies.insert(
            *id,
            bodies
                .get(id)
                .ok_or(Failure::Integrity(INVALID))?
                .typed()
                .body_bytes
                .clone(),
        );
    }
    let mut evaluator = state::Executor::new(&plans.program, limits);
    for (id, declaration) in program.callables().iter() {
        if declaration.kind() != CallableKind::ConstantInitializer {
            continue;
        }
        let value = match evaluator.evaluate_initializer(id) {
            Ok(value) => value,
            Err(error) => {
                if matches!(
                    error.rule,
                    state::Rule::InvalidPlan | state::Rule::ReachedUnreachable
                ) {
                    return Err(Failure::Integrity(INVALID));
                }
                let diagnostic =
                    project(context, bodies, &error).map_err(|failure| Failure::Body {
                        callable: id,
                        failure,
                    })?;
                return Err(Failure::Evaluation {
                    callable: id,
                    diagnostic: Box::new(diagnostic),
                });
            }
        };
        if !matches!(
            (program.data_kind(declaration.result()), value.as_ref()),
            (Some(BuiltinType::I32), Value::I32(_))
                | (Some(BuiltinType::Bool), Value::Bool(_))
                | (Some(BuiltinType::Void), Value::Unit)
        ) {
            return Err(Failure::Integrity(INVALID));
        }
    }
    Ok(Output {
        values: evaluator.into_completed(),
        dependencies: Arc::new(dependencies),
    })
}
/// The same expression/data identities, origins, control and resource products survive literal
/// materialization. No name lookup, type inference, flow solving or source reparse occurs here.
pub(super) fn materialize(
    query: &super::Query,
    values: &BTreeMap<AccessCallableId, Arc<Value>>,
    dependencies: &Arc<super::source_dependencies::SourceDependencies>,
) -> Result<super::Query, flow::IntegrityError> {
    let mut query = query.clone();
    query.evaluation_source = Some(dependencies.clone());
    let mut invalid = false;
    query.body.expressions = query.body.expressions.map(|_, expression| {
        let mut expression = expression.clone();
        if let super::ExpressionKind::Constant(id) = expression.kind {
            expression.kind = match values.get(&id).map(Arc::as_ref) {
                Some(Value::I32(value))
                    if expression.ty.access().is_some_and(|t| {
                        query.program().data_kind(t.data()) == Some(BuiltinType::I32)
                    }) =>
                {
                    super::ExpressionKind::Integer(*value)
                }
                Some(Value::Bool(value))
                    if expression.ty.access().is_some_and(|t| {
                        query.program().data_kind(t.data()) == Some(BuiltinType::Bool)
                    }) =>
                {
                    super::ExpressionKind::Boolean(*value)
                }
                Some(Value::Unit)
                    if expression.ty.access().is_some_and(|t| {
                        query.program().data_kind(t.data()) == Some(BuiltinType::Void)
                    }) =>
                {
                    super::ExpressionKind::Unit
                }
                _ => {
                    invalid = true;
                    super::ExpressionKind::Constant(id)
                }
            };
        }
        expression
    });
    if invalid {
        return Err(INVALID);
    }
    for nested in query.closures.values_mut() {
        nested.typed = Arc::new(materialize(&nested.typed, values, dependencies)?);
    }
    Ok(query)
}
