use nocter_model::{BorrowCapability, TypeId, TypeKind};
use nocter_syntax::{NodeId, NodeKind};

use super::BodyChecker;
use crate::body_check::error::BodyCheckError;
use crate::syntax::{direct_child, direct_nodes, is_transparent_expression};
use crate::{CheckedOperation, ReadonlyOperandPreparation};

pub(super) struct ReadonlyOperandDraft {
    pub(super) value: nocter_model::BodyNodeId,
    pub(super) ty: TypeId,
    pub(super) owner: TypeId,
    pub(super) preparation: ReadonlyOperandPreparation,
}

impl BodyChecker<'_, '_> {
    /// Checks an expression that a compiler-generated operation observes through `&self`.
    ///
    /// Places remain places instead of becoming implicit copies. Produced values remain ordinary
    /// temporaries, and existing borrow carriers preserve or weaken their capability explicitly.
    pub(super) fn check_readonly_operand(
        &mut self,
        root: NodeId,
        expected: Option<TypeId>,
    ) -> Result<ReadonlyOperandDraft, BodyCheckError> {
        let mut syntax = root;
        while self.kind(syntax).is_ok_and(is_transparent_expression) {
            let children = direct_nodes(self.tree(), syntax);
            let [child] = children.as_slice() else {
                break;
            };
            syntax = *child;
        }
        let constant = self.is_constant_reference(syntax);
        let place = match self.kind(syntax)? {
            _ if constant => None,
            NodeKind::ReferenceExpression => Some(self.named_place(syntax)?),
            NodeKind::PostfixExpression
                if direct_child(self.tree(), syntax, NodeKind::CallSuffix).is_none() =>
            {
                Some(self.postfix_place(syntax, BorrowCapability::Readonly)?)
            }
            _ => None,
        };
        let (value, ty, is_place) = if let Some(place) = place {
            (
                self.add_node(syntax, place.ty, CheckedOperation::Place(place.id))?,
                place.ty,
                true,
            )
        } else {
            let value = self.check_expression(root, expected)?;
            (value, self.node_type(value)?, false)
        };
        let (owner, preparation) = match self.types.get(ty) {
            Some(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent,
            }) => (*referent, ReadonlyOperandPreparation::UseReadonlyBorrow),
            Some(TypeKind::Borrow {
                capability: BorrowCapability::ReadWrite,
                referent,
            }) => (*referent, ReadonlyOperandPreparation::WeakenReadwriteBorrow),
            Some(_) if is_place => (ty, ReadonlyOperandPreparation::BorrowPlace),
            Some(_) => (ty, ReadonlyOperandPreparation::BorrowTemporary),
            None => return Err(crate::BodyCheckInternalError::UnknownType(ty).into()),
        };
        Ok(ReadonlyOperandDraft {
            value,
            ty,
            owner,
            preparation,
        })
    }
}
