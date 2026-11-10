//! One value query coordinates unpublished ordinary body drafts and executable const closures.
use super::{Completed, Output, Rule, final_diagnostic, projection};
use crate::value_access::{self, ExpressionKind, Failure as BodyFailure, Query, accepted, checker};
use nocter_constant_evaluation::{
    DependencyComputation, DependencyQuery, DependencyQueryError, state,
};
use nocter_declaration_lowering::access_declarations::Context;
use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{AccessCallableId, CompileTimeGuarantee, ValueFunctionOrigin};
use nocter_syntax::SyntaxOrigin;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
type Failure = accepted::Failure;
type Values = DependencyQuery<AccessCallableId, Completed>;
type QueryError = DependencyQueryError<AccessCallableId, Failure>;
const INVALID: Failure =
    Failure::Integrity(value_access::flow::IntegrityError::InconsistentTypedBody);
struct Preparation<'a, 'source> {
    context: &'a Context<'source>,
    budget: state::Budget,
    queries: BTreeMap<AccessCallableId, Query>,
    sources: value_access::source_dependencies::SourceDependencies,
}
pub(in crate::value_access) fn prepare(
    context: &Context<'_>,
    limits: state::Limits,
) -> Result<Output, Failure> {
    let mut preparation = Preparation {
        context,
        budget: state::Budget::new(limits),
        queries: BTreeMap::new(),
        sources: value_access::source_dependencies::SourceDependencies::default(),
    };
    let mut values = Values::default();
    for (id, declaration) in context.program().callables().iter() {
        if declaration.kind().is_initializer() {
            preparation.budget.restart_work();
            values
                .resolve(&mut preparation, id)
                .map_err(|failure| match failure {
                    QueryError::Computation(failure) => failure,
                    QueryError::Cycle(_) => INVALID,
                })?;
        }
    }
    // Ordinary bodies and unused const definitions use the same checker and all completed values.
    for (id, _) in context.program().callables().iter() {
        preparation.body(&mut values, id)?;
    }
    Ok(Output {
        queries: preparation.queries,
        values: values
            .into_completed()
            .into_iter()
            .map(|(id, value)| (id, Arc::new(value.frozen.clone())))
            .collect(),
        dependencies: Arc::new(preparation.sources),
    })
}
impl Preparation<'_, '_> {
    fn body(&mut self, values: &mut Values, id: AccessCallableId) -> Result<(), Failure> {
        if self.queries.contains_key(&id) {
            return self.require_statics(values, id);
        }
        loop {
            match checker::prepare(self.context, id, values) {
                Ok(query) => {
                    if query
                        .program()
                        .callables()
                        .get(id)
                        .ok_or(INVALID)?
                        .compile_time()
                        == CompileTimeGuarantee::Evaluatable
                    {
                        self.sources.insert(id, query.body_bytes.clone());
                    }
                    self.queries.insert(id, query);
                    return self.require_statics(values, id);
                }
                Err(BodyFailure::RequiredConstant {
                    initializer,
                    origin,
                }) => {
                    self.require_value(values, id, initializer, origin)?;
                    // No typed product was published. Retry this draft with its completed dependency;
                    // successful bodies are cached once and are never reopened for a later value.
                }
                Err(failure) => {
                    return Err(Failure::Body {
                        callable: id,
                        failure,
                    });
                }
            }
        }
    }
    fn require_value(
        &mut self,
        values: &mut Values,
        owner: AccessCallableId,
        initializer: AccessCallableId,
        origin: SyntaxOrigin,
    ) -> Result<(), Failure> {
        if values.completed(&initializer).is_some() {
            return Ok(());
        }
        self.budget.step().map_err(|rule| {
            super::failure_at(self.context, initializer, Rule::Execution(rule), origin)
        })?;
        if let Err(error) = values.resolve(self, initializer) {
            let mut failure = match error {
                QueryError::Computation(failure) => failure,
                QueryError::Cycle(path) => self.cycle(&path, origin)?,
            };
            super::add_dependency_note(&mut failure, self.context, origin).map_err(|failure| {
                Failure::Body {
                    callable: owner,
                    failure,
                }
            })?;
            return Err(failure);
        }
        Ok(())
    }
    fn require_statics(
        &mut self,
        values: &mut Values,
        owner: AccessCallableId,
    ) -> Result<(), Failure> {
        // A complete cached body still retains its authored requirements. A nested request may
        // reach that body while one of its static values is active; caching must not hide cycles.
        fn collect(
            query: &Query,
            context: &Context<'_>,
            pending: &mut Vec<(AccessCallableId, SyntaxOrigin)>,
        ) -> Result<(), Failure> {
            let source = query.bind(context).map_err(|failure| Failure::Body {
                callable: query.source_owner(),
                failure,
            })?;
            for (local, id) in query.body().static_views() {
                pending.push((id.initializer, source.local_origin(local).ok_or(INVALID)?));
            }
            for nested in query.closures.values() {
                collect(&nested.typed, context, pending)?;
            }
            Ok(())
        }
        let mut pending = Vec::new();
        collect(
            self.queries.get(&owner).ok_or(INVALID)?,
            self.context,
            &mut pending,
        )?;
        for (initializer, origin) in pending {
            self.require_value(values, owner, initializer, origin)?;
        }
        Ok(())
    }
    fn cycle(&self, path: &[AccessCallableId], origin: SyntaxOrigin) -> Result<Failure, Failure> {
        let root = *path.first().ok_or(INVALID)?;
        let names = path
            .iter()
            .map(|&id| {
                self.context
                    .program()
                    .callables()
                    .get(id)
                    .and_then(|d| self.context.program().symbols().spelling(d.name()))
                    .ok_or(INVALID)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let primary =
            value_access::source_origin(self.context.syntax(), origin).map_err(|failure| {
                Failure::Body {
                    callable: root,
                    failure,
                }
            })?;
        let (code, message) =
            Rule::DependencyCycle
                .diagnostic()
                .map_err(|failure| Failure::Body {
                    callable: root,
                    failure,
                })?;
        Ok(Failure::Evaluation {
            callable: root,
            diagnostic: Box::new(super::Diagnostic {
                rule: Rule::DependencyCycle,
                source: SourceDiagnostic::new(
                    code,
                    format!("{message}: {}", names.join(" -> ")),
                    primary,
                    [],
                    None::<&str>,
                ),
            }),
        })
    }
    fn graph(
        &mut self,
        values: &mut Values,
        root: AccessCallableId,
    ) -> Result<BTreeMap<AccessCallableId, Query>, Failure> {
        let mut selected = BTreeSet::new();
        let mut pending = vec![(root, Vec::new())];
        while let Some((id, through)) = pending.pop() {
            if !selected.insert(id) {
                continue;
            }
            if let Err(mut failure) = self.body(values, id) {
                for origin in through.into_iter().rev() {
                    super::add_dependency_note(&mut failure, self.context, origin).map_err(
                        |failure| Failure::Body {
                            callable: id,
                            failure,
                        },
                    )?;
                }
                return Err(failure);
            }
            // All authored direct callees participate in ordinary closure, including dead source.
            // Const-domain rejection follows ordinary safety; runtime callees are never executed.
            let query = self.queries.get(&id).ok_or(INVALID)?;
            let bound = query.bind(self.context).map_err(|failure| Failure::Body {
                callable: id,
                failure,
            })?;
            for (expression_id, expression) in query.body().expressions().iter() {
                if let ExpressionKind::Call {
                    target: ValueFunctionOrigin::Declaration(target),
                    ..
                } = expression.kind
                {
                    let mut path = through.clone();
                    path.push(bound.expression_origin(expression_id).ok_or(INVALID)?);
                    pending.push((target, path));
                }
            }
        }
        selected
            .into_iter()
            .map(|id| Ok((id, self.queries.get(&id).ok_or(INVALID)?.clone())))
            .collect()
    }
    fn initialize(
        &mut self,
        values: &mut Values,
        id: AccessCallableId,
    ) -> Result<Completed, Failure> {
        let declaration = self.context.program().callables().get(id).ok_or(INVALID)?;
        if !declaration.kind().is_initializer() {
            return Err(INVALID);
        }
        let queries = self.graph(values, id)?;
        // This is the ordinary acceptance routine over the selected const closure, not another
        // source checker or evaluator. Only established frozen data crosses into final Unit typing.
        let checked = accepted::check_queries(self.context, queries)?;
        if !super::super::constant::supports_final(&checked.program, declaration.result())
            .map_err(Failure::Integrity)?
        {
            let diagnostic =
                final_diagnostic(self.context, id).map_err(|failure| Failure::Body {
                    callable: id,
                    failure,
                })?;
            return Err(Failure::Evaluation {
                callable: id,
                diagnostic: Box::new(diagnostic),
            });
        }
        let program = projection::prepare(&checked.program, &checked.bodies, id, |id| {
            values.completed(&id).map(|value| value.execution.clone())
        })
        .map_err(Failure::Integrity)?;
        let mut executor =
            state::Executor::new(&program, &mut self.budget, values.active_depth() - 1);
        let value =
            match executor.evaluate_initializer(id) {
                Ok(value) => value,
                Err(error)
                    if matches!(
                        error.rule,
                        state::Rule::InvalidPlan | state::Rule::ReachedUnreachable
                    ) =>
                {
                    return Err(INVALID);
                }
                Err(error) => {
                    let diagnostic = super::project(self.context, &checked.bodies, &error)
                        .map_err(|failure| Failure::Body {
                            callable: id,
                            failure,
                        })?;
                    return Err(Failure::Evaluation {
                        callable: id,
                        diagnostic: Box::new(diagnostic),
                    });
                }
            };
        Ok(Completed {
            frozen: value.freeze(),
            execution: value,
        })
    }
}
impl DependencyComputation<AccessCallableId, Completed, Failure> for Preparation<'_, '_> {
    fn compute(
        &mut self,
        values: &mut Values,
        id: AccessCallableId,
    ) -> Result<Completed, QueryError> {
        if values.active_depth() > self.budget.limits().call_depth().get() as usize {
            let source = self
                .context
                .callable_source(id)
                .ok_or(QueryError::Computation(INVALID))?;
            return Err(QueryError::Computation(super::failure_at(
                self.context,
                id,
                Rule::Execution(state::Rule::CallDepthLimit),
                SyntaxOrigin::Node(source.body()),
            )));
        }
        let result = self.initialize(values, id);
        result.map_err(QueryError::Computation)
    }
}
