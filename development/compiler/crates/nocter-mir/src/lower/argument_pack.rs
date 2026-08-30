use nocter_model::{BodyNodeId, BuiltinType, LocalBindingId, LoopId, ParameterId, TypeId};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use super::loop_control::LoopTargets;
use crate::{
    MirBranchTarget, MirLocalKind, MirOperationKind, MirPlaceRoot, MirProjection,
    MirProjectionKind, MirReadMode, MirSwitchCase, MirSwitchSubject, MirSwitchValue, MirTerminator,
};

#[derive(Clone, Copy)]
pub(super) enum PackLoopBindings {
    Values {
        binding: LocalBindingId,
        item: TypeId,
    },
    Keyed {
        key_binding: LocalBindingId,
        value_binding: LocalBindingId,
        key: TypeId,
        value: TypeId,
    },
}

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
        parameter: ParameterId,
        bindings: PackLoopBindings,
        body: BodyNodeId,
    ) -> Result<(), MirLoweringError> {
        let pack = self.require_pack(node, parameter)?;
        let bindings = self.concrete_pack_bindings(loop_, pack.element(), bindings)?;
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
        let entry = pack.element();
        let entry_place = self.builder.add_place(
            MirPlaceRoot::Local(next_local),
            [MirProjection::new(
                MirProjectionKind::OptionalPayload,
                entry,
            )],
            entry,
        );
        match bindings {
            PackLoopBindings::Values { binding, item } => {
                self.move_pack_entry_component(loop_, entry_place, None, binding, item)?;
            }
            PackLoopBindings::Keyed {
                key_binding,
                value_binding,
                key,
                value,
            } => {
                self.move_pack_entry_component(
                    loop_,
                    entry_place,
                    Some(MirProjectionKind::PackEntryKey),
                    key_binding,
                    key,
                )?;
                self.move_pack_entry_component(
                    loop_,
                    entry_place,
                    Some(MirProjectionKind::PackEntryValue),
                    value_binding,
                    value,
                )?;
            }
        }
        self.lower_node(body)?;
        self.finish_loop_iteration(header)?;
        self.leave_loop(loop_)?;
        self.current = Some(exit);
        Ok(())
    }

    fn concrete_pack_bindings(
        &mut self,
        loop_: LoopId,
        element: TypeId,
        bindings: PackLoopBindings,
    ) -> Result<PackLoopBindings, MirLoweringError> {
        match bindings {
            PackLoopBindings::Values { binding, item } => {
                let item = self.concrete_type(item)?;
                if item != element {
                    return Err(MirLoweringError::InvalidLoop(loop_));
                }
                Ok(PackLoopBindings::Values { binding, item })
            }
            PackLoopBindings::Keyed {
                key_binding,
                value_binding,
                key,
                value,
            } => {
                let key = self.concrete_type(key)?;
                let value = self.concrete_type(value)?;
                if !matches!(
                    self.executable.types().get(element),
                    Some(nocter_model::TypeKind::PackEntry {
                        key: expected_key,
                        value: expected_value,
                    }) if *expected_key == key && *expected_value == value
                ) {
                    return Err(MirLoweringError::InvalidLoop(loop_));
                }
                Ok(PackLoopBindings::Keyed {
                    key_binding,
                    value_binding,
                    key,
                    value,
                })
            }
        }
    }

    fn move_pack_entry_component(
        &mut self,
        loop_: LoopId,
        entry: nocter_model::MirPlaceId,
        projection: Option<MirProjectionKind>,
        binding: LocalBindingId,
        ty: TypeId,
    ) -> Result<(), MirLoweringError> {
        let source = if let Some(kind) = projection {
            let entry = self
                .builder
                .place(entry)
                .cloned()
                .ok_or(MirLoweringError::InvalidLoop(loop_))?;
            let mut projections = entry.projections().to_vec();
            projections.push(MirProjection::new(kind, ty));
            self.builder.add_place(entry.root(), projections, ty)
        } else {
            entry
        };
        let value = self.append_value(
            ty,
            MirOperationKind::Read {
                place: source,
                mode: MirReadMode::Move,
            },
        )?;
        let local = self.ensure_local(binding)?;
        let destination = self.builder.add_place(MirPlaceRoot::Local(local), [], ty);
        self.append_effect(MirOperationKind::Initialize { destination, value })?;
        self.mark_binding_initialized(binding)
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
