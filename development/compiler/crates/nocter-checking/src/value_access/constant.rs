//! Definition-time operation-domain checks over the ordinary accepted body and resource ledger.
//! This is neither an evaluator nor evidence that a required constant has executed successfully.
use super::{CallKind, ExpressionKind, Failure, Statement, flow, source_origin};
use nocter_declarations::value_access::Program;
use nocter_diagnostics::{DiagnosticCode, SourceDiagnostic};
use nocter_model::{
    AccessCodeId, AccessExpressionId, AccessStatementId, CompileTimeGuarantee, TypeId,
    ValueFunctionOrigin,
};

mod captured;
pub(super) use captured::Availability;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rule {
    RuntimeCall(AccessCodeId),
    CapturedCall(nocter_model::ValueClosure),
    ValueInvocation,
    UnsupportedData(TypeId),
    UnsupportedInput(TypeId),
    UnsupportedCleanup(TypeId),
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

#[derive(Clone, Copy)]
enum Site {
    Expression(AccessExpressionId),
    Statement(AccessStatementId),
    Cleanup(flow::Location),
}
#[derive(Clone, Copy)]
pub(super) struct Violation {
    site: Site,
    rule: Rule,
}
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;

fn operation_rule(
    program: &Program,
    kind: &ExpressionKind,
    available: &dyn Fn(nocter_model::ValueClosure) -> bool,
) -> Result<Option<Rule>, flow::IntegrityError> {
    Ok(match kind {
        ExpressionKind::Call { kind, target, .. } => match (kind, target) {
            (_, ValueFunctionOrigin::Declaration(target)) => {
                let callable = program.codes().get(*target).ok_or(INVALID)?;
                (callable.compile_time() != CompileTimeGuarantee::Evaluatable)
                    .then_some(Rule::RuntimeCall(*target))
            }
            (CallKind::Value, ValueFunctionOrigin::Closure(code)) => {
                (!available(*code)).then_some(Rule::CapturedCall(*code))
            }
            (CallKind::Value, _) => Some(Rule::ValueInvocation),
            (CallKind::Direct, _) => return Err(INVALID),
        },
        ExpressionKind::Closure { .. }
        | ExpressionKind::Function(_)
        | ExpressionKind::AdaptFunction { .. }
        | ExpressionKind::RequirePrimary { .. }
        | ExpressionKind::GuardView { .. }
        | ExpressionKind::Scoped { .. }
        | ExpressionKind::PrimaryCondition { .. }
        | ExpressionKind::Block { .. }
        | ExpressionKind::Constant(_)
        | ExpressionKind::Integer(_)
        | ExpressionKind::Boolean(_)
        | ExpressionKind::Unit
        | ExpressionKind::Aggregate { .. }
        | ExpressionKind::Repeat { .. }
        | ExpressionKind::Select { .. }
        | ExpressionKind::Index { .. }
        | ExpressionKind::Place(_)
        | ExpressionKind::Move(_)
        | ExpressionKind::Boundary { .. }
        | ExpressionKind::Unary { .. }
        | ExpressionKind::Binary { .. }
        | ExpressionKind::If { .. } => None,
    })
}

fn call_inputs(
    program: &Program,
    expression: &ExpressionKind,
    domain: &mut super::evaluation::Types,
) -> Result<Option<Violation>, flow::IntegrityError> {
    let ExpressionKind::Call {
        target,
        kind,
        arguments,
    } = expression
    else {
        return Ok(None);
    };
    // Interrupted preparation performs no call. Concrete caller data cannot stand in for an
    // unclosed callee input shape, even when the callee never uses that parameter.
    if arguments.iter().any(|argument| {
        matches!(
            argument.action,
            super::ArgumentAction::Interrupted | super::ArgumentAction::Unproduced
        )
    }) {
        return Ok(None);
    }
    let parameters = match *target {
        ValueFunctionOrigin::Declaration(target) => program
            .codes()
            .get(target)
            .ok_or(INVALID)?
            .parameters()
            .iter()
            .map(|&id| Ok(program.parameters().get(id).ok_or(INVALID)?.data()))
            .collect::<Result<Vec<_>, flow::IntegrityError>>()?,
        ValueFunctionOrigin::Closure(code) => program
            .closure(code)
            .ok_or(INVALID)?
            .contract()
            .inputs
            .iter()
            .map(|input| input.data)
            .collect(),
        _ => return Ok(None),
    };
    let arguments = kind.explicit_arguments(arguments).ok_or(INVALID)?;
    if parameters.len() != arguments.len() {
        return Err(INVALID);
    }
    for (data, argument) in parameters.into_iter().zip(arguments) {
        if !domain.supports(program, data)? {
            return Ok(Some(Violation {
                site: Site::Expression(argument.value),
                rule: Rule::UnsupportedInput(data),
            }));
        }
    }
    Ok(None)
}

pub(super) fn check(
    query: &flow::Query,
    available: &dyn Fn(nocter_model::ValueClosure) -> bool,
) -> Result<Option<Violation>, flow::IntegrityError> {
    let program = query.typed().program();
    let mut domain = super::evaluation::Types::default();
    // Walk all typed expressions, not only reachable graph points. An unevaluated branch or
    // interrupted input cannot hide an authored runtime-only call or an unsupported operation.
    for (id, expression) in query.typed().body().expressions().iter() {
        let rule = operation_rule(program, &expression.kind, available)?;
        if let Some(rule) = rule {
            return Ok(Some(Violation {
                site: Site::Expression(id),
                rule,
            }));
        }
    }
    // An explicit drop has an operation-domain obligation even in an unevaluated branch.
    for (id, statement) in query.typed().body().statements().iter() {
        if let Statement::Drop(local) = statement {
            let data = query
                .typed()
                .body()
                .locals()
                .get(*local)
                .ok_or(INVALID)?
                .data;
            if !domain.supports(program, data)? {
                return Ok(Some(Violation {
                    site: Site::Statement(id),
                    rule: Rule::UnsupportedCleanup(data),
                }));
            }
        }
    }
    for (id, expression) in query.typed().body().expressions().iter() {
        if let Some(violation) = call_inputs(program, &expression.kind, &mut domain)? {
            return Ok(Some(violation));
        }
        // A known interrupted result is not a produced resource. There is no value to classify
        // here; operation checks above still apply independently of continuation.
        if let Some(value) = expression.ty.normal()
            && !domain.supports(program, value.data())?
        {
            return Ok(Some(Violation {
                site: Site::Expression(id),
                rule: Rule::UnsupportedData(value.data()),
            }));
        }
    }
    // Cleanup is an operation even when no source expression spells it. Consume actual sparse
    // ledger obligations, not annotations of every local (read inputs own nothing to destroy).
    for (point, block) in query.graph().points().iter() {
        for action in 0..block.actions.len() {
            let location = flow::Location { point, action };
            for transition in query.access().resources().at(location) {
                for step in transition.steps() {
                    let payload = match step {
                        flow::resources::Step::Dispose { payload, .. }
                        | flow::resources::Step::DisposeArrayRemainder { payload, .. }
                        | flow::resources::Step::DisposeArrayElements { payload, .. }
                        | flow::resources::Step::ReplaceReferent { payload, .. } => *payload,
                        flow::resources::Step::Capture { .. }
                        | flow::resources::Step::AdaptFunction { .. }
                        | flow::resources::Step::Assemble { .. }
                        | flow::resources::Step::Construct { .. }
                        | flow::resources::Step::Receive { .. }
                        | flow::resources::Step::Move { .. }
                        | flow::resources::Step::MoveArrayElement { .. }
                        | flow::resources::Step::ConsolidateArrayPrefix { .. }
                        | flow::resources::Step::ReleaseContainer { .. }
                        | flow::resources::Step::Invoke { .. } => continue,
                    };
                    if !domain.supports(program, payload.data())? {
                        return Ok(Some(Violation {
                            site: Site::Cleanup(location),
                            rule: Rule::UnsupportedCleanup(payload.data()),
                        }));
                    }
                }
            }
        }
    }
    Ok(None)
}

impl Violation {
    pub(super) fn project(self, context: &flow::Context<'_, '_>) -> Result<Diagnostic, Failure> {
        let typed = context.typed();
        let origin = match self.site {
            Site::Expression(id) => typed
                .expression_origin(id)
                .ok_or(Failure::InconsistentSyntax)?,
            Site::Statement(id) => typed
                .statement_origin(id)
                .ok_or(Failure::InconsistentSyntax)?,
            Site::Cleanup(location) => context.origin(location)?,
        };
        let message = match self.rule {
            Rule::RuntimeCall(target) => {
                let program = typed.query().program();
                let name = program
                    .code_label(target)
                    .ok_or(Failure::InconsistentSyntax)?;
                format!("const body calls runtime-only function `{name}`")
            }
            Rule::CapturedCall(_) => {
                "this captured callable is unavailable in constant evaluation".to_owned()
            }
            Rule::ValueInvocation => {
                "this callable witness has no closed constant-evaluation invocation".to_owned()
            }
            Rule::UnsupportedData(_) => {
                "this data operation is outside the private constant-evaluation domain".to_owned()
            }
            Rule::UnsupportedInput(_) => {
                "this callee input has no closed constant-evaluation representation".to_owned()
            }
            Rule::UnsupportedCleanup(_) => {
                "this cleanup requires a resource outside the private constant-evaluation domain"
                    .to_owned()
            }
        };
        Ok(Diagnostic {
            rule: self.rule,
            source: SourceDiagnostic::new(
                DiagnosticCode::E0421,
                message,
                source_origin(typed.syntax(), origin)?,
                [],
                None::<&str>,
            ),
        })
    }
}

/// Final-data evidence shares the shape authority but is narrower than temporary operations.
/// Neither Copy nor a temporary callable representation grants serialization permission.
pub(super) fn supports_final(
    program: &Program,
    data: TypeId,
) -> Result<bool, flow::IntegrityError> {
    super::evaluation::Types::default().supports_final(program, data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nocter_declarations::value_access::{CodeKind, DataReference, Reservations};
    use nocter_model::{BuiltinType, SymbolTable};

    #[test]
    fn homogeneous_array_domain_uses_shared_evaluator_shapes_without_expanding_counts() {
        for length in [0, 1, u64::MAX] {
            let symbols = SymbolTable::from_spellings(["run", "Holder", "value"]);
            let mut reservations = Reservations::new(symbols.clone());
            let holder = reservations.reserve_record(symbols.get("Holder").unwrap(), 1);
            let code = reservations.reserve(symbols.get("run").unwrap(), 0);
            let mut builder = reservations.finish().unwrap();
            builder
                .define(
                    code,
                    CodeKind::Function(CompileTimeGuarantee::RuntimeOnly),
                    [],
                    DataReference::Array {
                        element: Box::new(BuiltinType::I32.into()),
                        length,
                    },
                )
                .unwrap();
            builder
                .define_record(
                    holder,
                    false,
                    [nocter_declarations::value_access::FieldDefinition {
                        name: symbols.get("value").unwrap(),
                        data: DataReference::Array {
                            element: Box::new(BuiltinType::I32.into()),
                            length,
                        },
                    }],
                )
                .unwrap();
            let program = builder.finish().unwrap();
            let data = program.codes().get(code).unwrap().result();
            assert_eq!(program.data_copyable(data), Some(true));
            for data in [data, program.records().get(holder).unwrap().data()] {
                assert_eq!(
                    flow::resources::payload(&program, data).unwrap().disposal(),
                    flow::resources::Disposal::Aggregate
                );
                assert!(
                    super::super::evaluation::Types::default()
                        .supports(&program, data)
                        .unwrap()
                );
                assert!(supports_final(&program, data).unwrap());
            }
        }
    }
}
