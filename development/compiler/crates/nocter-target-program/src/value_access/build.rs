//! Root-driven interning closes selected calls once. Recursive edges reuse reserved instances.
use super::{Failure, Instance, InstanceKey, InstanceProgram, projection};
use nocter_checking::value_access::{ExpressionKind, accepted::Unit, flow::Operation};
use nocter_model::{AccessCallableId, AccessInstanceId, ArenaBuilder};
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
            .body(key.callable)
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
            calls.insert(call, self.intern(selected));
        }
        self.instances
            .get_mut(id)
            .ok_or(Failure::InvalidAcceptedUnit)?
            .definition = Some(Instance { key, calls, data });
        Ok(())
    }
}
pub(super) fn close(unit: Arc<Unit>, root: AccessCallableId) -> Result<InstanceProgram, Failure> {
    let declaration = unit
        .program()
        .callables()
        .get(root)
        .ok_or(Failure::UnknownRoot(root))?;
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
        callable: root,
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
    Ok(InstanceProgram {
        unit,
        root,
        instances,
    })
}
