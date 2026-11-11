//! Finite may-write footprints over environment/explicit input roots; unknown effects are the top element.
use super::super::{Operation, aliases::Aliases};
use super::{Control, INVALID, IntegrityError, TypedQuery, bindings, requests::Request};
use crate::value_access::Referent;
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{
    AccessExpressionId, BuiltinType, ValueAccessMode, ValueCode, ValueFunctionOrigin,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Writes {
    Known(BTreeSet<bindings::Input>),
    Unknown,
}
struct Body<'a> {
    typed: &'a TypedQuery,
    roots: Aliases,
    local: Writes,
    calls: Vec<AccessExpressionId>,
    targets: Vec<Referent>,
}
pub(super) struct Closed {
    pub(super) writes: BTreeMap<Request, Writes>,
    pub(super) callees: BTreeMap<Request, BTreeSet<Request>>,
}
struct BoundBody {
    local: Writes,
    calls: Vec<(AccessExpressionId, Option<Request>)>,
}
pub(super) fn close(controls: &[&Control]) -> Result<Closed, IntegrityError> {
    let mut bodies = BTreeMap::new();
    let program = controls.first().map(|control| &control.typed.program);
    for (code, control) in super::super::scope(controls.iter().copied())? {
        if program.is_none_or(|program| !std::sync::Arc::ptr_eq(program, &control.typed.program)) {
            continue;
        }
        // Every checked frame can supply root calls, including required expressions. Its
        // effect facts do not turn a non-callable frame into an invocation target.
        bodies.insert(code, local_summary(control)?);
    }
    let bound_bodies = bind_bodies(&bodies)?;
    let mut effects: BTreeMap<_, _> = bound_bodies
        .iter()
        .map(|(key, instance)| (key.clone(), instance.local.clone()))
        .collect();
    let mut reverse = BTreeMap::<Request, BTreeSet<Request>>::new();
    for (key, instance) in &bound_bodies {
        for (_, target) in &instance.calls {
            if let Some(target) = target {
                reverse
                    .entry(target.clone())
                    .or_default()
                    .insert(key.clone());
            }
        }
    }
    let mut pending: BTreeSet<_> = bound_bodies.keys().cloned().collect();
    while let Some(key) = pending.pop_first() {
        let instance = bound_bodies.get(&key).ok_or(INVALID)?;
        let next = if let Some(body) = bodies.get(&key.code) {
            combine(body, instance, &effects)?
        } else {
            Writes::Unknown
        };
        if effects.get(&key) != Some(&next) {
            effects.insert(key.clone(), next);
            if let Some(affected) = reverse.get(&key) {
                pending.extend(affected.iter().cloned());
            }
        }
    }
    let callees = bound_bodies
        .into_iter()
        .map(|(key, instance)| {
            (
                key,
                instance
                    .calls
                    .into_iter()
                    .filter_map(|(_, target)| target)
                    .collect(),
            )
        })
        .collect();
    Ok(Closed {
        writes: effects,
        callees,
    })
}
fn bind_bodies(
    bodies: &BTreeMap<ValueCode, Body<'_>>,
) -> Result<BTreeMap<Request, BoundBody>, IntegrityError> {
    let mut pending: BTreeSet<_> = bodies.keys().copied().map(Request::unbound).collect();
    let mut bound_bodies = BTreeMap::new();
    while let Some(request) = pending.pop_first() {
        if bound_bodies.contains_key(&request) {
            continue;
        }
        let Some(body) = bodies.get(&request.code) else {
            bound_bodies.insert(
                request,
                BoundBody {
                    local: Writes::Unknown,
                    calls: Vec::new(),
                },
            );
            continue;
        };
        let local = if closed_cleanup(body.typed, &body.targets, &request)? {
            body.local.clone()
        } else {
            Writes::Unknown
        };
        let mut calls = Vec::new();
        for &call in &body.calls {
            let target = request.call(body.typed, call)?;
            if let Some(target) = &target {
                pending.insert(target.clone());
            }
            calls.push((call, target));
        }
        bound_bodies.insert(request, BoundBody { local, calls });
    }
    Ok(bound_bodies)
}

