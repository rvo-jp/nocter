use nocter_model::BuiltinType;
use nocter_syntax::{NodeId, NodeKind, Punctuation, SyntaxElement, TokenKind};

use super::BodyChecker;
use super::arithmetic::primitive_arithmetic;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::body_check::literal::is_integer_type;
use crate::syntax::direct_nodes;
use crate::{CheckedControl, CheckedOperation};

impl BodyChecker<'_, '_> {
    pub(super) fn check_assignment(
        &mut self,
        statement: NodeId,
    ) -> Result<nocter_model::BodyNodeId, BodyCheckError> {
        let target = self.required_child(statement, NodeKind::AssignmentTarget)?;
        let target_nodes = direct_nodes(self.tree(), target);
        if target_nodes.len() != 1 {
            return Err(BodyCheckInternalError::InvalidSyntax(target).into());
        }
        let target_root = target_nodes[0];
        let operator = self
            .tree()
            .children(statement)
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) if is_assignment_operator(token.kind()) => Some(*token),
                SyntaxElement::Node(_) | SyntaxElement::Token(_) | SyntaxElement::Missing(_) => {
                    None
                }
            })
            .ok_or(BodyCheckInternalError::InvalidSyntax(statement))?;
        let compound = operator.kind() != TokenKind::Punctuation(Punctuation::Equal);
        let target_rule = if compound {
            BodyRule::InvalidCompoundAssignment
        } else {
            BodyRule::InvalidAssignmentTarget
        };
        let place = self.assignment_place(target_root, target, target_rule)?;
        if !self.is_writable_place(place.id)? {
            return Err(self.rule(target_rule, target)?);
        }
        let compound_operation = if compound {
            if !is_integer_type(self.types, place.ty) {
                return Err(self.rule(BodyRule::InvalidCompoundAssignment, target)?);
            }
            Some(
                primitive_arithmetic(operator.kind())
                    .ok_or(BodyCheckInternalError::InvalidSyntax(statement))?,
            )
        } else {
            None
        };
        let expression = self.required_child(statement, NodeKind::Expression)?;
        let value = match self.check_expression(expression, Some(place.ty)) {
            Err(error) if compound && error.rule() == Some(BodyRule::TypeMismatch) => {
                return Err(self.rule(BodyRule::InvalidCompoundAssignment, statement)?);
            }
            result => result?,
        };
        let ty = if self.node_type(value)? == self.types.builtin(BuiltinType::Never) {
            self.types.builtin(BuiltinType::Never)
        } else {
            self.types.builtin(BuiltinType::Void)
        };
        let control = if let Some(operation) = compound_operation {
            CheckedControl::CompoundAssign {
                target: place.id,
                value,
                operation,
            }
        } else {
            CheckedControl::Assign {
                target: place.id,
                value,
            }
        };
        self.add_node(statement, ty, CheckedOperation::Control(control))
    }
}

fn is_assignment_operator(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Punctuation(
            Punctuation::Equal
                | Punctuation::PlusEqual
                | Punctuation::MinusEqual
                | Punctuation::StarEqual
                | Punctuation::SlashEqual
                | Punctuation::PercentEqual
        )
    )
}
