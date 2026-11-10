//! Header-only query and exact-current binding for the future private-function grammar.
mod definitions;
mod headers;
mod projection;
mod source;
use source::RecordSource;
pub use source::{BodySource, BodySources, HeaderSource, SourceMismatch};
mod type_syntax;
pub use type_syntax::{DataTypeSyntax, FunctionTypeSyntax, ValueTypeSyntax, value_type_syntax};
#[cfg(test)]
mod tests;

use std::{collections::BTreeMap, sync::Arc};

use headers::RecordHeader;
use nocter_declarations::value_access::{
    DeclarationId, HeaderFailure, HeaderPublication, HeaderStep, Program, Reservations,
};
use nocter_dependency_query::{
    DependencyProgress, DependencyQuery, DependencyQueryError, DependencyStep,
};
use nocter_diagnostics::{SourceDiagnostic, syntax_diagnostics};
use nocter_model::{AccessCallableId, NominalTypeId};
use nocter_syntax::{
    BoundSyntax, DeclarationSyntaxSurface, NodeId, NodeKind, SyntaxToken,
    project_declaration_syntax,
};

pub use projection::HeaderDiagnostic;

#[derive(Debug)]
pub enum Failure {
    WrongParseGoal,
    Syntax(Box<[SourceDiagnostic]>),
    Headers(Vec<HeaderDiagnostic>),
    InconsistentSyntax,
    ChangedHeader,
}

/// Closed reusable declaration query; no body syntax or current source identity is retained.
#[derive(Clone, Debug)]
pub struct Query {
    program: Arc<Program>,
    surface: DeclarationSyntaxSurface,
    source_recipes: source::Recipes,
}

/// One current source/semantic pairing. Callers cannot assemble its parts independently.
#[derive(Clone, Debug)]
pub struct Context<'source> {
    program: Arc<Program>,
    syntax: BoundSyntax<'source>,
    sources: source::Sources,
}

#[derive(Clone, Debug)]
pub struct CallableSource {
    name: SyntaxToken,
    parameters: Box<[SyntaxToken]>,
    parameter_types: Box<[NodeId]>,
    result: NodeId,
    body: NodeId,
}

impl CallableSource {
    #[must_use]
    pub const fn name(&self) -> SyntaxToken {
        self.name
    }
    #[must_use]
    pub const fn parameters(&self) -> &[SyntaxToken] {
        &self.parameters
    }
    #[must_use]
    pub const fn result(&self) -> NodeId {
        self.result
    }
    #[must_use]
    pub const fn body(&self) -> NodeId {
        self.body
    }
}

