use nocter_constant_evaluation::{
    CompileTimeConstantResolver, CompileTimeEvaluationLimits, CompileTimeExecutionRule,
    CompileTimeExecutor, DependencyComputation, DependencyQuery, DependencyQueryError,
};
use nocter_declarations::{
    BodyOwner, DeclarationGraph, DeclarationValueTable, StructuralConstantTable,
};
use nocter_model::{ConstantId, ConstantValue, TypeStore};

use crate::compile_time_projection::ProjectedCompileTimePlans;

/// Failure to produce declaration values from the checked initializer authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompileTimeValueBuildError {
    owner: BodyOwner,
    body: Option<nocter_model::BodyId>,
    node: Option<nocter_model::BodyNodeId>,
    rule: CompileTimeValueBuildRule,
}

impl CompileTimeValueBuildError {
    pub(crate) const fn owner(self) -> BodyOwner {
        self.owner
    }

    pub(crate) const fn body(self) -> Option<nocter_model::BodyId> {
        self.body
    }

    pub(crate) const fn node(self) -> Option<nocter_model::BodyNodeId> {
        self.node
    }

    pub(crate) const fn rule(self) -> CompileTimeValueBuildRule {
        self.rule
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompileTimeValueBuildRule {
    MissingInitializerPlan,
    MissingCallPlan,
    ConstantDependencyCycle,
    Execution(CompileTimeExecutionRule),
    InvalidConstantValue,
    InvalidStaticValue,
}

#[derive(Debug)]
pub(crate) enum CompileTimeValueCompletionError {
    Authored(CompileTimeValueBuildError),
    Integrity(nocter_declarations::DeclarationValueTableError),
}

impl From<CompileTimeValueBuildError> for CompileTimeValueCompletionError {
    fn from(error: CompileTimeValueBuildError) -> Self {
        Self::Authored(error)
    }
}

struct CompletedConstants<'query> {
    query: &'query DependencyQuery<ConstantId, ConstantValue>,
}

impl CompileTimeConstantResolver for CompletedConstants<'_> {
    fn resolve_constant(&self, id: ConstantId) -> Option<&ConstantValue> {
        self.query.completed(&id)
    }
}

struct ConstantValueComputation<'program> {
    graph: &'program DeclarationGraph,
    projected: &'program ProjectedCompileTimePlans,
}

impl DependencyComputation<ConstantId, ConstantValue, CompileTimeValueBuildError>
    for ConstantValueComputation<'_>
{
    fn compute(
        &mut self,
        query: &mut DependencyQuery<ConstantId, ConstantValue>,
        constant: ConstantId,
    ) -> Result<ConstantValue, DependencyQueryError<ConstantId, CompileTimeValueBuildError>> {
        let declaration = self
            .graph
            .declarations()
            .constants()
            .get(constant)
            .ok_or_else(|| {
                DependencyQueryError::computation(CompileTimeValueBuildError {
                    owner: BodyOwner::Constant(constant),
                    body: None,
                    node: None,
                    rule: CompileTimeValueBuildRule::MissingInitializerPlan,
                })
            })?;
        let body = declaration.initializer();
        let owner = BodyOwner::Constant(constant);
        let plan = self
            .projected
            .initializers
            .get(body)
            .and_then(Option::as_ref)
            .ok_or_else(|| {
                DependencyQueryError::computation(CompileTimeValueBuildError {
                    owner,
                    body: Some(body),
                    node: None,
                    rule: CompileTimeValueBuildRule::MissingInitializerPlan,
                })
            })?;
        let dependencies = self
            .projected
            .callables
            .constant_dependency_closure(plan)
            .map_err(|_| {
                DependencyQueryError::computation(CompileTimeValueBuildError {
                    owner,
                    body: Some(body),
                    node: None,
                    rule: CompileTimeValueBuildRule::MissingCallPlan,
                })
            })?;
        for dependency in dependencies {
            query.resolve(self, dependency)?;
        }
        let constants = CompletedConstants { query };
        let mut executor = CompileTimeExecutor::new(
            &self.projected.callables,
            &constants,
            self.graph.target(),
            CompileTimeEvaluationLimits::default(),
        );
        executor
            .evaluate_initializer(body, plan)
            .map_err(|error| {
                DependencyQueryError::computation(CompileTimeValueBuildError {
                    owner,
                    body: Some(body),
                    node: error.node(),
                    rule: CompileTimeValueBuildRule::Execution(error.rule()),
                })
            })?
            .as_ref()
            .clone()
            .into_constant()
            .map_err(|rule| {
                DependencyQueryError::computation(CompileTimeValueBuildError {
                    owner,
                    body: Some(body),
                    node: Some(plan.root()),
                    rule: match rule {
                        CompileTimeExecutionRule::TypeMismatch => {
                            CompileTimeValueBuildRule::InvalidConstantValue
                        }
                        rule => CompileTimeValueBuildRule::Execution(rule),
                    },
                })
            })
    }
}

