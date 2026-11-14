//! Frozen caller-to-body input channels and normal adapter cleanup, without target selection.
use super::{Failure, InstanceKey, Payload};
use nocter_checking::value_access::{BodyInput, CallKind, CallValueSources, accepted::Unit};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_model::{AccessExpressionId, AccessInstanceId, TypeId, ValueCode, ValueInputOperation};
use nocter_runtime_contract::PrimitiveRole;

const INVALID: Failure = Failure::InvalidAcceptedUnit;

/// Normal completion only; this does not invent unwind or cancellation cleanup guarantees.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvironmentExit {
    Retained,
    /// The checked body owns the environment's remaining-payload cleanup.
    BodyOwned,
    /// The adapter owns this post-call environment until disposal before answer delivery.
    DisposeBeforeResult(Payload),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Environment {
    None,
    /// A named function value has a code token but no environment input in its body.
    CodeToken {
        /// Slot in the committed prepared bundle, not the original source place.
        argument: usize,
        supplied: ValueInputOperation,
        carrier: Payload,
        exit: EnvironmentExit,
    },
    Capture {
        /// The supplied instance: a Copy snapshot can differ from its source owner.
        argument: usize,
        supplied: ValueInputOperation,
        carrier: Payload,
        input: BodyInput,
        payload: Payload,
        exit: EnvironmentExit,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputBinding {
    /// Slot in the committed prepared bundle.
    pub argument: usize,
    /// The explicit input's ordinal is its position in this list; no callee local is invented.
    pub operation: ValueInputOperation,
    /// Concrete data obligation; the input operation, not this schema, grants ownership.
    pub payload: Payload,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallTarget {
    Body(AccessInstanceId),
    Primitive(PrimitiveRole),
}
impl CallTarget {
    #[must_use]
    pub const fn instance(self) -> Option<AccessInstanceId> {
        match self {
            Self::Body(id) => Some(id),
            Self::Primitive(_) => None,
        }
    }
}
/// One reached invocation, paired with its concrete target and checked input-slot mapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Invocation {
    target: CallTarget,
    kind: CallKind,
    inputs: Box<[InputBinding]>,
    environment: Environment,
    result: Payload,
}
impl Invocation {
    #[must_use]
    pub const fn target(&self) -> CallTarget {
        self.target
    }
    #[must_use]
    pub const fn kind(&self) -> CallKind {
        self.kind
    }
    #[must_use]
    pub fn inputs(&self) -> &[InputBinding] {
        &self.inputs
    }
    #[must_use]
    pub fn environment(&self) -> Environment {
        self.environment.clone()
    }
    #[must_use]
    pub fn result(&self) -> Payload {
        self.result.clone()
    }
}

pub(super) fn bind(
    unit: &Unit,
    frame: &InstanceKey,
    caller: &nocter_checking::value_access::flow::Query,
    call: AccessExpressionId,
    code: ValueCode,
    target: CallTarget,
    work: &mut WorkBudget,
) -> Result<Invocation, Failure> {
    if caller.typed().code() != frame.code() {
        return Err(INVALID);
    }
    let bundles = unit
        .value_sources(frame.code())
        .and_then(|graph| graph.call(call))
        .ok_or(INVALID)?;
    let mut binding = Binding {
        unit,
        frame,
        caller,
        call,
        code,
        target,
        work,
    };
    let mut selected = None;
    for bundle in bundles {
        let next = binding.bundle(bundle)?;
        if selected.as_ref().is_some_and(|previous| previous != &next) {
            return Err(
                binding.failure(nocter_checking::value_access::InstantiationFailure::Unresolved)
            );
        }
        selected = Some(next);
    }
    selected.ok_or(INVALID)
}
struct Binding<'a, 'work> {
    unit: &'a Unit,
    frame: &'a InstanceKey,
    caller: &'a nocter_checking::value_access::flow::Query,
    call: AccessExpressionId,
    code: ValueCode,
    target: CallTarget,
    work: &'work mut WorkBudget,
}
impl Binding<'_, '_> {
    fn bundle(&mut self, bundle: &CallValueSources) -> Result<Invocation, Failure> {
        let (unit, caller, call, code, target) =
            (self.unit, self.caller, self.call, self.code, self.target);
        let expression = caller
            .typed()
            .body()
            .expressions()
            .get(call)
            .ok_or(INVALID)?;
        let nocter_checking::value_access::ExpressionKind::Call {
            kind, arguments, ..
        } = &expression.kind
        else {
            return Err(INVALID);
        };
        let kind = *kind;
        if bundle.kind != kind || bundle.arguments.len() != arguments.len() {
            return Err(INVALID);
        }
        let result = self.result(expression.ty.normal().ok_or(INVALID)?.data(), bundle)?;
        let body = unit.code_body(code).map(|body| body.typed().body());
        let expected: Vec<_> = match target {
            CallTarget::Body(_) => body
                .ok_or(INVALID)?
                .inputs()
                .explicit()
                .iter()
                .map(|input| input.operation)
                .collect(),
            CallTarget::Primitive(role) => {
                let ValueCode::Declaration(id) = code else {
                    return Err(INVALID);
                };
                if unit.primitives().role(id) != Some(role) || body.is_some() {
                    return Err(INVALID);
                }
                role.value_signature()
                    .ok_or(INVALID)?
                    .inputs()
                    .iter()
                    .map(|input| input.operation())
                    .collect()
            }
        };
        let environment_input = body.and_then(|body| body.inputs().environment());
        let (offset, environment) = match kind {
            CallKind::Direct => {
                if !matches!(code, ValueCode::Declaration(_)) || environment_input.is_some() {
                    return Err(INVALID);
                }
                (0, Environment::None)
            }
            CallKind::Value => {
                let argument = arguments.first().ok_or(INVALID)?;
                let carrier = self.argument_payload(bundle, 0, argument)?;
                let Some(nocter_declarations::value_access::PayloadSchema::Function(witness)) =
                    unit.program().data_schema(carrier.data())
                else {
                    return Err(INVALID);
                };
                if Some(witness.origin) != code.origin()
                    || witness.invocation.operation() != argument.operation
                {
                    return Err(INVALID);
                }
                (
                    1,
                    environment(unit, code, carrier, argument.operation, environment_input)?,
                )
            }
        };
        let explicit = arguments.get(offset..).ok_or(INVALID)?;
        if explicit.len() != expected.len() {
            return Err(INVALID);
        }
        let mut inputs = Vec::new();
        for (index, (argument, operation)) in explicit.iter().zip(expected).enumerate() {
            if argument.operation != operation {
                return Err(INVALID);
            }
            inputs.push(InputBinding {
                argument: offset + index,
                operation,
                payload: self.argument_payload(bundle, offset + index, argument)?,
            });
        }
        Ok(Invocation {
            target,
            kind,
            inputs: inputs.into(),
            environment,
            result,
        })
    }
}

fn environment(
    unit: &Unit,
    code: ValueCode,
    carrier: Payload,
    supplied: ValueInputOperation,
    input: Option<BodyInput>,
) -> Result<Environment, Failure> {
    match code {
        ValueCode::Expression(_) => Err(INVALID),
        ValueCode::Declaration(_) => {
            if input.is_some() {
                return Err(INVALID);
            }
            let exit = if supplied == ValueInputOperation::Take {
                EnvironmentExit::DisposeBeforeResult(carrier.clone())
            } else {
                EnvironmentExit::Retained
            };
            Ok(Environment::CodeToken {
                argument: 0,
                supplied,
                carrier,
                exit,
            })
        }
        ValueCode::Closure(id) => {
            let input = input.ok_or(INVALID)?;
            unit.program().closure(id).ok_or(INVALID)?;
            let payload = carrier
                .environment(unit.program())
                .map_err(|_| INVALID)?
                .ok_or(INVALID)?;
            let exit = if input.operation == ValueInputOperation::Take {
                EnvironmentExit::BodyOwned
            } else if supplied == ValueInputOperation::Take {
                EnvironmentExit::DisposeBeforeResult(payload.clone())
            } else {
                EnvironmentExit::Retained
            };
            Ok(Environment::Capture {
                argument: 0,
                supplied,
                carrier,
                input,
                payload,
                exit,
            })
        }
    }
}

impl Binding<'_, '_> {
    fn failure(&self, failure: nocter_checking::value_access::InstantiationFailure) -> Failure {
        Failure::Invocation {
            site: nocter_model::ValueApplication {
                owner: self.frame.code(),
                call: self.call,
            },
            failure,
        }
    }
    fn argument_payload(
        &mut self,
        bundle: &CallValueSources,
        index: usize,
        argument: &nocter_checking::value_access::Argument,
    ) -> Result<Payload, Failure> {
        let data = self
            .caller
            .typed()
            .body()
            .expressions()
            .get(argument.value)
            .ok_or(INVALID)?
            .ty
            .normal()
            .ok_or(INVALID)?
            .data();
        let source = *bundle.arguments.get(index).ok_or(INVALID)?;
        let key = self
            .unit
            .instantiate_value_data(self.frame, source, data, self.work)
            .map_err(|failure| self.failure(failure))?;
        self.payload(data, &key)
    }
    fn result(&mut self, data: TypeId, bundle: &CallValueSources) -> Result<Payload, Failure> {
        let key = if let Ok(key) = self.unit.program().instantiate_closed_data(data) {
            key
        } else {
            let selected = nocter_checking::value_access::ValueSourceSelection {
                boundary: nocter_checking::value_access::ValueSourceBoundary {
                    point: bundle.origin.point,
                    action: bundle.origin.action.checked_add(1).ok_or(INVALID)?,
                },
                slot: nocter_checking::value_access::ValueSourceSlot::Expression(self.call),
                path: nocter_model::ValuePath::default(),
            };
            self.unit
                .instantiate_stored_data(self.frame, &selected, data, self.work)
                .map_err(|failure| self.failure(failure))?
        };
        self.payload(data, &key)
    }
    fn payload(
        &self,
        source: TypeId,
        key: &nocter_declarations::value_access::DataInstanceKey,
    ) -> Result<Payload, Failure> {
        self.unit
            .data_evidence(source)
            .map_err(|_| INVALID)?
            .payload()
            .ok_or(INVALID)?
            .instantiate(self.unit.program(), key)
            .map_err(|_| INVALID)
    }
}
