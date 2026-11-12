use super::{
    Failure, Rule,
    annotations::Annotation,
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
        input: nocter_declarations::value_access::FunctionInput,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        // A selected contract can complete anonymous inputs/results without turning its
        // formal witness into the concrete value identity expected by ordinary boundaries.
        let hint = self.contextual_type(input.data)?;
        self.expression_with_projection(
            node,
            Some(&hint),
            None,
            Some(super::ir::Projection::input(input.operation.access_mode())),
            reachable,
        )
    }
    pub(super) fn expression_with_context(
        &mut self,
        node: NodeId,
        hint: Option<nocter_model::TypeId>,
        expected: Option<Boundary>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let hint = hint.map(|data| self.contextual_type(data)).transpose()?;
        self.expression_with_hint(node, hint.as_ref(), expected, reachable)
    }
    pub(super) fn expression_with_hint(
        &mut self,
        node: NodeId,
        context: Option<&Annotation>,
        expected: Option<Boundary>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        self.expression_with_projection(
            node,
            context,
            expected,
            expected.map(|boundary| {
                if matches!(boundary, Boundary::Input(_)) {
                    super::ir::Projection::input(boundary.ty().mode())
                } else {
                    super::ir::Projection::place(boundary.ty().mode())
                }
            }),
            reachable,
        )
    }
    pub(super) fn expression_with_projection(
        &mut self,
        node: NodeId,
        context: Option<&Annotation>,
        expected: Option<Boundary>,
        projection: Option<super::ir::Projection>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let data_hint = context.and_then(Annotation::data);
        let hint = data_hint.map(|data| self.outcome_payload(data));
        let node = self.unwrap(node)?;
        let expr = match self.kind(node)? {
            NodeKind::ReferenceExpression => self.reference_value(node)?,
            NodeKind::ScalarLiteral
                if self.token(node)?.kind() == TokenKind::Keyword(Keyword::None) =>
            {
                self.absent(node, data_hint)?
            }
            NodeKind::ScalarLiteral => self.literal(node, false, hint)?,
            NodeKind::OutcomeExpression => self.outcome(node, projection, reachable)?,
            NodeKind::RecoveryExpression => self.recover_optional(node, projection, reachable)?,
            NodeKind::UnitExpression => self.unit(node)?,
            NodeKind::TransferExpression => self.transfer(node, reachable)?,
            NodeKind::ClosureExpression => {
                let contract = match context {
                    Some(Annotation::Function(contract)) => Some(contract),
                    _ => None,
                };
                self.closure(node, contract, reachable)?
            }
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
            NodeKind::PostfixExpression => self.postfix_value(node, projection, reachable)?,
            NodeKind::IfExpression => {
                return self.if_expression(
                    node,
                    expected.or_else(|| data_hint.map(Boundary::Value)),
                    context,
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
            && matches!(
                self.program()
                    .data_kind(data)
                    .and_then(nocter_model::BuiltinNumericType::for_builtin),
                Some(nocter_model::BuiltinNumericType::Integer { signed: false, .. })
            )
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
        hint: Option<&Annotation>,
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
        let branch_expected = if alternative.is_none() {
            Some(Boundary::Binding(Ty::new(
                self.data(BuiltinType::Void),
                Mode::Own,
            )))
        } else {
            expected
        };
        let unit_hint = Annotation::Data(self.data(BuiltinType::Void));
        let branch_hint = if alternative.is_none() {
            Some(&unit_hint)
        } else {
            hint
        };
        let then_block = self.block_with_hint(
            self.direct(node, NodeKind::Block)?,
            branch_expected,
            branch_hint,
            branches_reachable && known_condition != Some(false),
        )?;
        let else_block = if let Some(clause) = alternative {
            let child = self.first_child(clause)?;
            if self.kind(child)? == NodeKind::Block {
                self.block_with_hint(
                    child,
                    expected,
                    hint,
                    branches_reachable && known_condition != Some(true),
                )?
            } else {
                let value = self.if_expression(
                    child,
                    expected,
                    hint,
                    branches_reachable && known_condition != Some(true),
                )?;
                self.emit_block(clause, Vec::new(), Some(value), self.ty(value))?
            }
        } else {
            let mut unit = self.unit(node)?;
            if let Some(boundary) = branch_expected {
                unit = self.boundary(node, unit, boundary)?;
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
        let value = self.emit(
            node,
            ty,
            ExpressionKind::If {
                condition,
                then_block,
                else_block,
            },
        )?;
        if alternative.is_none()
            && let Some(boundary) = expected
        {
            self.boundary(node, value, boundary)
        } else {
            Ok(value)
        }
    }
    pub fn block_ty(&self, id: AccessBlockId) -> ValueType {
        self.blocks.get(id).expect("own block").ty
    }
}
