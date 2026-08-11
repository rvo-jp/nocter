//! Compile-unit semantic summaries shared by every source-level typecheck.
//!
//! Callable provenance is a fixed point over the complete compile unit. Computing it for an
//! individual source produces the same result, so source checks borrow one context instead of
//! rebuilding the complete fixed point for every file.

use super::TypecheckSource;
use super::provenance::CallableProvenanceSummaries;
use super::returns::callable_provenance_summaries;

pub(crate) struct TypecheckCompileUnitContext {
    pub(super) provenance_summaries: CallableProvenanceSummaries,
}

impl TypecheckCompileUnitContext {
    pub(crate) fn new(summary_sources: &[TypecheckSource<'_>]) -> Self {
        Self {
            provenance_summaries: callable_provenance_summaries(summary_sources),
        }
    }
}
