use nocter_constant_evaluation::{FloatBits, FloatFormat, TargetFloatEvaluator};
use nocter_model::{BodyNodeId, BuiltinType, ConstantValue, TypeId};
use nocter_syntax::{FloatLiteralSpelling, NodeId, SyntaxToken};

use super::BodyChecker;
use crate::CheckedOperation;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::BodyCheckError;
use crate::body_check::literal::contextual_float_type;

impl BodyChecker<'_, '_> {
    pub(super) fn check_float_literal(
        &mut self,
        node: NodeId,
        token: SyntaxToken,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let authored = self.token_text(token)?;
        let spelling = FloatLiteralSpelling::from_authored(authored);
        let suffix = spelling.suffix().map(FloatFormat::from);
        let contextual = contextual_float_type(self.types, expected);
        if suffix.is_some() && contextual.is_some_and(|(_, contextual)| Some(contextual) != suffix)
        {
            return Err(self.rule(BodyRule::TypeMismatch, node)?);
        }
        let format = suffix
            .or_else(|| contextual.map(|(_, format)| format))
            .unwrap_or(FloatFormat::Binary64);
        let ty = match format {
            FloatFormat::Binary32 => self.types.builtin(BuiltinType::F32),
            FloatFormat::Binary64 => self.types.builtin(BuiltinType::F64),
        };
        let Ok(bits) =
            TargetFloatEvaluator::new(self.graph.target()).decimal_bits(spelling.decimal(), format)
        else {
            return Err(self.rule(BodyRule::FloatOutOfRange, node)?);
        };
        let value = match bits {
            FloatBits::Binary32(bits) => ConstantValue::Float32(bits),
            FloatBits::Binary64(bits) => ConstantValue::Float64(bits),
        };
        let checked = self.add_node(node, ty, CheckedOperation::Constant(value))?;
        expected.map_or(Ok(checked), |expected| {
            self.apply_expected(node, checked, expected)
        })
    }
}
