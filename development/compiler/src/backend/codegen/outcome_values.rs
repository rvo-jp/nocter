use super::control_flow::BranchPatch;
use super::*;
use crate::ir::AggregateLocation;
use crate::outcomes::{OutcomeLayer, storage::OutcomeStorageLayout};

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_return_stored_outcome(
        &mut self,
        source: AggregateLocation,
        storage: &OutcomeStorageLayout,
        payload_type: &Type,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "stored outcome return emission requires a stack frame",
            )]);
        };
        let mut completion_branches = Vec::new();
        self.emit_stored_outcome_layer_to_return(
            source,
            storage,
            payload_type,
            0,
            0,
            frame,
            &mut completion_branches,
        )?;
        for branch in completion_branches {
            self.patch_branch_placeholder_to_current(branch, "stored outcome return target")?;
        }
        self.emit_return(Some(frame));
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_stored_outcome_layer_to_return(
        &mut self,
        source: AggregateLocation,
        storage: &OutcomeStorageLayout,
        payload_type: &Type,
        layer_index: usize,
        register_index: usize,
        frame: &FrameLayout,
        completion_branches: &mut Vec<BranchPatch>,
    ) -> Result<(), Vec<Diagnostic>> {
        let layer = storage.layers.get(layer_index).ok_or_else(|| {
            vec![Diagnostic::error(
                "E9002",
                "stored outcome layer is missing",
            )]
        })?;
        let tag_register = XReg::argument(register_index).ok_or_else(|| {
            vec![Diagnostic::error(
                "E9002",
                "stored outcome tag exceeds return registers",
            )]
        })?;
        let tag_offset = self.aggregate_slot_load_offset(
            source,
            u32::try_from(layer.tag_offset).map_err(|_| {
                vec![Diagnostic::error(
                    "E9002",
                    "stored outcome tag offset exceeds u32",
                )]
            })?,
            8,
            Some(frame),
        )?;
        self.encoder.emit_ldr_x_sp(tag_register, tag_offset);
        self.encoder.emit_cmp_x_zero(tag_register);
        let success = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        if layer.layer == OutcomeLayer::Fallible {
            let failure_offset = layer
                .failure_offset
                .expect("fallible layer has error storage");
            for word in 0..4 {
                let register = XReg::argument(register_index + 1 + word).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9002",
                        "stored error exceeds return registers",
                    )]
                })?;
                let offset = self.aggregate_slot_load_offset(
                    source,
                    u32::try_from(failure_offset + (word as u64) * 8).map_err(|_| {
                        vec![Diagnostic::error(
                            "E9002",
                            "stored error offset exceeds u32",
                        )]
                    })?,
                    8,
                    Some(frame),
                )?;
                self.encoder.emit_ldr_x_sp(register, offset);
            }
        }
        completion_branches.push(self.emit_branch_placeholder());
        self.patch_branch_placeholder_to_current(success, "stored outcome success return")?;
        if layer_index + 1 < storage.layers.len() {
            self.emit_stored_outcome_layer_to_return(
                source,
                storage,
                payload_type,
                layer_index + 1,
                register_index + 1,
                frame,
                completion_branches,
            )
        } else {
            self.emit_stored_outcome_payload_to_return(
                source,
                storage,
                payload_type,
                register_index + 1,
                frame,
            )
        }
    }

    fn emit_stored_outcome_payload_to_return(
        &mut self,
        source: AggregateLocation,
        storage: &OutcomeStorageLayout,
        payload_type: &Type,
        register_index: usize,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        let payload_offset = u32::try_from(storage.payload_offset).map_err(|_| {
            vec![Diagnostic::error(
                "E9002",
                "stored payload offset exceeds u32",
            )]
        })?;
        if matches!(payload_type, Type::Aggregate { .. }) {
            return self.emit_copy_aggregate_range(
                AggregateLocation::Return,
                0,
                source,
                payload_offset,
                storage.payload_layout,
                Some(frame),
            );
        }
        let words = match payload_type {
            Type::Str | Type::Slice { .. } => 2,
            Type::DirectAggregate { words, .. } => *words,
            Type::I32 | Type::U8 | Type::Usize | Type::Bool | Type::Borrow { .. } => 1,
            _ => {
                return Err(vec![Diagnostic::error(
                    "E9002",
                    "stored outcome payload cannot be returned by value",
                )]);
            }
        };
        for word in 0..words {
            let register = XReg::argument(register_index + word).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9002",
                    "stored outcome payload exceeds return registers",
                )]
            })?;
            let offset = self.aggregate_slot_load_offset(
                source,
                payload_offset + u32::try_from(word * 8).expect("ABI word offset"),
                8,
                Some(frame),
            )?;
            self.encoder.emit_ldr_x_sp(register, offset);
        }
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_call_stored_outcome(
        &mut self,
        destination: AggregateLocation,
        function: FunctionSymbol,
        arguments: &[ScalarArgument],
        storage: &OutcomeStorageLayout,
        payload_type: &Type,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "stored outcome call emission requires a stack frame",
            )]);
        };
        let AggregateLocation::Slot(slot_index) = destination else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "stored outcome calls require an aggregate slot destination",
            )]);
        };

        self.emit_scalar_spills(frame)?;
        if matches!(payload_type, Type::Aggregate { .. }) {
            let payload_offset = self.outcome_slot_offset(
                slot_index,
                storage.payload_offset,
                storage.payload_layout.size,
                frame,
            )?;
            self.encoder.emit_add_x_sp_imm(XReg::X8, payload_offset);
        }
        let outgoing_stack = self.emit_staged_scalar_arguments(arguments, frame)?;
        self.emit_call(function);
        self.emit_restore_outgoing_stack_arguments(outgoing_stack);

        self.emit_outcome_layer_to_slot(slot_index, storage, payload_type, 0, 0, frame)?;
        self.emit_scalar_reloads(frame)
    }

    pub(in crate::backend::codegen) fn emit_if_stored_outcome_tag(
        &mut self,
        source: AggregateLocation,
        tag_offset: u32,
        success_instructions: &[Instruction],
        outcome_instructions: &[Instruction],
        frame: Option<&FrameLayout>,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        let stack_offset = self.aggregate_slot_load_offset(source, tag_offset, 8, frame)?;
        self.encoder.emit_ldr_x_sp(XReg::X16, stack_offset);
        self.encoder.emit_cmp_x_zero(XReg::X16);
        let success = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        for instruction in outcome_instructions {
            self.emit_instruction(instruction, frame, return_type)?;
        }
        let done = self.emit_branch_placeholder();
        self.patch_branch_placeholder_to_current(success, "stored outcome payload target")?;
        for instruction in success_instructions {
            self.emit_instruction(instruction, frame, return_type)?;
        }
        self.patch_branch_placeholder_to_current(done, "stored outcome consumer target")
    }

    pub(in crate::backend::codegen) fn emit_check_stored_fallible(
        &mut self,
        source: AggregateLocation,
        tag_offset: u32,
        error_offset: u32,
        success_instructions: &[Instruction],
        failure_mode: &OutcomeFailureMode,
        frame: Option<&FrameLayout>,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(frame) = frame else {
            return Err(vec![Diagnostic::error(
                "E9005",
                "stored fallible consumption requires a stack frame",
            )]);
        };
        let tag = self.aggregate_slot_load_offset(source, tag_offset, 8, Some(frame))?;
        self.encoder.emit_ldr_x_sp(XReg::X16, tag);
        self.encoder.emit_cmp_x_zero(XReg::X16);
        let success = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        for word in 0..4 {
            let offset = self.aggregate_slot_load_offset(
                source,
                error_offset + (word as u32 * 8),
                8,
                Some(frame),
            )?;
            let register = XReg::argument(word + 1).expect("error register");
            self.encoder.emit_ldr_x_sp(register, offset);
        }
        emit_mov_i32_to_w0(&mut self.encoder, 1);
        self.emit_stored_fallible_failure_action(failure_mode, frame, return_type)?;
        let recover_done = self.emit_recover_done_branch_if_needed(failure_mode);
        self.patch_branch_placeholder_to_current(success, "stored fallible success target")?;
        for instruction in success_instructions {
            self.emit_instruction(instruction, Some(frame), return_type)?;
        }
        self.patch_recover_done_branch(recover_done)
    }

    fn emit_stored_fallible_failure_action(
        &mut self,
        failure_mode: &OutcomeFailureMode,
        frame: &FrameLayout,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        match failure_mode {
            OutcomeFailureMode::Propagate => {
                self.emit_return(Some(frame));
                Ok(())
            }
            OutcomeFailureMode::PropagateWithCleanup {
                code,
                message,
                instructions,
            } => {
                self.emit_x_pair_to_str_location(XReg::X1, XReg::X2, *code)?;
                self.emit_x_pair_to_str_location(XReg::X3, XReg::X4, *message)?;
                for instruction in instructions {
                    self.emit_instruction(instruction, Some(frame), return_type)?;
                }
                self.emit_str_value_to_x_pair(&StrValue::Location(*code), XReg::X1, XReg::X2)?;
                self.emit_str_value_to_x_pair(&StrValue::Location(*message), XReg::X3, XReg::X4)?;
                emit_mov_i32_to_w0(&mut self.encoder, 1);
                self.emit_return(Some(frame));
                Ok(())
            }
            OutcomeFailureMode::Trap => {
                self.emit_trap();
                Ok(())
            }
            OutcomeFailureMode::Handle { instructions }
            | OutcomeFailureMode::Recover { instructions } => {
                for instruction in instructions {
                    self.emit_instruction(instruction, Some(frame), return_type)?;
                }
                Ok(())
            }
            OutcomeFailureMode::Catch {
                code,
                message,
                instructions,
            } => {
                self.emit_x_pair_to_str_location(XReg::X1, XReg::X2, *code)?;
                self.emit_x_pair_to_str_location(XReg::X3, XReg::X4, *message)?;
                for instruction in instructions {
                    self.emit_instruction(instruction, Some(frame), return_type)?;
                }
                Ok(())
            }
        }
    }

    pub(in crate::backend::codegen) fn emit_load_stored_outcome_payload(
        &mut self,
        destination: ComposedOutcomeDestination,
        source: AggregateLocation,
        offset: u32,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        match destination {
            ComposedOutcomeDestination::I32(destination) => {
                let stack_offset = self.aggregate_slot_load_offset(source, offset, 4, frame)?;
                self.encoder.emit_ldr_w_sp(WReg::W16, stack_offset);
                self.emit_w_to_i32_location(WReg::W16, destination)
            }
            ComposedOutcomeDestination::U8(destination) => {
                let stack_offset = self.aggregate_slot_load_offset(source, offset, 1, frame)?;
                self.encoder.emit_ldrb_w_sp(WReg::W16, stack_offset);
                self.emit_w_to_u8_location(WReg::W16, destination)
            }
            ComposedOutcomeDestination::Bool(destination) => {
                let stack_offset = self.aggregate_slot_load_offset(source, offset, 1, frame)?;
                self.encoder.emit_ldrb_w_sp(WReg::W16, stack_offset);
                self.emit_w_to_bool_location(WReg::W16, destination)
            }
            ComposedOutcomeDestination::Usize(destination)
            | ComposedOutcomeDestination::Borrow(destination) => {
                let stack_offset = self.aggregate_slot_load_offset(source, offset, 8, frame)?;
                self.encoder.emit_ldr_x_sp(XReg::X16, stack_offset);
                self.emit_x_to_usize_location(XReg::X16, destination)
            }
            ComposedOutcomeDestination::Str(destination) => {
                let pointer = self.aggregate_slot_load_offset(source, offset, 8, frame)?;
                let len = self.aggregate_slot_load_offset(source, offset + 8, 8, frame)?;
                self.encoder.emit_ldr_x_sp(XReg::X16, pointer);
                self.encoder.emit_ldr_x_sp(XReg::X17, len);
                self.emit_x_pair_to_str_location(XReg::X16, XReg::X17, destination)
            }
            ComposedOutcomeDestination::Slice(destination) => {
                let pointer = self.aggregate_slot_load_offset(source, offset, 8, frame)?;
                let len = self.aggregate_slot_load_offset(source, offset + 8, 8, frame)?;
                self.encoder.emit_ldr_x_sp(XReg::X16, pointer);
                self.encoder.emit_ldr_x_sp(XReg::X17, len);
                self.emit_x_pair_to_slice_location(XReg::X16, XReg::X17, destination)
            }
        }
    }

    fn emit_outcome_layer_to_slot(
        &mut self,
        slot_index: usize,
        storage: &OutcomeStorageLayout,
        payload_type: &Type,
        layer_index: usize,
        register_index: usize,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        let layer = storage.layers.get(layer_index).ok_or_else(|| {
            vec![Diagnostic::error(
                "E9002",
                "stored outcome layer is missing",
            )]
        })?;
        let tag = XReg::argument(register_index).ok_or_else(|| {
            vec![Diagnostic::error(
                "E9002",
                "stored outcome tag exceeds return registers",
            )]
        })?;
        let tag_offset = self.outcome_slot_offset(slot_index, layer.tag_offset, 8, frame)?;
        self.encoder.emit_str_x_sp(tag, tag_offset);
        self.encoder.emit_cmp_x_zero(tag);
        let success = self.emit_cond_branch_placeholder(BranchCondition::Eq);

        if layer.layer == OutcomeLayer::Fallible {
            let failure_offset = layer.failure_offset.expect("fallible layer has storage");
            self.emit_outcome_error_to_slot(slot_index, failure_offset, register_index + 1, frame)?;
        }
        let done = self.emit_branch_placeholder();
        self.patch_branch_placeholder_to_current(success, "stored outcome success target")?;

        if layer_index + 1 < storage.layers.len() {
            self.emit_outcome_layer_to_slot(
                slot_index,
                storage,
                payload_type,
                layer_index + 1,
                register_index + 1,
                frame,
            )?;
        } else {
            self.emit_outcome_payload_to_slot(
                slot_index,
                storage,
                payload_type,
                register_index + 1,
                frame,
            )?;
        }
        self.patch_branch_placeholder_to_current(done, "stored outcome completion target")
    }

    fn emit_outcome_error_to_slot(
        &mut self,
        slot_index: usize,
        offset: u64,
        register_index: usize,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        for word in 0..4 {
            let register = XReg::argument(register_index + word).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9002",
                    "stored outcome error exceeds return registers",
                )]
            })?;
            let word_offset = offset + (word as u64 * 8);
            let stack_offset = self.outcome_slot_offset(slot_index, word_offset, 8, frame)?;
            self.encoder.emit_str_x_sp(register, stack_offset);
        }
        Ok(())
    }

    fn emit_outcome_payload_to_slot(
        &mut self,
        slot_index: usize,
        storage: &OutcomeStorageLayout,
        payload_type: &Type,
        register_index: usize,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        match payload_type {
            Type::I32 => {
                let offset =
                    self.outcome_slot_offset(slot_index, storage.payload_offset, 4, frame)?;
                let register = WReg::argument(register_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9002",
                        "stored outcome payload register is invalid",
                    )]
                })?;
                self.encoder.emit_str_w_sp(register, offset);
            }
            Type::U8 | Type::Bool => {
                let offset =
                    self.outcome_slot_offset(slot_index, storage.payload_offset, 1, frame)?;
                let register = WReg::argument(register_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9002",
                        "stored outcome payload register is invalid",
                    )]
                })?;
                self.encoder.emit_strb_w_sp(register, offset);
            }
            Type::Usize | Type::Borrow { .. } => {
                let offset =
                    self.outcome_slot_offset(slot_index, storage.payload_offset, 8, frame)?;
                let register = XReg::argument(register_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9002",
                        "stored outcome payload register is invalid",
                    )]
                })?;
                self.encoder.emit_str_x_sp(register, offset);
            }
            Type::Str | Type::Slice { .. } => {
                let offset =
                    self.outcome_slot_offset(slot_index, storage.payload_offset, 16, frame)?;
                for word in 0..2 {
                    let register = XReg::argument(register_index + word).ok_or_else(|| {
                        vec![Diagnostic::error(
                            "E9002",
                            "stored outcome view payload exceeds return registers",
                        )]
                    })?;
                    self.encoder
                        .emit_str_x_sp(register, offset + (word as u32 * 8));
                }
            }
            Type::DirectAggregate { words, .. } => {
                let offset = self.outcome_slot_offset(
                    slot_index,
                    storage.payload_offset,
                    storage.payload_layout.size,
                    frame,
                )?;
                for word in 0..*words {
                    let register = XReg::argument(register_index + word).ok_or_else(|| {
                        vec![Diagnostic::error(
                            "E9002",
                            "stored outcome direct payload exceeds return registers",
                        )]
                    })?;
                    self.encoder
                        .emit_str_x_sp(register, offset + (word as u32 * 8));
                }
            }
            Type::Aggregate { .. } => {}
            _ => {
                return Err(vec![Diagnostic::error(
                    "E9002",
                    "stored outcome has an unsupported payload type",
                )]);
            }
        }
        Ok(())
    }

    fn outcome_slot_offset(
        &self,
        slot_index: usize,
        offset: u64,
        bytes: u64,
        frame: &FrameLayout,
    ) -> Result<u32, Vec<Diagnostic>> {
        let offset = u32::try_from(offset).map_err(|_| {
            vec![Diagnostic::error(
                "E9005",
                "stored outcome offset exceeds u32",
            )]
        })?;
        let bytes = u32::try_from(bytes).map_err(|_| {
            vec![Diagnostic::error(
                "E9005",
                "stored outcome field exceeds u32",
            )]
        })?;
        self.aggregate_slot_field_offset(slot_index, offset, bytes, Some(frame))
    }
}
