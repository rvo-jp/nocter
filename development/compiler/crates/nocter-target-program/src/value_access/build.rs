//! Root-driven interning closes selected calls once. Recursive edges reuse reserved instances.
use super::{CallTarget, Failure, Instance, InstanceKey, InstanceProgram, invocation, projection};
#[cfg(test)]
mod tests;
use nocter_checking::value_access::{accepted::Unit, flow::Operation};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_model::{AccessCodeId, AccessInstanceId, ArenaBuilder};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

struct Reservation {
    key: InstanceKey,
    definition: Option<Instance>,
}
struct Builder {
    instances: ArenaBuilder<AccessInstanceId, Reservation>,
    keys: BTreeMap<InstanceKey, AccessInstanceId>,
    pending: Vec<AccessInstanceId>,
}
impl Builder {
    fn intern(&mut self, key: InstanceKey) -> AccessInstanceId {
        if let Some(&id) = self.keys.get(&key) {
            return id;
        }
        let id = self.instances.insert(Reservation {
            key: key.clone(),
            definition: None,
        });
        self.keys.insert(key, id);
        self.pending.push(id);
        id
    }
    fn define(
        &mut self,
        unit: &Unit,
        id: AccessInstanceId,
        work: &mut WorkBudget,
    ) -> Result<(), Failure> {
        let key = self
            .instances
            .get(id)
            .ok_or(Failure::InvalidAcceptedUnit)?
            .key
            .clone();
        let body = unit
            .code_body(key.code())
            .ok_or(Failure::InvalidAcceptedUnit)?;
        let inputs = projection::inputs(unit, &key, body, work)?;
        let result = projection::result(unit, &key, body, work)?;
        let prepared = projection::prepared(unit, &key, work)?;
        let metadata = projection::metadata(unit, body)?;
        let mut invocations = BTreeSet::new();
        for (point, block) in body.graph().points().iter() {
            if !body.initialization().reachable(point) {
                continue;
            }
            for action in &block.actions {
                if let Operation::CommitCall(call) = action.operation {
                    invocations.insert(call);
                }
            }
        }
        let mut calls = BTreeMap::new();
        for call in invocations {
            let site = nocter_model::ValueApplication {
                owner: key.code(),
                call,
            };
            let selected = unit
                .instantiate_invocation(&key, site, work)
                .map_err(|failure| Failure::Invocation { site, failure })?;
            let code = selected.code();
            let target = match code {
                nocter_model::ValueCode::Declaration(id)
                    if unit.primitives().role(id).is_some() =>
                {
                    CallTarget::Primitive(
                        unit.primitives()
                            .role(id)
                            .ok_or(Failure::InvalidAcceptedUnit)?,
                    )
                }
                _ => CallTarget::Body(self.intern(selected.clone())),
            };
            let edge = invocation::bind(unit, &key, body, call, &selected, target, work)?;
            let nocter_checking::value_access::ExpressionKind::Call { arguments, .. } = &body
                .typed()
                .body()
                .expressions()
                .get(call)
                .ok_or(Failure::InvalidAcceptedUnit)?
                .kind
            else {
                return Err(Failure::InvalidAcceptedUnit);
            };
            let supplied = arguments.len();
            for index in 0..supplied {
                if prepared.get(&(call, index)) != edge.argument_payload(index) {
                    return Err(Failure::InvalidAcceptedUnit);
                }
            }
            calls.insert(call, edge);
        }
        let values = projection::values(unit, &key, work)?;
        self.instances
            .get_mut(id)
            .ok_or(Failure::InvalidAcceptedUnit)?
            .definition = Some(Instance {
            key,
            calls,
            metadata,
            inputs,
            result,
            prepared,
            values,
        });
        Ok(())
    }
}
pub(super) fn close(
    unit: Arc<Unit>,
    root: AccessCodeId,
    work: &mut WorkBudget,
) -> Result<InstanceProgram, Failure> {
    let declaration = unit
        .program()
        .codes()
        .get(root)
        .ok_or(Failure::UnknownRoot(root))?;
    if declaration.kind().is_initializer() {
        return Err(Failure::InitializerRoot(root));
    }
    if unit.primitives().role(root).is_some() {
        return Err(Failure::BodylessRoot(root));
    }
    for &id in declaration.parameters() {
        if unit
            .program()
            .parameters()
            .get(id)
            .ok_or(Failure::InvalidAcceptedUnit)?
            .contract()
            .is_some()
        {
            return Err(Failure::UnboundRoot(id));
        }
    }
    let mut builder = Builder {
        instances: ArenaBuilder::new(),
        keys: BTreeMap::new(),
        pending: Vec::new(),
    };
    let root = builder.intern(
        unit.program()
            .bind_code(nocter_model::ValueCode::Declaration(root), [])
            .map_err(|_| Failure::InvalidAcceptedUnit)?,
    );
    while let Some(id) = builder.pending.pop() {
        builder.define(&unit, id, work)?;
    }
    let instances = builder.instances.finish().try_map(|_, reservation| {
        reservation
            .definition
            .clone()
            .ok_or(Failure::InvalidAcceptedUnit)
    })?;
    for (_, instance) in instances.iter() {
        for call in instance.calls.values() {
            let CallTarget::Body(target) = call.target() else {
                validate_primitive(&unit, call)?;
                continue;
            };
            let callee = instances.get(target).ok_or(Failure::InvalidAcceptedUnit)?;
            validate_body_edge(&unit, callee, call)?;
        }
    }
    Ok(InstanceProgram {
        unit,
        root,
        instances,
    })
}

fn validate_body_edge(
    unit: &Unit,
    callee: &Instance,
    call: &invocation::Invocation,
) -> Result<(), Failure> {
    let invalid = Failure::InvalidAcceptedUnit;
    let body = unit.code_body(callee.key.code()).ok_or(invalid)?.typed();
    match (body.body().inputs().environment(), call.environment()) {
        (
            Some(input),
            super::Environment::Capture {
                input: supplied,
                payload,
                ..
            },
        ) => {
            if input != supplied || callee.input(input.local).ok_or(invalid)? != &payload {
                return Err(invalid);
            }
        }
        (None, super::Environment::None | super::Environment::CodeToken { .. }) => {}
        _ => return Err(invalid),
    }
    if callee.result().payload().ok_or(invalid)? != call.result() {
        return Err(invalid);
    }
    let inputs = body.body().inputs().explicit();
    if inputs.len() != call.inputs().len() {
        return Err(invalid);
    }
    for (binding, input) in call.inputs().iter().zip(inputs) {
        if input.operation != binding.operation
            || !callee
                .input(input.local)
                .ok_or(invalid)?
                .instance()
                .has_same_representation(unit.program(), binding.payload.instance())
        {
            return Err(invalid);
        }
    }
    Ok(())
}

fn validate_primitive(unit: &Unit, call: &invocation::Invocation) -> Result<(), Failure> {
    let CallTarget::Primitive(role) = call.target() else {
        return Err(Failure::InvalidAcceptedUnit);
    };
    let signature = role.value_signature().ok_or(Failure::InvalidAcceptedUnit)?;
    if unit.program().data_kind(call.result().data()) != Some(signature.result())
        || call.inputs().len() != signature.inputs().len()
        || call
            .inputs()
            .iter()
            .zip(signature.inputs())
            .any(|(actual, expected)| {
                actual.operation != expected.operation()
                    || unit.program().data_kind(actual.payload.data()) != Some(expected.data())
            })
    {
        return Err(Failure::InvalidAcceptedUnit);
    }
    Ok(())
}
