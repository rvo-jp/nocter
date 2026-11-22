//! Entry signature validation against accepted value-access declarations.
use nocter_declarations::value_access::{CodeKind, PayloadSchema};
use nocter_model::{AccessCodeId, CompilationTarget};
use nocter_runtime_contract::RuntimeAbiIdentity;

use crate::{EntryContractRule, ProcessResultContract, TargetUnavailable};

/// Checks the selected declaration's entry signature and checked body.
/// The caller selects the declaration from the authored module contract.
///
/// # Errors
/// Returns the violated entry rule if this declaration cannot be used as a process entry.
pub fn immediate_entry_result(
    unit: &nocter_checking::value_access::accepted::Unit,
    callable: AccessCodeId,
) -> Result<ProcessResultContract, EntryContractRule> {
    let program = unit.program();
    let code = program
        .codes()
        .get(callable)
        .ok_or(EntryContractRule::TopLevelFunction)?;
    if !matches!(code.kind(), CodeKind::Function(_)) {
        return Err(EntryContractRule::TopLevelFunction);
    }
    if !code.parameters().is_empty() {
        return Err(EntryContractRule::ValueParameters);
    }
    if unit.body(callable).is_none() {
        return Err(EntryContractRule::MissingBody);
    }
    let (success, fallible) = match program.data_schema(code.result()) {
        Some(PayloadSchema::Fallible(success)) => (success, true),
        _ => (code.result(), false),
    };
    let Some(PayloadSchema::Builtin(success)) = program.data_schema(success) else {
        return Err(EntryContractRule::ResultType);
    };
    ProcessResultContract::from_success_type(success, fallible).ok_or(EntryContractRule::ResultType)
}

/// Selects the implemented backend ABI for the source-selected platform.
///
/// # Errors
/// Rejects recognized platforms without a backend.
pub const fn executable_abi(
    target: CompilationTarget,
) -> Result<RuntimeAbiIdentity, TargetUnavailable> {
    match crate::capabilities::capabilities_for(target) {
        Ok((_, abi, _)) => Ok(abi),
        Err(error) => Err(error),
    }
}