fn combine(
    body: &Body<'_>,
    instance: &BoundBody,
    effects: &BTreeMap<Request, Writes>,
) -> Result<Writes, IntegrityError> {
    let Writes::Known(mut writes) = instance.local.clone() else {
        return Ok(Writes::Unknown);
    };
    for (call, target) in &instance.calls {
        let Some(Writes::Known(inputs)) = target.as_ref().and_then(|target| effects.get(target))
        else {
            return Ok(Writes::Unknown);
        };
        let Some(sources) = bindings::actual_writes(body.typed, &body.roots, *call, inputs)? else {
            return Ok(Writes::Unknown);
        };
        for source in sources {
            match bindings::classify(body.typed, &body.roots, source) {
                Some(bindings::Root::Input(input)) => {
                    writes.insert(input);
                }
                Some(bindings::Root::Owned) => {}
                None => return Ok(Writes::Unknown),
            }
        }
    }
    Ok(Writes::Known(writes))
}

fn local_summary(control: &Control) -> Result<Body<'_>, IntegrityError> {
    let typed = &control.typed;
    if !typed.results_closed() {
        return Err(IntegrityError::UnclosedResultWitnesses);
    }
    let mut body = Body {
        typed,
        roots: Aliases::prepare(typed)?,
        local: Writes::Known(BTreeSet::new()),
        calls: Vec::new(),
        targets: Vec::new(),
    };
    for point in control.graph.reachable()? {
        let point = control.graph.points().get(point).ok_or(INVALID)?;
        for action in &point.actions {
            match action.operation {
                Operation::Assign { target, .. } | Operation::CompoundAssign { target, .. } => {
                    body.targets.push(target);
                    match bindings::classify(typed, &body.roots, target) {
                        Some(bindings::Root::Input(input)) => {
                            if let Writes::Known(writes) = &mut body.local {
                                writes.insert(input);
                            }
                        }
                        Some(bindings::Root::Owned) => {}
                        None => body.local = Writes::Unknown,
                    }
                }
                Operation::CommitCall(id) => body.calls.push(id),
                _ => {}
            }
        }
    }
    Ok(body)
}

fn closed_cleanup(
    typed: &TypedQuery,
    targets: &[Referent],
    request: &Request,
) -> Result<bool, IntegrityError> {
    // Edit targets also dispose their old values. Their cleanup cannot be inferred from
    // the replacement's type or from the absence of an owned parameter holder.
    let mut pending: Vec<_> = typed
        .body()
        .locals()
        .iter()
        .filter(|(_, local)| local.access().mode() == ValueAccessMode::Own)
        .map(|(_, local)| local.data)
        .chain(
            typed
                .body()
                .expressions()
                .iter()
                .filter_map(|(_, expression)| {
                    expression
                        .ty
                        .normal()
                        .filter(|access| access.mode() == ValueAccessMode::Own)
                        .map(nocter_model::ValueAccess::data)
                }),
        )
        .collect();
    if let ValueCode::Closure(id) = typed.code() {
        let Some(schema) = typed.program().closure(id) else {
            return Ok(false);
        };
        // A taking view may dispose a read/edit body's received environment after normal return.
        pending.push(schema.environment());
    }
    for &target in targets {
        let Some(data) = typed.body().referent_data(target) else {
            return Ok(false);
        };
        pending.push(data);
    }
    let mut seen = BTreeSet::new();
    while let Some(data) = pending.pop() {
        if !seen.insert(data) {
            continue;
        }
        match typed.program().data_schema(data) {
            Some(PayloadSchema::Builtin(
                BuiltinType::Bool
                | BuiltinType::I32
                | BuiltinType::Usize
                | BuiltinType::Void
                | BuiltinType::Error,
            )) => {}
            Some(PayloadSchema::Aggregate(schema)) => {
                pending.extend(schema.components().map(|field| field.data));
            }
            Some(PayloadSchema::Array(schema)) => {
                if !schema.is_empty() {
                    pending.push(schema.element());
                }
            }
            Some(PayloadSchema::Function(witness)) => {
                match request.resolve(witness)?.map(|witness| witness.origin) {
                    Some(ValueFunctionOrigin::Declaration(_)) => {}
                    Some(ValueFunctionOrigin::Closure(id)) => {
                        let Some(closure) = typed.program().closure(id) else {
                            return Ok(false);
                        };
                        // Destruction releases the captured payload; it does not call the body.
                        pending.push(closure.environment());
                    }
                    _ => return Ok(false),
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}
