//! One value query coordinates unpublished ordinary body drafts and executable const closures.
use super::{
    Completed, Output, Rule,
    dependencies::{self, Dependencies, Fact, INVALID, Request},
    final_diagnostic, projection,
};
use crate::value_access::staged::SourceFailure;
use crate::value_access::{ExpressionKind, Failure as BodyFailure, Query, accepted, checker};
use declarations::HeaderProgress;
use nocter_constant_evaluation::state;
use nocter_declaration_lowering::access_declarations::{self as declarations, Context};
use nocter_declarations::value_access::DeclarationId;
use nocter_dependency_query::{DependencyProgress, DependencyQueryError, DependencyStep};
use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{AccessCodeId, ValueCode, ValueCodeRoot, ValueExpression, ValueFunctionOrigin};
use nocter_syntax::SyntaxOrigin;
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[cfg(test)]
mod tests;
type Failure = accepted::Failure;
type QueryError = DependencyQueryError<Request, SourceFailure>;
mod headers;
use headers::{Headers, ModuleHeaders};
struct Preparation<'a, 'source> {
    context: Context<'source>,
    headers: Headers<'a, 'source>,
    budget: state::Budget,
    condition_work: state::WorkBudget,
    plan_work: state::WorkBudget,
    expressions: BTreeMap<ValueExpression, crate::value_access::RequiredExpression>,
    causes: HashMap<Request, (Request, DependencyCause)>,
}
enum DependencyCause {
    Header(SyntaxOrigin),
    Value(SyntaxOrigin),
    Calls(Vec<SyntaxOrigin>),
}
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum Demand {
    Types,
    Invoke,
}
#[derive(Clone, Copy)]
struct CallOrigin {
    parent: ValueCode,
    site: SyntaxOrigin,
}
#[derive(Clone, Copy)]
struct Needed {
    demand: Demand,
    origin: Option<CallOrigin>,
}
fn call_path(
    selected: &BTreeMap<ValueCode, Needed>,
    mut edge: Option<CallOrigin>,
) -> Result<Vec<SyntaxOrigin>, Failure> {
    let mut path = Vec::new();
    while let Some(origin) = edge {
        path.push(origin.site);
        edge = selected.get(&origin.parent).ok_or(INVALID)?.origin;
    }
    path.reverse();
    Ok(path)
}
struct Ready {
    resolved: accepted::Resolved,
    invoked: BTreeSet<ValueCode>,
}
enum GraphProgress {
    Require(ValueCodeRoot, Vec<SyntaxOrigin>),
    Ready(Ready),
}
pub(in crate::value_access) fn prepare(
    context: &Context<'_>,
    limits: state::Limits,
) -> Result<Output, Failure> {
    let ids: Vec<_> = context.program().codes().iter().map(|(id, _)| id).collect();
    let mut preparation = Preparation::new(context.clone(), Headers::Closed, limits);
    let mut requests = Dependencies::default();
    preparation
        .all(&mut requests, &ids)
        .map_err(|failure| match failure {
            SourceFailure::Checking(failure) => failure,
            SourceFailure::Declarations(_) | SourceFailure::Modules(_) => INVALID,
        })?;
    preparation.finish(requests)
}

