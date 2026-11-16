//! Complete identities map tags; each case retains independently selected containing offsets.
use super::{CallableCase, Failure, Field, MachineLayoutKind, MachineLayoutPlan, Pair, Plan};
use nocter_runtime_contract::RuntimeEnvironment;

pub(super) fn select(
    runtime: &RuntimeEnvironment,
    layouts: &MachineLayoutPlan,
    pair: Pair,
) -> Result<Plan, Failure> {
    let from = layouts.get(pair.0).ok_or(Failure::InvalidRoot)?.kind();
    let to = layouts.get(pair.1).ok_or(Failure::InvalidRoot)?.kind();
    let MachineLayoutKind::CallableAlternatives {
        payload_offset: target,
        ..
    } = to
    else {
        return Err(Failure::InvalidRoot);
    };
    let (source_choice, source) = match from {
        MachineLayoutKind::StaticCallable { .. } => (false, 0),
        MachineLayoutKind::CallableAlternatives { payload_offset, .. } => (true, *payload_offset),
        _ => return Err(Failure::InvalidRoot),
    };
    let cases = runtime
        .callable_case_targets(pair.0, pair.1)
        .ok_or(Failure::InvalidRoot)?;
    let cases = cases
        .iter()
        .map(|case| CallableCase {
            target_tag: case.target(),
            payload: Field {
                pair: (case.data(), case.data()),
                source,
                target: *target,
            },
        })
        .collect();
    Ok(Plan::Callable {
        source_choice,
        cases,
    })
}
