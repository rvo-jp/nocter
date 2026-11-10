//! Definition-time operation-domain checks over the ordinary accepted body and resource ledger.
//! This is neither an evaluator nor evidence that a required constant has executed successfully.
use super::{CallKind, ExpressionKind, Failure, Statement, flow, source_origin};
use nocter_declarations::value_access::{PayloadSchema, Program};
use nocter_diagnostics::{DiagnosticCode, SourceDiagnostic};
use nocter_model::{
    AccessCallableId, AccessExpressionId, AccessStatementId, CompileTimeGuarantee, TypeId,
    ValueFunctionOrigin,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rule {
    RuntimeCall(AccessCallableId),
    FunctionValue,
    ValueInvocation,
    UnsupportedData(TypeId),
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
pub(super) struct Violation {
    site: Site,
    rule: Rule,
}
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;

/// The declaration owner already proves finite record schemas. Memoize by type identity rather
/// than expanding repeated fields into a tree; this is domain support, not a Copy/resource rule.
enum Domain {
    Execution,
    Publication,
}
struct DataDomain<'a> {
    purpose: Domain,
    program: &'a Program,
    supported: BTreeMap<TypeId, bool>,
}
impl DataDomain<'_> {
    fn supports(&mut self, data: TypeId) -> Result<bool, flow::IntegrityError> {
        if let Some(&supported) = self.supported.get(&data) {
            return Ok(supported);
        }
        let mut pending = vec![(data, false)];
        while let Some((data, expanded)) = pending.pop() {
            if self.supported.contains_key(&data) {
                continue;
            }
            let supported = match self.program.data_schema(data).ok_or(INVALID)? {
                PayloadSchema::Builtin(builtin) => match self.purpose {
                    Domain::Execution => {
                        nocter_constant_evaluation::state::Type::builtin(builtin).is_some()
                    }
                    Domain::Publication => matches!(
                        builtin,
                        nocter_model::BuiltinType::Bool
                            | nocter_model::BuiltinType::I32
                            | nocter_model::BuiltinType::Void
                    ),
                },
                PayloadSchema::Function(_) => false,
                PayloadSchema::Record(record) => {
                    let children = record
                        .fields()
                        .iter()
                        .map(|field| {
                            self.program
                                .fields()
                                .get(*field)
                                .map(|field| field.data())
                                .ok_or(INVALID)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    if !expanded {
                        pending.push((data, true));
                        pending.extend(children.into_iter().rev().map(|child| (child, false)));
                        continue;
                    }
                    children
                        .iter()
                        .all(|child| self.supported.get(child) == Some(&true))
                }
            };
            self.supported.insert(data, supported);
        }
        self.supported.get(&data).copied().ok_or(INVALID)
    }
}

fn operation_rule(
    program: &Program,
    kind: &ExpressionKind,
) -> Result<Option<Rule>, flow::IntegrityError> {
    Ok(match kind {
        ExpressionKind::Call { kind, target, .. } => match (kind, target) {
            (CallKind::Direct, ValueFunctionOrigin::Declaration(target)) => {
                let callable = program.callables().get(*target).ok_or(INVALID)?;
                (callable.compile_time() != CompileTimeGuarantee::Evaluatable)
                    .then_some(Rule::RuntimeCall(*target))
            }
            (CallKind::Value, _) => Some(Rule::ValueInvocation),
            (CallKind::Direct, _) => return Err(INVALID),
        },
        ExpressionKind::Function(_)
        | ExpressionKind::Closure { .. }
        | ExpressionKind::AdaptFunction { .. } => Some(Rule::FunctionValue),
        ExpressionKind::Constant(_)
        | ExpressionKind::Integer(_)
        | ExpressionKind::Boolean(_)
        | ExpressionKind::Unit
        | ExpressionKind::Record { .. }
        | ExpressionKind::Select { .. }
        | ExpressionKind::Place(_)
        | ExpressionKind::Move(_)
        | ExpressionKind::Boundary { .. }
        | ExpressionKind::Unary { .. }
        | ExpressionKind::Binary { .. }
        | ExpressionKind::If { .. } => None,
    })
}

pub(super) fn check(query: &flow::Query) -> Result<Option<Violation>, flow::IntegrityError> {
    let program = query.typed().program();
    let mut domain = DataDomain {
        purpose: Domain::Execution,
        program,
        supported: BTreeMap::new(),
    };
    // Walk all typed expressions, not only reachable graph points. An unevaluated branch or
    // interrupted input cannot hide an authored runtime-only call or an unsupported operation.
    for (id, expression) in query.typed().body().expressions().iter() {
        let rule = operation_rule(program, &expression.kind)?;
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
            if !domain.supports(data)? {
                return Ok(Some(Violation {
                    site: Site::Statement(id),
                    rule: Rule::UnsupportedCleanup(data),
                }));
            }
        }
    }
    for (id, expression) in query.typed().body().expressions().iter() {
        // A known interrupted result is not a produced resource. There is no value to classify
        // here; operation checks above still apply independently of continuation.
        if let Some(value) = expression.ty.normal()
            && !domain.supports(value.data())?
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
                        | flow::resources::Step::ReplaceInput { payload, .. } => *payload,
                        flow::resources::Step::Capture { .. }
                        | flow::resources::Step::AdaptFunction { .. }
                        | flow::resources::Step::Assemble { .. }
                        | flow::resources::Step::Construct { .. }
                        | flow::resources::Step::Receive { .. }
                        | flow::resources::Step::Move { .. }
                        | flow::resources::Step::ReleaseContainer { .. }
                        | flow::resources::Step::Invoke { .. } => continue,
                    };
                    if !domain.supports(payload.data())? {
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
                let name = program.callables().get(target).and_then(|c| program.symbols().spelling(c.name()))
                    .ok_or(Failure::InconsistentSyntax)?;
                format!("const body calls runtime-only function `{name}`")
            }
            Rule::FunctionValue => "function values and captures are outside the private constant-evaluation domain".to_owned(),
            Rule::ValueInvocation => "invocation through a function value is outside the private constant-evaluation domain".to_owned(),
            Rule::UnsupportedData(_) => "this data operation is outside the private constant-evaluation domain".to_owned(),
            Rule::UnsupportedCleanup(_) => "this cleanup requires a resource outside the private constant-evaluation domain".to_owned(),
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

/// The admitted finite record schema has no user destructor or retained-resource variants.
/// Final admission is independent of Copy; broader schemas require their own final-data proof.
pub(super) fn supports_final(
    program: &Program,
    data: TypeId,
) -> Result<bool, flow::IntegrityError> {
    DataDomain {
        purpose: Domain::Publication,
        program,
        supported: BTreeMap::new(),
    }
    .supports(data)
}
