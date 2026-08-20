use nocter_model::{BodyNodeId, BuiltinType, TypeId, TypeKind};
use nocter_syntax::{NodeId, NodeKind};

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::syntax::direct_nodes;
use crate::{CheckedBorrowConversion, CheckedOperation, PrimitiveOperation};

impl BodyChecker<'_, '_> {
    /// Checks an authored `value as Target` conversion without using the target to infer `value`.
    ///
    /// The surface construct deliberately has only two meanings: a lossless integer conversion or
    /// one exact borrow conversion. Keeping selection here prevents later expected-type inference
    /// from silently widening that contract.
    pub(super) fn check_conversion(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let children = direct_nodes(self.tree(), node);
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
        } else if lossless_integer_conversion(self.types, source, target) {
            self.add_node(
                node,
                target,
                CheckedOperation::Primitive(PrimitiveOperation::IntegerConversion {
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

fn lossless_integer_conversion(
    types: &nocter_model::TypeStore,
    source: TypeId,
    target: TypeId,
) -> bool {
    let Some(source) = integer_range(types.get(source)) else {
        return false;
    };
    let Some(target) = integer_range(types.get(target)) else {
        return false;
    };
    match (source.signed, target.signed) {
        (false, false) | (true, true) => source.bits <= target.bits,
        (false, true) => source.bits < target.bits,
        (true, false) => false,
    }
}

#[derive(Clone, Copy)]
struct IntegerRange {
    signed: bool,
    bits: u8,
}

fn integer_range(kind: Option<&TypeKind>) -> Option<IntegerRange> {
    let TypeKind::Builtin(builtin) = kind? else {
        return None;
    };
    let (signed, bits) = match builtin {
        BuiltinType::I8 => (true, 8),
        BuiltinType::I16 => (true, 16),
        BuiltinType::I32 => (true, 32),
        BuiltinType::I64 | BuiltinType::Isize => (true, 64),
        BuiltinType::U8 => (false, 8),
        BuiltinType::U16 => (false, 16),
        BuiltinType::U32 => (false, 32),
        BuiltinType::U64 | BuiltinType::Usize => (false, 64),
        _ => return None,
    };
    Some(IntegerRange { signed, bits })
}
