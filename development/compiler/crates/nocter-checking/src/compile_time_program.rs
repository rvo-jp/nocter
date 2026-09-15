use std::sync::Arc;

use nocter_declarations::DeclarationValueTable;

use crate::CompileTimePlanTable;

/// Complete compile-time authority published by one accepted checked program.
///
/// Declaration construction produces values before bodies can be checked. Program finalization
/// then closes callable specializations and joins both immutable strata here. Keeping the strata
/// explicit prevents either producer from re-entering the other while giving every downstream
/// consumer one authority for compile-time facts.
#[derive(Clone, Debug)]
pub struct CompileTimeProgram {
    values: Arc<DeclarationValueTable>,
    plans: Arc<CompileTimePlanTable>,
}

impl CompileTimeProgram {
    pub(crate) fn new(values: Arc<DeclarationValueTable>, plans: CompileTimePlanTable) -> Self {
        Self {
            values,
            plans: Arc::new(plans),
        }
    }

    #[must_use]
    pub fn values(&self) -> &DeclarationValueTable {
        &self.values
    }

    #[must_use]
    pub fn plans(&self) -> &CompileTimePlanTable {
        &self.plans
    }

    /// Opens deterministic execution over this program's exact value and plan authorities.
    #[must_use]
    pub fn executor(
        &self,
        target: nocter_model::CompilationTarget,
        limits: nocter_constant_evaluation::CompileTimeEvaluationLimits,
    ) -> nocter_constant_evaluation::CompileTimeExecutor<'_> {
        nocter_constant_evaluation::CompileTimeExecutor::new(
            self.plans(),
            self.values().constants(),
            target,
            limits,
        )
    }
}
