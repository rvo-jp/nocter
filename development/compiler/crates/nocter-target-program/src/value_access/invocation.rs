//! Frozen caller-to-body input channels and normal adapter cleanup, without target selection.
use super::Failure;
use nocter_checking::value_access::{
    BodyInput, CallKind,
    accepted::{DataEvidence, Unit},
    flow::resources::Payload,
};
use nocter_model::{AccessExpressionId, AccessInstanceId, TypeId, ValueCode, ValueInputOperation};
use nocter_runtime_contract::PrimitiveRole;
use std::collections::BTreeMap;
const INVALID: Failure = Failure::InvalidAcceptedUnit;

/// Normal completion only; this does not invent unwind or cancellation cleanup guarantees.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentExit {
    Retained,
    /// The checked body owns the environment's remaining-payload cleanup.
    BodyOwned,
    /// The adapter owns this post-call environment until disposal before answer delivery.
    DisposeBeforeResult(Payload),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    pub const fn environment(&self) -> Environment {
        self.environment
    }
    #[must_use]
    pub const fn result(&self) -> Payload {
        self.result
    }
}

pub(super) fn bind(
    unit: &Unit,
    data: &BTreeMap<TypeId, DataEvidence>,
    caller: &nocter_checking::value_access::flow::Query,
    call: AccessExpressionId,
    code: ValueCode,
    target: CallTarget,
) -> Result<Invocation, Failure> {
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
    let result = data
        .get(&expression.ty.normal().ok_or(INVALID)?.data())
        .and_then(DataEvidence::payload)
        .ok_or(INVALID)?;
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
            let carrier = argument_payload(caller, data, argument)?;
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
                environment(
                    unit,
                    code,
                    carrier,
                    argument.operation,
                    environment_input,
                    data,
                )?,
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
            payload: argument_payload(caller, data, argument)?,
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

fn environment(
    unit: &Unit,
    code: ValueCode,
    carrier: Payload,
    supplied: ValueInputOperation,
    input: Option<BodyInput>,
    data: &BTreeMap<TypeId, DataEvidence>,
) -> Result<Environment, Failure> {
    match code {
        ValueCode::Expression(_) => Err(INVALID),
        ValueCode::Declaration(_) => {
            if input.is_some() {
                return Err(INVALID);
            }
            let exit = if supplied == ValueInputOperation::Take {
                EnvironmentExit::DisposeBeforeResult(carrier)
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
            let ty = unit.program().closure(id).ok_or(INVALID)?.environment();
            let payload = data
                .get(&ty)
                .and_then(DataEvidence::payload)
                .ok_or(INVALID)?;
            let exit = if input.operation == ValueInputOperation::Take {
                EnvironmentExit::BodyOwned
            } else if supplied == ValueInputOperation::Take {
                EnvironmentExit::DisposeBeforeResult(payload)
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

fn argument_payload(
    caller: &nocter_checking::value_access::flow::Query,
    data: &BTreeMap<TypeId, DataEvidence>,
    argument: &nocter_checking::value_access::Argument,
) -> Result<Payload, Failure> {
    let ty = caller
        .typed()
        .body()
        .expressions()
        .get(argument.value)
        .ok_or(INVALID)?
        .ty
        .normal()
        .ok_or(INVALID)?
        .data();
    data.get(&ty).and_then(DataEvidence::payload).ok_or(INVALID)
}
