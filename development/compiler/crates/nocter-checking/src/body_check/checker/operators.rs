use nocter_model::{BorrowCapability, BuiltinType, TypeId, TypeKind};
use nocter_syntax::{NodeId, NodeKind, Punctuation, TokenKind};

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::body_check::literal::{
    contextual_integer_type, fits_negative_integer, is_integer_type, is_signed_integer_type,
    parse_integer,
};
use crate::instance_operations::ComparisonCandidateImplementation;
use crate::syntax::{direct_nodes, direct_token, is_transparent_expression};
use crate::{
    CheckedComparison, CheckedControl, CheckedOperation, CheckedReadonlyOperand,
    ComparisonImplementation, ComparisonOperation, ConstantValue, LogicalOperation,
    PrimitiveBinary, PrimitiveOperation, PrimitiveUnary,
};

impl BodyChecker<'_, '_> {
    pub(super) fn check_unary(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<nocter_model::BodyNodeId, BodyCheckError> {
        let token =
            direct_token(self.tree(), node).ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        match token.kind() {
            TokenKind::Punctuation(Punctuation::Ampersand) => {
                self.check_borrow_unary(node, expected, BorrowCapability::Readonly)
            }
            TokenKind::Punctuation(Punctuation::ReadWrite) => {
                self.check_borrow_unary(node, expected, BorrowCapability::ReadWrite)
            }
            TokenKind::Punctuation(Punctuation::Bang) => self.check_logical_not(node, expected),
            TokenKind::Punctuation(Punctuation::Minus) => self.check_negation(node, expected),
            _ => Err(
                BodyCheckInternalError::UnsupportedSyntax(node, NodeKind::UnaryExpression).into(),
            ),
        }
    }

    fn check_borrow_unary(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
        capability: BorrowCapability,
    ) -> Result<nocter_model::BodyNodeId, BodyCheckError> {
        let operand = unary_operand(self, node)?;
        if self.is_constant_reference(operand) {
            let rule = if capability == BorrowCapability::ReadWrite {
                BodyRule::InvalidReadWriteBorrow
            } else {
                BodyRule::InvalidBorrowSource
            };
            return Err(self.rule(rule, operand)?);
        }
        let place = self.postfix_place(operand, capability)?;
        if capability == BorrowCapability::ReadWrite && !self.is_writable_place(place.id)? {
            return Err(self.rule(BodyRule::InvalidReadWriteBorrow, operand)?);
        }
        let ty = self
            .types
            .intern(TypeKind::Borrow {
                capability,
                referent: place.ty,
            })
            .map_err(|_| BodyCheckInternalError::UnknownType(place.ty))?;
        let checked = self.add_node(
            node,
            ty,
            CheckedOperation::Borrow {
                capability,
                place: place.id,
            },
        )?;
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
    }

    fn check_logical_not(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<nocter_model::BodyNodeId, BodyCheckError> {
        let operand_syntax = unary_operand(self, node)?;
        let operand =
            self.check_expression(operand_syntax, Some(self.types.builtin(BuiltinType::Bool)))?;
        let operand_ty = self.node_type(operand)?;
        let ty = if operand_ty == self.types.builtin(BuiltinType::Never) {
            operand_ty
        } else {
            self.types.builtin(BuiltinType::Bool)
        };
        let checked = self.add_node(
            node,
            ty,
            CheckedOperation::Primitive(PrimitiveOperation::Unary {
                operation: PrimitiveUnary::LogicalNot,
                operand,
            }),
        )?;
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
    }

    fn check_negation(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<nocter_model::BodyNodeId, BodyCheckError> {
        let operand_syntax = unary_operand(self, node)?;
        let contextual = contextual_integer_type(self.types, expected);
        if contextual.is_some_and(|ty| !is_signed_integer_type(self.types, ty)) {
            return Err(self.rule(BodyRule::TypeMismatch, node)?);
        }
        let literal_ty = contextual.unwrap_or_else(|| self.types.builtin(BuiltinType::I32));
        if let Some(token) = direct_integer_literal(self, operand_syntax) {
            let Some(magnitude) = parse_integer(self.token_text(token)?)
                .filter(|magnitude| fits_negative_integer(self.types, literal_ty, *magnitude))
            else {
                return Err(self.rule(BodyRule::IntegerOutOfRange, node)?);
            };
            let checked = self.add_node(
                node,
                literal_ty,
                CheckedOperation::Constant(ConstantValue::Integer(-i128::from(magnitude))),
            )?;
            return expected.map_or(Ok(checked), |expected| {
                self.apply_expected(node, checked, expected)
            });
        }
        let operand = self.check_expression(operand_syntax, contextual)?;
        let operand_ty = self.node_type(operand)?;
        let never = self.types.builtin(BuiltinType::Never);
        if operand_ty != never && !is_signed_integer_type(self.types, operand_ty) {
            return Err(self.rule(BodyRule::TypeMismatch, operand_syntax)?);
        }
        let checked = self.add_node(
            node,
            operand_ty,
            CheckedOperation::Primitive(PrimitiveOperation::Unary {
                operation: PrimitiveUnary::Negate,
                operand,
            }),
        )?;
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
    }

    pub(super) fn check_shift(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<nocter_model::BodyNodeId, BodyCheckError> {
        let [left_syntax, right_syntax] = binary_operands(self, node)?;
        let contextual = contextual_integer_type(self.types, expected);
        let left = self.check_expression(left_syntax, contextual)?;
        let left_ty = self.node_type(left)?;
        let never = self.types.builtin(BuiltinType::Never);
        let operand_ty = if left_ty == never {
            contextual
        } else if is_integer_type(self.types, left_ty) {
            Some(left_ty)
        } else {
            return Err(self.rule(BodyRule::TypeMismatch, left_syntax)?);
        };
        let right = self.check_expression(right_syntax, operand_ty)?;
        let right_ty = self.node_type(right)?;
        if right_ty != never && !is_integer_type(self.types, right_ty) {
            return Err(self.rule(BodyRule::TypeMismatch, right_syntax)?);
        }
        let operation_ty = operand_ty.unwrap_or_else(|| self.types.builtin(BuiltinType::I32));
        let punctuation = operator_punctuation(self, node)?;
        let operation = match punctuation {
            Punctuation::ShiftLeft => PrimitiveBinary::ShiftLeft,
            Punctuation::ShiftRight if is_signed_integer_type(self.types, operation_ty) => {
                PrimitiveBinary::ShiftRightSigned
            }
            Punctuation::ShiftRight => PrimitiveBinary::ShiftRightUnsigned,
            _ => return Err(BodyCheckInternalError::InvalidSyntax(node).into()),
        };
        let ty = if left_ty == never || right_ty == never {
            never
        } else {
            left_ty
        };
        let checked = self.add_node(
            node,
            ty,
            CheckedOperation::Primitive(PrimitiveOperation::Binary {
                operation,
                left,
                right,
            }),
        )?;
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
    }

    pub(super) fn check_comparison(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<nocter_model::BodyNodeId, BodyCheckError> {
        let [left_syntax, right_syntax] = binary_operands(self, node)?;
        let left = self.check_readonly_operand(left_syntax, None)?;
        let right_expected = (is_integer_type(self.types, left.owner)
            && is_contextual_integer_expression(self, right_syntax))
        .then_some(left.owner);
        let right = self.check_readonly_operand(right_syntax, right_expected)?;
        let never = self.types.builtin(BuiltinType::Never);
        let punctuation = operator_punctuation(self, node)?;
        let (operation, reverse, negate) = comparison_derivation(punctuation)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let (implementation, receiver_coercion, argument_coercion) = if left.ty == never
            || right.ty == never
        {
            (ComparisonImplementation::Unreachable, None, None)
        } else {
            let (semantic_left, semantic_right) = if reverse {
                (right.owner, left.owner)
            } else {
                (left.owner, right.owner)
            };
            let candidates = {
                let mut selector = self.instance_selector();
                selector
                    .select_comparison_operations(semantic_left, semantic_right, operation)
                    .map_err(BodyCheckInternalError::from)?
            };
            let mut candidates = candidates.into_iter();
            let Some(selected) = candidates.next() else {
                return Err(self.rule(BodyRule::InvalidComparisonOperation, node)?);
            };
            if candidates.next().is_some() {
                return Err(self.rule(BodyRule::InvalidComparisonOperation, node)?);
            }
            let implementation = match selected.implementation() {
                ComparisonCandidateImplementation::Primitive => ComparisonImplementation::Primitive,
                ComparisonCandidateImplementation::Selected(selection) => {
                    ComparisonImplementation::Selected(selection.clone())
                }
            };
            (
                implementation,
                selected.receiver_coercion().cloned(),
                selected.argument_coercion().cloned(),
            )
        };
        let (left_coercion, right_coercion) = if reverse {
            (argument_coercion, receiver_coercion)
        } else {
            (receiver_coercion, argument_coercion)
        };
        let ty = if left.ty == never || right.ty == never {
            never
        } else {
            self.types.builtin(BuiltinType::Bool)
        };
        let checked = self.add_node(
            node,
            ty,
            CheckedOperation::Comparison(CheckedComparison::new(
                operation,
                CheckedReadonlyOperand::new(left.value, left.preparation, left_coercion),
                CheckedReadonlyOperand::new(right.value, right.preparation, right_coercion),
                implementation,
                reverse,
                negate,
            )),
        )?;
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
    }

    pub(super) fn check_logical(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<nocter_model::BodyNodeId, BodyCheckError> {
        let [left_syntax, right_syntax] = binary_operands(self, node)?;
        let boolean = self.types.builtin(BuiltinType::Bool);
        let left = self.check_expression(left_syntax, Some(boolean))?;
        let right = self.check_expression(right_syntax, Some(boolean))?;
        let left_ty = self.node_type(left)?;
        let operation = match operator_punctuation(self, node)? {
            Punctuation::LogicalAnd => LogicalOperation::And,
            Punctuation::LogicalOr => LogicalOperation::Or,
            _ => return Err(BodyCheckInternalError::InvalidSyntax(node).into()),
        };
        let ty = if left_ty == self.types.builtin(BuiltinType::Never) {
            left_ty
        } else {
            boolean
        };
        let checked = self.add_node(
            node,
            ty,
            CheckedOperation::Control(CheckedControl::Logical {
                operation,
                left,
                right,
            }),
        )?;
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
    }
}

fn unary_operand(
    checker: &BodyChecker<'_, '_>,
    node: NodeId,
) -> Result<NodeId, BodyCheckInternalError> {
    let operands = direct_nodes(checker.tree(), node);
    let [operand] = operands.as_slice() else {
        return Err(BodyCheckInternalError::InvalidSyntax(node));
    };
    Ok(*operand)
}

fn binary_operands(
    checker: &BodyChecker<'_, '_>,
    node: NodeId,
) -> Result<[NodeId; 2], BodyCheckInternalError> {
    direct_nodes(checker.tree(), node)
        .try_into()
        .map_err(|_| BodyCheckInternalError::InvalidSyntax(node))
}

fn operator_punctuation(
    checker: &BodyChecker<'_, '_>,
    node: NodeId,
) -> Result<Punctuation, BodyCheckInternalError> {
    checker
        .tree()
        .children(node)
        .iter()
        .find_map(|element| match element {
            nocter_syntax::SyntaxElement::Token(token) => match token.kind() {
                TokenKind::Punctuation(punctuation) => Some(punctuation),
                _ => None,
            },
            nocter_syntax::SyntaxElement::Node(_) | nocter_syntax::SyntaxElement::Missing(_) => {
                None
            }
        })
        .ok_or(BodyCheckInternalError::InvalidSyntax(node))
}

fn direct_integer_literal(
    checker: &BodyChecker<'_, '_>,
    root: NodeId,
) -> Option<nocter_syntax::SyntaxToken> {
    let mut current = root;
    loop {
        let kind = checker.tree().node(current)?.kind();
        if is_transparent_expression(kind) {
            let children = direct_nodes(checker.tree(), current);
            let [child] = children.as_slice() else {
                return None;
            };
            current = *child;
            continue;
        }
        return (kind == NodeKind::ScalarLiteral)
            .then(|| direct_token(checker.tree(), current))
            .flatten()
            .filter(|token| token.kind() == TokenKind::IntegerLiteral);
    }
}

/// Identifies a built-in integer expression that can adopt a comparison operand's integer type.
///
/// Arithmetic owns exact operand compatibility after receiving the context. This keeps literal
/// inference available without forcing a left-owned type onto heterogeneous source operators.
fn is_contextual_integer_expression(checker: &BodyChecker<'_, '_>, root: NodeId) -> bool {
    let mut current = root;
    loop {
        let Some(kind) = checker
            .tree()
            .node(current)
            .map(nocter_syntax::SyntaxNode::kind)
        else {
            return false;
        };
        if is_transparent_expression(kind) {
            let children = direct_nodes(checker.tree(), current);
            if let [child] = children.as_slice() {
                current = *child;
                continue;
            }
        }
        if matches!(
            kind,
            NodeKind::AdditiveExpression
                | NodeKind::MultiplicativeExpression
                | NodeKind::ShiftExpression
        ) {
            return true;
        }
        if kind == NodeKind::UnaryExpression
            && direct_token(checker.tree(), current)
                .is_some_and(|token| token.kind() == TokenKind::Punctuation(Punctuation::Minus))
        {
            let children = direct_nodes(checker.tree(), current);
            let [operand] = children.as_slice() else {
                return false;
            };
            return direct_integer_literal(checker, *operand).is_some();
        }
        return direct_integer_literal(checker, current).is_some();
    }
}

fn comparison_derivation(punctuation: Punctuation) -> Option<(ComparisonOperation, bool, bool)> {
    match punctuation {
        Punctuation::EqualEqual => Some((ComparisonOperation::Equal, false, false)),
        Punctuation::BangEqual => Some((ComparisonOperation::Equal, false, true)),
        Punctuation::Less => Some((ComparisonOperation::Less, false, false)),
        Punctuation::LessEqual => Some((ComparisonOperation::Less, true, true)),
        Punctuation::Greater => Some((ComparisonOperation::Less, true, false)),
        Punctuation::GreaterEqual => Some((ComparisonOperation::Less, false, true)),
        _ => None,
    }
}