/// Exact-current construction with progressively published callable and record contracts.
#[derive(Debug)]
pub struct Preparation<'source> {
    headers: HeaderPublication,
    records: BTreeMap<NominalTypeId, RecordHeader>,
    functions: BTreeMap<AccessCallableId, definitions::Recipe>,
    syntax: BoundSyntax<'source>,
    surface: DeclarationSyntaxSurface,
    source_recipes: source::Recipes,
    sources: source::Sources,
}
impl<'source> Preparation<'source> {
    /// Closes names/slots and retains raw type recipes, without resolving types or checking bodies.
    /// # Errors
    /// Rejects a wrong goal, syntax/header-shape errors, name conflicts or broken syntax integrity.
    pub fn prepare(syntax: BoundSyntax<'source>) -> Result<Self, Failure> {
        validate_input(syntax)?;
        let tree = syntax.tree();
        let projection = project_declaration_syntax(syntax);
        let (functions, records) = headers::collect(syntax)?;
        let symbols = headers::symbols(syntax, &functions, &records);
        let symbol = |token| {
            symbols
                .get(text(syntax, token))
                .ok_or(Failure::InconsistentSyntax)
        };
        let mut reservations = Reservations::new(symbols.clone());
        let record_headers = records
            .into_iter()
            .map(|h| {
                Ok((
                    reservations.reserve_record(symbol(h.source.name)?, h.fields.len()),
                    h,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, Failure>>()?;
        let function_headers = functions
            .into_iter()
            .map(|h| Ok((reservations.reserve(symbol(h.name)?, h.parameters.len()), h)))
            .collect::<Result<BTreeMap<_, _>, Failure>>()?;
        let sources: BTreeMap<_, _> = function_headers
            .iter()
            .map(|(&id, h)| {
                (
                    id,
                    CallableSource {
                        name: h.name,
                        parameters: h.parameters.iter().map(|p| p.name).collect(),
                        parameter_types: h.parameters.iter().map(|p| p.ty).collect(),
                        result: h.result_syntax,
                        body: h.body,
                    },
                )
            })
            .collect();
        let record_sources: BTreeMap<_, _> = record_headers
            .iter()
            .map(|(&id, header)| (id, header.source.clone()))
            .collect();
        let sources = source::Sources {
            callables: Arc::new(sources),
            records: Arc::new(record_sources),
        };
        let builder = reservations.finish().map_err(|violations| {
            projection::project_violations(tree, &sources.callables, &sources.records, violations)
        })?;
        let headers = builder.into_publication();
        let source_recipes = source::Recipes::capture(&projection, &sources)?;
        Ok(Self {
            headers,
            records: record_headers,
            functions: function_headers
                .into_iter()
                .map(|(id, header)| (id, header.into()))
                .collect(),
            syntax,
            surface: projection.into_surface(),
            source_recipes,
            sources,
        })
    }

    #[must_use]
    pub fn callable_ids(&self) -> impl ExactSizeIterator<Item = AccessCallableId> + '_ {
        self.headers.callable_ids()
    }
    /// All reserved contracts, including unused records, in canonical namespace order.
    #[must_use]
    pub fn declaration_ids(&self) -> impl ExactSizeIterator<Item = DeclarationId> + '_ {
        self.headers.declaration_ids()
    }
    /// Publish one required declaration and its records, retaining every established contract.
    /// # Errors
    /// Reports rejected prerequisites or a declaration outside the reserved source domain.
    pub fn context(
        &mut self,
        required: impl Into<DeclarationId>,
    ) -> Result<Context<'source>, Failure> {
        self.publish(required.into())?;
        Ok(self.current())
    }
    /// Advance one requested header without scheduling its required record headers.
    /// # Errors
    /// Projects raw type/contract failures, or rejects an identity outside this source domain.
    pub fn advance(&mut self, required: impl Into<DeclarationId>) -> Result<HeaderStep, Failure> {
        let required = required.into();
        if !self
            .headers
            .is_defined(required)
            .ok_or(Failure::InconsistentSyntax)?
        {
            let violations = definitions::define(
                &mut self.headers,
                self.syntax,
                required,
                &self.functions,
                &self.sources.callables,
                &self.records,
            )?;
            if !violations.is_empty() {
                return Err(projection::project_violations(
                    self.syntax.tree(),
                    &self.sources.callables,
                    &self.sources.records,
                    violations,
                ));
            }
        }
        self.headers
            .advance(required)
            .map_err(|failure| self.project_failure(failure))
    }
    fn publish(&mut self, required: DeclarationId) -> Result<(), Failure> {
        let mut requests = DependencyQuery::default();
        match requests.resolve_steps(self, required) {
            Ok(_) => Ok(()),
            Err(DependencyQueryError::Computation(failure)) => Err(failure),
            Err(DependencyQueryError::Cycle(path)) => Err(self.reject_cycle(&path)),
        }
    }
    /// Share only the contracts already established by this construction.
    #[must_use]
    pub fn current(&self) -> Context<'source> {
        Context {
            program: Arc::new(self.headers.snapshot()),
            syntax: self.syntax,
            sources: self.sources.clone(),
        }
    }
    /// Project an exact active header cycle through the declaration owner's validation rule.
    #[must_use]
    pub fn reject_cycle(&self, path: &[DeclarationId]) -> Failure {
        self.project_failure(self.headers.reject_cycle(path))
    }
    fn project_failure(&self, failure: HeaderFailure) -> Failure {
        project_failure(
            self.syntax,
            &self.sources.callables,
            &self.sources.records,
            failure,
        )
    }
    /// Finish the complete reusable header query without re-normalizing published contracts.
    /// # Errors
    /// Reports invalid remaining contracts at their declaration-owned source positions.
    pub fn finish(mut self) -> Result<Query, Failure> {
        let ids: Vec<_> = self.declaration_ids().collect();
        for id in ids {
            self.publish(id)?;
        }
        let program = self.headers.finish().map_err(|failure| {
            project_failure(
                self.syntax,
                &self.sources.callables,
                &self.sources.records,
                failure,
            )
        })?;
        Ok(Query {
            program: Arc::new(program),
            surface: self.surface,
            source_recipes: self.source_recipes,
        })
    }
}

