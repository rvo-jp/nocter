use super::{
    BodyDiagnostic, Failure, Origins, Query, Rule, Site,
    ir::{
        Block, Boundary, BoundaryAction, Expression, ExpressionKind, Local, LocalOrigin, Place,
        Statement, TypedBody, ValueType,
    },
};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::{AggregateForm, PayloadSchema, Program};
use nocter_diagnostics::{DiagnosticNote, SourceDiagnostic};
use nocter_model::{
    AccessBlockId, AccessCodeId, AccessExpressionId, AccessLocalId, AccessLoopId, AccessPlaceId,
    AccessStatementId, ArenaBuilder, BuiltinType, FieldPath, TypeId, ValueAccess as Ty,
    ValueAccessMode as Mode,
};
use nocter_syntax::{
    BodySyntaxProjection, BoundSyntax, NodeId, NodeKind, Punctuation, SyntaxOrigin, SyntaxToken,
    TokenKind, child_node_iter, direct_node, direct_token, first_direct_token,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

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
    pub values: &'a dyn super::evaluation::Values,
    pub source: nocter_declaration_lowering::access_declarations::BodySource,
    pub result: TypeId,
    pub types: nocter_declarations::value_access::BodyTypes,
    pub pending_results: bool,
    pub equalities: Vec<super::results::Equality>,
    pub projection: Arc<BodySyntaxProjection>,
    pub origins: Origins,
    pub locals: ArenaBuilder<AccessLocalId, Local>,
    pub expressions: ArenaBuilder<AccessExpressionId, Expression>,
    pub places: ArenaBuilder<AccessPlaceId, Place>,
    pub integer_recipes: BTreeMap<NodeId, bool>,
    pub statics: BTreeMap<nocter_model::ValueStatic, AccessLocalId>,
    pub place_index: BTreeMap<(AccessLocalId, FieldPath), AccessPlaceId>,
    pub statements: ArenaBuilder<AccessStatementId, Statement>,
    pub blocks: ArenaBuilder<AccessBlockId, Block>,
    pub loops: ArenaBuilder<AccessLoopId, ()>,
    pub loop_stack: Vec<LoopFrame>,
    pub scopes: Vec<BTreeMap<String, AccessPlaceId>>,
    pub unavailable: BTreeSet<String>,
    pub required_roots:
        Option<nocter_declaration_lowering::access_declarations::BodyExpressions<'s>>,
}

pub(super) fn prepare(
    context: &DeclarationContext<'_>,
    callable: AccessCodeId,
    values: &dyn super::evaluation::Values,
) -> Result<Query, Failure> {
    let declaration = context
        .program()
        .codes()
        .get(callable)
        .ok_or(Failure::UnknownCallable)?;
    let source = context
        .code_source(callable)
        .ok_or(Failure::UnknownCallable)?;
    let syntax = context.syntax();
    let projection = BodySyntaxProjection::for_root(syntax.tree(), source.body())
        .ok_or(Failure::InconsistentSyntax)?;
    let body_source = context.body_source(callable)?;
    let mut checker = Checker::new(
        context,
        declaration.result(),
        context
            .program()
            .body_types(callable)
            .map_err(|_| Failure::InconsistentSyntax)?,
        Arc::new(projection),
        body_source,
        nocter_model::ValueCode::Declaration(callable),
        values,
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
        checker.bind_local(spelling, local);
    }
    let root = if declaration.kind().is_initializer() {
        let value = checker.expression(
            source.body(),
            Some(Boundary::Result(declaration.result())),
            true,
        )?;
        checker.emit_block(source.body(), Vec::new(), Some(value), checker.ty(value))?
    } else {
        checker.block(
            source.body(),
            Some(Boundary::Result(declaration.result())),
            true,
        )?
    };
    Ok(checker.finish(root))
}

