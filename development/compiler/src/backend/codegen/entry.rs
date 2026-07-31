use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn new() -> Self {
        Self {
            encoder: Encoder::new(),
            read_only_data: Vec::new(),
            data_address_patches: Vec::new(),
            function_offsets: HashMap::new(),
            call_patches: Vec::new(),
            tail_call_patches: Vec::new(),
            loop_contexts: Vec::new(),
            current_frame_size: None,
            current_parameter_spill_offsets: HashMap::new(),
            current_scalar_spill_offsets: HashMap::new(),
        }
    }

    pub(in crate::backend::codegen) fn emit_module(
        &mut self,
        module: &IrModule,
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(entry) = module
            .functions
            .iter()
            .find(|function| function.name == DEFAULT_ENTRY_NAME)
        else {
            return Err(vec![Diagnostic::error(
                "E9002",
                format!(
                    "codegen requires a lowered entry function `{}`",
                    DEFAULT_ENTRY_NAME
                ),
            )]);
        };
        validate_module_call_return_shapes(module)?;

        self.emit_process_entry(entry, module_uses_process_arguments(module))?;

        for function in &module.functions {
            self.emit_function(function)?;
        }

        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_process_entry(
        &mut self,
        entry: &Function,
        capture_process_stack: bool,
    ) -> Result<(), Vec<Diagnostic>> {
        if capture_process_stack {
            self.encoder.emit_add_x_sp_imm(XReg::X19, 0);
        }
        self.emit_call(FunctionSymbol::from_function(entry));

        if let Type::Fallible(success_type) = &entry.return_type {
            self.emit_fallible_process_exit(success_type)?;
            return Ok(());
        }

        if matches!(entry.return_type, Type::Void) {
            emit_mov_i32_to_w0(&mut self.encoder, 0);
        }
        emit_darwin_exit_syscall(&mut self.encoder);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_function(
        &mut self,
        function: &Function,
    ) -> Result<(), Vec<Diagnostic>> {
        self.function_offsets.insert(
            FunctionSymbol::from_function(function),
            self.encoder.position(),
        );
        let frame = plan_function_frame(function)?;
        self.emit_function_with_frame(function, &frame)
    }

    pub(in crate::backend::codegen) fn emit_function_with_frame(
        &mut self,
        function: &Function,
        frame: &FunctionFrame,
    ) -> Result<(), Vec<Diagnostic>> {
        let previous_frame_size = self.current_frame_size;
        let previous_parameter_spill_offsets =
            std::mem::take(&mut self.current_parameter_spill_offsets);
        let previous_scalar_spill_offsets = std::mem::take(&mut self.current_scalar_spill_offsets);
        let frame = match frame {
            FunctionFrame::Frameless => {
                self.current_frame_size = Some(0);
                None
            }
            FunctionFrame::Framed(layout) => {
                self.current_frame_size = Some(layout.frame_size());
                self.current_parameter_spill_offsets = layout
                    .parameter_spill_slots()
                    .iter()
                    .map(|slot| (slot.parameter_index(), slot.offset()))
                    .collect();
                self.current_scalar_spill_offsets = layout
                    .scalar_spill_slots()
                    .iter()
                    .map(|slot| (slot.local_index(), slot.offset()))
                    .collect();
                self.emit_prologue(layout)?;
                Some(layout)
            }
        };

        let result = (|| {
            for instruction in &function.instructions {
                self.emit_instruction(instruction, frame, &function.return_type)?;
            }
            Ok(())
        })();
        self.current_frame_size = previous_frame_size;
        self.current_parameter_spill_offsets = previous_parameter_spill_offsets;
        self.current_scalar_spill_offsets = previous_scalar_spill_offsets;
        result
    }
}
