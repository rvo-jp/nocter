use super::{
    BodyDiagnostic, Failure, Origins, Query, Rule, Site,
    ir::{
        Block, Boundary, BoundaryAction, Expression, ExpressionKind, Local, LocalOrigin, Place,
        Statement, TypedBody, ValueType,
    },
};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::{PayloadSchema, Program};
use nocter_diagnostics::{DiagnosticNote, SourceDiagnostic};
use nocter_model::{
    AccessBlockId, AccessCallableId, AccessExpressionId, AccessLocalId, AccessLoopId,
    AccessPlaceId, AccessStatementId, ArenaBuilder, BuiltinType, FieldPath, TypeId,
    ValueAccess as Ty, ValueAccessMode as Mode,
};
use nocter_syntax::{
    BodySyntaxProjection, BoundSyntax, NodeId, NodeKind, Punctuation, SyntaxOrigin, SyntaxToken,
    TokenKind, child_node_iter, direct_node, direct_token, first_direct_token,
};
use std::{collections::BTreeMap, sync::Arc};

pub(super) struct LoopFrame {
    pub id: AccessLoopId,
    pub reachable_break: bool,
}
pub(super) struct Checker<'a, 's> {
    pub closures: BTreeMap<nocter_model::ValueClosure, super::flow::Control>,
    pub code: nocter_model::ValueCode,
    pub environment: Option<super::BodyInput>,
    pub inputs: Vec<super::BodyInput>,
    pub context: &'a DeclarationContext<'s>,
    pub body_bytes: Arc<[u8]>,
    pub result: TypeId,
    pub types: nocter_declarations::value_access::BodyTypes,
    pub pending_results: bool,
    pub equalities: Vec<super::results::Equality>,
    pub projection: Arc<BodySyntaxProjection>,
    pub origins: Origins,
    pub locals: ArenaBuilder<AccessLocalId, Local>,
    pub expressions: ArenaBuilder<AccessExpressionId, Expression>,
    pub places: ArenaBuilder<AccessPlaceId, Place>,
    pub place_index: BTreeMap<(AccessLocalId, FieldPath), AccessPlaceId>,
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
    let projection = BodySyntaxProjection::for_root(syntax.tree(), source.body())
        .ok_or(Failure::InconsistentSyntax)?;
    let range = syntax
        .tree()
        .node(source.body())
        .ok_or(Failure::InconsistentSyntax)?
        .range();
    let body_bytes = syntax
        .text_at(range)
        .ok_or(Failure::InconsistentSyntax)?
        .as_bytes()
        .into();
    let mut checker = Checker::new(
        context,
        declaration.result(),
        context
            .program()
            .body_types(callable)
            .map_err(|_| Failure::InconsistentSyntax)?,
        Arc::new(projection),
        body_bytes,
        nocter_model::ValueCode::Declaration(callable),
    );
    for (&parameter, &token) in declaration.parameters().iter().zip(source.parameters()) {
        let parameter_declaration = context
            .program()
            .parameters()
            .get(parameter)
            .ok_or(Failure::InconsistentSyntax)?;
        let local = checker.locals.insert(Local {
            data: parameter_declaration.data(),
            kind: parameter_declaration.operation().binding_kind(),
            origin: LocalOrigin::Parameter(parameter),
        });
        checker.inputs.push(super::BodyInput {
            local,
            operation: parameter_declaration.operation(),
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
        Some(Boundary::Result(declaration.result())),
        true,
    )?;
    Ok(checker.finish(root))
}

impl<'a, 's> Checker<'a, 's> {
    pub(super) fn new(
        context: &'a DeclarationContext<'s>,
        result: TypeId,
        types: nocter_declarations::value_access::BodyTypes,
        projection: Arc<BodySyntaxProjection>,
        body_bytes: Arc<[u8]>,
        code: nocter_model::ValueCode,
    ) -> Self {
        let pending_results = super::results::is_result(types.program(), result);
        Self {
            context,
            result,
            types,
            projection,
            body_bytes,
            code,
            environment: None,
            inputs: Vec::new(),
            pending_results,
            closures: BTreeMap::new(),
            equalities: Vec::new(),
            origins: Origins::default(),
            locals: ArenaBuilder::new(),
            expressions: ArenaBuilder::new(),
            places: ArenaBuilder::new(),
            place_index: BTreeMap::new(),
            statements: ArenaBuilder::new(),
            blocks: ArenaBuilder::new(),
            loops: ArenaBuilder::new(),
            loop_stack: Vec::new(),
            scopes: vec![BTreeMap::new()],
        }
    }
    pub(super) fn finish(self, root: AccessBlockId) -> Query {
        Query {
            code: self.code,
            program: Arc::new(self.types.finish()),
            header: self.context.program_handle(),
            body_bytes: self.body_bytes,
            result_data: self.result,
            results_closed: !self.pending_results,
            result_evidence: None,
            equalities: self.equalities,
            closures: self.closures,
            origins: self.origins,
            body: TypedBody {
                inputs: super::BodyInputs {
                    environment: self.environment,
                    explicit: self.inputs.into(),
                },
                root,
                locals: self.locals.finish(),
                expressions: self.expressions.finish(),
                places: self.places.finish(),
                statements: self.statements.finish(),
                blocks: self.blocks.finish(),
                loops: self.loops.finish(),
            },
        }
    }

    pub fn syntax(&self) -> BoundSyntax<'_> {
        self.context.syntax()
    }
    pub fn program(&self) -> &Program {
        self.types.program()
    }
    pub fn kind(&self, node: NodeId) -> Result<NodeKind, Failure> {
        self.syntax()
            .tree()
            .node(node)
            .map(nocter_syntax::SyntaxNode::kind)
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
    pub fn literal_bool(&self, expression: AccessExpressionId) -> Option<bool> {
        match self.expressions.get(expression)?.kind {
            ExpressionKind::Boolean(value) => Some(value),
            _ => None,
        }
    }
    pub fn local_ty(&self, local: AccessLocalId) -> Ty {
        self.locals.get(local).expect("own local").access()
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
        self.pending_results |= ty
            .access()
            .is_some_and(|access| super::results::is_result(self.program(), access.data()));
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
            Mode::Update => "edit ",
            Mode::Own => "value ",
        };
        format!("{prefix}{}", self.data_label(ty.data()))
    }
    pub fn data_label(&self, data: TypeId) -> String {
        match self
            .program()
            .data_schema(data)
            .expect("closed data authority")
        {
            PayloadSchema::Builtin(data) => data.spelling().to_owned(),
            PayloadSchema::Record(record) => self
                .program()
                .symbols()
                .spelling(record.name())
                .expect("record name")
                .to_owned(),
            PayloadSchema::Function(witness) => format!(
                "{} witness `{}`",
                super::annotations::invocation_label(witness.invocation),
                self.program()
                    .symbols()
                    .spelling(match witness.origin {
                        nocter_model::ValueFunctionOrigin::Closure(id) => self
                            .program()
                            .callables()
                            .get(id.owner)
                            .expect("closure owner")
                            .name(),
                        nocter_model::ValueFunctionOrigin::Declaration(id)
                        | nocter_model::ValueFunctionOrigin::Result(id) =>
                            self.program().callables().get(id).expect("witness").name(),
                        nocter_model::ValueFunctionOrigin::Application(application) => self
                            .program()
                            .callables()
                            .get(application.callee)
                            .expect("application")
                            .name(),
                        nocter_model::ValueFunctionOrigin::Parameter(id) => self
                            .program()
                            .parameters()
                            .get(id)
                            .expect("formal witness")
                            .name(),
                    })
                    .expect("function name")
            ),
        }
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
        let binding_description = match (declaration.kind, declaration.origin) {
            (nocter_model::ValueBindingKind::FixedValue, _) => "a fixed let value",
            (nocter_model::ValueBindingKind::MutableValue, LocalOrigin::Parameter(_)) => {
                "a value received by take"
            }
            (nocter_model::ValueBindingKind::MutableValue, _) => "a mutable var value",
            (nocter_model::ValueBindingKind::ReadAlias, LocalOrigin::Parameter(_)) => {
                "a read input"
            }
            (nocter_model::ValueBindingKind::ReadAlias, _) => "a read alias",
            (nocter_model::ValueBindingKind::EditAlias, LocalOrigin::Parameter(_)) => {
                "an edit input"
            }
            (nocter_model::ValueBindingKind::EditAlias, _) => "an edit alias",
        };
        let note = DiagnosticNote::new(
            format!(
                "this place is {binding_description} of data type {}",
                self.data_label(declaration.data)
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
    pub fn require_data(
        &self,
        expr: AccessExpressionId,
        data: TypeId,
        node: NodeId,
    ) -> Result<(), Failure> {
        if let Some(ty) = self.ty(expr).access()
            && ty.data() != data
        {
            return self.mismatch(node, Ty::new(data, ty.mode()), ty);
        }
        Ok(())
    }
    pub fn boundary(
        &mut self,
        node: NodeId,
        mut source: AccessExpressionId,
        boundary: Boundary,
    ) -> Result<AccessExpressionId, Failure> {
        let Some(mut actual) = self.ty(source).access() else {
            return Ok(source);
        };
        let expected = boundary.ty();
        if matches!(boundary, Boundary::Result(_))
            && let Some(value) = self.result_boundary(node, source, expected.data())?
        {
            return Ok(value);
        }
        if actual.data() != expected.data()
            && !self
                .program()
                .function_adapter(actual.data(), expected.data())
        {
            if let Some(value) = self.result_assignment(node, source, expected)? {
                source = value;
                actual = self
                    .ty(source)
                    .access()
                    .ok_or(Failure::InconsistentSyntax)?;
            } else {
                return self.mismatch(node, expected, actual);
            }
        } else if actual.data() != expected.data() {
            if expected.mode() != Mode::Own
                || !self
                    .program()
                    .function_adapter(actual.data(), expected.data())
            {
                return self.mismatch(node, expected, actual);
            }
            source = self.boundary(node, source, Boundary::Value(actual.data()))?;
            source = self.emit(
                node,
                ValueType::known(expected).with_continuation(self.ty(source).may_complete()),
                ExpressionKind::AdaptFunction { source },
            )?;
            actual = expected;
        }
        let place = self.place_expression(source);
        let action = match expected.mode() {
            Mode::Read => {
                if place.is_none() {
                    return self.reject(Rule::TypeMismatch, node);
                }
                BoundaryAction::ReadPlace
            }
            Mode::Update => {
                if let Some(local) = place {
                    if !self.writable_place(local) {
                        return self.reject_local(
                            Rule::InvalidUpdate,
                            node,
                            self.selected_place(local).local,
                        );
                    }
                    BoundaryAction::DelegateUpdate
                } else {
                    return self.reject(Rule::InvalidUpdate, node);
                }
            }
            Mode::Own => {
                if let Some(local) = place {
                    use super::supply::PlaceSupply;
                    match super::supply::select(self.program(), actual.data())? {
                        PlaceSupply::Pending => BoundaryAction::PendingValue,
                        PlaceSupply::Copy => BoundaryAction::CopyPayload,
                        PlaceSupply::MissingCopy => {
                            return self.reject_local(
                                Rule::MissingTransfer,
                                node,
                                self.selected_place(local).local,
                            );
                        }
                    }
                } else if actual.mode() == Mode::Own {
                    BoundaryAction::TakeValue
                } else {
                    return Err(Failure::InconsistentSyntax);
                }
            }
        };
        self.emit(
            node,
            ValueType::known(expected).with_continuation(self.ty(source).may_complete()),
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
