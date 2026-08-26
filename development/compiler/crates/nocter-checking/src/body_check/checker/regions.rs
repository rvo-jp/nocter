use nocter_model::{BodyNodeId, BuiltinType};
use nocter_syntax::SyntaxOrigin;
use nocter_syntax::{NodeId, NodeKind};

use super::{BlockExpectation, BodyChecker};
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::syntax::{direct_identifier, direct_node};
use crate::{CheckedControl, CheckedOperation};

impl BodyChecker<'_, '_> {
    pub(super) fn check_region(&mut self, statement: NodeId) -> Result<BodyNodeId, BodyCheckError> {
        let token = direct_identifier(self.tree(), statement)
            .ok_or(BodyCheckInternalError::InvalidSyntax(statement))?;
        let binding = self
            .local_declarations
            .get(&SyntaxOrigin::Token(token))
            .copied()
            .ok_or(BodyCheckInternalError::MissingLocalDeclaration(statement))?;
        let context = self.allocation_context_type()?;
        self.builder.define_local(binding, context)?;

        let allocator = direct_node(self.tree(), statement, NodeKind::AllocatorPlace)
            .ok_or(BodyCheckInternalError::InvalidSyntax(statement))?;
        let allocator = self.check_region_parent(allocator)?;
        let block = direct_node(self.tree(), statement, NodeKind::Block)
            .ok_or(BodyCheckInternalError::InvalidSyntax(statement))?;
        let body = self.check_block(
            block,
            BlockExpectation::Value(Some(self.types.builtin(BuiltinType::Void))),
        )?;
        let ty = self.node_type(body)?;
        self.add_node(
            statement,
            ty,
            CheckedOperation::Control(CheckedControl::Region {
                binding,
                allocator,
                body,
            }),
        )
    }
}
