//! Value-access typing and closed private-unit front-end validation.
//! Accepted future units remain unavailable to released/native consumers.
pub mod accepted;
mod annotations;
mod calls;
mod checker;
mod closures;
mod construction;
mod diagnostics;
mod expressions;
pub mod flow;
mod inputs;
mod ir;
mod results;
mod selection;
mod statements;
mod supply;
#[cfg(test)]
mod tests;
mod types;

pub use ir::{
    Argument, ArgumentAction, Binary, Block, BoundaryAction, CallKind, Expression, ExpressionKind,
    FieldValue, Local, LocalOrigin, Place, SelectionAction, Statement, TypedBody, Unary, ValueType,
    WitnessBinding,
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
    UnknownField,
    UnknownType,
    UnknownResultType,
    ShadowedName,
    TypeMismatch,
    MissingTransfer,
    UnusedValue,
    MissingResult,
    IntegerRange,
    InvalidLoopControl,
    InvalidDrop,
    NotWritable,
    CompoundOperand,
    InvalidUpdate,
    InvalidCall,
    InvalidConstruction,
    AnnotatedDiscard,
}
impl Rule {
    const fn diagnostic(self) -> (DiagnosticCode, &'static str) {
        match self {
            Self::InvalidConstruction => (
                DiagnosticCode::E0370,
                "record construction requires each declared field exactly once",
            ),
            Self::UnknownResultType => (
                DiagnosticCode::E0370,
                "field selection needs a known record result type",
            ),
            Self::UnknownField => (
                DiagnosticCode::E0370,
                "the selected field does not exist in this data type",
            ),
            Self::UnknownType => (DiagnosticCode::E0370, "unknown or unsupported data type"),
            Self::UnknownName => (DiagnosticCode::E0340, "unknown local or function name"),
            Self::ShadowedName => (
                DiagnosticCode::E0341,
                "local name conflicts with an active binding, function, or built-in type",
            ),
            Self::TypeMismatch => (
                DiagnosticCode::E0370,
                "data type or operation does not match this value boundary",
            ),
            Self::MissingTransfer => (
                DiagnosticCode::E0371,
                "value construction needs Copy evidence or an eligible owning source",
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
                "assignment requires a mutable value or edit access",
            ),
            Self::CompoundOperand => (
                DiagnosticCode::E0386,
                "compound assignment requires i32 data",
            ),
            Self::InvalidUpdate => (
                DiagnosticCode::E0387,
                "edit access requires existing writable storage",
            ),
            Self::InvalidCall => (
                DiagnosticCode::E0390,
                "call requires a selected callable witness and its exact input contract",
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
    closures: BTreeMap<nocter_model::ValueClosure, flow::Control>,
    program: Arc<Program>,
    header: Arc<Program>,
    callable: AccessCallableId,
    body_bytes: Arc<[u8]>,
    result_data: nocter_model::TypeId,
    results_closed: bool,
    result_evidence: Option<Arc<results::Evidence>>,
    equalities: Vec<results::Equality>,
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
    pub const fn results_closed(&self) -> bool {
        self.results_closed
    }
    #[must_use]
    pub const fn result_data(&self) -> nocter_model::TypeId {
        self.result_data
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
        if !Arc::ptr_eq(&self.header, &context.program_handle()) {
            return Err(Failure::ChangedAuthority);
        }
        if let Some(evidence) = &self.result_evidence {
            let verified = evidence.verify(context)?;
            return self.bind_verified(&verified);
        }
        self.bind_current(context)
    }
    fn bind_verified<'query, 'source>(
        &'query self,
        verified: &results::Verified<'_, 'source>,
    ) -> Result<Context<'query, 'source>, Failure> {
        if let Some(evidence) = &self.result_evidence
            && !std::ptr::eq(evidence.as_ref(), verified.evidence)
        {
            return Err(Failure::ChangedAuthority);
        }
        self.bind_current(verified.context)
    }
    fn bind_current<'query, 'source>(
        &'query self,
        context: &DeclarationContext<'source>,
    ) -> Result<Context<'query, 'source>, Failure> {
        if !Arc::ptr_eq(&self.header, &context.program_handle()) {
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
