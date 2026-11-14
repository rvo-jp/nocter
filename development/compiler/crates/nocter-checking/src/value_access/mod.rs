//! Value-access typing and closed private-unit front-end validation.
//! Private native consumers require the complete Unit; the released pipeline has no adapter.
pub mod accepted;
mod annotations;
mod body_inputs;
mod booleans;
mod calls;
mod checker;
mod closures;
pub mod constant;
mod construction;
mod diagnostics;
pub mod evaluation;
mod expressions;
pub mod flow;
mod inputs;
mod instances;
pub use instances::CallTemplate;
mod ir;
mod paths;
mod postfix;
mod qualified;
mod required;
mod results;
pub use results::{
    CallValueSources, CreationRecipe, InstantiationFailure, InvocationRecipe, PreparedData,
    PreparedDataFailure, ProducerRelation, ProducerShape, ProducerTerm, ValueSource,
    ValueSourceBoundary, ValueSourceCarrier, ValueSourceFlow, ValueSourceId, ValueSourceInput,
    ValueSourceNode, ValueSourceRead, ValueSourceSelection, ValueSourceSlot, ValueSources,
};
pub use results::{
    PartialRootContracts, ResourceDataFailure, ResourceDataProjection, ResourceDataStep,
};
pub use results::{ReferentCarrierContracts, ReferentCarrierData};
pub use results::{ValueData, ValueDataFailure};
mod selection;
mod staged;
mod statements;
mod supply;
#[cfg(test)]
mod tests;
mod types;

pub use ir::{
    Argument, ArgumentAction, Binary, Block, BodyInput, BodyInputs, BoundaryAction, CallKind,
    Expression, ExpressionKind, FieldValue, IndexAction, Local, LocalOrigin, Place, Referent,
    SelectionAction, Statement, TypedBody, Unary, ValueType, WitnessBinding,
};
use nocter_declaration_lowering::access_declarations::{
    BodyDependencies, BodySource, Context as DeclarationContext, SourceMismatch,
};
use nocter_declarations::value_access::Program;
use nocter_diagnostics::{DiagnosticCode, SourceDiagnostic};
use nocter_model::{
    AccessBlockId, AccessCodeId, AccessExpressionId, AccessLocalId, AccessStatementId,
};
use nocter_source_index::SourceOrigin;
use nocter_syntax::{BodySyntaxLocator, BodySyntaxProjection, BoundSyntax, SyntaxOrigin};
pub use paths::{AccessPath, IndexFact};
pub use required::RequiredExpression;
use std::{collections::BTreeMap, sync::Arc};

