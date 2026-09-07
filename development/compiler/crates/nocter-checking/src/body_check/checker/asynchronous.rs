use nocter_model::{BodyNodeId, BorrowCapability, TypeId, TypeKind};
use nocter_syntax::{NodeId, NodeKind};

use super::BodyChecker;
use crate::CheckedOperation;
use crate::body_check::context::BodyExecution;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::checked::{CheckedAwait, CheckedCallExecution};
use crate::syntax::{child_nodes, direct_node, is_transparent_expression};

impl BodyChecker<'_, '_> {
    pub(super) fn declared_call_execution(
        &self,
        declaration: &nocter_declarations::CallableDeclaration,
        result: TypeId,
    ) -> Result<CheckedCallExecution, BodyCheckInternalError> {
        match declaration.execution() {
            nocter_declarations::CallableExecution::Immediate => {
                Ok(CheckedCallExecution::Immediate)
            }
            nocter_declarations::CallableExecution::Deferred { .. } => {
                let Some(TypeKind::Async(output)) = self.types.get(result) else {
                    return Err(BodyCheckInternalError::UnknownType(result));
                };
                Ok(CheckedCallExecution::Deferred { output: *output })
            }
        }
    }

    pub(super) fn check_await(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        if self.execution != BodyExecution::Deferred {
            return Err(self.rule(BodyRule::AwaitOutsideDeferredBody, node)?);
        }
        let children = child_nodes(self.tree(), node);
        let [operand] = children.as_slice() else {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        };
        let computation = self.consume_await_operand(*operand, node)?;
        let computation_type = self.node_type(computation)?;
        let Some(TypeKind::Async(output)) = self.types.get(computation_type) else {
            return Err(self.rule(BodyRule::InvalidAwaitOperand, node)?);
        };
        let output = *output;
        let checked = self.add_node(
            node,
            output,
            CheckedOperation::Await(CheckedAwait::new(computation)),
        )?;
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
    }

    fn consume_await_operand(
        &mut self,
        operand: NodeId,
        await_node: NodeId,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let mut syntax = operand;
        while self.kind(syntax).is_ok_and(is_transparent_expression) {
            let children = child_nodes(self.tree(), syntax);
            let [child] = children.as_slice() else {
                break;
            };
            syntax = *child;
        }

        let place = match self.kind(syntax)? {
            NodeKind::ReferenceExpression if !self.is_constant_reference(syntax) => {
                Some(self.named_place(syntax)?)
            }
            NodeKind::PostfixExpression
                if direct_node(self.tree(), syntax, NodeKind::CallSuffix).is_none()
                    && !self.is_constant_reference(syntax) =>
            {
                Some(self.postfix_place(syntax, BorrowCapability::Readonly)?)
            }
            _ => None,
        };
        let Some(place) = place else {
            return self.check_expression(syntax, None);
        };
        if self.is_region_place(place.id)? {
            return Err(self.rule(BodyRule::InvalidAwaitOperand, await_node)?);
        }
        self.consume_owned_place(
            await_node,
            place,
            BodyRule::InvalidAwaitOperand,
            BodyRule::InvalidAwaitOperand,
        )
    }
}
