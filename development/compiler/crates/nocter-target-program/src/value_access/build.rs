//! Root-driven interning closes selected calls once. Recursive edges reuse reserved instances.
use super::{
    CallTarget, Drive, DriveTarget, Failure, Instance, InstanceKey, InstanceProgram, invocation,
    projection,
};
#[cfg(test)]
mod tests;
use nocter_checking::value_access::{
    ValueSourceSlot,
    accepted::Unit,
    flow::{Location, Operation, resources::Holder},
};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_model::{AccessCodeId, AccessExpressionId, AccessInstanceId, ArenaBuilder, ValueCode};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

struct Reservation {
    key: InstanceKey,
    definition: Option<Instance>,
}
#[derive(Default)]
pub(super) struct Builder {
    instances: ArenaBuilder<AccessInstanceId, Reservation>,
    keys: BTreeMap<InstanceKey, AccessInstanceId>,
    pending: Vec<AccessInstanceId>,
    destructors: BTreeSet<AccessInstanceId>,
}
impl Builder {
    pub(super) fn retain(program: &InstanceProgram) -> Self {
        let mut builder = Self {
            destructors: program.destructors.clone(),
            ..Self::default()
        };
        for (_, instance) in program.instances.iter() {
            let key = instance.key().clone();
            let id = builder.instances.insert(Reservation {
                key: key.clone(),
                definition: Some(instance.clone()),
            });
            builder.keys.insert(key, id);
        }
        builder
    }

    pub(super) fn add_destructor(&mut self, frame: InstanceKey) {
        let id = self.intern(frame);
        self.destructors.insert(id);
    }

    pub(super) fn next(
        &mut self,
        unit: &Arc<Unit>,
        work: &mut WorkBudget,
    ) -> Result<Option<(AccessInstanceId, &Instance)>, Failure> {
        let Some(id) = self.pending.pop() else {
            return Ok(None);
        };
        self.define(unit, id, work)?;
        let definition = self
            .instances
            .get(id)
            .and_then(|instance| instance.definition.as_ref())
            .ok_or(Failure::InvalidAcceptedUnit)?;
        Ok(Some((id, definition)))
    }

