use super::{
    Failure, Rule,
    annotations::Annotation,
    checker::{Checker, LoopFrame},
    ir::{Boundary, Local, LocalOrigin, Statement, ValueType},
};
use nocter_model::{
    AccessBlockId, AccessExpressionId, AccessLocalId, AccessStatementId, BuiltinType, TypeId,
    ValueAccess as Ty, ValueAccessMode as Mode, ValueBindingKind,
};
use nocter_syntax::{
    ContextualSpelling, Keyword, NodeId, NodeKind, Punctuation, SyntaxOrigin,
    direct_identifier_iter,
};

mod patterns;

impl Checker<'_, '_> {
    pub fn block(
        &mut self,
        node: NodeId,
        expected: Option<Boundary>,
        reachable: bool,
    ) -> Result<AccessBlockId, Failure> {
        let hint = expected
            .map(|boundary| self.contextual_type(boundary.ty().data()))
            .transpose()?;
        self.block_with_hint(node, expected, hint.as_ref(), reachable)
    }
    pub(super) fn block_with_hint(
        &mut self,
        node: NodeId,
        expected: Option<Boundary>,
        hint: Option<&Annotation>,
        reachable: bool,
    ) -> Result<AccessBlockId, Failure> {
        self.block_with_construction(node, expected, hint, None, reachable)
    }
    pub(super) fn block_with_construction(
        &mut self,
        node: NodeId,
        expected: Option<Boundary>,
        hint: Option<&Annotation>,
        construction: Option<nocter_model::TypeId>,
        reachable: bool,
    ) -> Result<AccessBlockId, Failure> {
        // Construction refines supplied identities; an exact boundary cannot also select it.
        if expected.is_some() && construction.is_some() {
            return Err(Failure::InconsistentSyntax);
        }
        for &name in self.context.block_import_names(node)? {
            let spelling = self.text(name)?;
            if self.lookup(spelling).is_some() || self.unavailable.contains(spelling) {
                return self.reject(Rule::ShadowedName, name);
            }
        }
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
                let mut value = if let Some(data) = construction {
                    self.construction_value(expression, Some(data), reachable && continues)?
                } else {
                    self.expression_with_hint(
                        expression,
                        hint,
                        expected.filter(|_| continues),
                        reachable && continues,
                    )?
                };
                if expected.is_none()
                    && construction.is_none()
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
                let completes =
                    self.check_statement(entry, reachable && continues, &mut statements)?;
                continues &= completes;
            }
        }
        if result.is_none() && continues {
            result = Some(
                self.unit_completion(node, expected.or_else(|| construction.map(Boundary::Value)))?,
            );
        }
        let ty = result
            .map_or(ValueType::NEVER, |value| self.ty(value))
            .with_continuation(continues);
        self.scopes.pop();
        self.emit_block(node, statements, result, ty)
    }

    /// Implicit completion supplies the ordinary unit value through the selected result boundary.
    /// A missing non-unit payload remains a missing-result diagnostic, never implicit absence.
    fn unit_completion(
        &mut self,
        node: NodeId,
        expected: Option<Boundary>,
    ) -> Result<nocter_model::AccessExpressionId, Failure> {
        if let Some(Boundary::Value(data) | Boundary::Result(data)) = expected
            && self.outcome_payload(data) != self.data(BuiltinType::Void)
        {
            return self.reject(Rule::MissingResult, node);
        }
        let value = self.unit(node)?;
        expected.map_or(Ok(value), |boundary| self.boundary(node, value, boundary))
    }

    pub(super) fn allow_return(&self, node: NodeId) -> Result<(), Failure> {
        if matches!(self.code, nocter_model::ValueCode::Expression(_))
            || matches!(self.code, nocter_model::ValueCode::Declaration(code)
                if self.program().codes().get(code).is_some_and(|code| code.kind().is_initializer()))
        {
            return self.reject(Rule::InvalidReturn, node);
        }
        Ok(())
    }

    fn check_statement(
        &mut self,
        node: NodeId,
        reachable: bool,
        statements: &mut Vec<AccessStatementId>,
    ) -> Result<bool, Failure> {
        let (statement, continues) = match self.kind(node)? {
            NodeKind::BindingStatement => return self.binding(node, reachable, statements),
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
                self.allow_return(node)?;
                let expected = self
                    .result
                    .as_ref()
                    .and_then(Annotation::data)
                    .map(Boundary::Result);
                let hint = self.result_hint()?;
                let value = if let Some(expression) = self.children(node).first() {
                    self.expression_with_hint(*expression, hint.as_ref(), expected, reachable)?
                } else {
                    self.unit_completion(node, expected)?
                };
                let value = self.supplied_result(node, value)?;
                (Statement::Return(value), false)
            }
            NodeKind::ThrowStatement => {
                self.allow_return(node)?;
                let child = self.first_child(node)?;
                let error = self.expression(
                    child,
                    Some(Boundary::Value(self.data(BuiltinType::Error))),
                    reachable,
                )?;
                (Statement::Return(self.failure_value(node, error)?), false)
            }
            NodeKind::DropStatement => {
                let token = direct_identifier_iter(self.syntax().tree(), node)
                    .nth(1)
                    .ok_or(Failure::InconsistentSyntax)?;
                let place = self.place_name(token, Rule::InvalidDrop)?;
                let selected = self.selected_place(place).clone();
                if self.place_ty(place).mode() != Mode::Own {
                    return self.reject_local(Rule::InvalidDrop, token, selected.local);
                }
                let statement = if selected.fields.is_root() {
                    Statement::Drop(selected.local)
                } else {
                    // A captured name designates one environment component. Retire that
                    // component through ordinary transfer/discard, never its siblings.
                    Statement::Discard(self.transfer_place(node, place)?)
                };
                (statement, true)
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
        statements.push(self.statement(node, statement)?);
        Ok(continues)
    }

    fn binding(
        &mut self,
        node: NodeId,
        reachable: bool,
        statements: &mut Vec<AccessStatementId>,
    ) -> Result<bool, Failure> {
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
        let name = (!self.has(pattern, Punctuation::LeftParen))
            .then(|| self.token(pattern))
            .transpose()?;
        let annotation = self
            .direct(node, NodeKind::TypeAnnotation)
            .ok()
            .map(|annotation| {
                self.binding_annotation(self.direct(annotation, NodeKind::Type)?, kind)
            })
            .transpose()?;
        let expression = self.direct(node, NodeKind::Expression)?;
        if let Some(name) = name
            && self.text(name)? == "_"
        {
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
            statements.push(self.statement(node, Statement::Discard(value))?);
            return Ok(self.ty(value).may_complete());
        }
        let origin = name.map_or(SyntaxOrigin::Node(pattern), SyntaxOrigin::Token);
        if let Some(name) = name
            && self.shadows_name(self.text(name)?, name)?
        {
            return self.reject(Rule::ShadowedName, name);
        }
        let (value, data) = self.binding_value(
            expression,
            annotation.as_ref(),
            kind,
            origin,
            reachable,
            name.is_none(),
        )?;
        if let Some(name) = name {
            let local = self.binding_local(origin, kind, data)?;
            self.bind_local(self.text(name)?.to_owned(), local);
            statements.push(self.statement(node, Statement::Bind { local, value })?);
        } else {
            self.tuple_binding(node, pattern, kind, data, value, statements)?;
        }
        Ok(self.ty(value).may_complete())
    }

    fn binding_value(
        &mut self,
        expression: NodeId,
        annotation: Option<&Annotation>,
        kind: ValueBindingKind,
        origin: SyntaxOrigin,
        reachable: bool,
        tuple: bool,
    ) -> Result<(AccessExpressionId, TypeId), Failure> {
        let mode = kind.access_mode();
        let alias = matches!(
            kind,
            ValueBindingKind::ReadAlias | ValueBindingKind::EditAlias
        );
        let mut value = if kind == ValueBindingKind::ReadAlias {
            self.alias_value(expression, Rule::TypeMismatch, mode, reachable)?
        } else if kind == ValueBindingKind::EditAlias {
            self.alias_value(expression, Rule::InvalidUpdate, mode, reachable)?
        } else {
            self.expression_with_hint(
                expression,
                annotation,
                annotation
                    .and_then(Annotation::data)
                    .map(|data| Boundary::Binding(Ty::new(data, mode))),
                reachable,
            )?
        };
        let constrained = if let Some(Annotation::Function(expected)) = annotation {
            Some(self.function_pattern_data(expression, value, expected, alias)?)
        } else {
            None
        };
        let annotated_data = annotation.and_then(Annotation::data);
        let data = match (annotated_data.or(constrained), self.ty(value).access()) {
            (Some(data), _) => data,
            (None, Some(ty)) => ty.data(),
            (None, None) => return self.reject(Rule::TypeMismatch, origin),
        };
        if annotated_data.is_none() || alias {
            let expected = Ty::new(data, mode);
            let boundary = if alias && tuple {
                Boundary::Place(expected)
            } else {
                Boundary::Binding(expected)
            };
            value = self.boundary(expression, value, boundary)?;
        }
        Ok((value, data))
    }
    fn binding_local(
        &mut self,
        origin: SyntaxOrigin,
        kind: ValueBindingKind,
        data: TypeId,
    ) -> Result<AccessLocalId, Failure> {
        let local = self.locals.insert(Local {
            data,
            kind,
            origin: LocalOrigin::Binding,
        });
        self.origins.locals.insert(local, self.site(origin)?);
        Ok(local)
    }

    fn writable(&mut self, node: NodeId, reachable: bool) -> Result<super::Referent, Failure> {
        let target = if self.first_array_index(node)?.is_some() {
            let value = self.expression(node, None, reachable)?;
            let Some(target) = self.expression_referent(value) else {
                return self.reject(Rule::NotWritable, node);
            };
            target
        } else {
            self.place(node, Rule::NotWritable)?.into()
        };
        self.require_writable_referent(target, node, Rule::NotWritable)?;
        Ok(target)
    }
    fn assignment(&mut self, node: NodeId, reachable: bool) -> Result<(Statement, bool), Failure> {
        // The target identity is structural; evaluation and flow of the RHS precede replacement.
        let target = self.writable(self.direct(node, NodeKind::AssignmentTarget)?, reachable)?;
        let expression = self.direct(node, NodeKind::Expression)?;
        let target_type = target
            .value_type(|id| self.places.get(id), |id| self.expressions.get(id))
            .ok_or(Failure::InconsistentSyntax)?;
        let data = target_type
            .access()
            .ok_or(Failure::InconsistentSyntax)?
            .data();
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
        Ok((
            statement,
            self.ty(value).may_complete() && target_type.may_complete(),
        ))
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
