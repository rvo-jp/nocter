use super::{
    Failure, Rule,
    annotations::Annotation,
    checker::{Checker, LoopFrame},
    ir::{Boundary, Local, LocalOrigin, Statement, ValueType},
};
use nocter_model::{
    AccessBlockId, AccessPlaceId, AccessStatementId, BuiltinType, ValueAccess as Ty,
    ValueAccessMode as Mode, ValueBindingKind,
};
use nocter_syntax::{
    ContextualSpelling, Keyword, NodeId, NodeKind, Punctuation, SyntaxOrigin,
    direct_identifier_iter,
};

impl Checker<'_, '_> {
    pub fn block(
        &mut self,
        node: NodeId,
        expected: Option<Boundary>,
        reachable: bool,
    ) -> Result<AccessBlockId, Failure> {
        self.scopes.push(std::collections::BTreeMap::new());
        let entries = if let Ok(sequence) = self.direct(node, NodeKind::ExecutableSequence) {
            self.children(sequence)
        } else {
            Vec::new()
        };
        let mut statements = Vec::new();
        let mut result = None;
        let mut continues = true;
        for entry in entries {
            if self.kind(entry)? == NodeKind::BodyResult {
                let expression = self.first_child(entry)?;
                let mut value = self.expression(
                    expression,
                    expected.filter(|_| continues),
                    reachable && continues,
                )?;
                if expected.is_none()
                    && let Some(ty) = self.ty(value).access()
                {
                    value = self.boundary(
                        expression,
                        value,
                        Boundary::Binding(Ty::new(ty.data(), Mode::Own)),
                    )?;
                }
                if !self.ty(value).may_complete() {
                    continues = false;
                }
                result = Some(value);
            } else {
                let (statement, completes) = self.check_statement(entry, reachable && continues)?;
                statements.push(statement);
                continues &= completes;
            }
        }
        if result.is_none() && continues {
            if let Some(Boundary::Value(ty) | Boundary::Result(ty)) = expected
                && ty != self.data(BuiltinType::Void)
            {
                return self.reject(Rule::MissingResult, node);
            }
            let mut unit = self.unit(node)?;
            if let Some(boundary) = expected {
                unit = self.boundary(node, unit, boundary)?;
            }
            result = Some(unit);
        }
        let ty = result
            .map_or(ValueType::NEVER, |value| self.ty(value))
            .with_continuation(continues);
        self.scopes.pop();
        self.emit_block(node, statements, result, ty)
    }

