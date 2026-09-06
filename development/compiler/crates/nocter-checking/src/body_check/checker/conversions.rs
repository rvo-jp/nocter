use nocter_model::{BodyNodeId, BuiltinType, TypeId, TypeKind};
use nocter_syntax::{NodeId, NodeKind};

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::syntax::child_nodes;
use crate::{CheckedBorrowConversion, CheckedOperation, PrimitiveOperation};

impl BodyChecker<'_, '_> {
    /// Checks an authored `value as Target` conversion without using the target to infer `value`.
    ///
    /// The surface construct deliberately has only two meanings: a lossless numeric conversion or
    /// one exact borrow conversion. Keeping selection here prevents later expected-type inference
    /// from silently widening that contract.
    pub(super) fn check_conversion(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let children = child_nodes(self.tree(), node);
        let [operand, target] = children.as_slice() else {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        };
        if self.kind(*target)? != NodeKind::Type {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        }

        let target = self.resolve_data_type_use(*target)?;
        let operand = self.check_expression(*operand, None)?;
        let source = self.node_type(operand)?;
        let value = if source == self.types.builtin(BuiltinType::Never) {
            operand
        } else if lossless_numeric_conversion(self.types, source, target) {
            self.add_node(
                node,
                target,
                CheckedOperation::Primitive(PrimitiveOperation::NumericConversion {
                    operand,
                    target,
                }),
            )?
        } else if let Some((preparation, implementation)) =
            self.select_borrow_conversion(node, source, target)?
        {
            self.add_node(
                node,
                target,
                CheckedOperation::BorrowConversion(CheckedBorrowConversion::new(
                    operand,
                    target,
                    preparation,
                    implementation,
                )),
            )?
        } else {
            return Err(self.rule(BodyRule::TypeMismatch, node)?);
        };

        expected.map_or(Ok(value), |expected| {
            self.apply_expected(node, value, expected)
        })
    }
}

fn lossless_numeric_conversion(
    types: &nocter_model::TypeStore,
    source: TypeId,
    target: TypeId,
) -> bool {
    let Some(TypeKind::Builtin(source)) = types.get(source) else {
        return false;
    };
    let Some(TypeKind::Builtin(target)) = types.get(target) else {
        return false;
    };
    nocter_model::lossless_builtin_numeric_conversion(*source, *target)
}
