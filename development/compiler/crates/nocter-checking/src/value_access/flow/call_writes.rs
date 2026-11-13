//! Body-scoped closed caller-write evidence, projected through the selected input plans.
mod bindings;
mod requests;
mod sources;
mod summary;
#[cfg(test)]
mod tests;

use super::{Control, IntegrityError, Operation, aliases::Aliases};
use crate::value_access::{ExpressionKind, Query as TypedQuery, Referent};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::Program;
use nocter_model::{AccessExpressionId, ValueCode};
use requests::Request;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use summary::Writes;
const INVALID: IntegrityError = IntegrityError::InconsistentTypedBody;

/// Absence is unknown. A known empty list has no writes to existing caller storage.
/// Nonempty entries are conservative may-write footprints, not invocation addresses or
/// definite replacements; a frozen alias may widen to its existing containing storage.
#[derive(Clone, Debug, Default)]
pub(super) struct Calls {
    writes: BTreeMap<AccessExpressionId, Box<[Referent]>>,
    sources: sources::Selected,
}
impl Calls {
    pub(in crate::value_access::flow) fn at(
        &self,
        call: AccessExpressionId,
    ) -> Option<&[Referent]> {
        self.writes.get(&call).map(AsRef::as_ref)
    }
    pub(in crate::value_access::flow) fn verify(
        &self,
        context: &DeclarationContext<'_>,
    ) -> Result<(), crate::value_access::Failure> {
        self.sources.verify(context)
    }
}

pub(in crate::value_access) struct Catalog {
    program: Option<Arc<Program>>,
    scope: BTreeMap<ValueCode, Arc<TypedQuery>>,
    summaries: BTreeMap<Request, Writes>,
    sources: Arc<sources::Graph>,
}
impl Catalog {
    pub(in crate::value_access) fn prepare<'a>(
        controls: impl IntoIterator<Item = &'a Control>,
    ) -> Result<Self, IntegrityError> {
        Self::prepare_selected(controls, None)
    }
    pub(in crate::value_access) fn prepare_with_context<'a>(
        controls: impl IntoIterator<Item = &'a Control>,
        context: &DeclarationContext<'_>,
    ) -> Result<Self, IntegrityError> {
        Self::prepare_selected(controls, Some(context))
    }
    fn prepare_selected<'a>(
        controls: impl IntoIterator<Item = &'a Control>,
        context: Option<&DeclarationContext<'_>>,
    ) -> Result<Self, IntegrityError> {
        let roots: Vec<_> = controls.into_iter().collect();
        let program = roots.first().map(|control| control.typed.program.clone());
        for &control in &roots {
            if program
                .as_ref()
                .is_none_or(|program| !Arc::ptr_eq(program, &control.typed.program))
            {
                return Err(INVALID);
            }
        }
        if let (Some(program), Some(context)) = (&program, context)
            && !program.preserves_headers(context.program())
        {
            return Err(INVALID);
        }
        let scope = super::scope(roots.iter().copied())?
            .into_iter()
            .map(|(code, control)| (code, control.typed.clone()))
            .collect();
        let closed = summary::close(&roots, context.map(DeclarationContext::primitives))?;
        let sources = Arc::new(sources::Graph::new(&scope, closed.callees, context));
        Ok(Self {
            program,
            scope,
            summaries: closed.writes,
            sources,
        })
    }
    pub(super) fn calls(&self, control: &Control) -> Result<Calls, IntegrityError> {
        let typed = &control.typed;
        if self
            .scope
            .get(&typed.code())
            .is_none_or(|owner| !Arc::ptr_eq(owner, typed))
        {
            return Err(INVALID);
        }
        let mut result = Calls::default();
        // A standalone captured body can retain an earlier type suffix. Cross-body evidence
        // requires the shared final program and its exact typed owner.
        if self
            .program
            .as_ref()
            .is_none_or(|program| !Arc::ptr_eq(program, &typed.program))
        {
            return Ok(result);
        }
        let mut dependencies = BTreeSet::new();
        let roots = Aliases::prepare(typed)?;
        for point in control.graph.reachable()? {
            let point = control.graph.points().get(point).ok_or(INVALID)?;
            for action in &point.actions {
                let Operation::CommitCall(id) = action.operation else {
                    continue;
                };
                let expression = typed.body().expressions().get(id).ok_or(INVALID)?;
                if matches!(expression.kind, ExpressionKind::Call { .. })
                    && let Some(request) = Request::unbound(typed.code()).call(typed, id)?
                {
                    dependencies.insert(request.clone());
                    if let Some(Writes::Known(inputs)) = self.summaries.get(&request)
                        && let Some(writes) = bindings::actual_writes(typed, &roots, id, inputs)?
                    {
                        result.writes.insert(id, writes.into());
                    }
                }
            }
        }
        result.sources = self.sources.select(typed.code(), dependencies);
        Ok(result)
    }
}
