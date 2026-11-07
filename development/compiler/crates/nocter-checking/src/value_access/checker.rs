use super::{
    BodyDiagnostic, Failure, Origins, Query, Rule, Site,
    ir::{
        Block, Boundary, BoundaryAction, Expression, ExpressionKind, Local, LocalOrigin, Statement,
        TypedBody, ValueType,
    },
};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::Program;
use nocter_diagnostics::{DiagnosticNote, SourceDiagnostic};
use nocter_model::{
    AccessBlockId, AccessCallableId, AccessExpressionId, AccessLocalId, AccessLoopId,
    AccessStatementId, ArenaBuilder, BuiltinType, TypeId, ValueAccessMode as Mode,
    ValueAccessType as Ty,
};
use nocter_syntax::{
    BodySyntaxProjection, BoundSyntax, NodeId, NodeKind, Punctuation, SyntaxOrigin, SyntaxToken,
    TokenKind, child_node_iter, direct_node, direct_token, first_direct_token,
};
use std::collections::BTreeMap;

pub(super) struct LoopFrame {
    pub id: AccessLoopId,
    pub reachable_break: bool,
}
pub(super) struct Checker<'a, 's> {
    pub context: &'a DeclarationContext<'s>,
    pub result: Ty,
    pub projection: BodySyntaxProjection,
    pub origins: Origins,
    pub locals: ArenaBuilder<AccessLocalId, Local>,
    pub expressions: ArenaBuilder<AccessExpressionId, Expression>,
    pub statements: ArenaBuilder<AccessStatementId, Statement>,
    pub blocks: ArenaBuilder<AccessBlockId, Block>,
    pub loops: ArenaBuilder<AccessLoopId, ()>,
    pub loop_stack: Vec<LoopFrame>,
    pub scopes: Vec<BTreeMap<String, AccessLocalId>>,
}

pub(super) fn prepare(
    context: &DeclarationContext<'_>,
    callable: AccessCallableId,
) -> Result<Query, Failure> {
    let declaration = context
        .program()
        .callables()
        .get(callable)
        .ok_or(Failure::UnknownCallable)?;
    let source = context
        .callable_source(callable)
        .ok_or(Failure::UnknownCallable)?;
    let syntax = context.syntax();
    let mut checker = Checker {
        context,
        result: declaration.result(),
        projection: BodySyntaxProjection::for_root(syntax.tree(), source.body())
            .ok_or(Failure::InconsistentSyntax)?,
        origins: Origins::default(),
        locals: ArenaBuilder::new(),
        expressions: ArenaBuilder::new(),
        statements: ArenaBuilder::new(),
        blocks: ArenaBuilder::new(),
        loops: ArenaBuilder::new(),
        loop_stack: Vec::new(),
        scopes: vec![BTreeMap::new()],
    };
    for (&parameter, &token) in declaration.parameters().iter().zip(source.parameters()) {
        let parameter_declaration = context
            .program()
            .parameters()
            .get(parameter)
            .ok_or(Failure::InconsistentSyntax)?;
        let local = checker.locals.insert(Local {
            ty: parameter_declaration.ty(),
            origin: LocalOrigin::Parameter(parameter),
        });
        checker
            .origins
            .locals
            .insert(local, Site::Parameter(parameter_declaration.position()));
        let spelling = checker.text(token)?.to_owned();
        checker.scopes[0].insert(spelling, local);
    }
    let root = checker.block(
        source.body(),
        Some(Boundary::Return(declaration.result())),
        true,
    )?;
    let body_bytes = syntax
        .text_at(checker.node_range(source.body())?)
        .ok_or(Failure::InconsistentSyntax)?
        .as_bytes()
        .into();
    Ok(Query {
        program: context.program_handle(),
        callable,
        body_bytes,
        origins: checker.origins,
        body: TypedBody {
            root,
            locals: checker.locals.finish(),
            expressions: checker.expressions.finish(),
            statements: checker.statements.finish(),
            blocks: checker.blocks.finish(),
            loops: checker.loops.finish(),
        },
    })
}

