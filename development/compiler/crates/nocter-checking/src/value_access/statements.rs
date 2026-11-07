use super::{
    Failure, Rule,
    checker::{Checker, LoopFrame},
    ir::{Boundary, ExpressionKind, Local, LocalOrigin, Statement, ValueType},
};
use nocter_model::{
    AccessBlockId, AccessLocalId, AccessStatementId, BuiltinType, ValueAccessMode as Mode,
    ValueAccessType as Ty,
};
use nocter_syntax::{NodeId, NodeKind, Punctuation, SyntaxOrigin, direct_identifier_iter};

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
                let value = self.expression(
                    self.first_child(entry)?,
                    expected.filter(|_| continues),
                    reachable && continues,
                )?;
                if self.ty(value) == ValueType::Never {
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
            if let Some(Boundary::Return(ty)) = expected
                && ty.data() != self.data(BuiltinType::Void)
            {
                return self.reject(Rule::MissingResult, node);
            }
            let mut unit = self.unit(node)?;
            if let Some(boundary) = expected {
                unit = self.boundary(node, unit, boundary)?;
            }
            result = Some(unit);
        }
        let ty = if continues {
            result.map_or(ValueType::Never, |value| self.ty(value))
        } else {
            ValueType::Never
        };
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
                if let ValueType::Value(ty) = self.ty(value)
                    && ty.data() != self.data(BuiltinType::Void)
                {
                    return self.reject(Rule::UnusedValue, node);
                }
                (
                    Statement::Evaluate(value),
                    self.ty(value) != ValueType::Never,
                )
            }
            NodeKind::ReturnStatement => {
                let value = if let Some(expression) = self.children(node).first() {
                    self.expression(*expression, Some(Boundary::Return(self.result)), reachable)?
                } else {
                    if self.result.data() != self.data(BuiltinType::Void) {
                        return self.reject(Rule::MissingResult, node);
                    }
                    let unit = self.unit(node)?;
                    self.boundary(node, unit, Boundary::Return(self.result))?
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
        let pattern = self.direct(node, NodeKind::BindingPattern)?;
        let name = self.token(pattern)?;
        let annotation = self
            .direct(node, NodeKind::TypeAnnotation)
            .ok()
            .map(|annotation| self.annotation(self.direct(annotation, NodeKind::Type)?))
            .transpose()?;
        let expression = self.direct(node, NodeKind::Expression)?;
        if self.text(name)? == "_" {
            if annotation.is_some() {
                return self.reject(Rule::AnnotatedDiscard, name);
            }
            let value = self.expression(expression, None, reachable)?;
            return Ok((
                Statement::Discard(value),
                self.ty(value) != ValueType::Never,
            ));
        }
        let spelling = self.text(name)?.to_owned();
        if self.lookup(&spelling).is_some()
            || BuiltinType::from_spelling(&spelling).is_some()
            || self
                .program()
                .symbols()
                .get(&spelling)
                .and_then(|symbol| self.program().resolve(symbol))
                .is_some()
        {
            return self.reject(Rule::ShadowedName, name);
        }
        let value = self.expression(expression, annotation.map(Boundary::Binding), reachable)?;
        let ty = match (annotation, self.ty(value)) {
            (Some(ty), _) | (None, ValueType::Value(ty)) => ty,
            (None, ValueType::Never) => return self.reject(Rule::TypeMismatch, name),
        };
        let local = self.locals.insert(Local {
            ty,
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
            self.ty(value) != ValueType::Never,
        ))
    }

    fn writable(&self, node: NodeId) -> Result<AccessLocalId, Failure> {
        let local = self.place(node, Rule::NotWritable)?;
        if self.local_ty(local).mode() == Mode::Read {
            return self.reject_local(Rule::NotWritable, node, local);
        }
        Ok(local)
    }
    fn assignment(&mut self, node: NodeId, reachable: bool) -> Result<(Statement, bool), Failure> {
        // The target identity is structural; evaluation and flow of the RHS precede replacement.
        let target = self.writable(self.direct(node, NodeKind::AssignmentTarget)?)?;
        let expression = self.direct(node, NodeKind::Expression)?;
        let data = self.local_ty(target).data();
        let value = self.expression(
            expression,
            Some(Boundary::Binding(Ty::new(data, Mode::Own))),
            reachable,
        )?;
        let statement = if self.has(node, Punctuation::Equal) {
            Statement::Assign { target, value }
        } else {
            if data != self.data(BuiltinType::I32) {
                return self.reject(Rule::CompoundOperand, node);
            }
            Statement::CompoundAssign {
                target,
                operation: self.binary_operation(node)?,
                value,
            }
        };
        Ok((statement, self.ty(value) != ValueType::Never))
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
            reachable && condition.is_none_or(|expr| self.ty(expr) != ValueType::Never),
        )?;
        let frame = self.loop_stack.pop().expect("own loop frame");
        if let ValueType::Value(ty) = self.block_ty(body)
            && ty.data() != self.data(BuiltinType::Void)
        {
            return self.reject(Rule::UnusedValue, node);
        }
        if let Some(condition) = condition {
            let always_true = matches!(
                self.expressions.get(condition).map(|expr| &expr.kind),
                Some(ExpressionKind::Boolean(true))
            );
            Ok((
                Statement::While {
                    loop_id,
                    condition,
                    body,
                },
                self.ty(condition) != ValueType::Never && (!always_true || frame.reachable_break),
            ))
        } else {
            Ok((Statement::Loop { loop_id, body }, frame.reachable_break))
        }
    }
}
