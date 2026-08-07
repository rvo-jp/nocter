//! Source evidence for body-backed result-allocation contracts.
//!
//! Provenance summaries own the semantic fixed point. This module reruns the
//! same return-flow collector against the converged summaries and extracts a
//! deterministic source witness for diagnostics. It does not infer contracts
//! from syntax or callable names.

use super::*;

pub(in crate::typecheck) fn result_allocation_witness_for_callable_body(
    block: &Block,
    return_type: &Type,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ByteSpan> {
    let mut flow = ProvenanceFlow::default();
    let mut body_environment = environment.clone();
    let mut body_borrow_provenance = ProvenanceEnvironment::default();
    collect_return_statement_provenance(
        block,
        return_type,
        resolved,
        &mut body_environment,
        &mut body_borrow_provenance,
        summaries,
        &mut flow,
    );
    collect_block_result_provenance(
        block,
        return_type,
        resolved,
        environment,
        &ProvenanceEnvironment::default(),
        summaries,
        &mut flow,
    );
    flow.result_allocation_witness()
}
