use std::collections::{HashSet, VecDeque};

use nocter_constant_evaluation::{
    CompileTimeCallTarget, CompileTimeCallablePlan, CompileTimeCallableRecipe,
    CompileTimePlanTable, DependencyComputation, DependencyQuery, DependencyQueryError,
};
use nocter_declarations::BodyOwner;
use nocter_declarations::DeclarationGraph;
use nocter_model::{Arena, CompileTimeGuarantee, TypeStore};

use super::{
    CompileTimeProjectionError, CompileTimeProjectionRule, ProjectedCompileTimePlans,
    project_compile_time_callable_recipe, project_compile_time_initializer_plan,
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
                    owner: BodyOwner::Callable(callable),
                    body: None,
                    node: None,
                    rule: CompileTimeProjectionRule::InvalidPlan,
                })
            })?;
        if declaration.guarantees().compile_time() != CompileTimeGuarantee::Evaluatable {
            return Err(DependencyQueryError::computation(
                CompileTimeProjectionError {
                    owner: BodyOwner::Callable(callable),
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
                    owner: BodyOwner::Callable(callable),
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
) -> Result<ProjectedCompileTimePlans, CompileTimeProjectionError> {
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
                owner: BodyOwner::Callable(callable),
                body: Some(body_id),
                node: None,
                rule: CompileTimeProjectionRule::InvalidPlan,
            })?;
            project_compile_time_callable_recipe(graph, types, callable, body_id, body).map(Some)
        })?;
    let mut pending = VecDeque::new();
    let mut scheduled = HashSet::new();
    let initializers = build_initializer_plans(graph, types, bodies)?;
    for (_, plan) in initializers.iter() {
        let Some(plan) = plan else {
            continue;
        };
        schedule_calls(plan, &mut scheduled, &mut pending);
    }
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
                owner: BodyOwner::Callable(callable),
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
                    owner: BodyOwner::Callable(target.callable()),
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
        schedule_calls(&plan, &mut scheduled, &mut pending);
    }

    let callables = CompileTimePlanTable::new(query.into_completed()).map_err(|error| {
        let caller = error.caller().callable();
        CompileTimeProjectionError {
            owner: BodyOwner::Callable(caller),
            body: graph
                .declarations()
                .callables()
                .get(caller)
                .and_then(nocter_declarations::CallableDeclaration::body),
            node: Some(error.node()),
            rule: CompileTimeProjectionRule::UnavailableCallTarget,
        }
    })?;
    Ok(ProjectedCompileTimePlans {
        callables,
        initializers,
    })
}

fn build_initializer_plans(
    graph: &DeclarationGraph,
    types: &TypeStore,
    bodies: &Arena<nocter_model::BodyId, CheckedBody>,
) -> Result<Arena<nocter_model::BodyId, Option<CompileTimeCallablePlan>>, CompileTimeProjectionError>
{
    graph
        .declarations()
        .bodies()
        .try_map(|body_id, declaration| match declaration.owner() {
            BodyOwner::Constant(_) | BodyOwner::Static(_) => {
                let body = bodies.get(body_id).ok_or(CompileTimeProjectionError {
                    owner: declaration.owner(),
                    body: Some(body_id),
                    node: None,
                    rule: CompileTimeProjectionRule::InvalidPlan,
                })?;
                project_compile_time_initializer_plan(
                    graph,
                    types,
                    declaration.owner(),
                    body_id,
                    body,
                )
                .map(Some)
            }
            BodyOwner::Callable(_) | BodyOwner::Drop(_) | BodyOwner::Test(_) => Ok(None),
        })
}

fn schedule_calls(
    plan: &CompileTimeCallablePlan,
    scheduled: &mut HashSet<CompileTimeCallTarget>,
    pending: &mut VecDeque<CompileTimeCallTarget>,
) {
    for target in plan.call_dependencies() {
        if scheduled.insert(target.clone()) {
            pending.push_back(target.clone());
        }
    }
}