pub(in crate::value_access) fn prepare_source(
    mut headers: declarations::Preparation<'_>,
    limits: state::Limits,
) -> Result<(declarations::Query, Output), SourceFailure> {
    let ids: Vec<_> = headers.code_ids().collect();
    let contracts: Vec<_> = headers.declaration_ids().collect();
    let preparation =
        Preparation::new(headers.current(), Headers::Publishing(&mut headers), limits);
    let output = preparation.complete_source(&ids, &contracts)?;
    Ok((
        headers.finish_with_dependencies(output.header_sources.clone())?,
        output,
    ))
}
pub(in crate::value_access) fn prepare_module_source(
    mut headers: declarations::ScopedModuleSources<'_>,
    limits: state::Limits,
) -> Result<(declarations::ModuleContext<'_>, Output), SourceFailure> {
    let ids = headers.code_ids().collect::<Vec<_>>();
    let contracts = headers.declaration_ids().collect::<Vec<_>>();
    let binding = headers.source_binding();
    let context = binding.current(&headers)?;
    let mut owner = ModuleHeaders {
        owner: &mut headers,
        binding: &binding,
    };
    let preparation = Preparation::new(context, Headers::Publishing(&mut owner), limits);
    let output = preparation.complete_source(&ids, &contracts)?;
    Ok((
        headers.seal_with_dependencies(output.header_sources.clone())?,
        output,
    ))
}
impl<'a, 'source> Preparation<'a, 'source> {
    fn complete_source(
        mut self,
        ids: &[AccessCodeId],
        contracts: &[DeclarationId],
    ) -> Result<Output, SourceFailure> {
        let mut requests = Dependencies::default();
        self.all(&mut requests, ids)?;
        for &declaration in contracts {
            self.require_header(&mut requests, declaration)?;
        }
        self.finish(requests).map_err(Into::into)
    }
    fn source(
        &self,
        root: ValueCodeRoot,
    ) -> Result<std::borrow::Cow<'_, declarations::CodeSource>, BodyFailure> {
        match root {
            ValueCodeRoot::Declaration(id) => self
                .context
                .code_source(id)
                .map(std::borrow::Cow::Borrowed)
                .ok_or(BodyFailure::InconsistentSyntax),
            ValueCodeRoot::Expression(id) => self
                .expressions
                .get(&id)
                .ok_or(BodyFailure::InconsistentSyntax)?
                .source
                .bind(&self.context)
                .map_err(Into::into),
        }
    }
    fn new(
        context: Context<'source>,
        headers: Headers<'a, 'source>,
        limits: state::Limits,
    ) -> Self {
        Self {
            context,
            headers,
            budget: state::Budget::new(limits),
            condition_work: state::WorkBudget::new(limits.steps()),
            plan_work: state::WorkBudget::new(limits.steps()),
            expressions: BTreeMap::new(),
            causes: HashMap::new(),
        }
    }
    fn restart_work(&mut self) {
        self.budget.restart_work();
        self.condition_work.restart();
        self.plan_work.restart();
    }
    fn all(
        &mut self,
        requests: &mut Dependencies,
        ids: &[AccessCodeId],
    ) -> Result<(), SourceFailure> {
        for &id in ids {
            self.require_header(requests, id.into())?;
            let kind = self
                .context
                .program()
                .codes()
                .get(id)
                .ok_or(INVALID)?
                .kind();
            if kind == nocter_declarations::value_access::CodeKind::Primitive {
                if self.context.primitives().role(id).is_none() {
                    return Err(INVALID.into());
                }
            } else if kind.is_initializer() {
                self.restart_work();
                let fact = requests
                    .resolve_steps(self, Request::Value(id.into()))
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
        dependencies::finish(requests, &self.context)
    }
    fn require_header(
        &mut self,
        requests: &mut Dependencies,
        declaration: DeclarationId,
    ) -> Result<(), SourceFailure> {
        match requests.resolve_steps(self, Request::Header(declaration)) {
            Ok(fact) if matches!(&*fact, Fact::Header) => Ok(()),
            Err(QueryError::Computation(failure)) => Err(failure),
            _ => Err(INVALID.into()),
        }
    }
    fn body(
        &mut self,
        requests: &mut Dependencies,
        id: impl Into<ValueCodeRoot>,
    ) -> Result<(), QueryError> {
        let fact = requests.resolve_steps(self, Request::Body(id.into()))?;
        if matches!(&*fact, Fact::Body(_)) {
            Ok(())
        } else {
            Err(QueryError::Computation(INVALID.into()))
        }
    }
    fn advance_body(
        &mut self,
        requests: &Dependencies,
        id: ValueCodeRoot,
    ) -> Result<DependencyStep<Request, Fact>, SourceFailure> {
        if let ValueCodeRoot::Declaration(code) = id {
            let header = Request::Header(code.into());
            if requests.completed(&header).is_none() {
                return Ok(DependencyStep::Require(Box::new([header])));
            }
        }
        match self.check_draft(id, requests) {
            Ok(query) => {
                for (initializer, origin) in self.static_requirements(&query)? {
                    if dependencies::value(requests, initializer).is_none() {
                        return self.require_value_step(
                            requests,
                            Request::Body(id),
                            initializer.into(),
                            origin,
                        );
                    }
                }
                Ok(DependencyStep::Complete(Fact::Body(query)))
            }
            Err(BodyFailure::RequiredHeader {
                declaration,
                origin,
            }) => Ok(self.require_step(
                Request::Body(id),
                Request::Header(declaration),
                DependencyCause::Header(origin),
            )),
            Err(BodyFailure::RequiredConstant {
                initializer,
                origin,
            }) => self.require_value_step(requests, Request::Body(id), initializer.into(), origin),
            Err(BodyFailure::RequiredExpression(required)) => {
                let (root, origin) = self.register_expression(id.source_owner(), *required)?;
                self.require_value_step(requests, Request::Body(id), root, origin)
            }
            Err(failure) => Err(Failure::Body {
                callable: id.source_owner(),
                failure,
            }
            .into()),
        }
    }
    fn check_draft(
        &self,
        id: ValueCodeRoot,
        requests: &Dependencies,
    ) -> Result<Box<Query>, BodyFailure> {
        match id {
            ValueCodeRoot::Declaration(code) => checker::prepare(&self.context, code, requests),
            ValueCodeRoot::Expression(expression) => crate::value_access::required::prepare(
                &self.context,
                self.expressions
                    .get(&expression)
                    .ok_or(BodyFailure::InconsistentSyntax)?,
                requests,
            ),
        }
        .map(Box::new)
    }
    fn register_expression(
        &mut self,
        owner: AccessCodeId,
        required: crate::value_access::RequiredExpression,
    ) -> Result<(ValueCodeRoot, SyntaxOrigin), SourceFailure> {
        let ValueCodeRoot::Expression(key) = required.code() else {
            return Err(INVALID.into());
        };
        if key.owner != owner {
            return Err(INVALID.into());
        }
        let origin = SyntaxOrigin::Node(
            required
                .source
                .bind(&self.context)
                .map_err(|failure| Failure::Body {
                    callable: owner,
                    failure: failure.into(),
                })?
                .body()
                .ok_or(INVALID)?,
        );
        if let Some(known) = self.expressions.get(&key) {
            if known.unavailable != required.unavailable {
                return Err(INVALID.into());
            }
        } else {
            self.expressions.insert(key, required);
        }
        Ok((ValueCodeRoot::Expression(key), origin))
    }
    fn require_step(
        &mut self,
        parent: Request,
        child: Request,
        cause: DependencyCause,
    ) -> DependencyStep<Request, Fact> {
        self.causes.insert(parent, (child, cause));
        DependencyStep::Require(Box::new([child]))
    }
    fn require_value_step(
        &mut self,
        requests: &Dependencies,
        parent: Request,
        initializer: ValueCodeRoot,
        origin: SyntaxOrigin,
    ) -> Result<DependencyStep<Request, Fact>, SourceFailure> {
        if dependencies::value_depth(requests) == 0 {
            self.restart_work();
        }
        self.budget.step().map_err(|rule| {
            super::failure_at(
                &self.context,
                initializer.source_owner(),
                Rule::Execution(rule),
                origin,
            )
        })?;
        Ok(self.require_step(
            parent,
            Request::Value(initializer),
            DependencyCause::Value(origin),
        ))
    }
    fn static_requirements(
        &self,
        query: &Query,
    ) -> Result<Vec<(AccessCodeId, SyntaxOrigin)>, SourceFailure> {
        // A body becomes complete only after its retained static requirements. An active
        // value/body cycle cannot hide behind an already-published typed-body fact.
        fn collect(
            query: &Query,
            context: &Context<'_>,
            pending: &mut Vec<(AccessCodeId, SyntaxOrigin)>,
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
        Ok(pending)
    }
    fn cycle(&self, path: &[Request], origin: SyntaxOrigin) -> Result<Failure, Failure> {
        let code = |request| match request {
            Request::Body(id) | Request::Value(id) => Some(id.source_owner()),
            Request::Header(DeclarationId::Code(id)) => Some(id),
            Request::Header(DeclarationId::Record(_)) => None,
        };
        let root = code(*path.first().ok_or(INVALID)?)
            .or_else(|| {
                path.iter().find_map(|request| match request {
                    Request::Value(id) => Some(id.source_owner()),
                    _ => None,
                })
            })
            .ok_or(INVALID)?;
        let names = path
            .iter()
            .map(|&request| {
                let (name, role) = match request {
                    Request::Header(id) => (self.context.program().declaration_label(id), "header"),
                    Request::Body(id) => (self.context.program().root_label(id), "body"),
                    Request::Value(id) => (self.context.program().root_label(id), "value"),
                };
                let name = name.ok_or(INVALID)?;
                Ok(format!("{role} {name}"))
            })
            .collect::<Result<Vec<_>, Failure>>()?;
        let primary =
            super::source_origin(&self.context, origin).map_err(|failure| Failure::Body {
                callable: root,
                failure,
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
    fn requirements(
        &self,
        query: &Query,
        demand: Demand,
    ) -> Result<Vec<(ValueCode, Demand, SyntaxOrigin)>, Failure> {
        let bound = query.bind(&self.context).map_err(|failure| Failure::Body {
            callable: query.code().source_owner(),
            failure,
        })?;
        let mut dependencies = Vec::new();
        for (id, expression) in query.body().expressions().iter() {
            let target = match expression.kind {
                ExpressionKind::Closure { closure, .. } => {
                    Some((ValueCode::Closure(closure), Demand::Types))
                }
                ExpressionKind::Call {
                    target: ValueFunctionOrigin::Declaration(target),
                    ..
                } => {
                    if demand == Demand::Invoke {
                        Some((target.into(), Demand::Invoke))
                    } else if query
                        .program()
                        .codes()
                        .get(target)
                        .ok_or(INVALID)?
                        .result_contract()
                        .is_some()
                    {
                        Some((target.into(), Demand::Types))
                    } else {
                        None
                    }
                }
                ExpressionKind::Call {
                    target: ValueFunctionOrigin::Closure(target),
                    ..
                } if demand == Demand::Invoke => Some((ValueCode::Closure(target), Demand::Invoke)),
                _ => None,
            };
            if let Some((target, demand)) = target {
                dependencies.push((target, demand, bound.expression_origin(id).ok_or(INVALID)?));
            }
        }
        Ok(dependencies)
    }
    /// Discover only actual invocation dependencies after ordinary result closure. Metadata
    /// bindings do not demand the bodies of unused callback values or retype a source body.
    fn bound_requirements(
        &mut self,
        resolved: &accepted::Resolved,
        root: ValueCodeRoot,
    ) -> Result<Vec<(ValueCode, Demand, CallOrigin)>, Failure> {
        let program = resolved.program();
        let scope = resolved.scope().map_err(Failure::Integrity)?;
        let mut pending = vec![program.bind_code(root.code(), []).map_err(|_| INVALID)?];
        let independent = super::call_support::witness_independent(&scope);
        let mut plain = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut dependencies = Vec::new();
        while let Some(key) = pending.pop() {
            if !super::call_support::visit(&independent, &mut plain, &mut visited, &key) {
                continue;
            }
            let Some(query) = scope.get(&key.code()) else {
                continue;
            };
            let bound = query.bind(&self.context).map_err(|failure| Failure::Body {
                callable: query.code().source_owner(),
                failure,
            })?;
            for (id, expression) in query.body().expressions().iter() {
                if !crate::value_access::instances::is_completed_call(&expression.kind) {
                    continue;
                }
                let site = bound.expression_origin(id).ok_or(INVALID)?;
                let primary = crate::value_access::source_origin(bound.syntax(), site).map_err(
                    |failure| Failure::Body {
                        callable: key.code().source_owner(),
                        failure,
                    },
                )?;
                super::condition_step(
                    &mut self.condition_work,
                    key.code().source_owner(),
                    primary,
                )?;
                let selected =
                    crate::value_access::instances::select_call(program, &key, &expression.kind)
                        .map_err(Failure::Integrity)?
                        .ok_or(INVALID)?;
                // The caller's bound domain check reports a missing authored capability. A
                // runtime-only actual value does not create an evaluator body dependency.
                if let ValueCode::Declaration(code) = selected.code()
                    && program.codes().get(code).ok_or(INVALID)?.compile_time()
                        != nocter_model::CompileTimeGuarantee::Evaluatable
                {
                    continue;
                }
                dependencies.push((
                    selected.code(),
                    Demand::Invoke,
                    CallOrigin {
                        parent: key.code(),
                        site,
                    },
                ));
                pending.push(selected);
            }
        }
        Ok(dependencies)
    }
    fn graph(
        &mut self,
        requests: &Dependencies,
        root: ValueCodeRoot,
    ) -> Result<GraphProgress, SourceFailure> {
        let mut selected = BTreeMap::<ValueCode, Needed>::new();
        let mut pending = vec![(root.code(), Demand::Invoke, None)];
        loop {
            while let Some((code, demand, origin)) = pending.pop() {
                if selected
                    .get(&code)
                    .is_some_and(|needed| needed.demand >= demand)
                {
                    continue;
                }
                if let ValueCode::Declaration(id) = code
                    && self.context.primitives().role(id).is_some()
                {
                    // A runtime operation participates in the dependency graph without a source
                    // body. The ordinary const-domain check rejects invoking it before execution.
                    selected.insert(code, Needed { demand, origin });
                    continue;
                }
                let Some(root_query) = dependencies::body(requests, code.root()) else {
                    return Ok(GraphProgress::Require(
                        code.root(),
                        call_path(&selected, origin)?,
                    ));
                };
                let nested;
                let query = if root_query.code() == code {
                    root_query
                } else {
                    nested = super::super::flow::scope(root_query.closures.values())
                        .map_err(Failure::Integrity)?;
                    nested.get(&code).ok_or(INVALID)?.typed.as_ref()
                };
                selected.insert(code, Needed { demand, origin });
                for (target, needed, site) in self.requirements(query, demand)? {
                    pending.push((target, needed, Some(CallOrigin { parent: code, site })));
                }
            }
            let roots: BTreeSet<_> = selected
                .keys()
                .filter(|code| match code {
                    ValueCode::Declaration(id) => self.context.primitives().role(*id).is_none(),
                    _ => true,
                })
                .map(|code| code.root())
                .collect();
            let queries = roots
                .into_iter()
                .map(|id| Ok((id, dependencies::body(requests, id).ok_or(INVALID)?.clone())))
                .collect::<Result<_, Failure>>()?;
            let resolved = accepted::resolve_queries(&self.context, queries)?;
            for (code, query) in resolved.scope().map_err(Failure::Integrity)? {
                let Some(needed) = selected.get(&code) else {
                    continue;
                };
                for (target, demand, site) in self.requirements(query, needed.demand)? {
                    if selected
                        .get(&target)
                        .is_none_or(|needed| needed.demand < demand)
                    {
                        pending.push((target, demand, Some(CallOrigin { parent: code, site })));
                    }
                }
            }
            if pending.is_empty() {
                for (target, demand, origin) in self.bound_requirements(&resolved, root)? {
                    if selected
                        .get(&target)
                        .is_none_or(|needed| needed.demand < demand)
                    {
                        pending.push((target, demand, Some(origin)));
                    }
                }
            }
            if pending.is_empty() {
                let invoked = selected
                    .into_iter()
                    .filter_map(|(code, needed)| (needed.demand == Demand::Invoke).then_some(code))
                    .collect();
                return Ok(GraphProgress::Ready(Ready { resolved, invoked }));
            }
        }
    }
    fn advance_value(
        &mut self,
        requests: &Dependencies,
        id: ValueCodeRoot,
    ) -> Result<DependencyStep<Request, Fact>, SourceFailure> {
        self.check_value_depth(requests, id)?;
        let result_data = match id {
            ValueCodeRoot::Declaration(code) => {
                let header = Request::Header(code.into());
                if requests.completed(&header).is_none() {
                    return Ok(DependencyStep::Require(Box::new([header])));
                }
                let declaration = self.context.program().codes().get(code).ok_or(INVALID)?;
                if !declaration.kind().is_initializer() {
                    return Err(INVALID.into());
                }
                declaration.result()
            }
            ValueCodeRoot::Expression(expression) => {
                if !self.expressions.contains_key(&expression) {
                    return Err(INVALID.into());
                }
                self.context
                    .program()
                    .data_type(nocter_model::BuiltinType::Usize)
                    .ok_or(INVALID)?
            }
        };
        match self.graph(requests, id)? {
            GraphProgress::Require(target, through) => Ok(self.require_step(
                Request::Value(id),
                Request::Body(target),
                DependencyCause::Calls(through),
            )),
            GraphProgress::Ready(ready) => self
                .execute(requests, id, result_data, ready)
                .map(|value| DependencyStep::Complete(Fact::Value(value)))
                .map_err(Into::into),
        }
    }
    fn execute(
        &mut self,
        requests: &Dependencies,
        id: ValueCodeRoot,
        result_data: nocter_model::TypeId,
        ready: Ready,
    ) -> Result<Completed, Failure> {
        // This is the ordinary acceptance routine over the selected const closure, not another
        // source checker or evaluator. Only established frozen data crosses into final Unit typing.
        let checked =
            accepted::admit_resolved(&self.context, ready.resolved, Some(&ready.invoked))?;
        accepted::check_required_const(&self.context, &checked, id, &mut self.condition_work)?;
        if !super::super::constant::supports_final(&checked.program, result_data)
            .map_err(Failure::Integrity)?
        {
            let diagnostic = final_diagnostic(
                &self.context,
                &checked.bodies.get(&id).ok_or(INVALID)?.typed().source,
            )
            .map_err(|failure| Failure::Body {
                callable: id.source_owner(),
                failure,
            })?;
            return Err(Failure::Evaluation {
                callable: id.source_owner(),
                diagnostic: Box::new(diagnostic),
            });
        }
        let program = match projection::prepare(
            &checked,
            id,
            |id| dependencies::value(requests, id).map(|value| value.execution.clone()),
            &mut self.plan_work,
        ) {
            Ok(program) => program,
            Err(projection::PreparationError::Integrity(error)) => {
                return Err(Failure::Integrity(error));
            }
            Err(projection::PreparationError::WorkLimit) => {
                let primary = self
                    .source(id)
                    .and_then(|source| super::source_origin(&self.context, source.origin()))
                    .map_err(|failure| Failure::Body {
                        callable: id.source_owner(),
                        failure,
                    })?;
                return Err(super::failure_at_source(
                    id.source_owner(),
                    Rule::PlanWorkLimit,
                    primary,
                ));
            }
        };
        let mut executor = state::Executor::new(
            &program,
            &mut self.budget,
            dependencies::value_depth(requests) - 1,
        );
        let value = match executor.evaluate_initializer(id) {
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
                let diagnostic =
                    super::project(&self.context, &checked, &error).map_err(|failure| {
                        Failure::Body {
                            callable: id.source_owner(),
                            failure,
                        }
                    })?;
                return Err(Failure::Evaluation {
                    callable: id.source_owner(),
                    diagnostic: Box::new(diagnostic),
                });
            }
        };
        Ok(Completed {
            frozen: value.freeze(),
            execution: value,
        })
    }
    fn check_value_depth(
        &self,
        requests: &Dependencies,
        id: ValueCodeRoot,
    ) -> Result<(), SourceFailure> {
        if dependencies::value_depth(requests) <= self.budget.limits().call_depth().get() as usize {
            return Ok(());
        }
        let source = self.source(id).map_err(|failure| Failure::Body {
            callable: id.source_owner(),
            failure,
        })?;
        Err(super::failure_at(
            &self.context,
            id.source_owner(),
            Rule::Execution(state::Rule::CallDepthLimit),
            SyntaxOrigin::Node(source.body().ok_or(INVALID)?),
        )
        .into())
    }
    fn project_dependency_failure(
        &self,
        parent: Request,
        child: Request,
        cause: Option<DependencyCause>,
        error: QueryError,
    ) -> Result<QueryError, Failure> {
        let mut failure = match error {
            QueryError::Cycle(path) => {
                if let Some(headers) = path
                    .iter()
                    .map(|request| match request {
                        Request::Header(id) => Some(*id),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()
                {
                    return Ok(match &self.headers {
                        Headers::Publishing(owner) => {
                            QueryError::Computation(owner.reject_cycle(&headers))
                        }
                        Headers::Closed => QueryError::Computation(INVALID.into()),
                    });
                }
                let authored = |cause: &DependencyCause| match cause {
                    DependencyCause::Header(origin) | DependencyCause::Value(origin) => {
                        Some(*origin)
                    }
                    DependencyCause::Calls(_) => None,
                };
                // Call-closure scheduling can close a cycle at a pending body. The diagnostic
                // still belongs to its authored type/value requirement, not that graph edge.
                let origin = cause
                    .as_ref()
                    .and_then(authored)
                    .or_else(|| {
                        path.windows(2).rev().find_map(|edge| {
                            let (target, cause) = self.causes.get(&edge[0])?;
                            (*target == edge[1]).then(|| authored(cause)).flatten()
                        })
                    })
                    .or_else(|| match child {
                        Request::Value(root) => self
                            .source(root)
                            .ok()
                            .and_then(|source| source.body().map(SyntaxOrigin::Node)),
                        _ => None,
                    });
                let Some(origin) = origin else {
                    return Ok(QueryError::Cycle(path));
                };
                self.cycle(&path, origin)?.into()
            }
            QueryError::Computation(failure) => failure,
        };
        let origins = match cause {
            Some(DependencyCause::Value(origin)) => vec![origin],
            Some(DependencyCause::Calls(through)) => through,
            _ => Vec::new(),
        };
        for origin in origins.into_iter().rev() {
            add_dependency_note(&mut failure, &self.context, origin).map_err(|failure| {
                let callable = match parent {
                    Request::Body(root) | Request::Value(root) => root.source_owner(),
                    Request::Header(_) => {
                        unreachable!("only body/value requests retain dependency notes")
                    }
                };
                Failure::Body { callable, failure }
            })?;
        }
        Ok(QueryError::Computation(failure))
    }
}
impl DependencyProgress<Request, Fact, SourceFailure> for Preparation<'_, '_> {
    fn advance(
        &mut self,
        requests: &Dependencies,
        request: Request,
    ) -> Result<DependencyStep<Request, Fact>, SourceFailure> {
        self.causes.remove(&request);
        let declaration = match request {
            Request::Body(root) => return self.advance_body(requests, root),
            Request::Value(root) => return self.advance_value(requests, root),
            Request::Header(declaration) => declaration,
        };
        if self.context.program().header_ready(declaration) {
            return Ok(DependencyStep::Complete(Fact::Header));
        }
        let Headers::Publishing(headers) = &mut self.headers else {
            return Err(INVALID.into());
        };
        match headers.advance_with_values(declaration, &mut |id| {
            dependencies::value(requests, id).map(|value| &value.frozen)
        })? {
            HeaderProgress::Ready => {
                self.context = headers.current()?;
                Ok(DependencyStep::Complete(Fact::Header))
            }
            HeaderProgress::RequireHeaders(dependencies) => Ok(DependencyStep::Require(
                dependencies
                    .into_vec()
                    .into_iter()
                    .map(Request::Header)
                    .collect(),
            )),
            HeaderProgress::RequireValues(values) => {
                if dependencies::value_depth(requests) == 0 {
                    self.restart_work();
                }
                Ok(DependencyStep::Require(
                    values
                        .into_vec()
                        .into_iter()
                        .map(|id| Request::Value(id.into()))
                        .collect(),
                ))
            }
        }
    }
    fn no_progress(&mut self, _: Request) -> SourceFailure {
        INVALID.into()
    }
    fn dependency_failed(
        &mut self,
        parent: Request,
        child: Request,
        failure: QueryError,
    ) -> QueryError {
        let cause = self.causes.remove(&parent);
        if cause
            .as_ref()
            .is_some_and(|(expected, _)| *expected != child)
        {
            return QueryError::Computation(INVALID.into());
        }
        self.project_dependency_failure(parent, child, cause.map(|(_, cause)| cause), failure)
            .unwrap_or_else(|failure| QueryError::Computation(failure.into()))
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