    pub(super) fn finish(
        self,
        unit: Arc<Unit>,
        root: AccessInstanceId,
    ) -> Result<InstanceProgram, Failure> {
        let instances = self.instances.try_finish_with(|_, reservation| {
            reservation.definition.ok_or(Failure::InvalidAcceptedUnit)
        })?;
        for (_, instance) in instances.iter() {
            for call in instance.calls.values() {
                if call.cases().is_empty() {
                    validate_edge(&unit, &instances, call.target(), call.environment(), call)?;
                } else {
                    for case in call.cases() {
                        validate_edge(
                            &unit,
                            &instances,
                            case.target,
                            case.environment.clone(),
                            call,
                        )?;
                    }
                }
            }
            validate_drives(&unit, &instances, instance)?;
        }
        Ok(InstanceProgram {
            unit,
            root,
            instances,
            destructors: self.destructors,
        })
    }

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
    fn target(&mut self, unit: &Unit, frame: &InstanceKey) -> CallTarget {
        if let nocter_model::ValueCode::Declaration(id) = frame.code()
            && let Some(role) = unit.primitives().role(id)
        {
            return CallTarget::Primitive(role);
        }
        CallTarget::Body(self.intern(frame.clone()))
    }
    fn drive(
        &mut self,
        unit: &Unit,
        owner: ValueCode,
        future: AccessExpressionId,
        result: AccessExpressionId,
    ) -> Result<Drive, Failure> {
        let invalid = Failure::InvalidAcceptedUnit;
        let producers = unit.future_drive(owner, result).ok_or(invalid)?;
        let mut target = |producer: AccessCodeId| {
            let inputs = unit
                .future_inputs(owner, Holder::Expression(future), producer)
                .ok_or(invalid)?
                .clone();
            let key = unit
                .program()
                .bind_code(producer.into(), [])
                .map_err(|_| invalid)?;
            Ok(DriveTarget {
                instance: self.intern(key),
                inputs,
            })
        };
        if producers.len() == 1 {
            return target(*producers.first().ok_or(invalid)?).map(Drive::Direct);
        }
        producers
            .iter()
            .map(|&producer| {
                let tag = unit.future_producer_tag(owner, producer).ok_or(invalid)?;
                Ok((tag, target(producer)?))
            })
            .collect::<Result<Box<[_]>, _>>()
            .map(Drive::Tagged)
    }
    fn define(
        &mut self,
        unit: &Arc<Unit>,
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
        let queries = unit
            .instance_metadata(&key, work)
            .map_err(|failure| Failure::Control {
                code: key.code(),
                failure,
            })?;
        let control = queries.control();
        let inputs = projection::inputs(unit, &key, body, work)?;
        let result = projection::result(unit, &key, body, work)?;
        let prepared = projection::prepared(unit, &key, work)?;
        let metadata = projection::metadata(unit, body, control)?;
        let mut invocations = BTreeSet::new();
        let mut drives = BTreeMap::new();
        for (point, block) in body.graph().points().iter() {
            if !control.reachable(point) {
                continue;
            }
            for (action, event) in block.actions.iter().enumerate() {
                match event.operation {
                    Operation::CommitCall(call) => {
                        invocations.insert(call);
                    }
                    Operation::DriveFuture { future, result } => {
                        let target = self.drive(unit, key.code(), future, result)?;
                        drives.insert(Location { point, action }, target);
                    }
                    _ => {}
                }
            }
        }
        let mut calls = BTreeMap::new();
        for call in invocations {
            let site = nocter_model::ValueApplication {
                owner: key.code(),
                call,
            };
            let edge = match queries.invocation(site, work) {
                Ok(selected) => {
                    let target = self.target(unit, &selected);
                    invocation::bind(&queries, body, call, &selected, target, work)?
                }
                Err(nocter_checking::value_access::InstantiationFailure::Unresolved) => {
                    let domain = queries
                        .invocation_domain(site, work)
                        .map_err(|failure| Failure::Invocation { site, failure })?;
                    invocation::bind_domain(
                        &queries,
                        body,
                        &domain,
                        |frame| self.target(unit, frame),
                        work,
                    )?
                }
                Err(failure) => return Err(Failure::Invocation { site, failure }),
            };
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
            calls,
            drives,
            metadata,
            inputs,
            result,
            prepared,
            values,
            queries,
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
    if matches!(
        declaration.kind(),
        nocter_declarations::value_access::CodeKind::Destructor(_)
    ) {
        return Err(Failure::DestructorRoot(root));
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
    let mut builder = Builder::default();
    let root = builder.intern(
        unit.program()
            .bind_code(nocter_model::ValueCode::Declaration(root), [])
            .map_err(|_| Failure::InvalidAcceptedUnit)?,
    );
    while builder.next(&unit, work)?.is_some() {}
    builder.finish(unit, root)
}

fn validate_drives(
    unit: &Unit,
    instances: &nocter_model::Arena<AccessInstanceId, Instance>,
    caller: &Instance,
) -> Result<(), Failure> {
    let invalid = Failure::InvalidAcceptedUnit;
    let caller_body = unit.code_body(caller.key().code()).ok_or(invalid)?;
    for (&at, drive) in &caller.drives {
        let Operation::DriveFuture { result, .. } = caller_body
            .graph()
            .points()
            .get(at.point)
            .and_then(|point| point.actions.get(at.action))
            .ok_or(invalid)?
            .operation
        else {
            return Err(invalid);
        };
        let receipt = caller
            .definition(at, ValueSourceSlot::Expression(result))
            .ok_or(invalid)?;
        for target in drive.targets() {
            let child = instances.get(target.instance).ok_or(invalid)?;
            let body = unit.code_body(child.key().code()).ok_or(invalid)?;
            let inputs = body.typed().body().inputs();
            if inputs.environment().is_some()
                || inputs.explicit().len() != target.inputs.len()
                || inputs
                    .explicit()
                    .iter()
                    .zip(target.inputs.iter())
                    .any(|(input, expected)| {
                        input.operation != expected.operation
                            || child
                                .input(input.local)
                                .is_none_or(|payload| payload.data() != expected.data)
                    })
                || !child
                    .result()
                    .payload()
                    .ok_or(invalid)?
                    .instance()
                    .has_same_representation(unit.program(), receipt.instance())
            {
                return Err(invalid);
            }
        }
    }
    Ok(())
}

fn validate_edge(
    unit: &Unit,
    instances: &nocter_model::Arena<AccessInstanceId, Instance>,
    target: CallTarget,
    environment: super::Environment,
    call: &invocation::Invocation,
) -> Result<(), Failure> {
    match target {
        CallTarget::Body(id) => validate_body_edge(
            unit,
            instances.get(id).ok_or(Failure::InvalidAcceptedUnit)?,
            environment,
            call,
        ),
        CallTarget::Primitive(role) => validate_primitive(unit, role, call),
        CallTarget::Selected => Err(Failure::InvalidAcceptedUnit),
    }
}

fn validate_body_edge(
    unit: &Unit,
    callee: &Instance,
    environment: super::Environment,
    call: &invocation::Invocation,
) -> Result<(), Failure> {
    let invalid = Failure::InvalidAcceptedUnit;
    let body = unit.code_body(callee.key().code()).ok_or(invalid)?.typed();
    match (body.body().inputs().environment(), environment) {
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
    if !callee
        .result()
        .payload()
        .ok_or(invalid)?
        .instance()
        .has_same_representation(unit.program(), call.result().instance())
    {
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

fn validate_primitive(
    unit: &Unit,
    role: nocter_runtime_contract::PrimitiveRole,
    call: &invocation::Invocation,
) -> Result<(), Failure> {
    let signature = role.value_signature().ok_or(Failure::InvalidAcceptedUnit)?;
    if !signature
        .result()
        .matches_type(call.result().data(), |data| unit.program().type_kind(data))
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