impl<'a, 's> Checker<'a, 's> {
    pub(super) fn new(
        context: &'a DeclarationContext<'s>,
        result: TypeId,
        types: nocter_declarations::value_access::BodyTypes,
        projection: Arc<BodySyntaxProjection>,
        source: nocter_declaration_lowering::access_declarations::BodySource,
        code: nocter_model::ValueCode,
        values: &'a dyn super::evaluation::Values,
    ) -> Self {
        let pending_results = super::results::is_result(types.program(), result);
        Self {
            context,
            values,
            result,
            types,
            projection,
            source,
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
            integer_recipes: BTreeMap::new(),
            statics: BTreeMap::new(),
            place_index: BTreeMap::new(),
            statements: ArenaBuilder::new(),
            blocks: ArenaBuilder::new(),
            loops: ArenaBuilder::new(),
            loop_stack: Vec::new(),
            scopes: vec![BTreeMap::new()],
            unavailable: BTreeSet::new(),
            required_roots: None,
        }
    }
    pub(super) fn finish(self, root: AccessBlockId) -> Query {
        Query {
            code: self.code,
            program: Arc::new(self.types.finish()),
            source: self.source,
            result_data: self.result,
            results_closed: !self.pending_results,
            result_evidence: None,
            evaluation_source: None,
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
    pub fn normal_boolean(&self, expression: AccessExpressionId) -> Option<bool> {
        self.expressions.get(expression)?.normal_boolean()
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
        self.pending_results |= ty.access().is_some_and(|access| {
            super::results::is_result(self.program(), access.data())
                || self.program().payload_readiness(access.data())
                    == nocter_declarations::value_access::PayloadReadiness::Pending
        });
        let site = self.site(SyntaxOrigin::Node(node))?;
        let normal_value = ty.normal().and_then(|access| {
            if self.integer_data(access.data()) {
                self.integer_result(&kind)
                    .map(super::ir::NormalValue::Integer)
            } else if access.data() == self.data(BuiltinType::Bool) {
                self.boolean_result(&kind)
                    .map(super::ir::NormalValue::Boolean)
            } else {
                None
            }
        });
        let id = self.expressions.insert(Expression {
            ty,
            kind,
            normal_value,
        });
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
        self.data_label_with_budget(data, &mut 128)
    }
    fn data_label_with_budget(&self, data: TypeId, remaining: &mut usize) -> String {
        if *remaining == 0 {
            return "…".into();
        }
        *remaining -= 1;
        let Some(schema) = self.program().data_schema(data) else {
            return self
                .program()
                .record_identity(data)
                .and_then(|id| self.program().declaration_label(id.into()))
                .expect("declared data authority");
        };
        match schema {
            PayloadSchema::Optional(payload) => {
                format!("{}?", self.data_label_with_budget(payload, remaining))
            }
            PayloadSchema::Array(array) => format!(
                "[{}; {}]",
                self.data_label_with_budget(array.element(), remaining),
                array.length()
            ),
            PayloadSchema::Builtin(data) => data.spelling().to_owned(),
            PayloadSchema::Aggregate(schema) => match schema.form() {
                AggregateForm::Record(record) => self
                    .program()
                    .symbols()
                    .spelling(record.name())
                    .expect("record name")
                    .to_owned(),
                AggregateForm::Tuple(fields) => {
                    let mut components = Vec::new();
                    for &data in fields {
                        if *remaining == 0 {
                            components.push("…".into());
                            break;
                        }
                        components.push(self.data_label_with_budget(data, remaining));
                    }
                    format!("({})", components.join(", "))
                }
            },
            PayloadSchema::Function(witness) => format!(
                "{} witness `{}`",
                super::annotations::invocation_label(witness.invocation),
                match witness.origin {
                    nocter_model::ValueFunctionOrigin::Closure(id) =>
                        self.program().root_label(id.owner),
                    nocter_model::ValueFunctionOrigin::Declaration(id)
                    | nocter_model::ValueFunctionOrigin::Result(id) =>
                        self.program().code_label(id),
                    nocter_model::ValueFunctionOrigin::Application(application) =>
                        self.program().code_label(application.callee),
                    nocter_model::ValueFunctionOrigin::Parameter(id) => self
                        .program()
                        .parameters()
                        .get(id)
                        .and_then(|parameter| self.program().symbols().spelling(parameter.name()))
                        .map(str::to_owned),
                }
                .expect("closed witness source")
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
                    .code_source(owner)
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
    pub fn require_header(&self, token: SyntaxToken) -> Result<(), Failure> {
        let spelling = self.text(token)?;
        if self.unavailable.contains(spelling) && self.lookup(spelling).is_none() {
            return self.reject_message(
                Rule::RequiredValueScope,
                token,
                "this private required expression cannot capture an enclosing local value".into(),
                Vec::new(),
            );
        }
        if self.lookup(spelling).is_none()
            && let Some(declaration) = self
                .program()
                .symbols()
                .get(spelling)
                .and_then(|symbol| self.program().declaration(symbol))
            && !self.program().header_ready(declaration)
        {
            return Err(Failure::RequiredHeader {
                declaration,
                origin: SyntaxOrigin::Token(token),
            });
        }
        Ok(())
    }
    pub fn lookup(&self, name: &str) -> Option<AccessPlaceId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }
    pub fn bind_local(&mut self, name: String, local: AccessLocalId) {
        let place = self.intern_place(local, FieldPath::default(), self.local_ty(local).data());
        self.scopes
            .last_mut()
            .expect("active scope")
            .insert(name, place);
    }
    pub fn shadows_name(&self, name: &str) -> bool {
        self.lookup(name).is_some()
            || self.unavailable.contains(name)
            || BuiltinType::from_spelling(name).is_some()
            || self
                .program()
                .symbols()
                .get(name)
                .is_some_and(|symbol| self.program().declaration(symbol).is_some())
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
    fn place_boundary(
        &self,
        node: NodeId,
        source: AccessExpressionId,
        expected: Ty,
    ) -> Result<AccessExpressionId, Failure> {
        if !self.ty(source).may_complete() {
            return Ok(source);
        }
        let Some(place) = self.expression_referent(source) else {
            return self.reject_message(
                Rule::TypeMismatch,
                node,
                "this result must designate an existing surviving place".into(),
                Vec::new(),
            );
        };
        if expected.mode() == Mode::Update {
            self.require_writable_referent(place, node, Rule::InvalidUpdate)?;
        }
        Ok(source)
    }
    pub fn boundary(
        &mut self,
        node: NodeId,
        source: AccessExpressionId,
        boundary: Boundary,
    ) -> Result<AccessExpressionId, Failure> {
        if let Boundary::Input(expected) = boundary {
            if self.expression_referent(source).is_some() {
                self.require_data(source, expected.data(), node)?;
                return self.place_boundary(node, source, expected);
            }
            let source = if self
                .ty(source)
                .access()
                .is_some_and(|ty| ty.data() != expected.data())
            {
                self.value_boundary(node, source, Boundary::Value(expected.data()))?
            } else {
                source
            };
            if !self.ty(source).may_complete() {
                return Ok(source);
            }
            if self
                .ty(source)
                .normal()
                .is_none_or(|ty| ty.mode() != Mode::Own)
            {
                return Err(Failure::InconsistentSyntax);
            }
            return self.emit(
                node,
                ValueType::known(Ty::new(expected.data(), Mode::Read)),
                ExpressionKind::Boundary {
                    source,
                    action: BoundaryAction::TemporaryPlace,
                },
            );
        }
        if let Boundary::Place(expected) = boundary {
            self.require_data(source, expected.data(), node)?;
            return self.place_boundary(node, source, expected);
        }
        self.value_boundary(node, source, boundary)
    }
    fn value_boundary(
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
            && expected.mode() == Mode::Own
            && let Some(PayloadSchema::Optional(_)) = self.program().data_schema(expected.data())
        {
            source = self.inject_optional(node, source, expected.data())?;
            actual = self
                .ty(source)
                .access()
                .ok_or(Failure::InconsistentSyntax)?;
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
        let place = self.expression_referent(source);
        let action = if expected.mode() == Mode::Own && !self.ty(source).may_complete() {
            BoundaryAction::Unproduced
        } else {
            match expected.mode() {
                Mode::Read => {
                    if place.is_none() {
                        return self.reject(Rule::TypeMismatch, node);
                    }
                    BoundaryAction::ReadPlace
                }
                Mode::Update => {
                    if let Some(referent) = place {
                        self.require_writable_referent(referent, node, Rule::InvalidUpdate)?;
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
                                if let Some(place) = local.place() {
                                    return self.reject_local(
                                        Rule::MissingTransfer,
                                        node,
                                        self.selected_place(place).local,
                                    );
                                }
                                return self.reject(Rule::MissingTransfer, node);
                            }
                        }
                    } else if actual.mode() == Mode::Own {
                        BoundaryAction::TakeValue
                    } else {
                        return Err(Failure::InconsistentSyntax);
                    }
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
impl Authored for SyntaxOrigin {
    fn origin(self) -> SyntaxOrigin {
        self
    }
}
