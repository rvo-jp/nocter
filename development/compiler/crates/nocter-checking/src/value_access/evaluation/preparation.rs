//! One value query coordinates unpublished ordinary body drafts and executable const closures.
use super::{
    Completed, Output, Rule,
    dependencies::{self, Dependencies, Fact, Request},
    final_diagnostic, projection,
};
use crate::value_access::staged::SourceFailure;
use crate::value_access::{self, ExpressionKind, Failure as BodyFailure, Query, accepted, checker};
use nocter_constant_evaluation::state;
use nocter_declaration_lowering::access_declarations::{self as declarations, Context};
use nocter_declarations::value_access::{DeclarationId, HeaderStep};
use nocter_dependency_query::{
    DependencyComputation, DependencyProgress, DependencyQueryError, DependencyStep,
};
use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{AccessCallableId, CompileTimeGuarantee, ValueFunctionOrigin};
use nocter_syntax::SyntaxOrigin;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
#[cfg(test)]
mod tests;
type Failure = accepted::Failure;
type QueryError = DependencyQueryError<Request, SourceFailure>;
const INVALID: Failure =
    Failure::Integrity(value_access::flow::IntegrityError::InconsistentTypedBody);
enum Headers<'a, 'source> {
    Closed,
    Publishing(&'a mut declarations::Preparation<'source>),
}
struct Preparation<'a, 'source> {
    context: Context<'source>,
    headers: Headers<'a, 'source>,
    budget: state::Budget,
    sources: declarations::BodySources,
}
pub(in crate::value_access) fn prepare(
    context: &Context<'_>,
    limits: state::Limits,
) -> Result<Output, Failure> {
    let ids: Vec<_> = context
        .program()
        .callables()
        .iter()
        .map(|(id, _)| id)
        .collect();
    let mut preparation = Preparation::new(context.clone(), Headers::Closed, limits);
    let mut requests = Dependencies::default();
    preparation
        .all(&mut requests, &ids)
        .map_err(|failure| match failure {
            SourceFailure::Checking(failure) => failure,
            SourceFailure::Declarations(_) => INVALID,
        })?;
    preparation.finish(requests)
}

