use nocter_declarations::DeclarationGraph;
use nocter_model::{ArenaBuilder, BodyId, BodyScopeId, CaptureId, LocalBindingId, Symbol};
use nocter_source_index::SourceOrigin;
use nocter_syntax::{BodySyntaxLocator, BodySyntaxProjection, SyntaxOrigin};

use super::Projection;
use super::model::{
    BodyScope, Capture, LocalBinding, ResolvedBindingOrigins, ResolvedBodyNames, ResolvedNameUse,
    ScopeBinding,
};
use crate::BodySource;

#[derive(Debug)]
struct ScopeRecipe {
    parent: Option<BodyScopeId>,
    bindings: Box<[(Box<str>, super::NameTarget)]>,
}

#[derive(Debug)]
struct LocalRecipe {
    name: Box<str>,
    scope: BodyScopeId,
    kind: super::LocalBindingKind,
}

#[derive(Debug)]
struct CaptureRecipe {
    name: Box<str>,
    scope: BodyScopeId,
    source: super::NameTarget,
    mode: super::CaptureMode,
}

#[derive(Debug)]
struct UseRecipe {
    origin: BodySyntaxLocator,
    target: super::NameTarget,
}

#[derive(Debug)]
struct ProjectionRecipe {
    entity: nocter_source_index::SemanticEntity,
    role: nocter_source_index::SourceRole,
    origin: BodySyntaxLocator,
    documentation: Option<Box<str>>,
}

/// Source-neutral lexical result for one body under an exact declaration authority.
#[derive(Debug)]
pub struct ReusableBodyNames {
    body: BodyId,
    scopes: Box<[ScopeRecipe]>,
    locals: Box<[LocalRecipe]>,
    captures: Box<[CaptureRecipe]>,
    local_origins: Box<[BodySyntaxLocator]>,
    capture_origins: Box<[BodySyntaxLocator]>,
    block_scopes: Box<[(BodySyntaxLocator, BodyScopeId)]>,
    uses: Box<[UseRecipe]>,
    projections: Box<[ProjectionRecipe]>,
}

impl ReusableBodyNames {
    #[must_use]
    pub const fn body(&self) -> BodyId {
        self.body
    }

    pub(super) fn capture(
        graph: &DeclarationGraph,
        source: BodySource<'_>,
        names: &ResolvedBodyNames,
        projections: Vec<Projection>,
    ) -> Result<Self, ReusableBodyNamesError> {
        let syntax = BodySyntaxProjection::for_body(source.syntax(), source.block())
            .ok_or(ReusableBodyNamesError::InvalidBody)?;
        let scopes = names
            .scopes()
            .iter()
            .map(|(_, scope)| {
                Ok(ScopeRecipe {
                    parent: scope.parent(),
                    bindings: scope
                        .bindings()
                        .iter()
                        .map(|binding| Ok((spelling(graph, binding.name())?, binding.target())))
                        .collect::<Result<Vec<_>, ReusableBodyNamesError>>()?
                        .into_boxed_slice(),
                })
            })
            .collect::<Result<Vec<_>, ReusableBodyNamesError>>()?;
        let locals = names
            .locals()
            .iter()
            .map(|(_, local)| {
                Ok(LocalRecipe {
                    name: spelling(graph, local.name())?,
                    scope: local.scope(),
                    kind: local.kind(),
                })
            })
            .collect::<Result<Vec<_>, ReusableBodyNamesError>>()?;
        let captures = names
            .captures()
            .iter()
            .map(|(_, capture)| {
                Ok(CaptureRecipe {
                    name: spelling(graph, capture.name())?,
                    scope: capture.scope(),
                    source: capture.source(),
                    mode: capture.mode(),
                })
            })
            .collect::<Result<Vec<_>, ReusableBodyNamesError>>()?;
        let local_origins = names
            .local_origins()
            .iter()
            .map(|(_, origin)| locate(&syntax, *origin))
            .collect::<Result<Vec<_>, _>>()?;
        let capture_origins = names
            .capture_origins()
            .iter()
            .map(|(_, origin)| locate(&syntax, *origin))
            .collect::<Result<Vec<_>, _>>()?;
        let mut block_scopes = names
            .block_scopes()
            .iter()
            .map(|(block, scope)| Ok((locate(&syntax, SyntaxOrigin::Node(*block))?, *scope)))
            .collect::<Result<Vec<_>, ReusableBodyNamesError>>()?;
        block_scopes.sort_unstable_by_key(|(locator, _)| *locator);
        let uses = names
            .uses()
            .iter()
            .map(|use_| {
                Ok(UseRecipe {
                    origin: locate(&syntax, use_.origin())?,
                    target: use_.target(),
                })
            })
            .collect::<Result<Vec<_>, ReusableBodyNamesError>>()?;
        let projections = projections
            .into_iter()
            .map(|projection| {
                Ok(ProjectionRecipe {
                    entity: projection.entity,
                    role: projection.role,
                    origin: locate(&syntax, projection.origin.syntax())?,
                    documentation: projection.documentation,
                })
            })
            .collect::<Result<Vec<_>, ReusableBodyNamesError>>()?;
        Ok(Self {
            body: names.body(),
            scopes: scopes.into_boxed_slice(),
            locals: locals.into_boxed_slice(),
            captures: captures.into_boxed_slice(),
            local_origins: local_origins.into_boxed_slice(),
            capture_origins: capture_origins.into_boxed_slice(),
            block_scopes: block_scopes.into_boxed_slice(),
            uses: uses.into_boxed_slice(),
            projections: projections.into_boxed_slice(),
        })
    }

