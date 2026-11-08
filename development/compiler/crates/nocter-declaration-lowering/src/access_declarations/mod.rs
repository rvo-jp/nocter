//! Header-only query and exact-current binding for the future private-function grammar.
mod projection;
#[cfg(test)]
mod tests;

use std::{collections::BTreeMap, sync::Arc};

use nocter_declarations::value_access::{Builder, ParameterDefinition, Program};
use nocter_diagnostics::{SourceDiagnostic, syntax_diagnostics};
use nocter_model::{AccessCallableId, BuiltinType, SymbolTable, ValueInputOperation};
use nocter_syntax::{
    BoundSyntax, ContextualSpelling, DeclarationSyntaxLocator, DeclarationSyntaxSurface, NodeId,
    NodeKind, SyntaxOrigin, SyntaxToken, child_node_iter, declaration_name_token,
    direct_identifier, direct_node, direct_node_iter, first_direct_token,
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

#[derive(Clone, Debug)]
struct ParameterHeader {
    name: SyntaxToken,
    data: BuiltinType,
    operation: ValueInputOperation,
}

#[derive(Clone, Debug)]
struct Header {
    name: SyntaxToken,
    parameters: Vec<ParameterHeader>,
    result_syntax: NodeId,
    result_data: BuiltinType,
    body: NodeId,
}

/// Source-neutral recipe. Its locators have meaning only with an equal declaration surface.
#[derive(Clone, Debug)]
struct HeaderRecipe {
    name: DeclarationSyntaxLocator,
    parameters: Box<[DeclarationSyntaxLocator]>,
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
    sources: BTreeMap<AccessCallableId, CallableSource>,
}

#[derive(Clone, Debug)]
pub struct CallableSource {
    name: SyntaxToken,
    parameters: Box<[SyntaxToken]>,
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

impl Query {
    /// Computes header semantics for one private function unit, without checking its bodies.
    /// # Errors
    /// Rejects a wrong goal, syntax errors, header violations, or broken syntax integrity.
    pub fn prepare(syntax: BoundSyntax<'_>) -> Result<Self, Failure> {
        validate_input(syntax)?;
        let tree = syntax.tree();
        let projection = project_declaration_syntax(syntax);
        let mut headers = child_node_iter(tree, tree.root_id())
            .map(|item| {
                if tree
                    .node(item)
                    .is_none_or(|node| node.kind() != NodeKind::Item)
                {
                    return Err(Failure::InconsistentSyntax);
                }
                let children = child_node_iter(tree, item).collect::<Vec<_>>();
                let [declaration] = children.as_slice() else {
                    return Err(Failure::InconsistentSyntax);
                };
                if tree
                    .node(*declaration)
                    .is_none_or(|node| node.kind() != NodeKind::FunctionDeclaration)
                {
                    return Err(Failure::InconsistentSyntax);
                }
                header(syntax, *declaration)
            })
            .collect::<Result<Vec<_>, _>>()?;
        headers.sort_by(|left, right| text(syntax, left.name).cmp(text(syntax, right.name)));
        let symbols = SymbolTable::from_spellings(headers.iter().flat_map(|header| {
            std::iter::once(text(syntax, header.name)).chain(
                header
                    .parameters
                    .iter()
                    .map(|parameter| text(syntax, parameter.name)),
            )
        }));
        let mut builder = Builder::new(symbols.clone());
        let mut sources = BTreeMap::new();
        let mut recipes = BTreeMap::new();
        for header in headers {
            let name = symbols
                .get(text(syntax, header.name))
                .ok_or(Failure::InconsistentSyntax)?;
            let id = builder.reserve(name);
            let parameters = header
                .parameters
                .iter()
                .map(|parameter| {
                    Ok(ParameterDefinition {
                        name: symbols
                            .get(text(syntax, parameter.name))
                            .ok_or(Failure::InconsistentSyntax)?,
                        data: parameter.data,
                        operation: parameter.operation,
                    })
                })
                .collect::<Result<Vec<_>, Failure>>()?;
            builder
                .define(id, parameters, header.result_data)
                .map_err(|_| Failure::InconsistentSyntax)?;
            let source = CallableSource {
                name: header.name,
                parameters: header
                    .parameters
                    .iter()
                    .map(|parameter| parameter.name)
                    .collect(),
                result: header.result_syntax,
                body: header.body,
            };
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
                    result: locate(SyntaxOrigin::Node(source.result))?,
                    body: locate(SyntaxOrigin::Node(source.body))?,
                },
            );
            sources.insert(id, source);
        }
        let program = builder
            .finish()
            .map_err(|violations| projection::project_violations(tree, &sources, violations))?;
        Ok(Self {
            program: Arc::new(program),
            surface: projection.into_surface(),
            recipes,
        })
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
                        result: node(recipe.result)?,
                        body: node(recipe.body)?,
                    },
                ))
            })
            .collect::<Result<_, Failure>>()?;
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

fn header(syntax: BoundSyntax<'_>, declaration: NodeId) -> Result<Header, Failure> {
    let tree = syntax.tree();
    let name = declaration_name_token(tree, declaration).ok_or(Failure::InconsistentSyntax)?;
    let tail = direct_node(tree, declaration, NodeKind::CallableTail)
        .ok_or(Failure::InconsistentSyntax)?;
    let parameters_node =
        direct_node(tree, tail, NodeKind::Parameters).ok_or(Failure::InconsistentSyntax)?;
    let parameters = direct_node_iter(tree, parameters_node, NodeKind::Parameter)
        .map(|parameter| {
            let name = direct_identifier(tree, parameter).ok_or(Failure::InconsistentSyntax)?;
            let ty =
                direct_node(tree, parameter, NodeKind::Type).ok_or(Failure::InconsistentSyntax)?;
            let data = data_type(syntax, ty)?;
            let operation =
                if let Some(operation) = direct_node(tree, parameter, NodeKind::InputOperation) {
                    let token =
                        first_direct_token(tree, operation).ok_or(Failure::InconsistentSyntax)?;
                    match ContextualSpelling::from_spelling(text(syntax, token)) {
                        Some(ContextualSpelling::Edit) => ValueInputOperation::Edit,
                        Some(ContextualSpelling::Take) => ValueInputOperation::Take,
                        _ => return Err(Failure::InconsistentSyntax),
                    }
                } else {
                    ValueInputOperation::Read
                };
            Ok(ParameterHeader {
                name,
                data,
                operation,
            })
        })
        .collect::<Result<_, Failure>>()?;
    let result_syntax =
        direct_node(tree, tail, NodeKind::Type).ok_or(Failure::InconsistentSyntax)?;
    let result_data = data_type(syntax, result_syntax)?;
    let body =
        direct_node(tree, declaration, NodeKind::Block).ok_or(Failure::InconsistentSyntax)?;
    Ok(Header {
        name,
        parameters,
        result_syntax,
        result_data,
        body,
    })
}

fn data_type(syntax: BoundSyntax<'_>, ty: NodeId) -> Result<BuiltinType, Failure> {
    let tree = syntax.tree();
    let named = direct_node(tree, ty, NodeKind::NamedType).ok_or(Failure::InconsistentSyntax)?;
    let token = first_direct_token(tree, named).ok_or(Failure::InconsistentSyntax)?;
    BuiltinType::from_spelling(text(syntax, token)).ok_or(Failure::InconsistentSyntax)
}
