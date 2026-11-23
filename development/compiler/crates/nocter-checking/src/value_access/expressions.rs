use super::{
    Failure, Rule,
    annotations::Annotation,
    branch_patterns::IfTest,
    checker::Checker,
    ir::{Binary, Boundary, ExpressionKind, Projection, Unary, ValueType, value_type},
    statements::BranchBindings,
};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::{
    AccessBlockId, AccessExpressionId, BuiltinType, ValueAccess as Ty, ValueAccessMode as Mode,
};
use nocter_syntax::{
    Keyword, NodeId, NodeKind, Punctuation as P, SyntaxToken, TokenKind, decode_integer_literal,
};

struct IfBranches {
    reachable: bool,
    known_condition: Option<bool>,
    bindings: Option<BranchBindings>,
    fallback: Option<BranchBindings>,
}

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
        self.expression_with_type_hint(
            node,
            Some(input.data),
            Some(Projection::input(input.operation.access_mode())),
            reachable,
        )
    }
    pub(super) fn expression_with_type_hint(
        &mut self,
        node: NodeId,
        hint: Option<nocter_model::TypeId>,
        projection: Option<Projection>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let hint = hint.map(|data| self.contextual_type(data)).transpose()?;
        self.expression_with_projection(node, hint.as_ref(), None, projection, reachable)
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
            NodeKind::StringExpression => {
                let literal = self.direct(node, NodeKind::StringLiteral)?;
                let text = nocter_syntax::decode_string_literal(self.syntax(), literal)
                    .ok_or(Failure::InconsistentSyntax)?;
                let value = nocter_model::FrozenData::new(vec![nocter_model::FrozenNode::Scalar(
                    nocter_model::ConstantValue::Text(text),
                )])
                .map_err(|_| Failure::InconsistentSyntax)?;
                self.constant_value(node, self.data(BuiltinType::Str), &value)?
            }
            NodeKind::OutcomeExpression => self.outcome(node, projection, reachable)?,
            NodeKind::AwaitExpression => self.await_future(node, reachable)?,
            NodeKind::RecoveryExpression => self.recover_outcome(node, projection, reachable)?,
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
            NodeKind::ConversionExpression => self.conversion(node, reachable)?,
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
                    expected,
                    context,
                    expected.is_none().then_some(data_hint).flatten(),
                    reachable,
                );
            }
            NodeKind::MatchExpression => {
                return self.enum_match(node, expected, context, data_hint, reachable);
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
    fn await_future(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        if !matches!(
            self.code,
            nocter_model::ValueCode::Declaration(declaration)
                if matches!(
                    self.program().code_execution(declaration),
                    Some(nocter_declarations::CallableExecution::Deferred { .. })
                )
        ) {
            return self.reject(Rule::AwaitOutsideAsync, self.token(node)?);
        }
        let mut chain = Vec::new();
        let mut current = node;
        let leaf = loop {
            let operand = self.first_child(current)?;
            chain.push((current, operand));
            current = self.unwrap(operand)?;
            if self.kind(current)? != NodeKind::AwaitExpression {
                break operand;
            }
        };
        let mut future = self.expression(leaf, None, reachable)?;
        for (await_node, operand) in chain.into_iter().rev() {
            let ty = self.ty(future);
            let Some(access) = ty.access() else {
                return Ok(future);
            };
            let Some(PayloadSchema::Future(output)) = self.program().data_schema(access.data())
            else {
                return self.reject(Rule::InvalidAwaitOperand, operand);
            };
            if access.mode() != Mode::Own {
                return self.reject(Rule::InvalidAwaitOperand, operand);
            }
            future = self.emit(
                await_node,
                value_type(output, Mode::Own).with_continuation(ty.may_complete()),
                ExpressionKind::Await { future },
            )?;
        }
        Ok(future)
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
        if operation.is_comparison()
            && let Some(record) = left_data
                .or(right_data)
                .and_then(|data| self.program().nominal_identity(data))
        {
            if matches!((left_data, right_data), (Some(left), Some(right)) if left != right) {
                return self.reject(Rule::TypeMismatch, *right_node);
            }
            return self.nominal_comparison(
                node,
                operation,
                record,
                [(*left_node, left), (*right_node, right)],
            );
        }
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

    pub(super) fn binary_token(&self, node: NodeId) -> Result<SyntaxToken, Failure> {
        nocter_syntax::direct_token_iter(self.syntax().tree(), node)
            .find(|token| matches!(token.kind(), TokenKind::Punctuation(_)))
            .ok_or(Failure::InconsistentSyntax)
    }
    pub fn binary_operation(&self, node: NodeId) -> Result<Binary, Failure> {
        let token = self.binary_token(node)?;
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
        construction: Option<nocter_model::TypeId>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let (test, bindings) =
            self.if_test(self.direct(node, NodeKind::IfCondition)?, reachable)?;
        let fallback = test.fallback_bindings();
        let (source, known_condition) = match test {
            IfTest::Boolean(condition) => (condition, self.normal_boolean(condition)),
            IfTest::Enum { source, .. } => (source, None),
        };
        let test_completes = self.ty(source).may_complete();
        let branches_reachable = reachable && test_completes;
        // A terminating condition never reaches either branch's exact outgoing value boundary.
        // Retain construction shape for dormant declarations/operations without a value receipt.
        let expected = expected.filter(|_| test_completes);
        let alternative = self.direct(node, NodeKind::ElseClause).ok();
        let [then_block, else_block] = if alternative.is_some() {
            self.conditional_blocks(
                node,
                expected,
                hint,
                construction,
                IfBranches {
                    reachable: branches_reachable,
                    known_condition,
                    bindings,
                    fallback,
                },
            )?
        } else {
            // Without else the operation completes with unit, not a retained branch access.
            let unit = self.data(BuiltinType::Void);
            let boundary = Boundary::Binding(Ty::new(unit, Mode::Own));
            let then_block = self.block_with_case(
                self.direct(node, NodeKind::Block)?,
                Some(boundary),
                Some(&Annotation::Data(unit)),
                None,
                branches_reachable && known_condition != Some(false),
                bindings,
            )?;
            let value = self.unit(node)?;
            let value = self.boundary(node, value, boundary)?;
            let entry = self.form_case_bindings(node, fallback)?;
            let else_block = self.emit_block(node, entry, Some(value), self.ty(value))?;
            [then_block, else_block]
        };
        let ty = self.conditional_type(
            node,
            test_completes,
            known_condition,
            then_block,
            else_block,
        )?;
        if construction.is_some()
            && let Some(access) = ty.normal()
        {
            self.supply_conditional_results(node, [then_block, else_block], access)?;
        }
        let kind = match test {
            IfTest::Boolean(condition) => ExpressionKind::If {
                condition,
                then_block,
                else_block,
            },
            IfTest::Enum {
                source,
                operation,
                tag,
            } => ExpressionKind::EnumMatch {
                source,
                operation,
                cases: Box::new([(tag, then_block)]),
                fallback: Some(else_block),
            },
        };
        let value = self.emit(node, ty, kind)?;
        if alternative.is_none()
            && let Some(boundary) = expected
        {
            self.boundary(node, value, boundary)
        } else {
            Ok(value)
        }
    }
    fn conditional_blocks(
        &mut self,
        node: NodeId,
        expected: Option<Boundary>,
        hint: Option<&Annotation>,
        construction: Option<nocter_model::TypeId>,
        branches: IfBranches,
    ) -> Result<[AccessBlockId; 2], Failure> {
        let then_node = self.direct(node, NodeKind::Block)?;
        let clause = self.direct(node, NodeKind::ElseClause)?;
        let then_absent = construction.is_some() && self.absence_construction(then_node)?;
        let else_absent =
            construction.is_some() && self.absence_construction(self.first_child(clause)?)?;
        let first_reachable = branches.reachable && branches.known_condition != Some(false);
        let other_reachable = branches.reachable && branches.known_condition != Some(true);
        let check_first = |this: &mut Self, construction| {
            this.block_with_case(
                then_node,
                expected,
                hint,
                construction,
                first_reachable,
                branches.bindings,
            )
        };
        let check_other = |this: &mut Self, construction| {
            this.conditional_else(
                clause,
                expected,
                hint,
                construction,
                other_reachable,
                branches.fallback,
            )
        };
        let context_from = |this: &Self, block| {
            this.block_ty(block)
                .access()
                .map(Ty::data)
                .filter(|data| this.absence_type(*data).is_some())
                .or(construction)
        };
        if then_absent && !else_absent {
            let other = check_other(self, construction)?;
            let selected = context_from(self, other);
            let first = check_first(self, selected)?;
            return Ok([first, other]);
        }
        let first = check_first(self, construction)?;
        let selected = if else_absent && !then_absent {
            context_from(self, first)
        } else {
            construction
        };
        let other = check_other(self, selected)?;
        Ok([first, other])
    }
    fn conditional_else(
        &mut self,
        clause: NodeId,
        expected: Option<Boundary>,
        hint: Option<&Annotation>,
        construction: Option<nocter_model::TypeId>,
        reachable: bool,
        bindings: Option<BranchBindings>,
    ) -> Result<AccessBlockId, Failure> {
        let child = self.first_child(clause)?;
        if self.kind(child)? == NodeKind::Block {
            self.block_with_case(child, expected, hint, construction, reachable, bindings)
        } else {
            let entry = self.form_case_bindings(clause, bindings)?;
            let value = self.if_expression(child, expected, hint, construction, reachable)?;
            self.emit_block(clause, entry, Some(value), self.ty(value))
        }
    }
    pub(super) fn supply_conditional_results(
        &mut self,
        node: NodeId,
        blocks: impl IntoIterator<Item = AccessBlockId>,
        access: Ty,
    ) -> Result<(), Failure> {
        // Once branch identities agree, give their shared result one explicit receiving schema.
        // Keep each original producing expression and its independent source contract intact.
        for block in blocks {
            let body = self.blocks.get(block).ok_or(Failure::InconsistentSyntax)?;
            if body.ty.normal().is_none_or(|actual| actual == access) {
                continue;
            }
            let source = body.result.ok_or(Failure::InconsistentSyntax)?;
            let value = self.boundary(node, source, Boundary::Binding(access))?;
            let ty = self.ty(value);
            let body = self
                .blocks
                .get_mut(block)
                .ok_or(Failure::InconsistentSyntax)?;
            body.result = Some(value);
            body.ty = ty;
        }
        Ok(())
    }
    pub(super) fn merge_branch_access(
        &mut self,
        node: NodeId,
        left: Option<Ty>,
        right: Option<Ty>,
    ) -> Result<Option<Ty>, Failure> {
        match (left, right) {
            (None, other) | (other, None) => Ok(other),
            (Some(a), Some(b)) if a == b || self.defer_equal(a, b) => Ok(Some(a)),
            (Some(a), Some(b)) => self.reject_message(
                Rule::TypeMismatch,
                node,
                format!(
                    "branches supply {} and {}; an inferred conditional needs one data type",
                    self.type_label(a),
                    self.type_label(b)
                ),
                Vec::new(),
            ),
        }
    }
    fn conditional_type(
        &mut self,
        node: NodeId,
        test_completes: bool,
        known_condition: Option<bool>,
        then_block: AccessBlockId,
        else_block: AccessBlockId,
    ) -> Result<ValueType, Failure> {
        let (left, right) = (self.block_ty(then_block), self.block_ty(else_block));
        let checked_access = self.merge_branch_access(node, left.access(), right.access())?;
        // Check both known shapes above, but only a normally producing branch can supply the
        // continuing value's identity. An interrupted branch has no runtime environment to join.
        let access = match (left.normal(), right.normal()) {
            (Some(access), None) | (None, Some(access)) => Some(access),
            _ => checked_access,
        };
        let access = known_condition
            .and_then(|selected| {
                if selected {
                    left.normal()
                } else {
                    right.normal()
                }
            })
            .or(access);
        let completes = test_completes
            && match known_condition {
                Some(true) => left.may_complete(),
                Some(false) => right.may_complete(),
                None => left.may_complete() || right.may_complete(),
            };
        Ok(access
            .map_or(ValueType::NEVER, ValueType::known)
            .with_continuation(completes))
    }
    pub fn block_ty(&self, id: AccessBlockId) -> ValueType {
        self.blocks.get(id).expect("own block").ty
    }
}
