use nocter_checking::{CheckedControl, LogicalOperation};
use nocter_model::{BodyNodeId, BuiltinType, MirBlockId, MirValueId, TypeKind};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{MirBranchTarget, MirConstant, MirOperationKind, MirPlaceRoot, MirTerminator};

impl FunctionLowerer<'_> {
    pub(super) fn lower_control(
        &mut self,
        node: BodyNodeId,
        control: &CheckedControl,
    ) -> Result<Option<MirValueId>, MirLoweringError> {
        match control {
            CheckedControl::Block {
                statements, result, ..
            } => {
                for statement in statements {
                    self.lower_node(*statement)?;
                    if self.current.is_none() {
                        return Ok(None);
                    }
                }
                result.map_or(Ok(None), |result| self.lower_node(result))
            }
            CheckedControl::Bind {
                binding,
                initializer,
            } => {
                let value = self.require_value(*initializer)?;
                let local = self.ensure_local(*binding)?;
                let ty = self
                    .body
                    .locals()
                    .get(*binding)
                    .copied()
                    .map(nocter_checking::CheckedLocal::ty)
                    .ok_or(MirLoweringError::UnknownLocal(*binding))?;
                let place =
                    self.builder
                        .add_place(MirPlaceRoot::Local(local), [], self.concrete_type(ty)?);
                self.append_effect(MirOperationKind::Initialize {
                    destination: place,
                    value,
                })?;
                Ok(None)
            }
            CheckedControl::Assign { target, value } => {
                let value = self.require_value(*value)?;
                let destination = self.lower_place(*target)?;
                self.append_effect(MirOperationKind::Store { destination, value })?;
                Ok(None)
            }
            CheckedControl::Discard(value) => {
                self.lower_node(*value)?;
                Ok(None)
            }
            CheckedControl::Unreachable(_) => Ok(None),
            CheckedControl::Return(value) => {
                let value = value.map(|value| self.require_value(value)).transpose()?;
                if let Some(block) = self.current.take() {
                    self.builder
                        .terminate(block, MirTerminator::Return(value))?;
                }
                Ok(None)
            }
            CheckedControl::If {
                condition,
                then_branch,
                else_branch,
            } => self.lower_if(node, *condition, *then_branch, *else_branch),
            CheckedControl::Logical {
                operation,
                left,
                right,
            } => self.lower_logical(*operation, *left, *right).map(Some),
            CheckedControl::CompoundAssign { .. }
            | CheckedControl::Break(_)
            | CheckedControl::Continue(_)
            | CheckedControl::Drop(_)
            | CheckedControl::Pattern { .. }
            | CheckedControl::Loop(_)
            | CheckedControl::Region { .. } => Err(MirLoweringError::UnsupportedOperation(node)),
        }
    }

    fn lower_if(
        &mut self,
        node: BodyNodeId,
        condition: BodyNodeId,
        then_branch: BodyNodeId,
        else_branch: Option<BodyNodeId>,
    ) -> Result<Option<MirValueId>, MirLoweringError> {
        let condition = self.require_value(condition)?;
        let source = self.current.ok_or(MirLoweringError::MissingCurrentBlock)?;
        let (then_block, _) = self.builder.create_block([]);
        let (else_block, _) = self.builder.create_block([]);
        self.builder.terminate(
            source,
            MirTerminator::Branch {
                condition,
                then_target: MirBranchTarget::new(then_block, []),
                else_target: MirBranchTarget::new(else_block, []),
            },
        )?;
        let then_exit = self.lower_branch(then_block, Some(then_branch))?;
        let else_exit = self.lower_branch(else_block, else_branch)?;
        let ty = self
            .body
            .nodes()
            .get(node)
            .map(nocter_checking::CheckedNode::ty)
            .ok_or(MirLoweringError::UnknownNode(node))?;
        let ty = self.concrete_type(ty)?;
        let carries_value = !matches!(
            self.executable.types().get(ty),
            Some(TypeKind::Builtin(BuiltinType::Void | BuiltinType::Never))
        );
        self.join_branches(ty, carries_value, [then_exit, else_exit])
    }

    fn lower_branch(
        &mut self,
        block: MirBlockId,
        body: Option<BodyNodeId>,
    ) -> Result<Option<(MirBlockId, Option<MirValueId>)>, MirLoweringError> {
        self.current = Some(block);
        let value = body
            .map(|body| self.lower_node(body))
            .transpose()?
            .flatten();
        Ok(self.current.map(|block| (block, value)))
    }

    fn join_branches(
        &mut self,
        ty: nocter_model::TypeId,
        carries_value: bool,
        exits: [Option<(MirBlockId, Option<MirValueId>)>; 2],
    ) -> Result<Option<MirValueId>, MirLoweringError> {
        let live = exits.into_iter().flatten().collect::<Vec<_>>();
        if live.is_empty() {
            self.current = None;
            return Ok(None);
        }
        let parameter_types = carries_value.then_some(ty).into_iter();
        let (join, parameters) = self.builder.create_block(parameter_types);
        for (block, value) in live {
            let arguments = if carries_value {
                vec![value.ok_or(MirLoweringError::MissingCurrentBlock)?]
            } else {
                Vec::new()
            };
            self.builder.terminate(
                block,
                MirTerminator::Goto(MirBranchTarget::new(join, arguments)),
            )?;
        }
        self.current = Some(join);
        Ok(parameters.first().copied())
    }

    fn lower_logical(
        &mut self,
        operation: LogicalOperation,
        left: BodyNodeId,
        right: BodyNodeId,
    ) -> Result<MirValueId, MirLoweringError> {
        let left = self.require_value(left)?;
        let source = self.current.ok_or(MirLoweringError::MissingCurrentBlock)?;
        let (right_block, _) = self.builder.create_block([]);
        let (short_block, _) = self.builder.create_block([]);
        let (then_target, else_target, short_value) = match operation {
            LogicalOperation::And => (right_block, short_block, false),
            LogicalOperation::Or => (short_block, right_block, true),
        };
        self.builder.terminate(
            source,
            MirTerminator::Branch {
                condition: left,
                then_target: MirBranchTarget::new(then_target, []),
                else_target: MirBranchTarget::new(else_target, []),
            },
        )?;
        let bool_ = self.executable.types().builtin(BuiltinType::Bool);
        self.current = Some(right_block);
        let right = self.require_value(right)?;
        let right_exit = self.current.map(|block| (block, Some(right)));
        self.current = Some(short_block);
        let short = self.append_value(
            bool_,
            MirOperationKind::Constant(MirConstant::Bool(short_value)),
        )?;
        let short_exit = self.current.map(|block| (block, Some(short)));
        self.join_branches(bool_, true, [right_exit, short_exit])?
            .ok_or(MirLoweringError::MissingCurrentBlock)
    }
}
