//! Typed-only value-access bodies for the bounded future private-function goal.
//! This product is deliberately unavailable to executable/checked-program consumers.
mod checker;
mod expressions;
mod ir;
mod statements;
#[cfg(test)]
mod tests;

pub use ir::{
    Argument, ArgumentAction, Binary, Block, BoundaryAction, Expression, ExpressionKind, Local,
    LocalOrigin, Statement, TypedBody, Unary, ValueType,
};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::Program;
use nocter_diagnostics::{DiagnosticCode, SourceDiagnostic};
use nocter_model::{
    AccessBlockId, AccessCallableId, AccessExpressionId, AccessLocalId, AccessStatementId,
};
use nocter_source_index::SourceOrigin;
use nocter_syntax::{BodySyntaxLocator, BodySyntaxProjection, BoundSyntax, SyntaxOrigin};
use std::{collections::BTreeMap, sync::Arc};

/// Structural rule only: no initialization, lifetime, escape or conflict conclusion is implied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rule {
    UnknownName,
    ShadowedName,
    TypeMismatch,
    MissingTransfer,
    UnusedValue,
    MissingResult,
    IntegerRange,
    InvalidTransfer,
    InvalidLoopControl,
    InvalidDrop,
    NotWritable,
    CompoundOperand,
    InvalidUpdate,
    InvalidCall,
    AnnotatedDiscard,
}
impl Rule {
    const fn diagnostic(self) -> (DiagnosticCode, &'static str) {
        match self {
            Self::UnknownName => (DiagnosticCode::E0340, "unknown local or function name"),
            Self::ShadowedName => (
                DiagnosticCode::E0341,
                "local name conflicts with an active binding, function, or built-in type",
            ),
            Self::TypeMismatch => (
                DiagnosticCode::E0370,
                "data type or access mode does not match this value boundary",
            ),
            Self::MissingTransfer => (
                DiagnosticCode::E0371,
                "this boundary needs an owned value; transfer a non-Copy owner explicitly",
            ),
            Self::UnusedValue => (
                DiagnosticCode::E0372,
                "non-final value must be used or explicitly discarded",
            ),
            Self::MissingResult => (
                DiagnosticCode::E0373,
                "normal completion requires a result of the declared data type",
            ),
            Self::IntegerRange => (
                DiagnosticCode::E0375,
                "integer literal is outside the i32 range",
            ),
            Self::InvalidTransfer => (
                DiagnosticCode::E0377,
                "transfer requires an existing owned place",
            ),
            Self::InvalidLoopControl => (
                DiagnosticCode::E0382,
                "break or continue requires an enclosing loop",
            ),
            Self::InvalidDrop => (
                DiagnosticCode::E0383,
                "explicit drop requires an owned local",
            ),
            Self::NotWritable => (
                DiagnosticCode::E0384,
                "assignment requires an owned or update place",
            ),
            Self::CompoundOperand => (
                DiagnosticCode::E0386,
                "compound assignment requires i32 data",
            ),
            Self::InvalidUpdate => (
                DiagnosticCode::E0387,
                "update access requires an existing owned or update place",
            ),
            Self::InvalidCall => (
                DiagnosticCode::E0390,
                "direct call requires a declared function and its exact argument count",
            ),
            Self::AnnotatedDiscard => (
                DiagnosticCode::E0407,
                "discard uses let _ = expression without a type annotation",
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodyDiagnostic {
    rule: Rule,
    source: SourceDiagnostic,
}
impl BodyDiagnostic {
    #[must_use]
    pub const fn rule(&self) -> Rule {
        self.rule
    }
    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}

#[derive(Debug)]
pub enum Failure {
    UnknownCallable,
    InconsistentSyntax,
    Rejected(Box<BodyDiagnostic>),
    ChangedAuthority,
    ChangedBody,
}

#[derive(Clone, Copy, Debug)]
enum Site {
    Body(BodySyntaxLocator),
    Parameter(usize),
}

#[derive(Clone, Debug, Default)]
struct Origins {
    expressions: BTreeMap<AccessExpressionId, Site>,
    statements: BTreeMap<AccessStatementId, Site>,
    locals: BTreeMap<AccessLocalId, Site>,
    blocks: BTreeMap<AccessBlockId, Site>,
}

/// Reusable type/call selection evidence tied to its own callable and declaration authority.
#[derive(Clone, Debug)]
pub struct Query {
    program: Arc<Program>,
    callable: AccessCallableId,
    body_bytes: Box<[u8]>,
    body: TypedBody,
    origins: Origins,
}
impl Query {
    /// Type-checks one body, without certifying its memory safety or executable validity.
    /// # Errors
    /// Rejects structural typing/name/call errors or an inconsistent source pairing.
    pub fn prepare(
        context: &DeclarationContext<'_>,
        callable: AccessCallableId,
    ) -> Result<Self, Failure> {
        checker::prepare(context, callable)
    }
    #[must_use]
    pub const fn callable(&self) -> AccessCallableId {
        self.callable
    }
    #[must_use]
    pub const fn body(&self) -> &TypedBody {
        &self.body
    }
    #[must_use]
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// Projects existing evidence into equal current body syntax; never checks or selects again.
    /// # Errors
    /// Rejects a different declaration authority, changed body, or inconsistent locator.
    pub fn bind<'query, 'source>(
        &'query self,
        context: &DeclarationContext<'source>,
    ) -> Result<Context<'query, 'source>, Failure> {
        if !Arc::ptr_eq(&self.program, &context.program_handle()) {
            return Err(Failure::ChangedAuthority);
        }
        let source = context
            .callable_source(self.callable)
            .ok_or(Failure::UnknownCallable)?;
        let syntax = context.syntax();
        let range = syntax
            .tree()
            .node(source.body())
            .ok_or(Failure::InconsistentSyntax)?
            .range();
        if syntax
            .text_at(range)
            .ok_or(Failure::InconsistentSyntax)?
            .as_bytes()
            != self.body_bytes.as_ref()
        {
            return Err(Failure::ChangedBody);
        }
        let projection = BodySyntaxProjection::for_root(syntax.tree(), source.body())
            .ok_or(Failure::InconsistentSyntax)?;
        let resolve = |site| {
            match site {
                Site::Body(locator) => projection.resolve(locator),
                Site::Parameter(index) => source
                    .parameters()
                    .get(index)
                    .copied()
                    .map(SyntaxOrigin::Token),
            }
            .ok_or(Failure::InconsistentSyntax)
        };
        Ok(Context {
            query: self,
            syntax,
            expressions: self
                .origins
                .expressions
                .iter()
                .map(|(&id, &site)| Ok((id, resolve(site)?)))
                .collect::<Result<_, Failure>>()?,
            statements: self
                .origins
                .statements
                .iter()
                .map(|(&id, &site)| Ok((id, resolve(site)?)))
                .collect::<Result<_, Failure>>()?,
            locals: self
                .origins
                .locals
                .iter()
                .map(|(&id, &site)| Ok((id, resolve(site)?)))
                .collect::<Result<_, Failure>>()?,
            blocks: self
                .origins
                .blocks
                .iter()
                .map(|(&id, &site)| Ok((id, resolve(site)?)))
                .collect::<Result<_, Failure>>()?,
        })
    }
}

/// Exact-current projection beside a typed-only body; callers cannot pair parts independently.
#[derive(Debug)]
pub struct Context<'query, 'source> {
    query: &'query Query,
    syntax: BoundSyntax<'source>,
    expressions: BTreeMap<AccessExpressionId, SyntaxOrigin>,
    statements: BTreeMap<AccessStatementId, SyntaxOrigin>,
    locals: BTreeMap<AccessLocalId, SyntaxOrigin>,
    blocks: BTreeMap<AccessBlockId, SyntaxOrigin>,
}
impl Context<'_, '_> {
    #[must_use]
    pub const fn query(&self) -> &Query {
        self.query
    }
    #[must_use]
    pub const fn syntax(&self) -> BoundSyntax<'_> {
        self.syntax
    }
    #[must_use]
    pub fn expression_origin(&self, id: AccessExpressionId) -> Option<SyntaxOrigin> {
        self.expressions.get(&id).copied()
    }
    #[must_use]
    pub fn statement_origin(&self, id: AccessStatementId) -> Option<SyntaxOrigin> {
        self.statements.get(&id).copied()
    }
    #[must_use]
    pub fn local_origin(&self, id: AccessLocalId) -> Option<SyntaxOrigin> {
        self.locals.get(&id).copied()
    }
    #[must_use]
    pub fn block_origin(&self, id: AccessBlockId) -> Option<SyntaxOrigin> {
        self.blocks.get(&id).copied()
    }
}

fn source_origin(syntax: BoundSyntax<'_>, origin: SyntaxOrigin) -> Result<SourceOrigin, Failure> {
    match origin {
        SyntaxOrigin::Node(node) => {
            SourceOrigin::from_node(syntax.tree(), node).map_err(|_| Failure::InconsistentSyntax)
        }
        SyntaxOrigin::Token(token) => {
            SourceOrigin::from_token(syntax.tree(), token).map_err(|_| Failure::InconsistentSyntax)
        }
    }
}
