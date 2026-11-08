use super::{
    Failure, Rule,
    checker::Checker,
    ir::{
        Argument, ArgumentAction, Binary, Boundary, ExpressionKind, Unary, ValueType, value_type,
    },
};
use nocter_model::{
    AccessBlockId, AccessExpressionId, BuiltinType, ValueAccessMode as Mode, ValueAccessType as Ty,
};
use nocter_syntax::{
    Keyword, NodeId, NodeKind, Punctuation as P, TokenKind, decode_integer_literal,
};

impl Checker<'_, '_> {
    pub fn expression(
        &mut self,
        node: NodeId,
        expected: Option<Boundary>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let node = self.unwrap(node)?;
        let expr = match self.kind(node)? {
            NodeKind::ReferenceExpression => {
                let local = self.place(node, Rule::TypeMismatch)?;
                self.emit(
                    node,
                    value_type(self.local_ty(local).data(), Mode::Read),
                    ExpressionKind::Place(local),
                )?
            }
            NodeKind::ScalarLiteral => self.literal(node, false)?,
            NodeKind::UnitExpression => self.unit(node)?,
            NodeKind::UpdateExpression | NodeKind::TransferExpression => {
                let update = self.kind(node)? == NodeKind::UpdateExpression;
                let rule = if update {
                    Rule::InvalidUpdate
                } else {
                    Rule::InvalidTransfer
                };
                let local = self.place(self.first_child(node)?, rule)?;
                let stored = self.local_ty(local);
                if stored.mode() == Mode::Read || (!update && stored.mode() != Mode::Own) {
                    return self.reject_local(rule, node, local);
                }
                self.emit(
                    node,
                    value_type(stored.data(), if update { Mode::Update } else { Mode::Own }),
                    if update {
                        ExpressionKind::Update(local)
                    } else {
                        ExpressionKind::Transfer(local)
                    },
                )?
            }
            NodeKind::UnaryExpression => self.unary(node, reachable)?,
            NodeKind::LogicalOrExpression
            | NodeKind::LogicalAndExpression
            | NodeKind::EqualityExpression
            | NodeKind::OrderingExpression
            | NodeKind::AdditiveExpression
            | NodeKind::MultiplicativeExpression => self.binary(node, reachable)?,
            NodeKind::PostfixExpression => self.call(node, reachable)?,
            NodeKind::IfExpression => return self.if_expression(node, expected, reachable),
            _ => return Err(Failure::InconsistentSyntax),
        };
        if let Some(boundary) = expected {
            self.boundary(node, expr, boundary)
        } else {
            Ok(expr)
        }
    }