impl Checker<'_, '_> {
    pub fn syntax(&self) -> BoundSyntax<'_> {
        self.context.syntax()
    }
    pub fn program(&self) -> &Program {
        self.context.program()
    }
    pub fn kind(&self, node: NodeId) -> Result<NodeKind, Failure> {
        self.syntax()
            .tree()
            .node(node)
            .map(nocter_syntax::SyntaxNode::kind)
            .ok_or(Failure::InconsistentSyntax)
    }
    pub fn node_range(&self, node: NodeId) -> Result<nocter_source::TextRange, Failure> {
        self.syntax()
            .tree()
            .node(node)
            .map(nocter_syntax::SyntaxNode::range)
            .ok_or(Failure::InconsistentSyntax)
    }
    pub fn children(&self, node: NodeId) -> Vec<NodeId> {
        child_node_iter(self.syntax().tree(), node).collect()
    }
    pub fn direct(&self, node: NodeId, kind: NodeKind) -> Result<NodeId, Failure> {
        direct_node(self.syntax().tree(), node, kind).ok_or(Failure::InconsistentSyntax)
    }
    pub fn first_child(&self, node: NodeId) -> Result<NodeId, Failure> {
        self.children(node)
            .first()
            .copied()
            .ok_or(Failure::InconsistentSyntax)
    }
    pub fn token(&self, node: NodeId) -> Result<SyntaxToken, Failure> {
        first_direct_token(self.syntax().tree(), node).ok_or(Failure::InconsistentSyntax)
    }
    pub fn text(&self, token: SyntaxToken) -> Result<&str, Failure> {
        self.syntax()
            .text_at(token.range())
            .ok_or(Failure::InconsistentSyntax)
    }
    pub fn has(&self, node: NodeId, punctuation: Punctuation) -> bool {
        direct_token(
            self.syntax().tree(),
            node,
            TokenKind::Punctuation(punctuation),
        )
        .is_some()
    }
    pub fn data(&self, kind: BuiltinType) -> TypeId {
        self.program()
            .data_type(kind)
            .expect("core built-in authority")
    }
    pub fn copyable(&self, data: TypeId) -> bool {
        self.program()
            .data_copyable(data)
            .expect("core data authority")
    }
    pub fn ty(&self, expr: AccessExpressionId) -> ValueType {
        self.expressions.get(expr).expect("own expression").ty
    }
    pub fn local_ty(&self, local: AccessLocalId) -> Ty {
        self.locals.get(local).expect("own local").ty
    }
    pub fn site(&self, origin: SyntaxOrigin) -> Result<Site, Failure> {
        self.projection
            .locator(origin)
            .map(Site::Body)
            .ok_or(Failure::InconsistentSyntax)
    }
    pub fn emit(
        &mut self,
        node: NodeId,
        ty: ValueType,
        kind: ExpressionKind,
    ) -> Result<AccessExpressionId, Failure> {
        let site = self.site(SyntaxOrigin::Node(node))?;
        let id = self.expressions.insert(Expression { ty, kind });
        self.origins.expressions.insert(id, site);
        Ok(id)
    }
    pub fn statement(
        &mut self,
        node: NodeId,
        statement: Statement,
    ) -> Result<AccessStatementId, Failure> {
        let site = self.site(SyntaxOrigin::Node(node))?;
        let id = self.statements.insert(statement);
        self.origins.statements.insert(id, site);
        Ok(id)
    }
    pub fn emit_block(
        &mut self,
        node: NodeId,
        statements: Vec<AccessStatementId>,
        result: Option<AccessExpressionId>,
        ty: ValueType,
    ) -> Result<AccessBlockId, Failure> {
        let site = self.site(SyntaxOrigin::Node(node))?;
        let id = self.blocks.insert(Block {
            statements: statements.into_boxed_slice(),
            result,
            ty,
        });
        self.origins.blocks.insert(id, site);
        Ok(id)
    }
    pub fn reject<T>(&self, rule: Rule, origin: impl Authored) -> Result<T, Failure> {
        self.reject_message(rule, origin, rule.diagnostic().1.to_owned(), Vec::new())
    }
    pub fn reject_message<T>(
        &self,
        rule: Rule,
        origin: impl Authored,
        message: String,
        notes: Vec<DiagnosticNote>,
    ) -> Result<T, Failure> {
        let source = SourceDiagnostic::new(
            rule.diagnostic().0,
            message,
            super::source_origin(self.syntax(), origin.origin())?,
            notes,
            None::<&str>,
        );
        Err(Failure::Rejected(Box::new(BodyDiagnostic { rule, source })))
    }
    pub fn type_label(&self, ty: Ty) -> String {
        let prefix = match ty.mode() {
            Mode::Read => "",
            Mode::Update => "&",
            Mode::Own => "^",
        };
        format!(
            "{prefix}{}",
            self.program()
                .data_kind(ty.data())
                .expect("core data")
                .spelling()
        )
    }
    pub fn reject_local<T>(
        &self,
        rule: Rule,
        origin: impl Authored,
        local: AccessLocalId,
    ) -> Result<T, Failure> {
        let declaration = self.locals.get(local).ok_or(Failure::InconsistentSyntax)?;
        let site = self
            .origins
            .locals
            .get(&local)
            .ok_or(Failure::InconsistentSyntax)?;
        let related = match *site {
            Site::Body(locator) => self
                .projection
                .resolve(locator)
                .ok_or(Failure::InconsistentSyntax)?,
            Site::Parameter(position) => {
                let LocalOrigin::Parameter(parameter) = declaration.origin else {
                    return Err(Failure::InconsistentSyntax);
                };
                let owner = self
                    .program()
                    .parameters()
                    .get(parameter)
                    .ok_or(Failure::InconsistentSyntax)?
                    .owner();
                let token = self
                    .context
                    .callable_source(owner)
                    .and_then(|source| source.parameters().get(position))
                    .ok_or(Failure::InconsistentSyntax)?;
                SyntaxOrigin::Token(*token)
            }
        };
        let note = DiagnosticNote::new(
            format!(
                "this place is declared with {} access",
                self.type_label(declaration.ty)
            ),
            super::source_origin(self.syntax(), related)?,
        );
        self.reject_message(rule, origin, rule.diagnostic().1.to_owned(), vec![note])
    }
    pub fn mismatch<T>(&self, node: NodeId, expected: Ty, actual: Ty) -> Result<T, Failure> {
        self.reject_message(
            Rule::TypeMismatch,
            node,
            format!(
                "expected {}, but this expression supplies {}",
                self.type_label(expected),
                self.type_label(actual)
            ),
            Vec::new(),
        )
    }
    pub fn lookup(&self, name: &str) -> Option<AccessLocalId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }
    pub fn unwrap(&self, mut node: NodeId) -> Result<NodeId, Failure> {
        while matches!(
            self.kind(node)?,
            NodeKind::Expression | NodeKind::GroupedExpression | NodeKind::AssignmentTarget
        ) {
            node = self.first_child(node)?;
        }
        Ok(node)
    }
    pub fn name_token(&self, node: NodeId) -> Result<Option<SyntaxToken>, Failure> {
        let node = self.unwrap(node)?;
        if self.kind(node)? == NodeKind::ReferenceExpression {
            Ok(Some(self.token(node)?))
        } else {
            Ok(None)
        }
    }
    pub fn place(&self, node: NodeId, rule: Rule) -> Result<AccessLocalId, Failure> {
        let Some(token) = self.name_token(node)? else {
            return self.reject(rule, node);
        };
        self.place_name(token, rule)
    }
    pub fn place_name(&self, token: SyntaxToken, rule: Rule) -> Result<AccessLocalId, Failure> {
        let spelling = self.text(token)?;
        if let Some(local) = self.lookup(spelling) {
            return Ok(local);
        }
        if self
            .program()
            .symbols()
            .get(spelling)
            .and_then(|symbol| self.program().resolve(symbol))
            .is_some()
        {
            return self.reject(rule, token);
        }
        self.reject(Rule::UnknownName, token)
    }
    pub fn place_expression(&self, expr: AccessExpressionId) -> Option<AccessLocalId> {
        match self.expressions.get(expr)?.kind {
            ExpressionKind::Place(local) => Some(local),
            _ => None,
        }
    }
    pub fn annotation(&self, node: NodeId) -> Result<Ty, Failure> {
        let node = direct_node(self.syntax().tree(), node, NodeKind::AccessType).unwrap_or(node);
        let mode = if self.has(node, Punctuation::Ampersand) {
            Mode::Update
        } else if self.has(node, Punctuation::Caret) {
            Mode::Own
        } else {
            Mode::Read
        };
        let named = self.direct(node, NodeKind::NamedType)?;
        let kind = BuiltinType::from_spelling(self.text(self.token(named)?)?)
            .ok_or(Failure::InconsistentSyntax)?;
        Ok(Ty::new(self.data(kind), mode))
    }
    pub fn require_data(
        &self,
        expr: AccessExpressionId,
        data: TypeId,
        node: NodeId,
    ) -> Result<(), Failure> {
        if let ValueType::Value(ty) = self.ty(expr)
            && ty.data() != data
        {
            return self.mismatch(node, Ty::new(data, ty.mode()), ty);
        }
        Ok(())
    }
    pub fn boundary(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
        boundary: Boundary,
    ) -> Result<AccessExpressionId, Failure> {
        let ValueType::Value(actual) = self.ty(source) else {
            return Ok(source);
        };
        let expected = boundary.ty();
        if actual.data() != expected.data() {
            return self.mismatch(node, expected, actual);
        }
        let place = self.place_expression(source);
        let action = match expected.mode() {
            Mode::Read
                if matches!(boundary, Boundary::Return(_)) && self.copyable(actual.data()) =>
            {
                BoundaryAction::CopyPayload
            }
            Mode::Read => {
                if place.is_some() {
                    BoundaryAction::ReadPlace
                } else {
                    match actual.mode() {
                        Mode::Read => return Ok(source),
                        Mode::Update => BoundaryAction::ReadAccess,
                        Mode::Own if self.copyable(actual.data()) => BoundaryAction::CopyPayload,
                        Mode::Own => return self.reject(Rule::TypeMismatch, node),
                    }
                }
            }
            Mode::Update => {
                if let Some(local) = place {
                    if self.local_ty(local).mode() == Mode::Read {
                        return self.reject_local(Rule::InvalidUpdate, node, local);
                    }
                    BoundaryAction::DelegateUpdate
                } else if actual.mode() == Mode::Update {
                    return Ok(source);
                } else {
                    return self.reject(Rule::InvalidUpdate, node);
                }
            }
            Mode::Own => {
                if place.is_some() || actual.mode() != Mode::Own {
                    if !self.copyable(actual.data()) {
                        return self.reject(Rule::MissingTransfer, node);
                    }
                    BoundaryAction::CopyPayload
                } else {
                    BoundaryAction::TakeValue
                }
            }
        };
        self.emit(
            node,
            ValueType::Value(expected),
            ExpressionKind::Boundary { source, action },
        )
    }
}

pub(super) trait Authored {
    fn origin(self) -> SyntaxOrigin;
}
impl Authored for NodeId {
    fn origin(self) -> SyntaxOrigin {
        SyntaxOrigin::Node(self)
    }
}
impl Authored for SyntaxToken {
    fn origin(self) -> SyntaxOrigin {
        SyntaxOrigin::Token(self)
    }
}