    pub(crate) fn materialize(
        &self,
        graph: &DeclarationGraph,
        source: BodySource<'_>,
    ) -> Result<(ResolvedBodyNames, Vec<Projection>), ReusableBodyNamesError> {
        if source.body() != self.body {
            return Err(ReusableBodyNamesError::BodyMismatch);
        }
        let syntax = BodySyntaxProjection::for_body(source.syntax(), source.block())
            .ok_or(ReusableBodyNamesError::InvalidBody)?;
        let mut scopes = ArenaBuilder::new();
        for recipe in &self.scopes {
            let mut scope = BodyScope::new(recipe.parent);
            for (name, target) in &recipe.bindings {
                scope.add_binding(ScopeBinding::new(symbol(graph, name)?, *target));
            }
            scopes.insert(scope);
        }
        let mut locals = ArenaBuilder::new();
        for recipe in &self.locals {
            locals.insert(LocalBinding::new(
                symbol(graph, &recipe.name)?,
                recipe.scope,
                recipe.kind,
            ));
        }
        let mut captures = ArenaBuilder::new();
        for recipe in &self.captures {
            captures.insert(Capture::new(
                symbol(graph, &recipe.name)?,
                recipe.scope,
                recipe.source,
                recipe.mode,
            ));
        }
        let mut local_origins = ArenaBuilder::<LocalBindingId, SyntaxOrigin>::new();
        for locator in &self.local_origins {
            local_origins.insert(resolve(&syntax, *locator)?);
        }
        let mut capture_origins = ArenaBuilder::<CaptureId, SyntaxOrigin>::new();
        for locator in &self.capture_origins {
            capture_origins.insert(resolve(&syntax, *locator)?);
        }
        let block_scopes = self
            .block_scopes
            .iter()
            .map(|(locator, scope)| match resolve(&syntax, *locator)? {
                SyntaxOrigin::Node(node) => Ok((node, *scope)),
                SyntaxOrigin::Token(_) => Err(ReusableBodyNamesError::ExpectedNode),
            })
            .collect::<Result<_, ReusableBodyNamesError>>()?;
        let uses = self
            .uses
            .iter()
            .map(|recipe| {
                Ok(ResolvedNameUse::new(
                    resolve(&syntax, recipe.origin)?,
                    recipe.target,
                ))
            })
            .collect::<Result<Vec<_>, ReusableBodyNamesError>>()?;
        let projections = self
            .projections
            .iter()
            .map(|recipe| {
                Ok(Projection {
                    entity: recipe.entity,
                    role: recipe.role,
                    origin: source_origin(source, resolve(&syntax, recipe.origin)?)?,
                    documentation: recipe.documentation.clone(),
                })
            })
            .collect::<Result<Vec<_>, ReusableBodyNamesError>>()?;
        Ok((
            ResolvedBodyNames::new(
                self.body,
                scopes.finish(),
                locals.finish(),
                captures.finish(),
                ResolvedBindingOrigins {
                    locals: local_origins.finish(),
                    captures: capture_origins.finish(),
                },
                block_scopes,
                uses,
            ),
            projections,
        ))
    }
}

fn spelling(graph: &DeclarationGraph, symbol: Symbol) -> Result<Box<str>, ReusableBodyNamesError> {
    graph
        .symbols()
        .spelling(symbol)
        .map(Into::into)
        .ok_or(ReusableBodyNamesError::MissingSymbol)
}

fn symbol(graph: &DeclarationGraph, spelling: &str) -> Result<Symbol, ReusableBodyNamesError> {
    graph
        .symbols()
        .get(spelling)
        .ok_or(ReusableBodyNamesError::MissingSymbol)
}

fn locate(
    syntax: &BodySyntaxProjection,
    origin: SyntaxOrigin,
) -> Result<BodySyntaxLocator, ReusableBodyNamesError> {
    syntax
        .locator(origin)
        .ok_or(ReusableBodyNamesError::MissingOrigin)
}

fn resolve(
    syntax: &BodySyntaxProjection,
    locator: BodySyntaxLocator,
) -> Result<SyntaxOrigin, ReusableBodyNamesError> {
    syntax
        .resolve(locator)
        .ok_or(ReusableBodyNamesError::MissingOrigin)
}

fn source_origin(
    source: BodySource<'_>,
    origin: SyntaxOrigin,
) -> Result<SourceOrigin, ReusableBodyNamesError> {
    match origin {
        SyntaxOrigin::Node(node) => SourceOrigin::from_node(source.syntax(), node)
            .map_err(|_| ReusableBodyNamesError::MissingOrigin),
        SyntaxOrigin::Token(token) => SourceOrigin::from_token(source.syntax(), token)
            .map_err(|_| ReusableBodyNamesError::MissingOrigin),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReusableBodyNamesError {
    InvalidBody,
    BodyMismatch,
    MissingSymbol,
    MissingOrigin,
    ExpectedNode,
}

#[derive(Debug)]
pub enum ReusableBodyResolutionError {
    Resolution(super::NameResolutionError),
    Projection(ReusableBodyNamesError),
}

impl std::fmt::Display for ReusableBodyResolutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resolution(error) => error.fmt(formatter),
            Self::Projection(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ReusableBodyResolutionError {}

impl std::fmt::Display for ReusableBodyNamesError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid reusable body-name projection: {self:?}")
    }
}

impl std::error::Error for ReusableBodyNamesError {}
