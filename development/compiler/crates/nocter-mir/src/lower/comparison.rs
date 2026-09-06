use nocter_checking::{
    CheckedComparison, CheckedComparisonPlan, CheckedComparisonStep, ComparisonImplementation,
    ComparisonOperation, ReadonlyOperandPreparation, StaticSelection,
};
use nocter_model::{BodyNodeId, BorrowCapability, BuiltinType, MirValueId, TypeId, TypeKind};
use nocter_target_program::{ExecutableDispatchPlan, ExecutableDispatchStep};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{
    MirBranchTarget, MirConstant, MirOperationKind, MirStructuralCall, MirTerminator,
    MirUnaryOperation,
};

impl FunctionLowerer<'_> {
    pub(super) fn lower_comparison(
        &mut self,
        node: BodyNodeId,
        comparison: &CheckedComparison,
    ) -> Result<Option<MirValueId>, MirLoweringError> {
        if matches!(comparison.plan(), CheckedComparisonPlan::Unreachable) {
            let left = self.lower_node(comparison.left().value())?;
            if self.current.is_none() {
                return Ok(left);
            }
            return self.lower_node(comparison.right().value());
        }
        let left_type = self.readonly_operand_type(comparison.left())?;
        let right_type = self.readonly_operand_type(comparison.right())?;
        let left = self.lower_readonly_operand(
            node,
            comparison.left().value(),
            comparison.left().preparation(),
            left_type,
        )?;
        let right = self.lower_readonly_operand(
            node,
            comparison.right().value(),
            comparison.right().preparation(),
            right_type,
        )?;
        match comparison.plan() {
            CheckedComparisonPlan::Direct { step, negate } => {
                let result =
                    self.emit_comparison_step(node, step, [left, right], [left_type, right_type])?;
                if *negate {
                    let bool_ = self.executable.types().builtin(BuiltinType::Bool);
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
            CheckedComparisonPlan::Inclusive { strict, equal } => self
                .lower_inclusive_comparison(
                    node,
                    strict,
                    equal,
                    [left, right],
                    [left_type, right_type],
                )
                .map(Some),
            CheckedComparisonPlan::Unreachable => unreachable!(),
        }
    }

    fn comparison_plan(
        &self,
        node: BodyNodeId,
        step: &CheckedComparisonStep,
        source_types: [TypeId; 2],
    ) -> Result<ComparisonPlan, MirLoweringError> {
        match step.implementation() {
            ComparisonImplementation::Primitive => {
                let [left, right] = source_types;
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
        }
    }

    fn emit_comparison_step(
        &mut self,
        node: BodyNodeId,
        step: &CheckedComparisonStep,
        source_values: [MirValueId; 2],
        source_types: [TypeId; 2],
    ) -> Result<MirValueId, MirLoweringError> {
        let plan = self.comparison_plan(node, step, source_types)?;
        let expected = plan.source_parameter_types(step.reverse());
        let resolved_coercions = plan.source_coercions(step.reverse());
        let checked_coercions = [step.left_coercion(), step.right_coercion()];
        let mut arguments = [source_values[0], source_values[1]];
        for position in 0..2 {
            arguments[position] = self.apply_comparison_coercions(
                node,
                arguments[position],
                checked_coercions[position],
                resolved_coercions[position].as_ref(),
                expected[position],
            )?;
        }
        if step.reverse() {
            arguments.swap(0, 1);
        }
        let bool_ = self.executable.types().builtin(BuiltinType::Bool);
        if let Some(operation) = plan.operation {
            self.emit_dispatch_step(node, bool_, &operation, arguments)
        } else {
            self.emit_primitive_comparison(
                node,
                step.operation(),
                plan.parameter_types[0],
                arguments,
            )
        }
    }

    fn apply_comparison_coercions(
        &mut self,
        node: BodyNodeId,
        mut value: MirValueId,
        checked_coercion: Option<&StaticSelection>,
        resolved_coercion: Option<&ExecutableDispatchStep>,
        expected: TypeId,
    ) -> Result<MirValueId, MirLoweringError> {
        let checked_coercion = checked_coercion
            .map(|selection| self.invocation_step(node, selection))
            .transpose()?;
        let coercions = [checked_coercion.as_ref(), resolved_coercion]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
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

    fn lower_inclusive_comparison(
        &mut self,
        node: BodyNodeId,
        strict: &CheckedComparisonStep,
        equal: &CheckedComparisonStep,
        source_values: [MirValueId; 2],
        source_types: [TypeId; 2],
    ) -> Result<MirValueId, MirLoweringError> {
        let strict = self.emit_comparison_step(node, strict, source_values, source_types)?;
        let source = self.current.ok_or(MirLoweringError::MissingCurrentBlock)?;
        let (short_block, _) = self.builder.create_block([]);
        let (equal_block, _) = self.builder.create_block([]);
        self.builder.terminate(
            source,
            MirTerminator::Branch {
                condition: strict,
                then_target: MirBranchTarget::new(short_block, []),
                else_target: MirBranchTarget::new(equal_block, []),
            },
        )?;

        let bool_ = self.executable.types().builtin(BuiltinType::Bool);
        self.current = Some(short_block);
        let short =
            self.append_value(bool_, MirOperationKind::Constant(MirConstant::Bool(true)))?;
        let short_exit = self.current.map(|block| (block, Some(short)));

        self.current = Some(equal_block);
        let equal = self.emit_comparison_step(node, equal, source_values, source_types)?;
        let equal_exit = self.current.map(|block| (block, Some(equal)));

        self.join_branches(bool_, true, [short_exit, equal_exit])?
            .ok_or(MirLoweringError::MissingCurrentBlock)
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
        operation: ComparisonOperation,
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
        let target = match operation {
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
