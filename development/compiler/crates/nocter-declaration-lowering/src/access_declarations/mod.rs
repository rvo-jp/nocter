//! Header-only query and exact-current binding for the future private-function grammar.
mod definitions;
mod headers;
mod projection;
mod type_syntax;
pub use type_syntax::{DataTypeSyntax, FunctionTypeSyntax, ValueTypeSyntax, value_type_syntax};
#[cfg(test)]
mod tests;

use std::{collections::BTreeMap, sync::Arc};

use headers::RecordHeader;
use nocter_declarations::value_access::{DeclarationId, HeaderPublication, Program, Reservations};
use nocter_diagnostics::{SourceDiagnostic, syntax_diagnostics};
use nocter_model::AccessCallableId;
use nocter_syntax::{
    BoundSyntax, DeclarationSyntaxLocator, DeclarationSyntaxSurface, NodeId, NodeKind,
    SyntaxOrigin, SyntaxToken, project_declaration_syntax,
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

/// Source-neutral recipe. Its locators have meaning only with an equal declaration surface.
#[derive(Clone, Debug)]
struct HeaderRecipe {
    name: DeclarationSyntaxLocator,
    parameters: Box<[DeclarationSyntaxLocator]>,
    parameter_types: Box<[DeclarationSyntaxLocator]>,
    result: DeclarationSyntaxLocator,
    body: DeclarationSyntaxLocator,
}

/// Closed reusable declaration query; no body syntax or current source identity is retained.
#[derive(Clone, Debug)]
pub struct Query {
    program: Arc<Program>,
    surface: DeclarationSyntaxSurface,
    recipes: BTreeMap<AccessCallableId, HeaderRecipe>,
}

/// One current source/semantic pairing. Callers cannot assemble its parts independently.
#[derive(Debug)]
pub struct Context<'source> {
    program: Arc<Program>,
    syntax: BoundSyntax<'source>,
    sources: Arc<BTreeMap<AccessCallableId, CallableSource>>,
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
    syntax: BoundSyntax<'source>,
    surface: DeclarationSyntaxSurface,
    recipes: BTreeMap<AccessCallableId, HeaderRecipe>,
    sources: Arc<BTreeMap<AccessCallableId, CallableSource>>,
}
impl<'source> Preparation<'source> {
    /// Computes header semantics for one private function unit, without checking its bodies.
    /// # Errors
    /// Rejects a wrong goal, syntax errors, header violations, or broken syntax integrity.
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
                    reservations.reserve_record(symbol(h.name)?, h.fields.len()),
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
        let mut builder = reservations.finish().map_err(|violations| {
            projection::project_violations(tree, &sources, &record_headers, violations)
        })?;
        let violations = definitions::define(
            &mut builder,
            syntax,
            &symbols,
            &function_headers,
            &record_headers,
        )?;
        if !violations.is_empty() {
            return Err(projection::project_violations(
                tree,
                &sources,
                &record_headers,
                violations,
            ));
        }
        let headers = builder.into_publication().map_err(|violations| {
            projection::project_violations(tree, &sources, &record_headers, violations)
        })?;
        let mut recipes = BTreeMap::new();
        for (&id, source) in &sources {
            let locate = |origin| projection.locate(origin).ok_or(Failure::InconsistentSyntax);
            recipes.insert(
                id,
                HeaderRecipe {
                    name: locate(SyntaxOrigin::Token(source.name))?,
                    parameters: source
                        .parameters
                        .iter()
                        .map(|&name| locate(SyntaxOrigin::Token(name)))
                        .collect::<Result<_, _>>()?,
                    parameter_types: source
                        .parameter_types
                        .iter()
                        .map(|&ty| locate(SyntaxOrigin::Node(ty)))
                        .collect::<Result<_, _>>()?,
                    result: locate(SyntaxOrigin::Node(source.result))?,
                    body: locate(SyntaxOrigin::Node(source.body))?,
                },
            );
        }
        Ok(Self {
            headers,
            syntax,
            surface: projection.into_surface(),
            recipes,
            sources: Arc::new(sources),
        })
    }

    #[must_use]
    pub fn callable_ids(&self) -> impl ExactSizeIterator<Item = AccessCallableId> + '_ {
        self.recipes.keys().copied()
    }
    /// Publish one required declaration and its records, retaining every established contract.
    /// # Errors
    /// Rejects a declaration outside the reserved source domain.
    pub fn context(
        &mut self,
        required: impl Into<DeclarationId>,
    ) -> Result<Context<'source>, Failure> {
        self.headers
            .publish(required)
            .map_err(|_| Failure::InconsistentSyntax)?;
        Ok(Context {
            program: Arc::new(self.headers.snapshot()),
            syntax: self.syntax,
            sources: self.sources.clone(),
        })
    }
    /// Finish the complete reusable header query without re-normalizing published contracts.
    #[must_use]
    pub fn finish(self) -> Query {
        Query {
            program: Arc::new(self.headers.finish()),
            surface: self.surface,
            recipes: self.recipes,
        }
    }
}

impl Query {
    /// Computes complete private headers through the same incremental publication authority.
    /// # Errors
    /// Rejects a wrong goal, syntax errors, header violations, or broken syntax integrity.
    pub fn prepare(syntax: BoundSyntax<'_>) -> Result<Self, Failure> {
        Ok(Preparation::prepare(syntax)?.finish())
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
        let sources = self
            .recipes
            .iter()
            .map(|(&id, recipe)| {
                let token = |locator| match projection.resolve(locator) {
                    Some(SyntaxOrigin::Token(token)) => Ok(token),
                    _ => Err(Failure::InconsistentSyntax),
                };
                let node = |locator| match projection.resolve(locator) {
                    Some(SyntaxOrigin::Node(node)) => Ok(node),
                    _ => Err(Failure::InconsistentSyntax),
                };
                Ok((
                    id,
                    CallableSource {
                        name: token(recipe.name)?,
                        parameters: recipe
                            .parameters
                            .iter()
                            .map(|&locator| token(locator))
                            .collect::<Result<_, _>>()?,
                        parameter_types: recipe
                            .parameter_types
                            .iter()
                            .map(|&locator| node(locator))
                            .collect::<Result<_, _>>()?,
                        result: node(recipe.result)?,
                        body: node(recipe.body)?,
                    },
                ))
            })
            .collect::<Result<_, Failure>>()?;
        Ok(Context {
            program: Arc::clone(&self.program),
            syntax,
            sources: Arc::new(sources),
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
        self.sources.get(&callable)
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
