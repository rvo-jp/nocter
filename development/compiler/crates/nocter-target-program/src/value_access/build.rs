//! Root-driven interning closes selected calls once. Recursive edges reuse reserved instances.
use super::{CallTarget, Failure, Instance, InstanceKey, InstanceProgram, invocation, projection};
use nocter_checking::value_access::{ExpressionKind, accepted::Unit, flow::Operation};
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
    fn define(&mut self, unit: &Unit, id: AccessInstanceId) -> Result<(), Failure> {
        let key = self
            .instances
            .get(id)
            .ok_or(Failure::InvalidAcceptedUnit)?
            .key
            .clone();
        let body = unit
            .code_body(key.code)
            .ok_or(Failure::InvalidAcceptedUnit)?;
        let data = projection::data(unit, &key, body)?;
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
            let ExpressionKind::Call {
                target, arguments, ..
            } = &body
                .typed()
                .body()
                .expressions()
                .get(call)
                .ok_or(Failure::InvalidAcceptedUnit)?
                .kind
            else {
                return Err(Failure::InvalidAcceptedUnit);
            };
            let selected = projection::callee(unit.program(), &key, *target, arguments)?;
            let code = selected.code;
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
                _ => CallTarget::Body(self.intern(selected)),
            };
            let edge = invocation::bind(unit, &data, body, call, code, target)?;
            calls.insert(call, edge);
        }
        self.instances
            .get_mut(id)
            .ok_or(Failure::InvalidAcceptedUnit)?
            .definition = Some(Instance { key, calls, data });
        Ok(())
    }
}
pub(super) fn close(unit: Arc<Unit>, root: AccessCodeId) -> Result<InstanceProgram, Failure> {
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
    let root = builder.intern(InstanceKey {
        code: nocter_model::ValueCode::Declaration(root),
        witnesses: BTreeMap::new(),
    });
    while let Some(id) = builder.pending.pop() {
        builder.define(&unit, id)?;
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
            let body = unit
                .code_body(callee.key.code)
                .ok_or(Failure::InvalidAcceptedUnit)?
                .typed();
            let result = body.result_data();
            if callee
                .data
                .get(&result)
                .and_then(nocter_checking::value_access::accepted::DataEvidence::payload)
                != Some(call.result())
            {
                return Err(Failure::InvalidAcceptedUnit);
            }
            let inputs = body.body().inputs().explicit();
            if inputs.len() != call.inputs().len() {
                return Err(Failure::InvalidAcceptedUnit);
            }
            for (binding, input) in call.inputs().iter().zip(inputs) {
                let local = body
                    .body()
                    .locals()
                    .get(input.local)
                    .ok_or(Failure::InvalidAcceptedUnit)?;
                if input.operation != binding.operation
                    || callee
                        .data
                        .get(&local.data)
                        .and_then(nocter_checking::value_access::accepted::DataEvidence::payload)
                        != Some(binding.payload)
                {
                    return Err(Failure::InvalidAcceptedUnit);
                }
            }
        }
    }
    Ok(InstanceProgram {
        unit,
        root,
        instances,
    })
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
