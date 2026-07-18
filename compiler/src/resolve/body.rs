use super::builtins::is_builtin_type_name;
use super::diagnostics::{
    builtin_name_reuse_diagnostic, duplicate_visible_name_diagnostic,
    unresolved_identifier_diagnostic,
};
use super::{LocalSymbolId, LocalSymbolKind, Resolver, SymbolId};
use crate::ast::{
    AstFile, Block, Expr, IdentifierExpr, ImplDecl, ImplMember, InterpolatedStringPart, Item,
    Parameter, Stmt,
};
use crate::source::ByteSpan;
use std::collections::HashMap;

impl Resolver<'_> {
    pub(super) fn resolve_callable_bodies(&mut self, ast: &AstFile) {
        for item in &ast.items {
            match item {
                Item::Function(function) => {
                    let mut scope = Scope::new();
                    self.define_parameters(&function.parameters.parameters, &mut scope);
                    self.resolve_block(&function.body, &mut scope);
                }
                Item::Impl(impl_) => self.resolve_impl_bodies(impl_),
                Item::Use(_)
                | Item::Import(_)
                | Item::FromImport(_)
                | Item::Primitive(_)
                | Item::TypeAlias(_)
                | Item::Struct(_)
                | Item::Enum(_)
                | Item::Interface(_) => {}
            }
        }
    }

    fn resolve_impl_bodies(&mut self, impl_: &ImplDecl) {
        for member in &impl_.members {
            match member {
                ImplMember::Method(method) => {
                    let Some(body) = &method.body else {
                        continue;
                    };
                    let mut scope = Scope::new();
                    self.define_local_name(
                        method.receiver.name.clone(),
                        method.receiver.name_span,
                        LocalSymbolKind::Parameter,
                        &mut scope,
                    );
                    self.define_parameters(&method.parameters.parameters, &mut scope);
                    self.resolve_block(body, &mut scope);
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

    fn define_parameters(&mut self, parameters: &[Parameter], scope: &mut Scope) {
        for parameter in parameters {
            self.define_parameter_name(parameter.name.clone(), parameter.name_span, scope);
        }
    }

    fn resolve_block(&mut self, block: &Block, scope: &mut Scope) {
        for statement in &block.statements {
            self.resolve_statement(statement, scope);
        }
    }

    fn resolve_statement(&mut self, statement: &Stmt, scope: &mut Scope) {
        match statement {
            Stmt::Return(statement) => {
                if let Some(expression) = &statement.expression {
                    self.resolve_expression(expression, scope);
                }
            }
            Stmt::Binding(statement) => {
                self.resolve_expression(&statement.initializer, scope);
                if let Some(else_block) = &statement.else_block {
                    let mut else_scope = scope.clone();
                    self.resolve_block(else_block, &mut else_scope);
                }
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
                if let Some(payload) = &statement.payload {
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
            Stmt::IfLet(statement) => {
                self.resolve_expression(&statement.initializer, scope);
                let mut then_scope = scope.clone();
                self.define_local_name(
                    statement.name.clone(),
                    statement.name_span,
                    LocalSymbolKind::Binding(statement.kind),
                    &mut then_scope,
                );
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
                    if let Some(payload) = &arm.payload {
                        self.define_local_name(
                            payload.name.clone(),
                            payload.span,
                            LocalSymbolKind::PatternPayload,
                            &mut arm_scope,
                        );
                    }
                    self.resolve_block(&arm.body, &mut arm_scope);
                }
                if let Some(else_arm) = &statement.else_arm {
                    let mut else_scope = scope.clone();
                    self.resolve_block(&else_arm.body, &mut else_scope);
                }
            }
            Stmt::While(statement) => {
                self.resolve_expression(&statement.condition, scope);
                let mut body_scope = scope.clone();
                self.resolve_block(&statement.body, &mut body_scope);
            }
            Stmt::WhileLet(statement) => {
                self.resolve_expression(&statement.initializer, scope);
                let mut body_scope = scope.clone();
                self.define_local_name(
                    statement.name.clone(),
                    statement.name_span,
                    LocalSymbolKind::Binding(statement.kind),
                    &mut body_scope,
                );
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
            Stmt::Loop(statement) => {
                let mut body_scope = scope.clone();
                self.resolve_block(&statement.body, &mut body_scope);
            }
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

    fn resolve_expression(&mut self, expression: &Expr, scope: &mut Scope) {
        match expression {
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
            Expr::OptionalDefault(expression) => {
                self.resolve_expression(&expression.value, scope);
                self.resolve_expression(&expression.default, scope);
            }
            Expr::PatternConditional(expression) => {
                self.resolve_expression(&expression.target, scope);
                for arm in &expression.arms {
                    let mut arm_scope = scope.clone();
                    if let Some(payload) = &arm.payload {
                        self.define_local_name(
                            payload.name.clone(),
                            payload.span,
                            LocalSymbolKind::PatternPayload,
                            &mut arm_scope,
                        );
                    }
                    self.resolve_expression(&arm.expression, &mut arm_scope);
                }
                self.resolve_expression(&expression.fallback, scope);
            }
            Expr::IntegerLiteral(_)
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

        if let Some(symbol_id) = self.resolve_top_level_name(identifier, scope) {
            self.output
                .identifier_targets
                .insert(identifier.span, symbol_id);
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

        self.output
            .symbols
            .symbol_by_name(&identifier.name)
            .map(|symbol| symbol.id)
    }

    fn define_local_name(
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
            self.output
                .diagnostics
                .push(duplicate_visible_name_diagnostic(
                    self.sources,
                    &name,
                    first_span,
                    span,
                ));
        } else if let Some(symbol) = self.output.symbols.symbol_by_name(&name) {
            self.output
                .diagnostics
                .push(duplicate_visible_name_diagnostic(
                    self.sources,
                    &name,
                    symbol.name_span,
                    span,
                ));
        }

        let id = self.output.define_local_symbol(name.clone(), span, kind);
        scope.define(name, span, id);
    }

    fn define_parameter_name(&mut self, name: String, span: ByteSpan, scope: &mut Scope) {
        if is_builtin_type_name(&name) {
            self.output
                .diagnostics
                .push(builtin_name_reuse_diagnostic(self.sources, &name, span));
        } else if scope.get(&name).is_none() {
            if let Some(symbol) = self.output.symbols.symbol_by_name(&name) {
                self.output
                    .diagnostics
                    .push(duplicate_visible_name_diagnostic(
                        self.sources,
                        &name,
                        symbol.name_span,
                        span,
                    ));
            }
        }

        let id = self
            .output
            .define_local_symbol(name.clone(), span, LocalSymbolKind::Parameter);
        scope.define(name, span, id);
    }
}

#[derive(Debug, Clone, Default)]
struct Scope {
    locals: HashMap<String, LocalBinding>,
}

#[derive(Debug, Clone, Copy)]
struct LocalBinding {
    span: ByteSpan,
    id: LocalSymbolId,
}

impl Scope {
    fn new() -> Self {
        Self::default()
    }

    fn get(&self, name: &str) -> Option<ByteSpan> {
        self.locals.get(name).map(|local| local.span)
    }

    fn resolve(&self, name: &str) -> Option<LocalSymbolId> {
        self.locals.get(name).map(|local| local.id)
    }

    fn define(&mut self, name: String, span: ByteSpan, id: LocalSymbolId) {
        self.locals.entry(name).or_insert(LocalBinding { span, id });
    }
}