/// Produces the complete declaration-value table from checked initializer plans.
pub(crate) fn build_checked_declaration_values(
    graph: &DeclarationGraph,
    types: &TypeStore,
    structural: &StructuralConstantTable,
    projected: &ProjectedCompileTimePlans,
) -> Result<DeclarationValueTable, CompileTimeValueCompletionError> {
    let mut computation = ConstantValueComputation { graph, projected };
    let mut query = DependencyQuery::default();
    for (constant, declaration) in graph.declarations().constants().iter() {
        if let Err(error) = query.resolve(&mut computation, constant) {
            return Err(project_query_error(graph, declaration.initializer(), error).into());
        }
    }
    let constants = graph
        .declarations()
        .constants()
        .try_map(|constant, declaration| {
            query
                .completed(&constant)
                .cloned()
                .ok_or(CompileTimeValueBuildError {
                    owner: BodyOwner::Constant(constant),
                    body: Some(declaration.initializer()),
                    node: None,
                    rule: CompileTimeValueBuildRule::MissingInitializerPlan,
                })
        })?;
    let statics = graph
        .declarations()
        .statics()
        .try_map(|static_id, declaration| {
            let owner = BodyOwner::Static(static_id);
            let body = declaration.initializer();
            let plan = projected
                .initializers
                .get(body)
                .and_then(Option::as_ref)
                .ok_or(CompileTimeValueBuildError {
                    owner,
                    body: Some(body),
                    node: None,
                    rule: CompileTimeValueBuildRule::MissingInitializerPlan,
                })?;
            projected
                .callables
                .constant_dependency_closure(plan)
                .map_err(|_| CompileTimeValueBuildError {
                    owner,
                    body: Some(body),
                    node: None,
                    rule: CompileTimeValueBuildRule::MissingCallPlan,
                })?;
            let mut executor = CompileTimeExecutor::new(
                &projected.callables,
                &constants,
                graph.target(),
                CompileTimeEvaluationLimits::default(),
            );
            executor
                .evaluate_initializer(body, plan)
                .map_err(|error| CompileTimeValueBuildError {
                    owner,
                    body: Some(body),
                    node: error.node(),
                    rule: CompileTimeValueBuildRule::Execution(error.rule()),
                })?
                .as_ref()
                .clone()
                .into_frozen()
                .map_err(|rule| CompileTimeValueBuildError {
                    owner,
                    body: Some(body),
                    node: Some(plan.root()),
                    rule: match rule {
                        CompileTimeExecutionRule::TypeMismatch => {
                            CompileTimeValueBuildRule::InvalidStaticValue
                        }
                        rule => CompileTimeValueBuildRule::Execution(rule),
                    },
                })
        })?;
    DeclarationValueTable::for_program(graph, types, structural, constants, statics)
        .map_err(CompileTimeValueCompletionError::Integrity)
}

fn project_query_error(
    graph: &DeclarationGraph,
    requested_body: nocter_model::BodyId,
    error: DependencyQueryError<ConstantId, CompileTimeValueBuildError>,
) -> CompileTimeValueBuildError {
    match error {
        DependencyQueryError::Computation(error) => error,
        DependencyQueryError::Cycle(cycle) => {
            let constant = cycle.first().copied().unwrap_or_else(|| {
                unreachable!("a dependency cycle always contains its repeated identity")
            });
            let body = graph.declarations().constants().get(constant).map_or(
                requested_body,
                nocter_declarations::ConstantDeclaration::initializer,
            );
            CompileTimeValueBuildError {
                owner: BodyOwner::Constant(constant),
                body: Some(body),
                node: None,
                rule: CompileTimeValueBuildRule::ConstantDependencyCycle,
            }
        }
    }
}