/// Structural rule only: no initialization, lifetime, escape or conflict conclusion is implied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rule {
    UnknownName,
    UnknownNamespace,
    InaccessibleName,
    NamespaceValue,
    UnknownField,
    InaccessibleField,
    InaccessibleRepresentation,
    UnknownType,
    UnsupportedData,
    UnknownResultType,
    ShadowedName,
    TypeMismatch,
    MissingTransfer,
    UnusedValue,
    MissingResult,
    IntegerRange,
    InvalidLoopControl,
    InvalidReturn,
    InvalidDrop,
    NotWritable,
    CompoundOperand,
    InvalidUpdate,
    InvalidCall,
    InvalidConstruction,
    AnnotatedDiscard,
    RequiredValueScope,
}
impl Rule {
    const fn diagnostic(self) -> (DiagnosticCode, &'static str) {
        match self {
            Self::UnknownNamespace => (
                DiagnosticCode::E0340,
                "this path component does not name a module namespace",
            ),
            Self::InaccessibleName => (
                DiagnosticCode::E0412,
                "this qualified name is outside its visibility boundary",
            ),
            Self::NamespaceValue => (DiagnosticCode::E0370, "a module namespace is not a value"),
            Self::RequiredValueScope => (
                DiagnosticCode::E0421,
                "an enclosing local is unavailable to this required expression",
            ),
            Self::InvalidConstruction => (
                DiagnosticCode::E0370,
                "record construction requires each declared field exactly once",
            ),
            Self::UnknownResultType => (
                DiagnosticCode::E0370,
                "field selection needs a known record result type",
            ),
            Self::InaccessibleField => (
                DiagnosticCode::E0370,
                "this source cannot access the selected record field",
            ),
            Self::InaccessibleRepresentation => (
                DiagnosticCode::E0370,
                "record construction requires a visible complete definition or an explicit structural grant",
            ),
            Self::UnknownField => (
                DiagnosticCode::E0370,
                "the selected field does not exist in this data type",
            ),
            Self::UnsupportedData => (
                DiagnosticCode::E0370,
                "data operations are outside this private fragment",
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
                "value construction needs Copy evidence, a produced value or explicit postfix transfer",
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
                "integer literal is outside the selected integer range",
            ),
            Self::InvalidReturn => (
                DiagnosticCode::E0370,
                "return requires an enclosing source callable and cannot leave a value initializer",
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
                "compound assignment requires integer data",
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
    /// The unpublished draft requires a known declaration's complete contract.
    RequiredHeader {
        declaration: nocter_declarations::value_access::DeclarationId,
        origin: nocter_syntax::SyntaxOrigin,
    },
    /// The unpublished body draft needs a completed declaration value before typing continues.
    RequiredConstant {
        initializer: AccessCodeId,
        origin: nocter_syntax::SyntaxOrigin,
    },
    RequiredExpression(Box<RequiredExpression>),
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

impl From<SourceMismatch> for Failure {
    fn from(failure: SourceMismatch) -> Self {
        match failure {
            SourceMismatch::UnknownSource => Self::UnknownCallable,
            SourceMismatch::ChangedAuthority => Self::ChangedAuthority,
            SourceMismatch::ChangedSource => Self::ChangedBody,
            SourceMismatch::InconsistentSyntax | SourceMismatch::InconsistentDependencies => {
                Self::InconsistentSyntax
            }
        }
    }
}

/// Reusable type/call selection evidence tied to its own callable and declaration authority.
#[derive(Clone, Debug)]
pub struct Query {
    closures: BTreeMap<nocter_model::ValueClosure, flow::Control>,
    program: Arc<Program>,
    source: BodySource,
    code: nocter_model::ValueCode,
    result_data: nocter_model::TypeId,
    results_closed: bool,
    result_evidence: Option<Arc<results::Evidence>>,
    evaluation_source: Option<BodyDependencies>,
    equalities: Vec<results::Equality>,
    body: TypedBody,
    origins: Origins,
}
impl Query {
    fn freeze_constant_sources(&mut self, source: &BodyDependencies) {
        self.evaluation_source = Some(source.clone());
        for nested in self.closures.values_mut() {
            Arc::make_mut(&mut nested.typed).freeze_constant_sources(source);
        }
    }
    /// Type-checks one body, without certifying its memory safety or executable validity.
    /// # Errors
    /// Rejects structural typing/name/call errors or an inconsistent source pairing. A missing
    /// declaration value is a typed dependency request; complete Unit preparation resolves it.
    pub fn prepare(
        context: &DeclarationContext<'_>,
        callable: AccessCodeId,
    ) -> Result<Self, Failure> {
        checker::prepare(context, callable, &())
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
    pub const fn code(&self) -> nocter_model::ValueCode {
        self.code
    }
    #[must_use]
    pub const fn source_owner(&self) -> AccessCodeId {
        self.source.owner()
    }
    fn source_for_code(&self) -> Option<&BodySource> {
        (self.code.root() == self.source.code()).then_some(&self.source)
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
        if let Some(source) = &self.evaluation_source {
            source.verify(context)?;
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
        let source = self
            .source_for_code()
            .ok_or(Failure::InconsistentSyntax)?
            .bind(context)?;
        let syntax = context.for_code(self.source_owner())?.syntax();
        let projection = BodySyntaxProjection::for_root(
            syntax.tree(),
            source.body().ok_or(Failure::InconsistentSyntax)?,
        )
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

mod integers;
mod outcomes;
