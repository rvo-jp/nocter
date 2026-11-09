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
        let node = self.unwrap(node)?;
        let expr = match self.kind(node)? {
            NodeKind::ReferenceExpression => self.reference_value(node)?,
            NodeKind::ScalarLiteral => self.literal(node, false)?,
            NodeKind::UnitExpression => self.unit(node)?,
            NodeKind::ClosureExpression => self.closure(node, reachable)?,
            NodeKind::StructLiteral => self.record_value(node, reachable)?,
            NodeKind::UnaryExpression => self.unary(node, reachable)?,
            NodeKind::LogicalOrExpression
            | NodeKind::LogicalAndExpression
            | NodeKind::EqualityExpression
            | NodeKind::OrderingExpression
            | NodeKind::AdditiveExpression
            | NodeKind::MultiplicativeExpression => self.binary(node, reachable)?,
            NodeKind::PostfixExpression => {
                let suffix = *self
                    .children(node)
                    .last()
                    .ok_or(Failure::InconsistentSyntax)?;
                if self.kind(suffix)? == NodeKind::MemberSuffix {
                    self.member_value(node, reachable)?
                } else {
                    self.call(node, reachable)?
                }
            }
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
        let ty = value_type(data, Mode::Own).with_continuation(self.ty(operand).may_complete());
        self.emit(node, ty, ExpressionKind::Unary { operation, operand })
    }
    fn binary(&mut self, node: NodeId, reachable: bool) -> Result<AccessExpressionId, Failure> {
        let children = self.children(node);
        let [left_node, right_node] = children.as_slice() else {
            return Err(Failure::InconsistentSyntax);
        };
        let operation = self.binary_operation(node)?;
        let left = self.expression(*left_node, None, reachable)?;
        let right =
            self.expression(*right_node, None, reachable && self.ty(left).may_complete())?;
        let boolean = self.data(BuiltinType::Bool);
        let integer = self.data(BuiltinType::I32);
        let logical = matches!(operation, Binary::And | Binary::Or);
        let equality = matches!(operation, Binary::Equal | Binary::NotEqual);
        if equality {
            let left_data = self.ty(left).access().map(Ty::data);
            let right_data = self.ty(right).access().map(Ty::data);
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
        let ty = value_type(result_data, Mode::Own).with_continuation(
            self.ty(left).may_complete() && (logical || self.ty(right).may_complete()),
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
        let literal_condition = self.literal_bool(condition);
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
            branches_reachable && literal_condition != Some(false),
        )?;
        let else_block = if let Some(clause) = alternative {
            let child = self.first_child(clause)?;
            if self.kind(child)? == NodeKind::Block {
                self.block(
                    child,
                    expected,
                    branches_reachable && literal_condition != Some(true),
                )?
            } else {
                let value = self.if_expression(
                    child,
                    expected,
                    branches_reachable && literal_condition != Some(true),
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
            && match literal_condition {
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