    fn check_statement(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<(AccessStatementId, bool), Failure> {
        let (statement, continues) = match self.kind(node)? {
            NodeKind::BindingStatement => self.binding(node, reachable)?,
            NodeKind::ExpressionStatement => {
                let expression = self.first_child(node)?;
                let value = self.expression(expression, None, reachable)?;
                if let Some(ty) = self.ty(value).normal()
                    && ty.data() != self.data(BuiltinType::Void)
                {
                    return self.reject(Rule::UnusedValue, node);
                }
                (Statement::Evaluate(value), self.ty(value).may_complete())
            }
            NodeKind::ReturnStatement => {
                if let nocter_model::ValueCode::Declaration(code) = self.code
                    && self
                        .program()
                        .callables()
                        .get(code)
                        .is_some_and(|c| c.kind().is_initializer())
                {
                    return self.reject(Rule::InvalidReturn, node);
                }
                let value = if let Some(expression) = self.children(node).first() {
                    self.expression(*expression, Some(Boundary::Result(self.result)), reachable)?
                } else {
                    if self.result != self.data(BuiltinType::Void) {
                        return self.reject(Rule::MissingResult, node);
                    }
                    let unit = self.unit(node)?;
                    self.boundary(node, unit, Boundary::Result(self.result))?
                };
                (Statement::Return(value), false)
            }
            NodeKind::DropStatement => {
                let token = direct_identifier_iter(self.syntax().tree(), node)
                    .nth(1)
                    .ok_or(Failure::InconsistentSyntax)?;
                let local = self.place_name(token, Rule::InvalidDrop)?;
                if self.local_ty(local).mode() != Mode::Own {
                    return self.reject_local(Rule::InvalidDrop, token, local);
                }
                (Statement::Drop(local), true)
            }
            NodeKind::AssignmentStatement => self.assignment(node, reachable)?,
            NodeKind::BreakStatement | NodeKind::ContinueStatement => {
                let is_break = self.kind(node)? == NodeKind::BreakStatement;
                let Some(frame) = self.loop_stack.last_mut() else {
                    return self.reject(Rule::InvalidLoopControl, node);
                };
                if is_break && reachable {
                    frame.reachable_break = true;
                }
                (
                    if is_break {
                        Statement::Break(frame.id)
                    } else {
                        Statement::Continue(frame.id)
                    },
                    false,
                )
            }
            NodeKind::WhileStatement | NodeKind::LoopStatement => {
                self.loop_statement(node, reachable)?
            }
            _ => return Err(Failure::InconsistentSyntax),
        };
        Ok((self.statement(node, statement)?, continues))
    }

    fn binding(&mut self, node: NodeId, reachable: bool) -> Result<(Statement, bool), Failure> {
        let operation = self.token(self.direct(node, NodeKind::BindingOperation)?)?;
        let kind = match operation.kind() {
            nocter_syntax::TokenKind::Keyword(Keyword::Let) => ValueBindingKind::FixedValue,
            nocter_syntax::TokenKind::Keyword(Keyword::Var) => ValueBindingKind::MutableValue,
            _ => match ContextualSpelling::from_spelling(self.text(operation)?) {
                Some(ContextualSpelling::Read) => ValueBindingKind::ReadAlias,
                Some(ContextualSpelling::Edit) => ValueBindingKind::EditAlias,
                _ => return Err(Failure::InconsistentSyntax),
            },
        };
        let pattern = self.direct(node, NodeKind::BindingPattern)?;
        let name = self.token(pattern)?;
        let annotation = self
            .direct(node, NodeKind::TypeAnnotation)
            .ok()
            .map(|annotation| self.annotation(self.direct(annotation, NodeKind::Type)?))
            .transpose()?;
        let expression = self.direct(node, NodeKind::Expression)?;
        if self.text(name)? == "_" {
            if annotation.is_some() || kind != ValueBindingKind::FixedValue {
                return self.reject(Rule::AnnotatedDiscard, name);
            }
            let mut value = self.expression(expression, None, reachable)?;
            if let Some(ty) = self.ty(value).normal() {
                value = self.boundary(
                    expression,
                    value,
                    Boundary::Binding(Ty::new(ty.data(), Mode::Own)),
                )?;
            }
            return Ok((Statement::Discard(value), self.ty(value).may_complete()));
        }
        let spelling = self.text(name)?.to_owned();
        if self.shadows_name(&spelling) {
            return self.reject(Rule::ShadowedName, name);
        }
        let mode = kind.access_mode();
        let alias = matches!(
            kind,
            ValueBindingKind::ReadAlias | ValueBindingKind::EditAlias
        );
        let mut value = if alias {
            self.place_value(
                expression,
                if kind == ValueBindingKind::EditAlias {
                    Rule::InvalidUpdate
                } else {
                    Rule::TypeMismatch
                },
            )?
        } else {
            self.expression(
                expression,
                annotation
                    .as_ref()
                    .and_then(Annotation::data)
                    .map(|data| Boundary::Binding(Ty::new(data, mode))),
                reachable,
            )?
        };
        let constrained = if let Some(Annotation::Function(expected)) = &annotation {
            Some(self.function_constraint_data(expression, value, expected, alias)?)
        } else {
            None
        };
        let annotated_data = annotation.as_ref().and_then(Annotation::data);
        let data = match (annotated_data.or(constrained), self.ty(value).access()) {
            (Some(data), _) => data,
            (None, Some(ty)) => ty.data(),
            (None, None) => return self.reject(Rule::TypeMismatch, name),
        };
        if annotated_data.is_none() || alias {
            value = self.boundary(expression, value, Boundary::Binding(Ty::new(data, mode)))?;
        }
        let local = self.locals.insert(Local {
            data,
            kind,
            origin: LocalOrigin::Binding,
        });
        self.origins
            .locals
            .insert(local, self.site(SyntaxOrigin::Token(name))?);
        self.scopes
            .last_mut()
            .expect("active block scope")
            .insert(spelling, local);
        Ok((
            Statement::Bind { local, value },
            self.ty(value).may_complete(),
        ))
    }

    fn writable(&mut self, node: NodeId) -> Result<AccessPlaceId, Failure> {
        let local = self.place(node, Rule::NotWritable)?;
        if !self.writable_place(local) {
            return self.reject_local(Rule::NotWritable, node, self.selected_place(local).local);
        }
        Ok(local)
    }
    fn assignment(&mut self, node: NodeId, reachable: bool) -> Result<(Statement, bool), Failure> {
        // The target identity is structural; evaluation and flow of the RHS precede replacement.
        let target = self.writable(self.direct(node, NodeKind::AssignmentTarget)?)?;
        let expression = self.direct(node, NodeKind::Expression)?;
        let data = self.place_ty(target).data();
        let value = self.expression(
            expression,
            Some(Boundary::Binding(Ty::new(data, Mode::Own))),
            reachable,
        )?;
        let statement = if self.has(node, Punctuation::Equal) {
            Statement::Assign { target, value }
        } else {
            if !self.integer_data(data) {
                return self.reject(Rule::CompoundOperand, node);
            }
            Statement::CompoundAssign {
                target,
                operation: self.binary_operation(node)?,
                value,
            }
        };
        Ok((statement, self.ty(value).may_complete()))
    }
    fn loop_statement(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<(Statement, bool), Failure> {
        let is_while = self.kind(node)? == NodeKind::WhileStatement;
        let condition = if is_while {
            let syntax = self.direct(node, NodeKind::Expression)?;
            let condition = self.expression(syntax, None, reachable)?;
            self.require_data(condition, self.data(BuiltinType::Bool), syntax)?;
            Some(condition)
        } else {
            None
        };
        let loop_id = self.loops.insert(());
        self.loop_stack.push(LoopFrame {
            id: loop_id,
            reachable_break: false,
        });
        let body = self.block(
            self.direct(node, NodeKind::Block)?,
            None,
            reachable
                && condition.is_none_or(|expr| {
                    self.ty(expr).may_complete() && self.normal_boolean(expr) != Some(false)
                }),
        )?;
        let frame = self.loop_stack.pop().expect("own loop frame");
        if let Some(ty) = self.block_ty(body).normal()
            && ty.data() != self.data(BuiltinType::Void)
        {
            return self.reject(Rule::UnusedValue, node);
        }
        if let Some(condition) = condition {
            let always_true = self.normal_boolean(condition) == Some(true);
            Ok((
                Statement::While {
                    loop_id,
                    condition,
                    body,
                },
                self.ty(condition).may_complete() && (!always_true || frame.reachable_break),
            ))
        } else {
            Ok((Statement::Loop { loop_id, body }, frame.reachable_break))
        }
    }
}
