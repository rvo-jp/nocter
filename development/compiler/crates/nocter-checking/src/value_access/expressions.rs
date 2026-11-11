use super::{
    Failure, Rule,
    checker::Checker,
    ir::{Binary, Boundary, ExpressionKind, Unary, ValueType, value_type},
};
use nocter_model::{
    AccessBlockId, AccessExpressionId, BuiltinType, ValueAccess as Ty, ValueAccessMode as Mode,
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
        self.expression_with_context(
            node,
            expected.map(|boundary| boundary.ty().data()),
            expected,
            reachable,
        )
    }
    pub fn input_expression(
        &mut self,
        node: NodeId,
        data: nocter_model::TypeId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        // Code contracts select invocation adapters later; their formal witness is not a
        // concrete expected value identity. Ordinary data can constrain literal construction.
        let hint = (!matches!(
            self.program().data_schema(data),
            Some(nocter_declarations::value_access::PayloadSchema::Function(
                _
            ))
        ))
        .then_some(data);
        self.expression_with_context(node, hint, None, reachable)
    }
    pub(super) fn expression_with_context(
        &mut self,
        node: NodeId,
        hint: Option<nocter_model::TypeId>,
        expected: Option<Boundary>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let node = self.unwrap(node)?;
        let expr = match self.kind(node)? {
            NodeKind::ReferenceExpression => self.reference_value(node)?,
            NodeKind::ScalarLiteral => self.literal(node, false, hint)?,
            NodeKind::UnitExpression => self.unit(node)?,
            NodeKind::TransferExpression => self.transfer(node, reachable)?,
            NodeKind::ClosureExpression => self.closure(node, reachable)?,
            NodeKind::StructLiteral => self.record_value(node, reachable)?,
            NodeKind::TupleExpression => {
                self.tuple_value(node, hint.map(Boundary::Value), reachable)?
            }
            NodeKind::ArrayRepeatLiteral => self.array_repetition(node, hint, reachable)?,
            NodeKind::ArrayLiteral => self.array_value(node, hint, reachable)?,
            NodeKind::UnaryExpression => self.unary(node, hint, reachable)?,
            NodeKind::LogicalOrExpression
            | NodeKind::LogicalAndExpression
            | NodeKind::EqualityExpression
            | NodeKind::OrderingExpression
            | NodeKind::AdditiveExpression
            | NodeKind::MultiplicativeExpression => self.binary(node, hint, reachable)?,
            NodeKind::PostfixExpression => {
                let suffix = *self
                    .children(node)
                    .last()
                    .ok_or(Failure::InconsistentSyntax)?;
                match self.kind(suffix)? {
                    NodeKind::MemberSuffix => self.member_value(node, reachable)?,
                    NodeKind::IndexSuffix => self.indexed_value(node, reachable)?,
                    NodeKind::CallSuffix => self.call(node, reachable)?,
                    _ => return Err(Failure::InconsistentSyntax),
                }
            }
            NodeKind::IfExpression => {
                return self.if_expression(
                    node,
                    expected.or_else(|| hint.map(Boundary::Value)),
                    reachable,
                );
            }
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
    fn literal(
        &mut self,
        node: NodeId,
        negative: bool,
        hint: Option<nocter_model::TypeId>,
    ) -> Result<AccessExpressionId, Failure> {
        let token = self.token(node)?;
        match token.kind() {
            TokenKind::IntegerLiteral => {
                let Some(magnitude) = decode_integer_literal(self.text(token)?) else {
                    return self.reject(Rule::IntegerRange, node);
                };
                let data = self
                    .integer_hint(hint)
                    .unwrap_or(self.data(BuiltinType::I32));
                let builtin = self
                    .program()
                    .data_kind(data)
                    .ok_or(Failure::InconsistentSyntax)?;
                let (minimum, maximum) = nocter_model::BuiltinNumericType::for_builtin(builtin)
                    .and_then(nocter_model::BuiltinNumericType::integer_bounds)
                    .ok_or(Failure::InconsistentSyntax)?;
                let value = i128::from(magnitude) * if negative { -1 } else { 1 };
                if value < minimum || value > maximum {
                    return self.reject(Rule::IntegerRange, node);
                }
                self.emit(
                    node,
                    value_type(data, Mode::Own),
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
    fn unary(
        &mut self,
        node: NodeId,
        hint: Option<nocter_model::TypeId>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
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
            return self.literal(unwrapped, true, hint);
        }
        let operand = self.expression_with_context(
            child,
            if operation == Unary::Not {
                None
            } else {
                self.integer_hint(hint)
            },
            None,
            reachable,
        )?;
        let data = if operation == Unary::Not {
            self.data(BuiltinType::Bool)
        } else {
            self.ty(operand)
                .access()
                .map(Ty::data)
                .filter(|&data| self.integer_data(data))
                .or(self.integer_hint(hint))
                .unwrap_or(self.data(BuiltinType::I32))
        };
        self.require_data(operand, data, child)?;
        let ty = value_type(data, Mode::Own).with_continuation(self.ty(operand).may_complete());
        if operation == Unary::CheckedNegate
            && self.program().data_kind(data) == Some(BuiltinType::Usize)
        {
            // Unsigned negation uses the ordinary checked subtraction kernel, including its trap.
            let zero = self.emit(
                node,
                value_type(data, Mode::Own),
                ExpressionKind::Integer(0),
            )?;
            self.emit(
                node,
                ty,
                ExpressionKind::Binary {
                    operation: Binary::CheckedSubtract,
                    left: zero,
                    right: operand,
                },
            )
        } else {
            self.emit(node, ty, ExpressionKind::Unary { operation, operand })
        }
    }
    fn binary(
        &mut self,
        node: NodeId,
        hint: Option<nocter_model::TypeId>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let children = self.children(node);
        let [left_node, right_node] = children.as_slice() else {
            return Err(Failure::InconsistentSyntax);
        };
        let mut operation = self.binary_operation(node)?;
        let logical = matches!(operation, Binary::And | Binary::Or);
        let arithmetic = matches!(
            operation,
            Binary::CheckedAdd
                | Binary::CheckedSubtract
                | Binary::CheckedMultiply
                | Binary::CheckedDivide
                | Binary::CheckedRemainder
        );
        let hint = arithmetic.then(|| self.integer_hint(hint)).flatten();
        let (left, right, right_demand) =
            self.binary_operands(*left_node, *right_node, operation, hint, reachable)?;
        let boolean = self.data(BuiltinType::Bool);
        let left_data = self.ty(left).access().map(Ty::data);
        let right_data = self.ty(right).access().map(Ty::data);
        let integer = self
            .integer_hint(left_data)
            .or(self.integer_hint(right_data))
            .or(hint)
            .unwrap_or(self.data(BuiltinType::I32));
        if matches!(operation, Binary::Equal | Binary::NotEqual) {
            let valid = |data| {
                data == boolean || self.integer_data(data) || data == self.data(BuiltinType::Void)
            };
            if left_data.is_some_and(|data| !valid(data))
                || right_data.is_some_and(|data| !valid(data))
                || matches!((left_data, right_data), (Some(a), Some(b)) if a != b)
            {
                return self.reject(Rule::TypeMismatch, node);
            }
            if left_data == Some(self.data(BuiltinType::Void)) && right_data == left_data {
                operation = if operation == Binary::Equal {
                    Binary::UnitEqual
                } else {
                    Binary::UnitNotEqual
                };
            }
        } else {
            let expected = if logical { boolean } else { integer };
            self.require_data(left, expected, *left_node)?;
            self.require_data(right, expected, *right_node)?;
        }
        let result_data = if arithmetic { integer } else { boolean };
        let ty = value_type(result_data, Mode::Own).with_continuation(
            self.ty(left).may_complete()
                && (right_demand != super::booleans::Demand::Always
                    || self.ty(right).may_complete()),
        );
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
        let token = nocter_syntax::direct_token_iter(self.syntax().tree(), node)
            .find(|token| matches!(token.kind(), TokenKind::Punctuation(_)))
            .ok_or(Failure::InconsistentSyntax)?;
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
    fn if_expression(
        &mut self,
        node: NodeId,
        expected: Option<Boundary>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let condition_node = self.first_child(self.direct(node, NodeKind::IfCondition)?)?;
        let condition = self.expression(condition_node, None, reachable)?;
        self.require_data(condition, self.data(BuiltinType::Bool), condition_node)?;
        let branches_reachable = reachable && self.ty(condition).may_complete();
        let known_condition = self.normal_boolean(condition);
        // A terminating condition never reaches either branch's outgoing value boundary.
        // Still type-check each branch's own declarations, operations and explicit returns.
        let expected = expected.filter(|_| self.ty(condition).may_complete());
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
            branches_reachable && known_condition != Some(false),
        )?;
        let else_block = if let Some(clause) = alternative {
            let child = self.first_child(clause)?;
            if self.kind(child)? == NodeKind::Block {
                self.block(
                    child,
                    expected,
                    branches_reachable && known_condition != Some(true),
                )?
            } else {
                let value = self.if_expression(
                    child,
                    expected,
                    branches_reachable && known_condition != Some(true),
                )?;
                self.emit_block(clause, Vec::new(), Some(value), self.ty(value))?
            }
        } else {
            let mut unit = self.unit(node)?;
            if let Some(boundary) = branch_expected {
                unit = self.boundary(node, unit, boundary)?;
            }
            // An absent else forces a normally producing then branch to unit too.
            if let Some(ty) = self.block_ty(then_block).normal()
                && ty.data() != self.data(BuiltinType::Void)
            {
                return self.reject(Rule::TypeMismatch, node);
            }
            self.emit_block(node, Vec::new(), Some(unit), self.ty(unit))?
        };
        let (left, right) = (self.block_ty(then_block), self.block_ty(else_block));
        let checked_access =
            match (left.access(), right.access()) {
                (None, other) | (other, None) => other,
                (Some(a), Some(b)) if a == b => Some(a),
                (Some(a), Some(b)) if self.defer_equal(a, b) => Some(a),
                (Some(a), Some(b)) => return self.reject_message(
                    Rule::TypeMismatch,
                    node,
                    format!(
                        "branches supply {} and {}; an inferred conditional needs one data type",
                        self.type_label(a),
                        self.type_label(b)
                    ),
                    Vec::new(),
                ),
            };
        // Check both known shapes above, but only a normally producing branch can supply the
        // continuing value's identity. An interrupted branch has no runtime environment to join.
        let access = match (left.normal(), right.normal()) {
            (Some(access), None) | (None, Some(access)) => Some(access),
            _ => checked_access,
        };
        let completes = self.ty(condition).may_complete()
            && match known_condition {
                Some(true) => left.may_complete(),
                Some(false) => right.may_complete(),
                None => left.may_complete() || right.may_complete(),
            };
        let ty = access
            .map_or(ValueType::NEVER, ValueType::known)
            .with_continuation(completes);
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
