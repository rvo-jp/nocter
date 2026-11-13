//! Concrete metadata selection uses original source recipes, not runtime values or signatures.
use super::*;
use crate::value_access::{CallKind, ExpressionKind, InstantiationFailure};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::CodeInstanceKey;
use nocter_model::{AccessCodeId, ValueApplication};
use std::num::NonZeroU64;

fn budget(units: u64) -> WorkBudget {
    WorkBudget::new(NonZeroU64::new(units).unwrap())
}
fn code(unit: &Unit, name: &str) -> AccessCodeId {
    unit.program()
        .resolve(unit.program().symbols().get(name).unwrap())
        .unwrap()
}
fn value_call(unit: &Unit, owner: AccessCodeId) -> ValueApplication {
    let body = unit.body(owner).unwrap().typed().body();
    let call = body
        .expressions()
        .iter()
        .find_map(|(id, expression)| {
            matches!(
                &expression.kind,
                ExpressionKind::Call {
                    kind: CallKind::Value,
                    ..
                }
            )
            .then_some(id)
        })
        .unwrap();
    ValueApplication {
        owner: owner.into(),
        call,
    }
}
const RECURSIVE: &str = "func first(): i32 { 42 }\nfunc forward(take action: func(): i32, depth: i32): func(): i32 { if depth > 0 { return forward(action^, depth - 1) }\naction^ }\nfunc run(take action: func(): i32): i32 { let next = forward(action^, 2)\nnext() }";
fn frame(unit: &Unit) -> CodeInstanceKey {
    let run = code(unit, "run");
    let parameter = unit.program().codes().get(run).unwrap().parameters()[0].into();
    unit.program()
        .bind_code(run.into(), [(parameter, code(unit, "first").into())])
        .unwrap()
}

#[test]
fn concrete_invocation_closes_an_anchored_recursive_result_without_host_recursion() {
    let unit = accepted(RECURSIVE);
    let caller = frame(&unit);
    let resolved = unit
        .instantiate_invocation(
            &caller,
            value_call(&unit, code(&unit, "run")),
            &mut budget(10_000),
        )
        .unwrap();
    assert_eq!(
        resolved,
        unit.program()
            .bind_code(code(&unit, "first").into(), [])
            .unwrap()
    );
}

#[test]
fn work_failure_does_not_publish_or_poison_a_later_invocation_request() {
    let unit = accepted(RECURSIVE);
    let caller = frame(&unit);
    let site = value_call(&unit, code(&unit, "run"));
    assert_eq!(
        unit.instantiate_invocation(&caller, site, &mut budget(1)),
        Err(InstantiationFailure::WorkLimit)
    );
    let resolved = unit
        .instantiate_invocation(&caller, site, &mut budget(10_000))
        .unwrap();
    assert_eq!(resolved.code(), code(&unit, "first").into());
    assert_eq!(
        unit.instantiate_invocation(&caller, site, &mut budget(1)),
        Err(InstantiationFailure::WorkLimit)
    );
}

#[test]
fn metadata_resolution_rejects_a_foreign_owner_and_an_uncommitted_site() {
    let unit = accepted(RECURSIVE);
    let caller = frame(&unit);
    let mut site = value_call(&unit, code(&unit, "run"));
    site.owner = code(&unit, "first").into();
    assert_eq!(
        unit.instantiate_invocation(&caller, site, &mut budget(1)),
        Err(InstantiationFailure::WrongOwner)
    );
    let body = unit.body(code(&unit, "run")).unwrap().typed().body();
    site.owner = caller.code();
    site.call = body
        .expressions()
        .iter()
        .find_map(|(id, expression)| {
            (!matches!(&expression.kind, ExpressionKind::Call { .. })).then_some(id)
        })
        .unwrap();
    assert_eq!(
        unit.instantiate_invocation(&caller, site, &mut budget(1)),
        Err(InstantiationFailure::MissingCall)
    );
}