impl DependencyProgress<DeclarationId, (), Failure> for Preparation<'_> {
    fn advance(
        &mut self,
        _: &DependencyQuery<DeclarationId, ()>,
        id: DeclarationId,
    ) -> Result<DependencyStep<DeclarationId, ()>, Failure> {
        Ok(match self.advance(id)? {
            HeaderStep::Ready => DependencyStep::Complete(()),
            HeaderStep::Require(dependencies) => DependencyStep::Require(dependencies),
        })
    }
    fn no_progress(&mut self, _: DeclarationId) -> Failure {
        Failure::InconsistentSyntax
    }
}

fn project_failure(
    syntax: BoundSyntax<'_>,
    sources: &BTreeMap<AccessCallableId, CallableSource>,
    records: &BTreeMap<NominalTypeId, Arc<RecordSource>>,
    failure: HeaderFailure,
) -> Failure {
    match failure {
        HeaderFailure::Rejected(violations) => {
            projection::project_violations(syntax.tree(), sources, records, violations)
        }
        HeaderFailure::UnknownId | HeaderFailure::Inconsistent => Failure::InconsistentSyntax,
    }
}

impl Query {
    /// Computes complete private headers through the same incremental publication authority.
    /// # Errors
    /// Rejects a wrong goal, syntax errors, header violations, or broken syntax integrity.
    pub fn prepare(syntax: BoundSyntax<'_>) -> Result<Self, Failure> {
        Preparation::prepare(syntax)?.finish()
    }
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// Rebinds this query to equal header syntax, including a different source identity/body.
    /// # Errors
    /// Rejects a changed header, wrong goal, invalid current syntax, or a locator mismatch.
    pub fn bind<'source>(&self, syntax: BoundSyntax<'source>) -> Result<Context<'source>, Failure> {
        validate_input(syntax)?;
        let projection = project_declaration_syntax(syntax);
        if projection.surface() != &self.surface {
            return Err(Failure::ChangedHeader);
        }
        let sources = self.source_recipes.bind(&projection)?;
        Ok(Context {
            program: Arc::clone(&self.program),
            syntax,
            sources,
        })
    }
}

impl<'source> Context<'source> {
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }
    /// Shares this exact immutable declaration authority with a semantic query.
    #[must_use]
    pub fn program_handle(&self) -> Arc<Program> {
        Arc::clone(&self.program)
    }
    #[must_use]
    pub const fn syntax(&self) -> BoundSyntax<'source> {
        self.syntax
    }
    #[must_use]
    pub fn callable_source(&self, callable: AccessCallableId) -> Option<&CallableSource> {
        self.sources.callables.get(&callable)
    }
}

fn validate_input(syntax: BoundSyntax<'_>) -> Result<(), Failure> {
    if syntax.tree().root().kind() != NodeKind::V2CoreUnit {
        return Err(Failure::WrongParseGoal);
    }
    if syntax.tree().has_errors() {
        return Err(Failure::Syntax(syntax_diagnostics(std::slice::from_ref(
            syntax.tree(),
        ))));
    }
    Ok(())
}

fn text(syntax: BoundSyntax<'_>, token: SyntaxToken) -> &str {
    syntax.text_at(token.range()).expect("bound token text")
}

/// Decode the operation node of one complete future parameter, including an annotation input.
/// # Errors
/// Rejects syntax inconsistent with the private goal; this does not resolve names or infer modes.
pub fn input_operation(
    syntax: nocter_syntax::BoundSyntax<'_>,
    parameter: nocter_syntax::NodeId,
) -> Result<nocter_model::ValueInputOperation, Failure> {
    use nocter_model::ValueInputOperation as Operation;
    use nocter_syntax::{ContextualSpelling, NodeKind, direct_node, first_direct_token};
    let Some(operation) = direct_node(syntax.tree(), parameter, NodeKind::InputOperation) else {
        return Ok(Operation::Read);
    };
    let token = first_direct_token(syntax.tree(), operation).ok_or(Failure::InconsistentSyntax)?;
    match ContextualSpelling::from_spelling(text(syntax, token)) {
        Some(ContextualSpelling::Edit) => Ok(Operation::Edit),
        Some(ContextualSpelling::Take) => Ok(Operation::Take),
        _ => Err(Failure::InconsistentSyntax),
    }
}
