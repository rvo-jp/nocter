//! Closed evidence that named calls cannot write existing caller storage. This is not purity,
//! allocation freedom or permission to bypass access checks. Unknown cleanup/call effects stay open.
use super::{Control, IntegrityError, Operation};
use crate::value_access::{ExpressionKind, Query as TypedQuery};
use nocter_declarations::value_access::{CodeKind, PayloadSchema, Program};
use nocter_model::{
    AccessCodeId, AccessExpressionId, BuiltinType, ValueAccessMode, ValueCode, ValueFunctionOrigin,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

const INVALID: IntegrityError = IntegrityError::InconsistentTypedBody;

pub(in crate::value_access) struct Catalog {
    program: Option<Arc<Program>>,
    scope: BTreeMap<ValueCode, Arc<TypedQuery>>,
    no_writes: BTreeSet<AccessCodeId>,
}
impl Catalog {
    pub(in crate::value_access) fn prepare<'a>(
        controls: impl IntoIterator<Item = &'a Control>,
    ) -> Result<Self, IntegrityError> {
        let roots: Vec<_> = controls.into_iter().collect();
        let program = roots.first().map(|control| control.typed.program.clone());
        let mut scope = BTreeMap::new();
        let mut dependencies = BTreeMap::new();
        for &control in &roots {
            if program
                .as_ref()
                .is_none_or(|program| !Arc::ptr_eq(program, &control.typed.program))
            {
                return Err(INVALID);
            }
            retain_scope(control, &mut scope)?;
            if let ValueCode::Declaration(code) = control.typed.code()
                && control
                    .typed
                    .program()
                    .codes()
                    .get(code)
                    .is_some_and(|code| matches!(code.kind(), CodeKind::Function(_)))
                && let Some(calls) = local_summary(control)?
            {
                dependencies.insert(code, calls);
            }
        }
        // Greatest closed set: recursion alone introduces no writes. Any unknown/local
        // external effect removes its callers transitively, including recursive components.
        let mut no_writes: BTreeSet<_> = dependencies.keys().copied().collect();
        let mut callers: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
        let mut pending = BTreeSet::new();
        for (&caller, calls) in &dependencies {
            for &callee in calls {
                callers.entry(callee).or_default().insert(caller);
                if !no_writes.contains(&callee) {
                    pending.insert(caller);
                }
            }
        }
        while let Some(caller) = pending.pop_first() {
            if no_writes.remove(&caller)
                && let Some(affected) = callers.get(&caller)
            {
                pending.extend(affected);
            }
        }
        Ok(Self {
            program,
            scope,
            no_writes,
        })
    }
    pub(super) fn calls(
        &self,
        typed: &Arc<TypedQuery>,
    ) -> Result<BTreeSet<AccessExpressionId>, IntegrityError> {
        if self
            .scope
            .get(&typed.code())
            .is_none_or(|owner| !Arc::ptr_eq(owner, typed))
        {
            return Err(INVALID);
        }
        // Standalone body queries may retain an earlier captured-body type suffix. Only a
        // shared final program can consume cross-body evidence; absent evidence is conservative.
        if self
            .program
            .as_ref()
            .is_none_or(|program| !Arc::ptr_eq(program, &typed.program))
        {
            return Ok(BTreeSet::new());
        }
        Ok(typed
            .body()
            .expressions()
            .iter()
            .filter_map(|(id, expression)| match expression.kind {
                ExpressionKind::Call {
                    target: ValueFunctionOrigin::Declaration(code),
                    ..
                } if self.no_writes.contains(&code) => Some(id),
                _ => None,
            })
            .collect())
    }
}

fn retain_scope(
    control: &Control,
    scope: &mut BTreeMap<ValueCode, Arc<TypedQuery>>,
) -> Result<(), IntegrityError> {
    let mut pending = vec![control];
    while let Some(control) = pending.pop() {
        if let Some(previous) = scope.get(&control.typed.code()) {
            if !Arc::ptr_eq(previous, &control.typed) {
                return Err(INVALID);
            }
            continue;
        }
        scope.insert(control.typed.code(), control.typed.clone());
        pending.extend(control.typed.closures.values());
    }
    Ok(())
}

fn local_summary(control: &Control) -> Result<Option<BTreeSet<AccessCodeId>>, IntegrityError> {
    let typed = &control.typed;
    if !typed.results_closed() {
        return Err(IntegrityError::UnclosedResultWitnesses);
    }
    if !closed_cleanup(typed) {
        return Ok(None);
    }
    let mut calls = BTreeSet::new();
    for (_, point) in control.graph.points().iter() {
        for action in &point.actions {
            match action.operation {
                Operation::Assign { target, .. } | Operation::CompoundAssign { target, .. } => {
                    if !super::owned_base(typed, target) {
                        return Ok(None);
                    }
                }
                Operation::CommitCall(id) => {
                    let ExpressionKind::Call {
                        target: ValueFunctionOrigin::Declaration(code),
                        ..
                    } = typed.body().expressions().get(id).ok_or(INVALID)?.kind
                    else {
                        return Ok(None);
                    };
                    calls.insert(code);
                }
                _ => {}
            }
        }
    }
    Ok(Some(calls))
}

fn closed_cleanup(typed: &TypedQuery) -> bool {
    // This closed leaf/aggregate domain has no user destruction hooks. Formal/captured
    // environments stay unknown; admitting other destructors must consume their effects here.
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
            Some(PayloadSchema::Function(witness))
                if matches!(witness.origin, ValueFunctionOrigin::Declaration(_)) => {}
            // Formal/captured environments and future destruction protocols need closed effects.
            _ => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests;
