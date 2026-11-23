//! Definition-time operation-domain checks over the ordinary accepted body and resource ledger.
//! This is neither an evaluator nor evidence that a required constant has executed successfully.
use super::{CallKind, ExpressionKind, Failure, Statement, flow, source_origin};
use nocter_declarations::value_access::{CodeInstanceKey, FunctionWitness, PayloadSchema, Program};
use nocter_diagnostics::{DiagnosticCode, SourceDiagnostic};
use nocter_model::{
    AccessCodeId, AccessExpressionId, AccessStatementId, CompileTimeGuarantee, TypeId,
    ValueFunctionOrigin,
};

mod catalog;
mod conditions;
mod requirements;
mod traversal;
mod value_context;
pub use requirements::Requirements;
pub(in crate::value_access) use requirements::{
    BoundCheck, BoundCheckFailure, InvocationCondition,
};
mod representation;
pub(super) use catalog::Catalog;
use traversal::Traversal;

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
    pub(super) fn note_witness(&mut self, origin: nocter_source_index::SourceOrigin) {
        self.source
            .push_note(nocter_diagnostics::DiagnosticNote::new(
                "this argument supplies the callable witness required by this operation",
                origin,
            ));
    }
    pub(super) fn note_call(&mut self, origin: nocter_source_index::SourceOrigin) {
        self.source
            .push_note(nocter_diagnostics::DiagnosticNote::new(
                "required constant invocation depends on this call",
                origin,
            ));
    }
    #[must_use]
    pub const fn rule(&self) -> Rule {
        self.rule
    }
    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}

