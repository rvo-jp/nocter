use nocter_checking::{
    CheckedComparison, CheckedReadonlyOperand, ComparisonImplementation, ComparisonOperation,
    ReadonlyOperandPreparation,
};
use nocter_model::{BodyNodeId, BorrowCapability, MirValueId, TypeId, TypeKind};
use nocter_target_program::{ExecutableDispatchPlan, ExecutableDispatchStep};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{MirOperationKind, MirStructuralCall, MirUnaryOperation};

impl FunctionLowerer<'_> {
    pub(super) fn lower_comparison(
        &mut self,
        node: BodyNodeId,
        comparison: &CheckedComparison,
    ) -> Result<Option<MirValueId>, MirLoweringError> {
        if matches!(
            comparison.implementation(),
            ComparisonImplementation::Unreachable
        ) {
            let left = self.lower_node(comparison.left().value())?;
            if self.current.is_none() {
                return Ok(left);
            }
            return self.lower_node(comparison.right().value());
        }
        let plan = self.comparison_plan(node, comparison)?;
        let source_expected = plan.source_parameter_types(comparison.reverse());
        let source_coercions = plan.source_coercions(comparison.reverse());
        let left = self.lower_comparison_operand(
            node,
            comparison.left(),
            source_coercions[0].as_ref(),
            source_expected[0],
        )?;
        let right = self.lower_comparison_operand(
            node,
            comparison.right(),
            source_coercions[1].as_ref(),
            source_expected[1],
        )?;
        let arguments = if comparison.reverse() {
            [right, left]
        } else {
            [left, right]
        };
        let bool_ = self
            .executable
            .types()
            .builtin(nocter_model::BuiltinType::Bool);
        let result = if let Some(step) = plan.operation {
            self.emit_dispatch_step(node, bool_, &step, arguments)?
        } else {
            self.emit_primitive_comparison(node, comparison, plan.parameter_types[0], arguments)?
        };
        if comparison.negate() {
            self.append_value(
                bool_,
                MirOperationKind::Unary {
                    operation: MirUnaryOperation::LogicalNot,
                    operand: result,
                },
            )
            .map(Some)
        } else {
            Ok(Some(result))
        }
    }

    fn comparison_plan(
        &self,
        node: BodyNodeId,
        comparison: &CheckedComparison,
    ) -> Result<ComparisonPlan, MirLoweringError> {
        match comparison.implementation() {
            ComparisonImplementation::Primitive => {
                let left = self.readonly_operand_type(comparison.left())?;
                let right = self.readonly_operand_type(comparison.right())?;
                if left != right {
                    return Err(MirLoweringError::InvalidDispatch(node));
                }
                Ok(ComparisonPlan {
                    operation: None,
                    parameter_types: [left, right],
                    coercions: [None, None],
                })
            }
            ComparisonImplementation::Selected(selection) => {
                let plan = self
                    .item
                    .body()
                    .dispatch(selection)
                    .ok_or(MirLoweringError::InvalidDispatch(node))?;
                let (operation, coercions) = match plan {
                    ExecutableDispatchPlan::Invocation(operation) => {
                        (operation.clone(), [None, None])
                    }
                    ExecutableDispatchPlan::Comparison {
                        left_coercion,
                        right_coercion,
                        operation,
                    } => (
                        operation.clone(),
                        [left_coercion.clone(), right_coercion.clone()],
                    ),
                    ExecutableDispatchPlan::Index { .. }
                    | ExecutableDispatchPlan::OpaqueInvocation { .. } => {
                        return Err(MirLoweringError::InvalidDispatch(node));
                    }
                };
                let signature = self.step_signature(&operation)?;
                let [left, right] = signature.parameters() else {
                    return Err(MirLoweringError::InvalidDispatch(node));
                };
                Ok(ComparisonPlan {
                    operation: Some(operation),
                    parameter_types: [*left, *right],
                    coercions,
                })
            }
            ComparisonImplementation::Unreachable => Err(MirLoweringError::InvalidDispatch(node)),
        }
    }

    fn lower_comparison_operand(
        &mut self,
        node: BodyNodeId,
        operand: &CheckedReadonlyOperand,
        resolved_coercion: Option<&ExecutableDispatchStep>,
        expected: TypeId,
    ) -> Result<MirValueId, MirLoweringError> {
        let checked_coercion = operand
            .coercion()
            .map(|selection| self.invocation_step(node, selection))
            .transpose()?;
        let coercions = [checked_coercion.as_ref(), resolved_coercion]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let prepared_type = coercions
            .first()
            .map(|step| self.unary_step_types(node, step).map(|types| types.0))
            .transpose()?
            .unwrap_or(expected);
        let mut value = self.lower_readonly_operand(
            node,
            operand.value(),
            operand.preparation(),
            prepared_type,
        )?;
        for coercion in coercions {
            let (input, result) = self.unary_step_types(node, coercion)?;
            if self.builder.value_type(value) != Some(input) {
                return Err(MirLoweringError::InvalidDispatch(node));
            }
            value = self.emit_dispatch_step(node, result, coercion, [value])?;
        }
        if self.builder.value_type(value) != Some(expected) {
            return Err(MirLoweringError::InvalidDispatch(node));
        }
        Ok(value)
    }

    fn unary_step_types(
        &self,
        node: BodyNodeId,
        step: &ExecutableDispatchStep,
    ) -> Result<(TypeId, TypeId), MirLoweringError> {
        let signature = self.step_signature(step)?;
        let [input] = signature.parameters() else {
            return Err(MirLoweringError::InvalidDispatch(node));
        };
        Ok((*input, signature.result()))
    }

    fn emit_primitive_comparison(
        &mut self,
        node: BodyNodeId,
        comparison: &CheckedComparison,
        operand: TypeId,
        arguments: [MirValueId; 2],
    ) -> Result<MirValueId, MirLoweringError> {
        let Some(TypeKind::Borrow {
            capability: BorrowCapability::Readonly,
            referent: subject,
        }) = self.executable.types().get(operand)
        else {
            return Err(MirLoweringError::InvalidDispatch(node));
        };
        let target = match comparison.operation() {
            ComparisonOperation::Equal => MirStructuralCall::Equality {
                subject: *subject,
                operand,
            },
            ComparisonOperation::Less => MirStructuralCall::Ordering {
                subject: *subject,
                operand,
            },
        };
        let bool_ = self
            .executable
            .types()
            .builtin(nocter_model::BuiltinType::Bool);
        self.emit_call(bool_, crate::MirCallTarget::Structural(target), arguments)
    }

    fn readonly_operand_type(
        &self,
        operand: &nocter_checking::CheckedReadonlyOperand,
    ) -> Result<TypeId, MirLoweringError> {
        let source = self
            .body
            .nodes()
            .get(operand.value())
            .map(nocter_checking::CheckedNode::ty)
            .ok_or(MirLoweringError::UnknownNode(operand.value()))?;
        match operand.preparation() {
            ReadonlyOperandPreparation::BorrowPlace
            | ReadonlyOperandPreparation::BorrowTemporary => self
                .item
                .body()
                .prepared_borrow(source, BorrowCapability::Readonly)
                .ok_or(MirLoweringError::InvalidDispatch(operand.value())),
            ReadonlyOperandPreparation::UseReadonlyBorrow => self.concrete_type(source),
            ReadonlyOperandPreparation::WeakenReadwriteBorrow => {
                let Some(TypeKind::Borrow { referent, .. }) = self.executable.types().get(source)
                else {
                    return Err(MirLoweringError::InvalidDispatch(operand.value()));
                };
                self.item
                    .body()
                    .prepared_borrow(*referent, BorrowCapability::Readonly)
                    .ok_or(MirLoweringError::InvalidDispatch(operand.value()))
            }
        }
    }
}

struct ComparisonPlan {
    operation: Option<ExecutableDispatchStep>,
    parameter_types: [TypeId; 2],
    /// Coercions in semantic receiver/argument order, before source reversal is applied.
    coercions: [Option<ExecutableDispatchStep>; 2],
}

impl ComparisonPlan {
    fn source_parameter_types(&self, reverse: bool) -> [TypeId; 2] {
        if reverse {
            [self.parameter_types[1], self.parameter_types[0]]
        } else {
            self.parameter_types
        }
    }

    fn source_coercions(&self, reverse: bool) -> [Option<ExecutableDispatchStep>; 2] {
        if reverse {
            [self.coercions[1].clone(), self.coercions[0].clone()]
        } else {
            self.coercions.clone()
        }
    }
}
