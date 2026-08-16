use nocter_model::{BorrowCapability, BuiltinType};
use nocter_syntax::{NodeId, NodeKind, Punctuation, SyntaxElement, TokenKind};

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::syntax::direct_nodes;
use crate::{CheckedControl, CheckedOperation, LocalBindingKind, PlaceAccess, PlaceRoot};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AssignmentPlaceShape {
    Named,
    Indexed,
    Invalid,
}

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
        match assignment_place_shape(self.tree(), target_root)? {
            AssignmentPlaceShape::Named => {}
            AssignmentPlaceShape::Indexed => {
                return Err(BodyCheckInternalError::UnsupportedSyntax(
                    target,
                    NodeKind::IndexSuffix,
                )
                .into());
            }
            AssignmentPlaceShape::Invalid => {
                return Err(self.rule(BodyRule::InvalidAssignmentTarget, target)?.into());
            }
        }
        let place = self.named_place(target_root)?;
        if !self.is_writable_named_place(place.id)? {
            return Err(self.rule(BodyRule::InvalidAssignmentTarget, target)?.into());
        }
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
        if operator.kind() != TokenKind::Punctuation(Punctuation::Equal) {
            return Err(BodyCheckInternalError::UnsupportedSyntax(
                statement,
                NodeKind::AssignmentStatement,
            )
            .into());
        }
        let expression = self.required_child(statement, NodeKind::Expression)?;
        let value = self.check_expression(expression, Some(place.ty))?;
        let ty = if self.node_type(value)? == self.types.builtin(BuiltinType::Never) {
            self.types.builtin(BuiltinType::Never)
        } else {
            self.types.builtin(BuiltinType::Void)
        };
        self.add_node(
            statement,
            ty,
            CheckedOperation::Control(CheckedControl::Assign {
                target: place.id,
                value,
            }),
        )
    }

    fn is_writable_named_place(
        &self,
        place: nocter_model::PlaceId,
    ) -> Result<bool, BodyCheckInternalError> {
        let place = self
            .builder
            .place(place)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
        match place.access() {
            PlaceAccess::Borrowed(BorrowCapability::Readonly) => Ok(false),
            PlaceAccess::Borrowed(BorrowCapability::ReadWrite) => {
                Ok(!place.projections().is_empty())
            }
            PlaceAccess::Owned => match place.root() {
                PlaceRoot::Local(local) => Ok(self
                    .names
                    .locals()
                    .get(local)
                    .is_some_and(|local| local.kind() == LocalBindingKind::Mutable)),
                PlaceRoot::Parameter(_) | PlaceRoot::Capture(_) => Ok(false),
            },
        }
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

fn assignment_place_shape(
    tree: &nocter_syntax::SyntaxTree,
    node: NodeId,
) -> Result<AssignmentPlaceShape, BodyCheckInternalError> {
    let kind = tree
        .node(node)
        .map(nocter_syntax::SyntaxNode::kind)
        .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
    match kind {
        NodeKind::ReferenceExpression => Ok(AssignmentPlaceShape::Named),
        NodeKind::GroupedExpression
        | NodeKind::Expression
        | NodeKind::LogicalOrExpression
        | NodeKind::LogicalAndExpression
        | NodeKind::EqualityExpression
        | NodeKind::OrderingExpression
        | NodeKind::ShiftExpression
        | NodeKind::AdditiveExpression
        | NodeKind::MultiplicativeExpression
        | NodeKind::ConversionExpression => {
            let children = direct_nodes(tree, node);
            if children.len() != 1 {
                return Ok(AssignmentPlaceShape::Invalid);
            }
            assignment_place_shape(tree, children[0])
        }
        NodeKind::PostfixExpression => {
            let children = direct_nodes(tree, node);
            if children.len() != 2 {
                return Ok(AssignmentPlaceShape::Invalid);
            }
            let base = assignment_place_shape(tree, children[0])?;
            let suffix = tree
                .node(children[1])
                .map(nocter_syntax::SyntaxNode::kind)
                .ok_or(BodyCheckInternalError::InvalidSyntax(children[1]))?;
            Ok(match suffix {
                NodeKind::MemberSuffix => base,
                NodeKind::IndexSuffix if base != AssignmentPlaceShape::Invalid => {
                    AssignmentPlaceShape::Indexed
                }
                _ => AssignmentPlaceShape::Invalid,
            })
        }
        _ => Ok(AssignmentPlaceShape::Invalid),
    }
}
