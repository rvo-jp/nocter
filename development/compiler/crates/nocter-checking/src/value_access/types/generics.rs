//! Close the applications retained at original source calls over the canonical body arena.
use super::{DeclarationContext, Program, Query, accepted};
use crate::value_access::{CallKind, ExpressionKind, Rule, flow};
use nocter_declarations::value_access::GenericFrameError;
use nocter_model::{TypeId, ValueCodeRoot, ValueFunctionOrigin};
use std::collections::{BTreeMap, BTreeSet};

const INVALID: accepted::Failure =
    accepted::Failure::Integrity(flow::IntegrityError::InconsistentTypedBody);

pub(super) fn project(
    context: &DeclarationContext<'_>,
    program: Program,
    bodies: &BTreeMap<ValueCodeRoot, Query>,
) -> Result<Program, accepted::Failure> {
    let mut pending = bodies.values().collect::<Vec<_>>();
    let mut applications = BTreeMap::new();
    while let Some(body) = pending.pop() {
        pending.extend(body.closures.values().map(|closure| closure.typed.as_ref()));
        for (call, expression) in body.body().expressions().iter() {
            let ExpressionKind::Call {
                target,
                kind,
                arguments,
            } = &expression.kind
            else {
                continue;
            };
            let Some(application) = arguments.generic_application() else {
                continue;
            };
            let (CallKind::Direct, ValueFunctionOrigin::Declaration(code)) = (kind, target) else {
                return Err(INVALID);
            };
            applications
                .entry((*code, application))
                .or_insert((body, call));
        }
    }
    if applications.is_empty() {
        return Ok(program);
    }
    let mut roots = BTreeMap::new();
    for &(code, _) in applications.keys() {
        roots
            .entry(code)
            .or_insert_with(|| body_types(bodies.get(&code.into())));
    }
    let frames = applications
        .keys()
        .map(|&(code, application)| (code, application, roots[&code].iter().copied()));
    program.project_generic_frames(frames).map_err(|failure| {
        let GenericFrameError::Application { code, arguments } = failure else {
            return INVALID;
        };
        let Some(&(body, call)) = applications.get(&(code, &arguments)) else {
            return INVALID;
        };
        accepted::Failure::Body {
            callable: body.source_owner(),
            failure: crate::value_access::diagnostics::query_rejection(
                context,
                body,
                Some(call),
                Rule::UnsupportedData,
                "this application cannot represent all the data used by the generic body",
            ),
        }
    })
}

fn body_types(body: Option<&Query>) -> BTreeSet<TypeId> {
    let mut roots = BTreeSet::new();
    let mut pending: Vec<_> = body.into_iter().collect();
    while let Some(body) = pending.pop() {
        pending.extend(body.closures.values().map(|closure| closure.typed.as_ref()));
        roots.insert(body.result_data());
        roots.extend(body.body().locals().iter().map(|(_, local)| local.data));
        roots.extend(body.body().places().iter().map(|(_, place)| place.data));
        roots.extend(
            body.body()
                .expressions()
                .iter()
                .filter_map(|(_, expression)| {
                    expression.ty.access().map(nocter_model::ValueAccess::data)
                }),
        );
        roots.extend(
            body.body()
                .blocks()
                .iter()
                .filter_map(|(_, block)| block.ty.access().map(nocter_model::ValueAccess::data)),
        );
    }
    roots
}