#[derive(Clone, Copy, Debug)]
enum Site {
    Expression(AccessExpressionId),
    Statement(AccessStatementId),
    Cleanup(flow::Location),
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Violation {
    site: Site,
    rule: Rule,
}
const INVALID: flow::IntegrityError = flow::IntegrityError::InconsistentTypedBody;

fn operation_rule(
    program: &Program,
    kind: &ExpressionKind,
    available: &dyn Fn(nocter_model::ValueClosure) -> bool,
    callee: Option<ValueFunctionOrigin>,
) -> Result<Option<Rule>, flow::IntegrityError> {
    Ok(match kind {
        ExpressionKind::Await { .. } => return Err(INVALID),
        ExpressionKind::Call {
            kind,
            target,
            arguments,
        } => {
            // The template selected an input witness, not a fixed source operation. Its
            // invocation obligation is inactive when frozen preparation cannot commit a call.
            // Receiver/environment operations retain their independent data/cleanup clauses.
            if uncommitted_parameter_call(*kind, *target, arguments) {
                return Ok(None);
            }
            // A field signature supplies no capability. Interrupted preparation needs a
            // witness from the original receiver independently of invocation completion.
            if matches!(target, ValueFunctionOrigin::Contract(_))
                && !super::instances::completed(arguments)
                && callee.is_none()
            {
                return Ok(Some(Rule::ValueInvocation));
            }
            let target = match callee {
                Some(callee) => callee,
                None => *target,
            };
            invocation_rule(program, *kind, target, available)?
        }
        ExpressionKind::Closure { .. }
        | ExpressionKind::Function(_)
        | ExpressionKind::AdaptFunction { .. }
        | ExpressionKind::RequirePrimary { .. }
        | ExpressionKind::GuardView { .. }
        | ExpressionKind::Scoped { .. }
        | ExpressionKind::PrimaryCondition { .. }
        | ExpressionKind::Block { .. }
        | ExpressionKind::Constant(_)
        | ExpressionKind::GenericConstant(_)
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
        | ExpressionKind::If { .. }
        | ExpressionKind::EnumMatch { .. } => None,
    })
}

fn interrupted_contract_call(expression: &ExpressionKind) -> bool {
    matches!(expression, ExpressionKind::Call {
        kind: CallKind::Value,
        target: ValueFunctionOrigin::Contract(_),
        arguments,
    } if !super::instances::completed(arguments))
}

fn uncommitted_parameter_call(
    kind: CallKind,
    target: ValueFunctionOrigin,
    arguments: &[super::Argument],
) -> bool {
    kind == CallKind::Value
        && matches!(target, ValueFunctionOrigin::Parameter(_))
        && !super::instances::completed(arguments)
}
fn invocation_rule(
    program: &Program,
    kind: CallKind,
    target: ValueFunctionOrigin,
    available: &dyn Fn(nocter_model::ValueClosure) -> bool,
) -> Result<Option<Rule>, flow::IntegrityError> {
    Ok(match (kind, target) {
        (_, ValueFunctionOrigin::Declaration(target)) => {
            let callable = program.codes().get(target).ok_or(INVALID)?;
            (callable.compile_time() != CompileTimeGuarantee::Evaluatable)
                .then_some(Rule::RuntimeCall(target))
        }
        (CallKind::Value, ValueFunctionOrigin::Closure(code)) => {
            (!available(code)).then_some(Rule::CapturedCall(code))
        }
        (CallKind::Value, ValueFunctionOrigin::Parameter(_) | ValueFunctionOrigin::Contract(_)) => {
            None
        }
        (CallKind::Value, _) => Some(Rule::ValueInvocation),
        (CallKind::Direct, _) => return Err(INVALID),
    })
}

fn supports(
    program: &Program,
    data: TypeId,
    domain: &mut super::evaluation::Types,
    instance: Option<&CodeInstanceKey>,
) -> Result<bool, flow::IntegrityError> {
    match instance {
        Some(instance) => domain.supports_in(program, instance, data),
        None => domain.supports_contract(program, data),
    }
}

fn call_inputs(
    program: &Program,
    expression: &ExpressionKind,
    domain: &mut super::evaluation::Types,
    callee: Option<&CodeInstanceKey>,
) -> Result<Option<Violation>, flow::IntegrityError> {
    let ExpressionKind::Call {
        target,
        kind,
        arguments,
    } = expression
    else {
        return Ok(None);
    };
    // Interrupted preparation performs no call and requires no completed callee instance.
    if !super::instances::completed(arguments) {
        return Ok(None);
    }
    let target = match callee {
        Some(key) => key.code().origin().ok_or(INVALID)?,
        None => *target,
    };
    let Some(contract) = input_contract(program, target, *kind, arguments)? else {
        return Ok(None);
    };
    let parameters = contract.inputs;
    let arguments = kind.explicit_arguments(arguments).ok_or(INVALID)?;
    if parameters.len() != arguments.len() {
        return Err(INVALID);
    }
    for (index, (input, argument)) in parameters.into_iter().zip(arguments).enumerate() {
        let data = input.data;
        let supported = match callee {
            Some(instance) => domain.supports_input(program, instance, index, data)?,
            None => supports(program, data, domain, None)?,
        };
        if !supported {
            return Ok(Some(Violation {
                site: Site::Expression(argument.value),
                rule: Rule::UnsupportedInput(data),
            }));
        }
    }
    Ok(None)
}

fn input_contract(
    program: &Program,
    target: ValueFunctionOrigin,
    kind: CallKind,
    arguments: &super::CallArguments,
) -> Result<Option<nocter_declarations::value_access::FunctionConstraint>, flow::IntegrityError> {
    if !matches!(
        target,
        ValueFunctionOrigin::Declaration(_) | ValueFunctionOrigin::Closure(_)
    ) {
        return Ok(None);
    }
    super::instances::call_contract(
        program,
        FunctionWitness {
            origin: target,
            invocation: kind.invocation(arguments).ok_or(INVALID)?,
        },
        arguments,
    )
    .map(Some)
    .ok_or(INVALID)
}

impl Violation {
    /// Preserve the ordinary witness position behind a bound condition failure.
    pub(super) fn dependency(
        self,
        query: &flow::Query,
        key: &CodeInstanceKey,
    ) -> Result<Option<nocter_model::ValueParameter>, flow::IntegrityError> {
        if query.typed().code() != key.code() {
            return Err(INVALID);
        }
        let body = query.typed().body();
        let data = match self.rule {
            Rule::RuntimeCall(_) | Rule::CapturedCall(_) | Rule::ValueInvocation => {
                let Site::Expression(id) = self.site else {
                    return Err(INVALID);
                };
                let ExpressionKind::Call { target, .. } =
                    body.expressions().get(id).ok_or(INVALID)?.kind
                else {
                    return Err(INVALID);
                };
                return match target {
                    ValueFunctionOrigin::Parameter(parameter)
                        if key.witnesses().contains_key(&parameter) =>
                    {
                        Ok(Some(parameter))
                    }
                    _ => Ok(None),
                };
            }
            Rule::UnsupportedInput(_) => {
                let Site::Expression(id) = self.site else {
                    return Err(INVALID);
                };
                body.expressions()
                    .get(id)
                    .ok_or(INVALID)?
                    .ty
                    .access()
                    .map(nocter_model::ValueAccess::data)
            }
            Rule::UnsupportedData(data) | Rule::UnsupportedCleanup(data) => Some(data),
        };
        let Some(data) = data else {
            return Ok(None);
        };
        Ok(
            match query.typed().program().data_schema(data).ok_or(INVALID)? {
                PayloadSchema::Function(FunctionWitness {
                    origin: ValueFunctionOrigin::Parameter(parameter),
                    ..
                }) if key.witnesses().contains_key(&parameter) => Some(parameter),
                _ => None,
            },
        )
    }
    pub(super) const fn rule(self) -> Rule {
        self.rule
    }
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
                        element: std::sync::Arc::new(BuiltinType::I32.into()),
                        length: length.into(),
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
                            element: std::sync::Arc::new(BuiltinType::I32.into()),
                            length: length.into(),
                        },
                    }],
                )
                .unwrap();
            let program = builder.finish().unwrap();
            let data = program.codes().get(code).unwrap().result();
            assert_eq!(program.data_copyable(data), Some(true));
            for data in [data, program.record(holder).unwrap().data()] {
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
    #[test]
    fn an_incomplete_symbolic_invocation_does_not_turn_a_direct_parameter_call_into_valid_ir() {
        use crate::value_access::{
            accepted::SourceUnit,
            tests::{bound, input},
        };
        let (sources, tree) = input(
            "const func f(action: func(i32): i32): i32 { action(if true { return 42 } else { return 0 }) }",
        );
        let unit = SourceUnit::prepare(bound(&sources, &tree))
            .unwrap()
            .into_unit();
        let (_, body) = unit.bodies().next().unwrap();
        let (_, expression) = body
            .typed()
            .body()
            .expressions()
            .iter()
            .find(|(_, expression)| matches!(expression.kind, ExpressionKind::Call { .. }))
            .unwrap();
        let ExpressionKind::Call {
            target, arguments, ..
        } = &expression.kind
        else {
            unreachable!();
        };
        let malformed = ExpressionKind::Call {
            target: *target,
            kind: CallKind::Direct,
            arguments: arguments.clone(),
        };
        assert!(operation_rule(unit.program(), &malformed, &|_| true, None).is_err());
    }
}
