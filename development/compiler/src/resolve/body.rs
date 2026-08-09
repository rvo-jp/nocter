use super::builtins::is_builtin_type_name;
use super::diagnostics::{
    builtin_name_reuse_diagnostic, implicit_closure_capture_diagnostic,
    unqualified_enum_variant_constructor_diagnostic, unresolved_identifier_diagnostic,
};
use super::{LocalSymbolId, LocalSymbolKind, Resolver, SymbolId, SymbolKind, TypeSymbolKind};
use crate::ast::{
    AstFile, Block, Expr, IdentifierExpr, ImplDecl, ImplMember, InterpolatedStringPart, Item,
    MemberExpr, Parameter, ResultProvenanceClause, ResultProvenanceOriginKind, Stmt,
};
use crate::source::ByteSpan;
use std::collections::HashMap;

impl Resolver<'_> {
    pub(super) fn resolve_callable_bodies(&mut self, ast: &AstFile) {
        for item in &ast.items {
            match item {
                Item::Function(function) => {
                    self.resolve_function_body(function);
                }
                Item::Test(test) => {
                    let mut scope = Scope::new();
                    self.resolve_block(&test.body, &mut scope);
                }
                Item::Primitive(primitive) => {
                    let mut scope = Scope::new();
                    self.define_declaration_parameters(
                        &primitive.parameters.parameters,
                        &mut scope,
                    );
                    self.resolve_result_provenance(primitive.result_provenance.as_ref(), &scope);
                }
                Item::Interface(interface) => {
                    for method in &interface.methods {
                        self.resolve_method(method);
                    }
                }
                Item::Construct(construct) => {
                    for (_, function) in construct.functions() {
                        self.resolve_function_body(function);
                    }
                    for (_, literal) in construct.literals() {
                        self.resolve_literal_body(literal);
                    }
                }
                Item::Coerce(coerce) => {
                    for entry in &coerce.entries {
                        let mut scope = Scope::new();
                        self.define_local_name(
                            entry.receiver.name.clone(),
                            entry.receiver.name_span,
                            LocalSymbolKind::Parameter,
                            &mut scope,
                        );
                        self.resolve_result_provenance(entry.result_provenance.as_ref(), &scope);
                        if let Some(body) = &entry.body {
                            self.resolve_block(body, &mut scope);
                        }
                    }
                }
                Item::Impl(impl_) => self.resolve_impl_bodies(impl_),
                Item::Import(_)
                | Item::FromImport(_)
                | Item::TypeAlias(_)
                | Item::Struct(_)
                | Item::Enum(_) => {}
            }
        }
    }

    fn resolve_function_body(&mut self, function: &crate::ast::FunctionDecl) {
        let mut scope = Scope::new();
        if function.body.is_some() {
            self.define_parameters(&function.parameters.parameters, &mut scope);
        } else {
            self.define_declaration_parameters(&function.parameters.parameters, &mut scope);
        }
        self.resolve_result_provenance(function.result_provenance.as_ref(), &scope);
        if let Some(body) = &function.body {
            self.resolve_block(body, &mut scope);
        }
    }

    fn resolve_literal_body(&mut self, literal: &crate::ast::LiteralDecl) {
        let mut scope = Scope::new();
        self.define_parameters(&literal.parameters.parameters, &mut scope);
        if let Some(capture) = &literal.capture {
            self.define_local_name(
                capture.name.clone(),
                capture.name_span,
                LocalSymbolKind::LiteralCapture,
                &mut scope,
            );
        }
        self.resolve_result_provenance(literal.result_provenance.as_ref(), &scope);
        if let Some(body) = &literal.body {
            self.resolve_block(body, &mut scope);
        }
    }

    fn resolve_impl_bodies(&mut self, impl_: &ImplDecl) {
        for member in &impl_.members {
            match member {
                ImplMember::AssociatedType(_) => {}
                ImplMember::Method(method) => {
                    self.resolve_method(method);
                }
                ImplMember::Drop(drop_) => {
                    let mut scope = Scope::new();
                    self.define_local_name(
                        drop_.binding.name.clone(),
                        drop_.binding.name_span,
                        LocalSymbolKind::Parameter,
                        &mut scope,
                    );
                    self.resolve_block(&drop_.body, &mut scope);
                }
            }
        }
    }

    fn resolve_method(&mut self, method: &crate::ast::MethodDecl) {
        let mut scope = Scope::new();
        if method.body.is_some() {
            self.define_local_name(
                method.receiver.name.clone(),
                method.receiver.name_span,
                LocalSymbolKind::Parameter,
                &mut scope,
            );
            self.define_parameters(&method.parameters.parameters, &mut scope);
        } else {
            self.define_declaration_parameter_name(
                method.receiver.name.clone(),
                method.receiver.name_span,
                &mut scope,
            );
            self.define_declaration_parameters(&method.parameters.parameters, &mut scope);
        }
        self.resolve_result_provenance(method.result_provenance.as_ref(), &scope);
        if let Some(body) = &method.body {
            self.resolve_block(body, &mut scope);
        }
    }

    fn define_parameters(&mut self, parameters: &[Parameter], scope: &mut Scope) {
        for parameter in parameters {
            self.define_parameter_name(parameter.name.clone(), parameter.name_span, scope);
        }
    }

    fn define_declaration_parameters(&mut self, parameters: &[Parameter], scope: &mut Scope) {
        for parameter in parameters {
            self.define_declaration_parameter_name(
                parameter.name.clone(),
                parameter.name_span,
                scope,
            );
        }
    }

    /// Declaration-only callables have no executable local scope. Their parameter
    /// names still need symbols for editor navigation, but cannot conflict with
    /// module names because neither name is visible from the other's scope.
    fn define_declaration_parameter_name(
        &mut self,
        name: String,
        span: ByteSpan,
        scope: &mut Scope,
    ) {
        let id = self
            .output
            .define_local_symbol(name.clone(), span, LocalSymbolKind::Parameter);
        scope.define(name, span, id);
    }

    fn resolve_result_provenance(
        &mut self,
        clause: Option<&ResultProvenanceClause>,
        scope: &Scope,
    ) {
        let Some(clause) = clause else {
            return;
        };
        for origin in &clause.origins {
            let name = match &origin.kind {
                ResultProvenanceOriginKind::Receiver => "self",
                ResultProvenanceOriginKind::Parameter(name) => name,
                ResultProvenanceOriginKind::Static => continue,
            };
            if let Some(local_id) = scope.resolve(name) {
                self.output
                    .local_identifier_targets
                    .insert(origin.span, local_id);
            }
        }
    }

    pub(super) fn resolve_block(&mut self, block: &Block, scope: &mut Scope) {
        for statement in &block.statements {
            self.resolve_statement(statement, scope);
        }
        if let Some(result) = &block.result {
            self.resolve_expression(result, scope);
        }
    }

    fn resolve_statement(&mut self, statement: &Stmt, scope: &mut Scope) {
        match statement {
            Stmt::Return(statement) => {
                if let Some(expression) = &statement.expression {
                    self.resolve_expression(expression, scope);
                }
            }
            Stmt::Import(statement) => {
                self.collect_scoped_import_namespace_symbol(statement, scope)
            }
            Stmt::FromImport(statement) => self.collect_scoped_imported_symbols(statement, scope),
            Stmt::Binding(statement) => {
                self.resolve_expression(&statement.initializer, scope);
                self.define_local_name(
                    statement.name.clone(),
                    statement.name_span,
                    LocalSymbolKind::Binding(statement.kind),
                    scope,
                );
            }
            Stmt::Assignment(statement) => {
                self.resolve_expression(&statement.target, scope);
                self.resolve_expression(&statement.value, scope);
            }
            Stmt::If(statement) => {
                self.resolve_expression(&statement.condition, scope);
                let mut then_scope = scope.clone();
                self.resolve_block(&statement.then_block, &mut then_scope);
                if let Some(else_block) = &statement.else_block {
                    let mut else_scope = scope.clone();
                    self.resolve_block(else_block, &mut else_scope);
                }
            }
            Stmt::IfIs(statement) => {
                self.resolve_expression(&statement.expression, scope);
                let mut then_scope = scope.clone();
                if let Some(payload) = statement
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.binding())
                {
                    self.define_local_name(
                        payload.name.clone(),
                        payload.span,
                        LocalSymbolKind::PatternPayload,
                        &mut then_scope,
                    );
                }
                self.resolve_block(&statement.then_block, &mut then_scope);
                if let Some(else_block) = &statement.else_block {
                    let mut else_scope = scope.clone();
                    self.resolve_block(else_block, &mut else_scope);
                }
            }
            Stmt::Switch(statement) => {
                self.resolve_expression(&statement.expression, scope);
                for arm in &statement.arms {
                    let mut arm_scope = scope.clone();
                    if let Some(payload) =
                        arm.payload.as_ref().and_then(|payload| payload.binding())
                    {
                        self.define_local_name(
                            payload.name.clone(),
                            payload.span,
                            LocalSymbolKind::PatternPayload,
                            &mut arm_scope,
                        );
                    }
                    self.resolve_block(&arm.body, &mut arm_scope);
                }
                if let Some(wildcard_arm) = &statement.wildcard_arm {
                    let mut else_scope = scope.clone();
                    self.resolve_block(&wildcard_arm.body, &mut else_scope);
                }
            }
            Stmt::While(statement) => {
                self.resolve_expression(&statement.condition, scope);
                let mut body_scope = scope.clone();
                self.resolve_block(&statement.body, &mut body_scope);
            }
            Stmt::ForRange(statement) => {
                self.resolve_expression(&statement.start, scope);
                self.resolve_expression(&statement.end, scope);
                let mut body_scope = scope.clone();
                self.define_local_name(
                    statement.name.clone(),
                    statement.name_span,
                    LocalSymbolKind::ForRange,
                    &mut body_scope,
                );
                self.resolve_block(&statement.body, &mut body_scope);
            }
            Stmt::CollectionFor(statement) => {
                self.resolve_expression(&statement.source, scope);
                let mut body_scope = scope.clone();
                self.define_local_name(
                    statement.name.clone(),
                    statement.name_span,
                    LocalSymbolKind::CollectionFor,
                    &mut body_scope,
                );
                self.resolve_block(&statement.body, &mut body_scope);
            }
            Stmt::LiteralPackFor(statement) => {
                if let Some(local_id) = scope.resolve(&statement.pack_name) {
                    self.output
                        .local_identifier_targets
                        .insert(statement.pack_span, local_id);
                } else {
                    self.output
                        .diagnostics
                        .push(unresolved_identifier_diagnostic(
                            self.sources,
                            &statement.pack_name,
                            statement.pack_span,
                        ));
                }
                let mut body_scope = scope.clone();
                self.define_local_name(
                    statement.name.clone(),
                    statement.name_span,
                    LocalSymbolKind::LiteralPackFor,
                    &mut body_scope,
                );
                self.resolve_block(&statement.body, &mut body_scope);
            }
            Stmt::Loop(statement) => {
                let mut body_scope = scope.clone();
                self.resolve_block(&statement.body, &mut body_scope);
            }
            Stmt::Region(statement) => self.resolve_region_statement(statement, scope),
            Stmt::Break(_) | Stmt::Continue(_) => {}
            Stmt::Drop(statement) => {
                if let Some(local_id) = scope.resolve(&statement.name) {
                    self.output
                        .local_identifier_targets
                        .insert(statement.name_span, local_id);
                } else {
                    self.output
                        .diagnostics
                        .push(unresolved_identifier_diagnostic(
                            self.sources,
                            &statement.name,
                            statement.name_span,
                        ));
                }
            }
            Stmt::Expression(statement) => self.resolve_expression(&statement.expression, scope),
        }
    }

    pub(super) fn resolve_expression(&mut self, expression: &Expr, scope: &mut Scope) {
        match expression {
            Expr::Closure(expression) => self.resolve_closure(expression, scope),
            Expr::Identifier(expression) => self.resolve_identifier(expression, scope),
            Expr::Propagate(expression) => self.resolve_expression(&expression.expression, scope),
            Expr::Force(expression) => self.resolve_expression(&expression.expression, scope),
            Expr::Catch(expression) => {
                self.resolve_expression(&expression.expression, scope);
                let mut catch_scope = scope.clone();
                self.define_local_name(
                    expression.error_name.clone(),
                    expression.error_span,
                    LocalSymbolKind::CatchError,
                    &mut catch_scope,
                );
                self.resolve_block(&expression.catch_block, &mut catch_scope);
            }
            Expr::Borrow(expression) => self.resolve_expression(&expression.expression, scope),
            Expr::Binary(expression) => {
                self.resolve_expression(&expression.left, scope);
                self.resolve_expression(&expression.right, scope);
            }
            Expr::Unary(expression) => self.resolve_expression(&expression.operand, scope),
            Expr::TypeConversion(expression) => {
                self.resolve_expression(&expression.expression, scope)
            }
            Expr::Call(expression) => {
                self.resolve_expression(&expression.callee, scope);
                if let Expr::Identifier(callee) = expression.callee.as_ref()
                    && let Some(symbol_id) = self.resolve_top_level_name(callee, scope)
                {
                    self.output.call_targets.insert(expression.span, symbol_id);
                } else if let Expr::Member(member) = expression.callee.as_ref()
                    && let Some(symbol_id) = self.resolve_namespace_member_call(member, scope)
                {
                    self.output.call_targets.insert(expression.span, symbol_id);
                }
                for argument in &expression.arguments {
                    self.resolve_expression(argument, scope);
                }
            }
            Expr::Member(expression) => self.resolve_expression(&expression.object, scope),
            Expr::Index(expression) => {
                self.resolve_expression(&expression.object, scope);
                self.resolve_expression(&expression.index, scope);
            }
            Expr::ArrayLiteral(expression) => {
                for element in &expression.elements {
                    self.resolve_expression(element, scope);
                }
            }
            Expr::TypedSequenceLiteral(expression) => {
                self.resolve_typed_literal(
                    &expression.target,
                    crate::ast::LiteralShape::Sequence,
                    expression.span,
                );
                for element in &expression.elements {
                    self.resolve_expression(element, scope);
                }
                if let Some(using) = &expression.using {
                    self.resolve_expression(&using.allocator, scope);
                }
            }
            Expr::TypedStringLiteral(expression) => {
                self.resolve_typed_literal(
                    &expression.target,
                    crate::ast::LiteralShape::String,
                    expression.span,
                );
                if let Some(using) = &expression.using {
                    self.resolve_expression(&using.allocator, scope);
                }
            }
            Expr::StructLiteral(expression) => {
                for field in &expression.fields {
                    self.resolve_expression(&field.value, scope);
                }
            }
            Expr::Group(expression) => self.resolve_expression(&expression.expression, scope),
            Expr::InterpolatedString(expression) => {
                for part in &expression.parts {
                    if let InterpolatedStringPart::Expression(part) = part {
                        self.resolve_expression(&part.expression, scope);
                    }
                }
            }
            Expr::Otherwise(expression) => {
                self.resolve_expression(&expression.value, scope);
                let mut fallback_scope = scope.clone();
                self.resolve_block(&expression.fallback, &mut fallback_scope);
            }
            Expr::If(expression) => {
                self.resolve_expression(&expression.condition, scope);
                let mut then_scope = scope.clone();
                self.resolve_block(&expression.then_block, &mut then_scope);
                if let Some(else_block) = &expression.else_block {
                    let mut else_scope = scope.clone();
                    self.resolve_block(else_block, &mut else_scope);
                }
            }
            Expr::IfIs(expression) => {
                self.resolve_expression(&expression.expression, scope);
                let mut then_scope = scope.clone();
                if let Some(payload) = expression
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.binding())
                {
                    self.define_local_name(
                        payload.name.clone(),
                        payload.span,
                        LocalSymbolKind::PatternPayload,
                        &mut then_scope,
                    );
                }
                self.resolve_block(&expression.then_block, &mut then_scope);
                if let Some(else_block) = &expression.else_block {
                    let mut else_scope = scope.clone();
                    self.resolve_block(else_block, &mut else_scope);
                }
            }
            Expr::Match(expression) => {
                self.resolve_expression(&expression.expression, scope);
                for arm in &expression.arms {
                    let mut arm_scope = scope.clone();
                    if let Some(payload) =
                        arm.payload.as_ref().and_then(|payload| payload.binding())
                    {
                        self.define_local_name(
                            payload.name.clone(),
                            payload.span,
                            LocalSymbolKind::PatternPayload,
                            &mut arm_scope,
                        );
                    }
                    self.resolve_block(&arm.body, &mut arm_scope);
                }
                if let Some(wildcard_arm) = &expression.wildcard_arm {
                    let mut else_scope = scope.clone();
                    self.resolve_block(&wildcard_arm.body, &mut else_scope);
                }
            }
            Expr::IntegerLiteral(_)
            | Expr::ByteLiteral(_)
            | Expr::StringLiteral(_)
            | Expr::BoolLiteral(_)
            | Expr::NoneLiteral(_) => {}
        }
    }

    fn resolve_identifier(&mut self, identifier: &IdentifierExpr, scope: &Scope) {
        if let Some(local_id) = scope.resolve(&identifier.name) {
            self.output
                .local_identifier_targets
                .insert(identifier.span, local_id);
            return;
        }

        if let Some(declaration_span) = scope.blocked_local(&identifier.name) {
            self.output
                .diagnostics
                .push(implicit_closure_capture_diagnostic(
                    self.sources,
                    &identifier.name,
                    identifier.span,
                    declaration_span,
                ));
            return;
        }

        if let Some(symbol_id) = self.resolve_top_level_name(identifier, scope) {
            self.output
                .identifier_targets
                .insert(identifier.span, symbol_id);
            return;
        }

        if let Some((enum_name, variant_span)) = self.unqualified_enum_variant(&identifier.name) {
            self.output
                .diagnostics
                .push(unqualified_enum_variant_constructor_diagnostic(
                    self.sources,
                    &identifier.name,
                    variant_span,
                    &enum_name,
                    identifier.span,
                ));
            return;
        }

        self.output
            .diagnostics
            .push(unresolved_identifier_diagnostic(
                self.sources,
                &identifier.name,
                identifier.span,
            ));
    }

    fn resolve_top_level_name(
        &self,
        identifier: &IdentifierExpr,
        scope: &Scope,
    ) -> Option<SymbolId> {
        if scope.resolve(&identifier.name).is_some() {
            return None;
        }

        if let Some(symbol_id) = scope.resolve_symbol(&identifier.name) {
            return Some(symbol_id);
        }

        self.output
            .symbols
            .symbol_by_name(&identifier.name)
            .map(|symbol| symbol.id)
    }

    fn resolve_namespace_member_call(
        &mut self,
        member: &MemberExpr,
        scope: &Scope,
    ) -> Option<SymbolId> {
        let Expr::Identifier(namespace) = member.object.as_ref() else {
            return None;
        };
        let symbol_id = self.resolve_top_level_name(namespace, scope)?;
        let namespace = match self
            .output
            .symbols
            .get(symbol_id)
            .map(|symbol| &symbol.kind)
        {
            Some(SymbolKind::Imported(namespace)) => namespace.clone(),
            Some(SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Type(_))
            | None => return None,
        };

        self.resolve_namespace_member_symbol(&namespace, &member.member, member.member_span)
    }

    fn unqualified_enum_variant(&self, variant_name: &str) -> Option<(String, ByteSpan)> {
        self.output
            .symbols
            .symbols()
            .find_map(|symbol| match &symbol.kind {
                SymbolKind::Type(type_symbol) if type_symbol.kind == TypeSymbolKind::Enum => {
                    type_symbol
                        .variants
                        .iter()
                        .find(|variant| variant.name == variant_name)
                        .map(|variant| (symbol.name.clone(), variant.name_span))
                }
                _ => None,
            })
    }

    pub(super) fn define_local_name(
        &mut self,
        name: String,
        span: ByteSpan,
        kind: LocalSymbolKind,
        scope: &mut Scope,
    ) {
        if is_builtin_type_name(&name) {
            self.output
                .diagnostics
                .push(builtin_name_reuse_diagnostic(self.sources, &name, span));
        } else if let Some(first_span) = scope.get(&name) {
            let diagnostic = self.duplicate_visible_symbol_diagnostic(&name, first_span, span);
            self.output.diagnostics.push(diagnostic);
        } else if let Some(symbol) = self.output.symbols.symbol_by_name(&name) {
            let diagnostic =
                self.duplicate_visible_symbol_diagnostic(&name, symbol.name_span, span);
            self.output.diagnostics.push(diagnostic);
        }

        let id = self.output.define_local_symbol(name.clone(), span, kind);
        scope.define(name, span, id);
    }

    fn define_parameter_name(&mut self, name: String, span: ByteSpan, scope: &mut Scope) {
        if is_builtin_type_name(&name) {
            self.output
                .diagnostics
                .push(builtin_name_reuse_diagnostic(self.sources, &name, span));
        } else if scope.get(&name).is_none()
            && let Some(symbol) = self.output.symbols.symbol_by_name(&name)
        {
            let diagnostic =
                self.duplicate_visible_symbol_diagnostic(&name, symbol.name_span, span);
            self.output.diagnostics.push(diagnostic);
        }

        let id = self
            .output
            .define_local_symbol(name.clone(), span, LocalSymbolKind::Parameter);
        scope.define(name, span, id);
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct Scope {
    locals: HashMap<String, LocalBinding>,
    symbols: HashMap<String, ScopedSymbolBinding>,
    blocked_locals: HashMap<String, ByteSpan>,
}

#[derive(Debug, Clone, Copy)]
struct LocalBinding {
    span: ByteSpan,
    id: LocalSymbolId,
}

#[derive(Debug, Clone, Copy)]
struct ScopedSymbolBinding {
    span: ByteSpan,
    id: SymbolId,
}

impl Scope {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn get(&self, name: &str) -> Option<ByteSpan> {
        self.locals
            .get(name)
            .map(|local| local.span)
            .or_else(|| self.symbols.get(name).map(|symbol| symbol.span))
    }

    pub(super) fn resolve(&self, name: &str) -> Option<LocalSymbolId> {
        self.locals.get(name).map(|local| local.id)
    }

    pub(super) fn resolve_symbol(&self, name: &str) -> Option<SymbolId> {
        self.symbols.get(name).map(|symbol| symbol.id)
    }

    pub(super) fn blocked_local(&self, name: &str) -> Option<ByteSpan> {
        self.blocked_locals.get(name).copied()
    }

    pub(super) fn without_locals(&self) -> Self {
        let mut blocked_locals = self.blocked_locals.clone();
        blocked_locals.extend(
            self.locals
                .iter()
                .map(|(name, binding)| (name.clone(), binding.span)),
        );
        Self {
            locals: HashMap::new(),
            symbols: self.symbols.clone(),
            blocked_locals,
        }
    }

    pub(super) fn unblock_local(&mut self, name: &str) {
        self.blocked_locals.remove(name);
    }

    pub(super) fn define(&mut self, name: String, span: ByteSpan, id: LocalSymbolId) {
        self.locals.entry(name).or_insert(LocalBinding { span, id });
    }

    pub(super) fn define_symbol(&mut self, name: String, span: ByteSpan, id: SymbolId) {
        self.symbols
            .entry(name)
            .or_insert(ScopedSymbolBinding { span, id });
    }
}
