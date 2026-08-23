use nocter_model::{BodyNodeId, BuiltinType, LocalBindingId, LoopId, ParameterId, TypeId};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use super::loop_control::LoopTargets;
use crate::{
    MirBranchTarget, MirLocalKind, MirOperationKind, MirPlaceRoot, MirProjection,
    MirProjectionKind, MirReadMode, MirSwitchCase, MirSwitchSubject, MirSwitchValue, MirTerminator,
};

impl FunctionLowerer<'_> {
    pub(super) fn lower_pack_length(
        &mut self,
        node: BodyNodeId,
        ty: TypeId,
        parameter: ParameterId,
    ) -> Result<nocter_model::MirValueId, MirLoweringError> {
        self.require_pack(node, parameter)?;
        if ty != self.executable.types().builtin(BuiltinType::Usize) {
            return Err(MirLoweringError::InvalidDispatch(node));
        }
        self.append_value(ty, MirOperationKind::PackLength)
    }

    pub(super) fn lower_argument_pack_loop(
        &mut self,
        node: BodyNodeId,
        loop_: LoopId,
        binding: LocalBindingId,
        parameter: ParameterId,
        item: TypeId,
        body: BodyNodeId,
    ) -> Result<(), MirLoweringError> {
        let pack = self.require_pack(node, parameter)?;
        let item = self.concrete_type(item)?;
        if item != pack.element() {
            return Err(MirLoweringError::InvalidLoop(loop_));
        }
        let next = pack.next();
        let next_local = self.builder.add_local(next, MirLocalKind::Temporary, true);
        let next_place = self
            .builder
            .add_place(MirPlaceRoot::Local(next_local), [], next);
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
        let value = self.append_value(next, MirOperationKind::PackNext)?;
        self.append_effect(MirOperationKind::Initialize {
            destination: next_place,
            value,
        })?;
        let header = self
            .current
            .take()
            .ok_or(MirLoweringError::MissingCurrentBlock)?;
        self.builder.terminate(
            header,
            MirTerminator::Switch {
                subject: MirSwitchSubject::Place(next_place),
                cases: Box::new([MirSwitchCase::new(
                    MirSwitchValue::OptionalPresent,
                    MirBranchTarget::new(body_block, []),
                )]),
                fallback: MirBranchTarget::new(exit, []),
            },
        )?;

        self.current = Some(body_block);
        let item_place = self.builder.add_place(
            MirPlaceRoot::Local(next_local),
            [MirProjection::new(MirProjectionKind::OptionalPayload, item)],
            item,
        );
        let value = self.append_value(
            item,
            MirOperationKind::Read {
                place: item_place,
                mode: MirReadMode::Move,
            },
        )?;
        let binding_local = self.ensure_local(binding)?;
        let binding_place = self
            .builder
            .add_place(MirPlaceRoot::Local(binding_local), [], item);
        self.append_effect(MirOperationKind::Initialize {
            destination: binding_place,
            value,
        })?;
        self.mark_binding_initialized(binding)?;
        self.lower_node(body)?;
        self.finish_loop_iteration(header)?;
        self.leave_loop(loop_)?;
        self.current = Some(exit);
        Ok(())
    }

    pub(super) fn destroy_pack(&mut self) -> Result<(), MirLoweringError> {
        if self.item.signature().pack().is_some() {
            self.append_effect(MirOperationKind::DestroyPack)?;
        }
        Ok(())
    }

    fn require_pack(
        &self,
        node: BodyNodeId,
        parameter: ParameterId,
    ) -> Result<nocter_target_program::ExecutablePackInput, MirLoweringError> {
        self.item
            .signature()
            .pack()
            .filter(|pack| pack.source() == parameter)
            .ok_or(MirLoweringError::UnsupportedOperation(node))
    }
}