pub(in crate::value_access) fn prepare_source(
    mut headers: declarations::Preparation<'_>,
    limits: state::Limits,
) -> Result<(declarations::Query, Output), SourceFailure> {
    let ids: Vec<_> = headers.callable_ids().collect();
    let contracts: Vec<_> = headers.declaration_ids().collect();
    let mut preparation =
        Preparation::new(headers.current(), Headers::Publishing(&mut headers), limits);
    let mut requests = Dependencies::default();
    preparation.all(&mut requests, &ids)?;
    for declaration in contracts {
        preparation.require_header(&mut requests, declaration)?;
    }
    let output = preparation.finish(requests)?;
    Ok((headers.finish()?, output))
}
impl<'a, 'source> Preparation<'a, 'source> {
    fn new(
        context: Context<'source>,
        headers: Headers<'a, 'source>,
        limits: state::Limits,
    ) -> Self {
        Self {
            context,
            headers,
            budget: state::Budget::new(limits),
            sources: declarations::BodySources::default(),
        }
    }
    fn all(
        &mut self,
        requests: &mut Dependencies,
        ids: &[AccessCallableId],
    ) -> Result<(), SourceFailure> {
        for &id in ids {
            self.require_header(requests, id.into())?;
            if self
                .context
                .program()
                .callables()
                .get(id)
                .ok_or(INVALID)?
                .kind()
                .is_initializer()
            {
                self.budget.restart_work();
                let fact = requests
                    .resolve(self, Request::Value(id))
                    .map_err(|failure| match failure {
                        QueryError::Computation(failure) => failure,
                        QueryError::Cycle(_) => INVALID.into(),
                    })?;
                if !matches!(&*fact, Fact::Value(_)) {
                    return Err(INVALID.into());
                }
            } else {
                self.body(requests, id).map_err(|error| match error {
                    QueryError::Computation(failure) => failure,
                    QueryError::Cycle(_) => INVALID.into(),
                })?;
            }
        }
        Ok(())
    }
    fn finish(self, requests: Dependencies) -> Result<Output, Failure> {
        let mut queries = BTreeMap::new();
        let mut completed = BTreeMap::new();
        for (key, fact) in requests.into_completed() {
            match (key, Arc::unwrap_or_clone(fact)) {
                (Request::Header(_), Fact::Header) => {}
                (Request::Body(id), Fact::Body(query)) => {
                    queries.insert(id, *query);
                }
                (Request::Value(id), Fact::Value(value)) => {
                    completed.insert(id, Arc::new(value.frozen));
                }
                _ => return Err(INVALID),
            }
        }
        Ok(Output {
            queries,
            values: completed,
            dependencies: Arc::new(self.sources),
        })
    }
    fn require_header(
        &mut self,
        requests: &mut Dependencies,
        declaration: DeclarationId,
    ) -> Result<(), SourceFailure> {
        match requests.resolve_steps(self, Request::Header(declaration)) {
            Ok(fact) if matches!(&*fact, Fact::Header) => Ok(()),
            Err(QueryError::Computation(failure)) => Err(failure),
            Err(QueryError::Cycle(path)) => {
                let headers = path
                    .iter()
                    .map(|request| match request {
                        Request::Header(id) => Some(*id),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or(INVALID)?;
                match &self.headers {
                    Headers::Publishing(owner) => Err(owner.reject_cycle(&headers).into()),
                    Headers::Closed => Err(INVALID.into()),
                }
            }
            _ => Err(INVALID.into()),
        }
    }
    fn body(
        &mut self,
        requests: &mut Dependencies,
        id: AccessCallableId,
    ) -> Result<(), QueryError> {
        let fact = requests.resolve(self, Request::Body(id))?;
        if matches!(&*fact, Fact::Body(_)) {
            Ok(())
        } else {
            Err(QueryError::Computation(INVALID.into()))
        }
    }
    fn check_body(
        &mut self,
        requests: &mut Dependencies,
        id: AccessCallableId,
    ) -> Result<Query, SourceFailure> {
        self.require_header(requests, id.into())?;
        loop {
            match checker::prepare(&self.context, id, requests) {
                Ok(query) => {
                    self.require_statics(requests, &query)?;
                    if query
                        .program()
                        .callables()
                        .get(id)
                        .ok_or(INVALID)?
                        .compile_time()
                        == CompileTimeGuarantee::Evaluatable
                    {
                        self.sources.insert(&query.source).map_err(|_| INVALID)?;
                    }
                    return Ok(query);
                }
                Err(BodyFailure::RequiredHeader { declaration, .. }) => {
                    self.require_header(requests, declaration)?;
                }
                Err(BodyFailure::RequiredConstant {
                    initializer,
                    origin,
                }) => self.require_value(requests, id, initializer, origin)?,
                Err(failure) => {
                    return Err(Failure::Body {
                        callable: id,
                        failure,
                    }
                    .into());
                }
            }
        }
    }
    fn require_value(
        &mut self,
        requests: &mut Dependencies,
        owner: AccessCallableId,
        initializer: AccessCallableId,
        origin: SyntaxOrigin,
    ) -> Result<(), SourceFailure> {
        if dependencies::value(requests, initializer).is_some() {
            return Ok(());
        }
        if dependencies::value_depth(requests) == 0 {
            self.budget.restart_work();
        }
        self.budget.step().map_err(|rule| {
            super::failure_at(&self.context, initializer, Rule::Execution(rule), origin)
        })?;
        let error = match requests.resolve(self, Request::Value(initializer)) {
            Ok(fact) if matches!(&*fact, Fact::Value(_)) => return Ok(()),
            Ok(_) => return Err(INVALID.into()),
            Err(error) => error,
        };
        let mut failure = match error {
            QueryError::Computation(failure) => failure,
            QueryError::Cycle(path) => self.cycle(&path, origin)?.into(),
        };
        add_dependency_note(&mut failure, &self.context, origin).map_err(|failure| {
            Failure::Body {
                callable: owner,
                failure,
            }
        })?;
        Err(failure)
    }
    fn require_statics(
        &mut self,
        requests: &mut Dependencies,
        query: &Query,
    ) -> Result<(), SourceFailure> {
        // A body becomes complete only after its retained static requirements. An active
        // value/body cycle cannot hide behind an already-published typed-body fact.
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
        collect(query, &self.context, &mut pending)?;
        for (initializer, origin) in pending {
            self.require_value(requests, query.source_owner(), initializer, origin)?;
        }
        Ok(())
    }
    fn cycle(&self, path: &[Request], origin: SyntaxOrigin) -> Result<Failure, Failure> {
        let code = |request| match request {
            Request::Body(id)
            | Request::Value(id)
            | Request::Header(DeclarationId::Callable(id)) => Ok(id),
            Request::Header(DeclarationId::Record(_)) => Err(INVALID),
        };
        let root = code(*path.first().ok_or(INVALID)?)?;
        let names = path
            .iter()
            .map(|&request| {
                let id = code(request)?;
                let name = self
                    .context
                    .program()
                    .callables()
                    .get(id)
                    .and_then(|d| self.context.program().symbols().spelling(d.name()))
                    .ok_or(INVALID)?;
                let role = match request {
                    Request::Header(_) => "header",
                    Request::Body(_) => "body",
                    Request::Value(_) => "value",
                };
                Ok(format!("{role} {name}"))
            })
            .collect::<Result<Vec<_>, Failure>>()?;
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
        requests: &mut Dependencies,
        root: AccessCallableId,
    ) -> Result<BTreeMap<AccessCallableId, Query>, QueryError> {
        let mut selected = BTreeSet::new();
        let mut pending = vec![(root, Vec::new())];
        while let Some((id, through)) = pending.pop() {
            if !selected.insert(id) {
                continue;
            }
            if let Err(error) = self.body(requests, id) {
                let QueryError::Computation(mut failure) = error else {
                    return Err(error);
                };
                for origin in through.into_iter().rev() {
                    add_dependency_note(&mut failure, &self.context, origin).map_err(
                        |failure| {
                            QueryError::Computation(
                                Failure::Body {
                                    callable: id,
                                    failure,
                                }
                                .into(),
                            )
                        },
                    )?;
                }
                return Err(QueryError::Computation(failure));
            }
            // All authored direct callees participate in ordinary closure, including dead source.
            // Const-domain rejection follows ordinary safety; runtime callees are never executed.
            let query =
                dependencies::body(requests, id).ok_or(QueryError::Computation(INVALID.into()))?;
            let bound = query.bind(&self.context).map_err(|failure| {
                QueryError::Computation(
                    Failure::Body {
                        callable: id,
                        failure,
                    }
                    .into(),
                )
            })?;
            for (expression_id, expression) in query.body().expressions().iter() {
                if let ExpressionKind::Call {
                    target: ValueFunctionOrigin::Declaration(target),
                    ..
                } = expression.kind
                {
                    let mut path = through.clone();
                    path.push(
                        bound
                            .expression_origin(expression_id)
                            .ok_or(QueryError::Computation(INVALID.into()))?,
                    );
                    pending.push((target, path));
                }
            }
        }
        selected
            .into_iter()
            .map(|id| {
                Ok((
                    id,
                    dependencies::body(requests, id)
                        .ok_or(QueryError::Computation(INVALID.into()))?
                        .clone(),
                ))
            })
            .collect()
    }
    fn initialize(
        &mut self,
        requests: &mut Dependencies,
        id: AccessCallableId,
    ) -> Result<Completed, QueryError> {
        self.require_header(requests, id.into())
            .map_err(QueryError::Computation)?;
        let declaration = self
            .context
            .program()
            .callables()
            .get(id)
            .ok_or(QueryError::Computation(INVALID.into()))?;
        if !declaration.kind().is_initializer() {
            return Err(QueryError::Computation(INVALID.into()));
        }
        let result_data = declaration.result();
        let queries = self.graph(requests, id)?;
        self.execute(requests, id, result_data, queries)
            .map_err(|failure| QueryError::Computation(failure.into()))
    }
    fn execute(
        &mut self,
        requests: &Dependencies,
        id: AccessCallableId,
        result_data: nocter_model::TypeId,
        queries: BTreeMap<AccessCallableId, Query>,
    ) -> Result<Completed, Failure> {
        // This is the ordinary acceptance routine over the selected const closure, not another
        // source checker or evaluator. Only established frozen data crosses into final Unit typing.
        let checked = accepted::check_queries(&self.context, queries)?;
        if !super::super::constant::supports_final(&checked.program, result_data)
            .map_err(Failure::Integrity)?
        {
            let diagnostic =
                final_diagnostic(&self.context, id).map_err(|failure| Failure::Body {
                    callable: id,
                    failure,
                })?;
            return Err(Failure::Evaluation {
                callable: id,
                diagnostic: Box::new(diagnostic),
            });
        }
        let program = projection::prepare(&checked.program, &checked.bodies, id, |id| {
            dependencies::value(requests, id).map(|value| value.execution.clone())
        })
        .map_err(Failure::Integrity)?;
        let mut executor = state::Executor::new(
            &program,
            &mut self.budget,
            dependencies::value_depth(requests) - 1,
        );
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
                    let diagnostic = super::project(&self.context, &checked.bodies, &error)
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
impl DependencyComputation<Request, Fact, SourceFailure> for Preparation<'_, '_> {
    fn compute(
        &mut self,
        requests: &mut Dependencies,
        request: Request,
    ) -> Result<Fact, QueryError> {
        match request {
            // Header dependencies use the query's iterative progress entry exclusively.
            Request::Header(_) => Err(QueryError::Computation(INVALID.into())),
            Request::Body(id) => self
                .check_body(requests, id)
                .map(|query| Fact::Body(Box::new(query)))
                .map_err(QueryError::Computation),
            Request::Value(id) => {
                if dependencies::value_depth(requests)
                    > self.budget.limits().call_depth().get() as usize
                {
                    let source = self
                        .context
                        .callable_source(id)
                        .ok_or(QueryError::Computation(INVALID.into()))?;
                    return Err(QueryError::Computation(
                        super::failure_at(
                            &self.context,
                            id,
                            Rule::Execution(state::Rule::CallDepthLimit),
                            SyntaxOrigin::Node(source.body()),
                        )
                        .into(),
                    ));
                }
                self.initialize(requests, id).map(Fact::Value)
            }
        }
    }
}
impl DependencyProgress<Request, Fact, SourceFailure> for Preparation<'_, '_> {
    fn advance(
        &mut self,
        _: &Dependencies,
        request: Request,
    ) -> Result<DependencyStep<Request, Fact>, SourceFailure> {
        let Request::Header(declaration) = request else {
            return Err(INVALID.into());
        };
        if self.context.program().header_ready(declaration) {
            return Ok(DependencyStep::Complete(Fact::Header));
        }
        let Headers::Publishing(headers) = &mut self.headers else {
            return Err(INVALID.into());
        };
        match headers.advance(declaration)? {
            HeaderStep::Ready => {
                self.context = headers.current();
                Ok(DependencyStep::Complete(Fact::Header))
            }
            HeaderStep::Require(dependencies) => Ok(DependencyStep::Require(
                dependencies
                    .into_vec()
                    .into_iter()
                    .map(Request::Header)
                    .collect(),
            )),
        }
    }
    fn no_progress(&mut self, _: Request) -> SourceFailure {
        INVALID.into()
    }
}

fn add_dependency_note(
    failure: &mut SourceFailure,
    context: &Context<'_>,
    origin: SyntaxOrigin,
) -> Result<(), BodyFailure> {
    if let SourceFailure::Checking(failure) = failure {
        super::add_dependency_note(failure, context, origin)?;
    }
    Ok(())
}
