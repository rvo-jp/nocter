//! Finite may-write footprints over explicit input roots; unknown effects are the top element.
use super::super::Operation;
use super::{Control, INVALID, IntegrityError, TypedQuery, bindings};
use crate::value_access::{ExpressionKind, Referent};
use nocter_declarations::value_access::{CodeKind, PayloadSchema};
use nocter_model::{
    AccessCodeId, AccessExpressionId, BuiltinType, ValueAccessMode, ValueCode, ValueFunctionOrigin,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Writes {
    Known(BTreeSet<usize>),
    Unknown,
}
struct Body<'a> {
    typed: &'a TypedQuery,
    local: Writes,
    calls: Vec<(AccessExpressionId, AccessCodeId)>,
}
pub(super) struct Closed {
    pub(super) writes: BTreeMap<AccessCodeId, Writes>,
    pub(super) callees: BTreeMap<AccessCodeId, BTreeSet<AccessCodeId>>,
}

pub(super) fn close(controls: &[&Control]) -> Result<Closed, IntegrityError> {
    let mut bodies = BTreeMap::new();
    for control in controls {
        if let ValueCode::Declaration(code) = control.typed.code()
            && control
                .typed
                .program()
                .codes()
                .get(code)
                .is_some_and(|code| matches!(code.kind(), CodeKind::Function(_)))
        {
            bodies.insert(code, local_summary(control)?);
        }
    }
    let mut effects: BTreeMap<_, _> = bodies
        .iter()
        .map(|(&code, body)| (code, body.local.clone()))
        .collect();
    let mut reverse: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
    for (&caller, body) in &bodies {
        for &(_, callee) in &body.calls {
            reverse.entry(callee).or_default().insert(caller);
        }
    }
    let mut pending: BTreeSet<_> = bodies.keys().copied().collect();
    while let Some(code) = pending.pop_first() {
        let next = combine(bodies.get(&code).ok_or(INVALID)?, &effects)?;
        if effects.get(&code) != Some(&next) {
            effects.insert(code, next);
            if let Some(affected) = reverse.get(&code) {
                pending.extend(affected);
            }
        }
    }
    let callees = bodies
        .into_iter()
        .map(|(code, body)| {
            (
                code,
                body.calls.into_iter().map(|(_, callee)| callee).collect(),
            )
        })
        .collect();
    Ok(Closed {
        writes: effects,
        callees,
    })
}

fn combine(
    body: &Body<'_>,
    effects: &BTreeMap<AccessCodeId, Writes>,
) -> Result<Writes, IntegrityError> {
    let Writes::Known(mut writes) = body.local.clone() else {
        return Ok(Writes::Unknown);
    };
    for &(call, code) in &body.calls {
        let Some(Writes::Known(inputs)) = effects.get(&code) else {
            return Ok(Writes::Unknown);
        };
        let Some(sources) = bindings::actual_writes(body.typed, call, inputs)? else {
            return Ok(Writes::Unknown);
        };
        for source in sources {
            match bindings::classify_root(body.typed, source) {
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
        local: Writes::Known(BTreeSet::new()),
        calls: Vec::new(),
    };
    let mut targets = Vec::new();
    for point in control.graph.reachable()? {
        let point = control.graph.points().get(point).ok_or(INVALID)?;
        for action in &point.actions {
            match action.operation {
                Operation::Assign { target, .. } | Operation::CompoundAssign { target, .. } => {
                    targets.push(target);
                    match bindings::classify_root(typed, target) {
                        Some(bindings::Root::Input(input)) => {
                            if let Writes::Known(writes) = &mut body.local {
                                writes.insert(input);
                            }
                        }
                        Some(bindings::Root::Owned) => {}
                        None => body.local = Writes::Unknown,
                    }
                }
                Operation::CommitCall(id) => {
                    if let ExpressionKind::Call {
                        target: ValueFunctionOrigin::Declaration(code),
                        ..
                    } = typed.body().expressions().get(id).ok_or(INVALID)?.kind
                    {
                        body.calls.push((id, code));
                    } else {
                        body.local = Writes::Unknown;
                    }
                }
                _ => {}
            }
        }
    }
    if !closed_cleanup(typed, &targets) {
        body.local = Writes::Unknown;
    }
    Ok(body)
}

fn closed_cleanup(typed: &TypedQuery, targets: &[Referent]) -> bool {
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
    for &target in targets {
        let Some(data) = typed.body().referent_data(target) else {
            return false;
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
            Some(PayloadSchema::Function(witness)) => match witness.origin {
                ValueFunctionOrigin::Declaration(_) => {}
                ValueFunctionOrigin::Closure(id) => {
                    let Some(closure) = typed.program().closure(id) else {
                        return false;
                    };
                    // Destruction releases the captured payload; it does not call the body.
                    pending.push(closure.environment());
                }
                _ => return false,
            },
            _ => return false,
        }
    }
    true
}