    pub fn unit(&mut self, node: NodeId) -> Result<AccessExpressionId, Failure> {
        self.emit(
            node,
            value_type(self.data(BuiltinType::Void), Mode::Own),
            ExpressionKind::Unit,
        )
    }
    fn literal(&mut self, node: NodeId, negative: bool) -> Result<AccessExpressionId, Failure> {
        let token = self.token(node)?;
        match token.kind() {
            TokenKind::IntegerLiteral => {
                let Some(magnitude) = decode_integer_literal(self.text(token)?) else {
                    return self.reject(Rule::IntegerRange, node);
                };
                let signed = i64::try_from(magnitude)
                    .ok()
                    .and_then(|value| i32::try_from(if negative { -value } else { value }).ok());
                let Some(value) = signed else {
                    return self.reject(Rule::IntegerRange, node);
                };
                self.emit(
                    node,
                    value_type(self.data(BuiltinType::I32), Mode::Own),
                    ExpressionKind::Integer(value),
                )
            }
            TokenKind::Keyword(keyword @ (Keyword::True | Keyword::False)) if !negative => self
                .emit(
                    node,
                    value_type(self.data(BuiltinType::Bool), Mode::Own),
                    ExpressionKind::Boolean(keyword == Keyword::True),
                ),
            _ => Err(Failure::InconsistentSyntax),
        }
    }
    fn unary(&mut self, node: NodeId, reachable: bool) -> Result<AccessExpressionId, Failure> {
        let operation = if self.has(node, P::Bang) {
            Unary::Not
        } else {
            Unary::CheckedNegate
        };
        let child = self.first_child(node)?;
        let unwrapped = self.unwrap(child)?;
        if operation == Unary::CheckedNegate
            && self.kind(unwrapped)? == NodeKind::ScalarLiteral
            && self.token(unwrapped)?.kind() == TokenKind::IntegerLiteral
        {
            return self.literal(unwrapped, true);
        }
        let operand = self.expression(child, None, reachable)?;
        let data = self.data(if operation == Unary::Not {
            BuiltinType::Bool
        } else {
            BuiltinType::I32
        });
        self.require_data(operand, data, child)?;
        let ty = if self.ty(operand) == ValueType::Never {
            ValueType::Never
        } else {
            value_type(data, Mode::Own)
        };
        self.emit(node, ty, ExpressionKind::Unary { operation, operand })
    }
    fn binary(&mut self, node: NodeId, reachable: bool) -> Result<AccessExpressionId, Failure> {
        let children = self.children(node);
        let [left_node, right_node] = children.as_slice() else {
            return Err(Failure::InconsistentSyntax);
        };
        let operation = self.binary_operation(node)?;
        let left = self.expression(*left_node, None, reachable)?;
        let right = self.expression(
            *right_node,
            None,
            reachable && self.ty(left) != ValueType::Never,
        )?;
        let boolean = self.data(BuiltinType::Bool);
        let integer = self.data(BuiltinType::I32);
        let logical = matches!(operation, Binary::And | Binary::Or);
        let equality = matches!(operation, Binary::Equal | Binary::NotEqual);
        if equality {
            let left_data = match self.ty(left) {
                ValueType::Value(ty) => Some(ty.data()),
                ValueType::Never => None,
            };
            let right_data = match self.ty(right) {
                ValueType::Value(ty) => Some(ty.data()),
                ValueType::Never => None,
            };
            let valid =
                |data| data == boolean || data == integer || data == self.data(BuiltinType::Void);
            if left_data.is_some_and(|data| !valid(data))
                || right_data.is_some_and(|data| !valid(data))
                || matches!((left_data, right_data), (Some(a), Some(b)) if a != b)
            {
                return self.reject(Rule::TypeMismatch, node);
            }
        } else {
            let expected = if logical { boolean } else { integer };
            self.require_data(left, expected, *left_node)?;
            self.require_data(right, expected, *right_node)?;
        }
        let result_data = if matches!(
            operation,
            Binary::CheckedAdd
                | Binary::CheckedSubtract
                | Binary::CheckedMultiply
                | Binary::CheckedDivide
                | Binary::CheckedRemainder
        ) {
            integer
        } else {
            boolean
        };
        let ty = if self.ty(left) == ValueType::Never
            || (!logical && self.ty(right) == ValueType::Never)
        {
            ValueType::Never
        } else {
            value_type(result_data, Mode::Own)
        };
        self.emit(
            node,
            ty,
            ExpressionKind::Binary {
                operation,
                left,
                right,
            },
        )
    }
    pub fn binary_operation(&self, node: NodeId) -> Result<Binary, Failure> {
        let token = self.token(node)?;
        let TokenKind::Punctuation(punctuation) = token.kind() else {
            return Err(Failure::InconsistentSyntax);
        };
        Ok(match punctuation {
            P::Plus | P::PlusEqual => Binary::CheckedAdd,
            P::Minus | P::MinusEqual => Binary::CheckedSubtract,
            P::Star | P::StarEqual => Binary::CheckedMultiply,
            P::Slash | P::SlashEqual => Binary::CheckedDivide,
            P::Percent | P::PercentEqual => Binary::CheckedRemainder,
            P::EqualEqual => Binary::Equal,
            P::BangEqual => Binary::NotEqual,
            P::Less => Binary::Less,
            P::LessEqual => Binary::LessEqual,
            P::Greater => Binary::Greater,
            P::GreaterEqual => Binary::GreaterEqual,
            P::LogicalAnd => Binary::And,
            P::LogicalOr => Binary::Or,
            _ => return Err(Failure::InconsistentSyntax),
        })
    }
    fn call(&mut self, node: NodeId, reachable: bool) -> Result<AccessExpressionId, Failure> {
        let children = self.children(node);
        let [callee, suffix] = children.as_slice() else {
            return Err(Failure::InconsistentSyntax);
        };
        let Some(name) = self.name_token(*callee)? else {
            return self.reject(Rule::InvalidCall, *callee);
        };
        let text = self.text(name)?;
        if self.lookup(text).is_some() {
            return self.reject(Rule::InvalidCall, name);
        }
        let Some(symbol) = self.program().symbols().get(text) else {
            return self.reject(Rule::UnknownName, name);
        };
        let Some(target) = self.program().resolve(symbol) else {
            return self.reject(Rule::InvalidCall, name);
        };
        let declaration = self
            .program()
            .callables()
            .get(target)
            .ok_or(Failure::InconsistentSyntax)?;
        let expected = declaration
            .parameters()
            .iter()
            .map(|&id| {
                self.program()
                    .parameters()
                    .get(id)
                    .expect("frozen parameter")
                    .ty()
            })
            .collect::<Vec<_>>();
        let result = declaration.result();
        let arguments = self.children(*suffix);
        if arguments.len() != expected.len() {
            return self.reject(Rule::InvalidCall, node);
        }
        let mut plans = Vec::new();
        let mut continues = true;
        for (syntax, expected) in arguments.into_iter().zip(expected) {
            // A call contract selects a preparation action after natural expression typing.
            // It cannot manufacture early branch updates/moves by contextual inference.
            let value = self.expression(syntax, None, reachable && continues)?;
            self.require_data(value, expected.data(), syntax)?;
            let action = if let ValueType::Value(actual) = self.ty(value) {
                if let Some(local) = self.place_expression(value) {
                    match expected.mode() {
                        Mode::Read => ArgumentAction::ReserveRead(local),
                        Mode::Update => {
                            if self.local_ty(local).mode() == Mode::Read {
                                return self.reject_local(Rule::InvalidUpdate, syntax, local);
                            }
                            ArgumentAction::ReserveUpdate(local)
                        }
                        Mode::Own if self.local_ty(local).mode() == Mode::Own => {
                            ArgumentAction::ReserveTransfer(local)
                        }
                        Mode::Own if self.copyable(actual.data()) => ArgumentAction::CopyPayload,
                        Mode::Own => return self.reject(Rule::MissingTransfer, syntax),
                    }
                } else {
                    match (expected.mode(), actual.mode()) {
                        (Mode::Read, Mode::Read) => ArgumentAction::ReadValue,
                        (Mode::Read, Mode::Own) => ArgumentAction::ReadOwnedValue,
                        (Mode::Read, Mode::Update) => ArgumentAction::ReadAccessValue,
                        (Mode::Update, Mode::Update) => ArgumentAction::UpdateValue,
                        (Mode::Own, Mode::Own) => ArgumentAction::TakeValue,
                        (Mode::Own, _) if self.copyable(actual.data()) => {
                            ArgumentAction::CopyPayload
                        }
                        (Mode::Own, _) => return self.reject(Rule::MissingTransfer, syntax),
                        (Mode::Update, _) => return self.reject(Rule::InvalidUpdate, syntax),
                    }
                }
            } else {
                continues = false;
                ArgumentAction::Interrupted
            };
            plans.push(Argument { value, action });
        }
        self.emit(
            node,
            if continues {
                ValueType::Value(result)
            } else {
                ValueType::Never
            },
            ExpressionKind::Call {
                target,
                arguments: plans.into_boxed_slice(),
            },
        )
    }
    fn if_expression(
        &mut self,
        node: NodeId,
        expected: Option<Boundary>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let condition_node = self.first_child(self.direct(node, NodeKind::IfCondition)?)?;
        let condition = self.expression(condition_node, None, reachable)?;
        self.require_data(condition, self.data(BuiltinType::Bool), condition_node)?;
        let branches_reachable = reachable && self.ty(condition) != ValueType::Never;
        // A terminating condition never reaches either branch's outgoing value boundary.
        // Still type-check each branch's own declarations, operations and explicit returns.
        let expected = expected.filter(|_| self.ty(condition) != ValueType::Never);
        let alternative = self.direct(node, NodeKind::ElseClause).ok();
        // Without else the operation completes with unit, not a retained branch access.
        let branch_expected = expected.or_else(|| {
            alternative.is_none().then_some(Boundary::Binding(Ty::new(
                self.data(BuiltinType::Void),
                Mode::Own,
            )))
        });
        let then_block = self.block(
            self.direct(node, NodeKind::Block)?,
            branch_expected,
            branches_reachable,
        )?;
        let else_block = if let Some(clause) = alternative {
            let child = self.first_child(clause)?;
            if self.kind(child)? == NodeKind::Block {
                self.block(child, expected, branches_reachable)?
            } else {
                let value = self.if_expression(child, expected, branches_reachable)?;
                self.emit_block(clause, Vec::new(), Some(value), self.ty(value))?
            }
        } else {
            let mut unit = self.unit(node)?;
            if let Some(boundary) = branch_expected {
                unit = self.boundary(node, unit, boundary)?;
            }
            // An absent else forces a normally producing then branch to unit too.
            if let ValueType::Value(ty) = self.block_ty(then_block)
                && ty.data() != self.data(BuiltinType::Void)
            {
                return self.reject(Rule::TypeMismatch, node);
            }
            self.emit_block(node, Vec::new(), Some(unit), self.ty(unit))?
        };
        let (left, right) = (self.block_ty(then_block), self.block_ty(else_block));
        let ty = if self.ty(condition) == ValueType::Never {
            ValueType::Never
        } else {
            match (left, right) {
                (ValueType::Never, other) | (other, ValueType::Never) => other,
                (ValueType::Value(a), ValueType::Value(b)) if a == b => left,
                (ValueType::Value(a), ValueType::Value(b)) => return self.reject_message(Rule::TypeMismatch, node,
                    format!("branches supply {} and {}; an inferred conditional needs one data type and access mode", self.type_label(a), self.type_label(b)), Vec::new()),
            }
        };
        self.emit(
            node,
            ty,
            ExpressionKind::If {
                condition,
                then_block,
                else_block,
            },
        )
    }
    pub fn block_ty(&self, id: AccessBlockId) -> ValueType {
        self.blocks.get(id).expect("own block").ty
    }
}
