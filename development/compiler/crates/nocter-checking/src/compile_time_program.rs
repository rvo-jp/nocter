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
    target: nocter_model::CompilationTarget,
    values: Arc<DeclarationValueTable>,
    plans: Arc<CompileTimePlanTable>,
    initializer_plans: Arc<
        nocter_model::Arena<
            nocter_model::BodyId,
            Option<nocter_constant_evaluation::CompileTimeCallablePlan>,
        >,
    >,
}

impl CompileTimeProgram {
    pub(crate) fn new(
        target: nocter_model::CompilationTarget,
        values: Arc<DeclarationValueTable>,
        projected: crate::compile_time_projection::ProjectedCompileTimePlans,
    ) -> Self {
        Self {
            target,
            values,
            plans: Arc::new(projected.callables),
            initializer_plans: Arc::new(projected.initializers),
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

    #[must_use]
    pub fn initializer_plan(
        &self,
        body: nocter_model::BodyId,
    ) -> Option<&nocter_constant_evaluation::CompileTimeCallablePlan> {
        self.initializer_plans.get(body).and_then(Option::as_ref)
    }

    /// Opens deterministic execution over this program's exact value and plan authorities.
    #[must_use]
    pub fn executor(
        &self,
        limits: nocter_constant_evaluation::CompileTimeEvaluationLimits,
    ) -> nocter_constant_evaluation::CompileTimeExecutor<'_> {
        nocter_constant_evaluation::CompileTimeExecutor::new(
            self.plans(),
            self.values().constants(),
            self.target,
            limits,
        )
    }
}
