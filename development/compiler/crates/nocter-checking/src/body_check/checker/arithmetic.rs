use nocter_model::{BuiltinType, TypeId};
use nocter_syntax::{NodeId, Punctuation, SyntaxElement, TokenKind};

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::body_check::literal::{contextual_integer_type, is_integer_type};
use crate::syntax::child_nodes;
use crate::{CheckedOperation, PrimitiveBinary, PrimitiveOperation};

impl BodyChecker<'_, '_> {
    pub(super) fn check_arithmetic(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<nocter_model::BodyNodeId, BodyCheckError> {
        let operands = child_nodes(self.tree(), node);
        if operands.len() != 2 {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        }
        let operation = self
            .tree()
            .children(node)
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) => primitive_arithmetic(token.kind()),
                SyntaxElement::Node(_) | SyntaxElement::Missing(_) => None,
            })
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let contextual = contextual_integer_type(self.types, expected);
        let left = self.check_expression(operands[0], contextual)?;
        let left_ty = self.node_type(left)?;
        let never = self.types.builtin(BuiltinType::Never);
        let operand_ty = if left_ty == never {
            contextual
        } else if is_integer_type(self.types, left_ty) {
            Some(left_ty)
        } else {
            return Err(self.rule(BodyRule::TypeMismatch, operands[0])?);
        };
        let right = self.check_expression(operands[1], operand_ty)?;
        let right_ty = self.node_type(right)?;
        if right_ty != never && !is_integer_type(self.types, right_ty) {
            return Err(self.rule(BodyRule::TypeMismatch, operands[1])?);
        }
        let result_ty = if left_ty == never || right_ty == never {
            never
        } else {
            left_ty
        };
        let checked = self.add_node(
            node,
            result_ty,
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
}

pub(super) fn primitive_arithmetic(kind: TokenKind) -> Option<PrimitiveBinary> {
    match kind {
        TokenKind::Punctuation(Punctuation::Plus | Punctuation::PlusEqual) => {
            Some(PrimitiveBinary::Add)
        }
        TokenKind::Punctuation(Punctuation::Minus | Punctuation::MinusEqual) => {
            Some(PrimitiveBinary::Subtract)
        }
        TokenKind::Punctuation(Punctuation::Star | Punctuation::StarEqual) => {
            Some(PrimitiveBinary::Multiply)
        }
        TokenKind::Punctuation(Punctuation::Slash | Punctuation::SlashEqual) => {
            Some(PrimitiveBinary::Divide)
        }
        TokenKind::Punctuation(Punctuation::Percent | Punctuation::PercentEqual) => {
            Some(PrimitiveBinary::Remainder)
        }
        _ => None,
    }
}
