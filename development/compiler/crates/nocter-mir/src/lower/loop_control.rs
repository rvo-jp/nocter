use nocter_checking::{CheckedLoop, LoopKind};
use nocter_model::{BodyNodeId, BuiltinType, LoopId, MirBlockId, TypeId, TypeKind};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{
    MirBinaryOperation, MirBranchTarget, MirConstant, MirOperationKind, MirPlaceRoot, MirReadMode,
    MirTerminator,
};

#[derive(Clone, Copy)]
pub(super) struct LoopTargets {
    continue_: MirBlockId,
    break_: Option<MirBlockId>,
}

impl FunctionLowerer<'_> {
    pub(super) fn lower_loop(
        &mut self,
        node: BodyNodeId,
        loop_: LoopId,
    ) -> Result<(), MirLoweringError> {
        let definition = self
            .body
            .loops()
            .get(loop_)
            .cloned()
            .ok_or(MirLoweringError::InvalidLoop(loop_))?;
        match definition.kind() {
            LoopKind::Infinite => self.lower_infinite_loop(node, loop_, &definition),
            LoopKind::While { condition } => {
                self.lower_while_loop(loop_, *condition, definition.body())
            }
            LoopKind::Range {
                binding,
                start,
                end,
            } => self.lower_range_loop(loop_, *binding, *start, *end, definition.body()),
            LoopKind::For { .. } => Err(MirLoweringError::UnsupportedOperation(node)),
        }
    }

    pub(super) fn lower_loop_transfer(
        &mut self,
        node: BodyNodeId,
        loop_: LoopId,
        is_break: bool,
    ) -> Result<(), MirLoweringError> {
        let target = self
            .loops
            .get(&loop_)
            .copied()
            .ok_or(MirLoweringError::InvalidLoop(loop_))?;
        self.lower_cleanup(node, nocter_checking::CleanupTiming::BeforeTransfer)?;
        let source = self
            .current
            .take()
            .ok_or(MirLoweringError::MissingCurrentBlock)?;
        let destination = if is_break {
            target.break_.ok_or(MirLoweringError::InvalidLoop(loop_))?
        } else {
            target.continue_
        };
        self.builder.terminate(
            source,
            MirTerminator::Goto(MirBranchTarget::new(destination, [])),
        )?;
        Ok(())
    }

    fn lower_infinite_loop(
        &mut self,
        node: BodyNodeId,
        loop_: LoopId,
        definition: &CheckedLoop,
    ) -> Result<(), MirLoweringError> {
        let ty = self
            .body
            .nodes()
            .get(node)
            .map(nocter_checking::CheckedNode::ty)
            .ok_or(MirLoweringError::UnknownNode(node))?;
        let ty = self.concrete_type(ty)?;
        let has_exit =
            self.executable.types().get(ty) != Some(&TypeKind::Builtin(BuiltinType::Never));
        let source = self
            .current
            .take()
            .ok_or(MirLoweringError::MissingCurrentBlock)?;
        let (header, _) = self.builder.create_block([]);
        let exit = has_exit.then(|| self.builder.create_block([]).0);
        self.builder.terminate(
            source,
            MirTerminator::Goto(MirBranchTarget::new(header, [])),
        )?;
        self.enter_loop(
            loop_,
            LoopTargets {
                continue_: header,
                break_: exit,
            },
        )?;
        self.current = Some(header);
        self.lower_node(definition.body())?;
        self.finish_loop_iteration(header)?;
        self.leave_loop(loop_)?;

        self.current = exit;
        Ok(())
    }

    fn lower_while_loop(
        &mut self,
        loop_: LoopId,
        condition: BodyNodeId,
        body: BodyNodeId,
    ) -> Result<(), MirLoweringError> {
        let source = self
            .current
            .take()
            .ok_or(MirLoweringError::MissingCurrentBlock)?;
        let (header, _) = self.builder.create_block([]);
        let (body_block, _) = self.builder.create_block([]);
        let (exit, _) = self.builder.create_block([]);
        self.builder.terminate(
            source,
            MirTerminator::Goto(MirBranchTarget::new(header, [])),
        )?;
        self.enter_loop(
            loop_,
            LoopTargets {
                continue_: header,
                break_: Some(exit),
            },
        )?;

        self.current = Some(header);
        let condition = self.require_value(condition)?;
        let condition_exit = self
            .current
            .take()
            .ok_or(MirLoweringError::MissingCurrentBlock)?;
        self.builder.terminate(
            condition_exit,
            MirTerminator::Branch {
                condition,
                then_target: MirBranchTarget::new(body_block, []),
                else_target: MirBranchTarget::new(exit, []),
            },
        )?;
        self.current = Some(body_block);
        self.lower_node(body)?;
        self.finish_loop_iteration(header)?;
        self.leave_loop(loop_)?;
        self.current = Some(exit);
        Ok(())
    }

    fn lower_range_loop(
        &mut self,
        loop_: LoopId,
        binding: nocter_model::LocalBindingId,
        start: BodyNodeId,
        end: BodyNodeId,
        body: BodyNodeId,
    ) -> Result<(), MirLoweringError> {
        let start = self.require_value(start)?;
        let end = self.require_value(end)?;
        let ty = self
            .builder
            .value_type(start)
            .filter(|ty| self.builder.value_type(end) == Some(*ty))
            .ok_or(MirLoweringError::InvalidLoop(loop_))?;
        let local = self.ensure_local(binding)?;
        let place = self.builder.add_place(MirPlaceRoot::Local(local), [], ty);
        self.append_effect(MirOperationKind::Initialize {
            destination: place,
            value: start,
        })?;
        self.mark_binding_initialized(binding)?;

        let source = self
            .current
            .take()
            .ok_or(MirLoweringError::MissingCurrentBlock)?;
        let (header, _) = self.builder.create_block([]);
        let (body_block, _) = self.builder.create_block([]);
        let (latch, _) = self.builder.create_block([]);
        let (exit, _) = self.builder.create_block([]);
        self.builder.terminate(
            source,
            MirTerminator::Goto(MirBranchTarget::new(header, [])),
        )?;
        self.enter_loop(
            loop_,
            LoopTargets {
                continue_: latch,
                break_: Some(exit),
            },
        )?;

        self.current = Some(header);
        let current = self.read_range_binding(place, ty)?;
        let condition = self.append_value(
            self.executable.types().builtin(BuiltinType::Bool),
            MirOperationKind::Binary {
                operation: MirBinaryOperation::Less,
                left: current,
                right: end,
            },
        )?;
        let header = self
            .current
            .take()
            .ok_or(MirLoweringError::MissingCurrentBlock)?;
        self.builder.terminate(
            header,
            MirTerminator::Branch {
                condition,
                then_target: MirBranchTarget::new(body_block, []),
                else_target: MirBranchTarget::new(exit, []),
            },
        )?;

        self.current = Some(body_block);
        self.lower_node(body)?;
        self.finish_loop_iteration(latch)?;

        self.current = Some(latch);
        let current = self.read_range_binding(place, ty)?;
        let one = self.append_value(ty, MirOperationKind::Constant(MirConstant::Integer(1)))?;
        let next = self.append_value(
            ty,
            MirOperationKind::Binary {
                operation: MirBinaryOperation::Add,
                left: current,
                right: one,
            },
        )?;
        self.append_effect(MirOperationKind::Initialize {
            destination: place,
            value: next,
        })?;
        let latch = self
            .current
            .take()
            .ok_or(MirLoweringError::MissingCurrentBlock)?;
        self.builder
            .terminate(latch, MirTerminator::Goto(MirBranchTarget::new(header, [])))?;
        self.leave_loop(loop_)?;
        self.current = Some(exit);
        Ok(())
    }

    fn read_range_binding(
        &mut self,
        place: nocter_model::MirPlaceId,
        ty: TypeId,
    ) -> Result<nocter_model::MirValueId, MirLoweringError> {
        self.append_value(
            ty,
            MirOperationKind::Read {
                place,
                mode: MirReadMode::Copy,
            },
        )
    }

    fn finish_loop_iteration(&mut self, target: MirBlockId) -> Result<(), MirLoweringError> {
        if let Some(block) = self.current.take() {
            self.builder
                .terminate(block, MirTerminator::Goto(MirBranchTarget::new(target, [])))?;
        }
        Ok(())
    }

    fn enter_loop(&mut self, loop_: LoopId, targets: LoopTargets) -> Result<(), MirLoweringError> {
        if self.loops.insert(loop_, targets).is_some() {
            return Err(MirLoweringError::InvalidLoop(loop_));
        }
        Ok(())
    }

    fn leave_loop(&mut self, loop_: LoopId) -> Result<(), MirLoweringError> {
        self.loops
            .remove(&loop_)
            .map(|_| ())
            .ok_or(MirLoweringError::InvalidLoop(loop_))
    }
}
