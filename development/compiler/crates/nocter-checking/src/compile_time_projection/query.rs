use std::collections::{HashSet, VecDeque};

use nocter_constant_evaluation::{
    CompileTimeCallTarget, CompileTimeCallablePlan, CompileTimeCallableRecipe,
    CompileTimeOperation, CompileTimePlanTable, DependencyComputation, DependencyQuery,
    DependencyQueryError,
};
use nocter_declarations::DeclarationGraph;
use nocter_model::{Arena, CompileTimeGuarantee, TypeStore};

use super::{
    CompileTimeProjectionError, CompileTimeProjectionRule, project_compile_time_callable_recipe,
    specialize_compile_time_callable_recipe,
};
use crate::CheckedBody;

struct PlanComputation<'a> {
    graph: &'a DeclarationGraph,
    types: &'a TypeStore,
    recipes: &'a Arena<nocter_model::CallableId, Option<CompileTimeCallableRecipe>>,
}

impl
    DependencyComputation<
        CompileTimeCallTarget,
        CompileTimeCallablePlan,
        CompileTimeProjectionError,
    > for PlanComputation<'_>
{
    fn compute(
        &mut self,
        _query: &mut DependencyQuery<CompileTimeCallTarget, CompileTimeCallablePlan>,
        target: CompileTimeCallTarget,
    ) -> Result<
        CompileTimeCallablePlan,
        DependencyQueryError<CompileTimeCallTarget, CompileTimeProjectionError>,
    > {
        let callable = target.callable();
        let declaration = self
            .graph
            .declarations()
            .callables()
            .get(callable)
            .ok_or_else(|| {
                DependencyQueryError::computation(CompileTimeProjectionError {
                    callable,
                    body: None,
                    node: None,
                    rule: CompileTimeProjectionRule::InvalidPlan,
                })
            })?;
        if declaration.guarantees().compile_time() != CompileTimeGuarantee::Evaluatable {
            return Err(DependencyQueryError::computation(
                CompileTimeProjectionError {
                    callable,
                    body: declaration.body(),
                    node: None,
                    rule: CompileTimeProjectionRule::RuntimeOnlyCall,
                },
            ));
        }
        let body = declaration.body();
        let recipe = self
            .recipes
            .get(callable)
            .and_then(Option::as_ref)
            .ok_or_else(|| {
                DependencyQueryError::computation(CompileTimeProjectionError {
                    callable,
                    body,
                    node: None,
                    rule: CompileTimeProjectionRule::UnavailableCallTarget,
                })
            })?;
        specialize_compile_time_callable_recipe(self.graph, self.types, &target, recipe)
            .map_err(DependencyQueryError::computation)
    }
}

/// Builds the only closed compile-time specialization authority for one checked generation.
///
/// Every non-generic compile-time body is a root. Specialized recipe call edges discover further
/// closed specializations, including generic callees, and one dependency query memoizes each
/// target.
/// Source recursion is not a plan-construction cycle: a plan completes before its call edges are
/// scheduled, so recursive call graphs close normally and remain governed by evaluation depth.
pub(crate) fn build_compile_time_plan_table(
    graph: &DeclarationGraph,
    types: &TypeStore,
    bodies: &Arena<nocter_model::BodyId, CheckedBody>,
) -> Result<CompileTimePlanTable, CompileTimeProjectionError> {
    let recipes = graph
        .declarations()
        .callables()
        .try_map(|callable, declaration| {
            if declaration.guarantees().compile_time() != CompileTimeGuarantee::Evaluatable {
                return Ok(None);
            }
            let Some(body_id) = declaration.body() else {
                return Ok(None);
            };
            let body = bodies.get(body_id).ok_or(CompileTimeProjectionError {
                callable,
                body: Some(body_id),
                node: None,
                rule: CompileTimeProjectionRule::InvalidPlan,
            })?;
            project_compile_time_callable_recipe(graph, types, callable, body_id, body).map(Some)
        })?;
    let mut pending = VecDeque::new();
    let mut scheduled = HashSet::new();
    for (callable, declaration) in graph.declarations().callables().iter() {
        if declaration.guarantees().compile_time() != CompileTimeGuarantee::Evaluatable
            || declaration.body().is_none()
            || !graph
                .declarations()
                .callable_generic_domain(callable)
                .is_some_and(|domain| domain.is_empty())
        {
            continue;
        }
        let target =
            CompileTimeCallTarget::new(callable, []).map_err(|_| CompileTimeProjectionError {
                callable,
                body: declaration.body(),
                node: None,
                rule: CompileTimeProjectionRule::InvalidPlan,
            })?;
        scheduled.insert(target.clone());
        pending.push_back(target);
    }

    let mut computation = PlanComputation {
        graph,
        types,
        recipes: &recipes,
    };
    let mut query = DependencyQuery::default();
    while let Some(target) = pending.pop_front() {
        let plan = match query.resolve(&mut computation, target.clone()) {
            Ok(plan) => plan,
            Err(DependencyQueryError::Computation(error)) => return Err(error),
            Err(DependencyQueryError::Cycle(_)) => {
                return Err(CompileTimeProjectionError {
                    callable: target.callable(),
                    body: graph
                        .declarations()
                        .callables()
                        .get(target.callable())
                        .and_then(nocter_declarations::CallableDeclaration::body),
                    node: None,
                    rule: CompileTimeProjectionRule::InvalidPlan,
                });
            }
        };
        for (_, node) in plan.nodes().iter() {
            let CompileTimeOperation::Call { target, .. } = node.operation() else {
                continue;
            };
            if scheduled.insert(target.clone()) {
                pending.push_back(target.clone());
            }
        }
    }

    CompileTimePlanTable::new(query.into_completed()).map_err(|error| {
        let caller = error.caller().callable();
        CompileTimeProjectionError {
            callable: caller,
            body: graph
                .declarations()
                .callables()
                .get(caller)
                .and_then(nocter_declarations::CallableDeclaration::body),
            node: Some(error.node()),
            rule: CompileTimeProjectionRule::UnavailableCallTarget,
        }
    })
}
