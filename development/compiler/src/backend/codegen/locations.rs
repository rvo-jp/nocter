use super::EntryEmitter;
use crate::abi::{ABI_WORD_SIZE, ARGUMENT_REGISTER_COUNT};
use crate::diagnostics::Diagnostic;
use crate::ir::{BoolLocation, I32Location, SliceLocation, StrLocation, U8Location, UsizeLocation};
use crate::target::arm64::{WReg, XReg};

impl EntryEmitter {
    pub(super) fn emit_parameter_word_to_x(
        &mut self,
        index: usize,
        destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        if let Some(offset) = self.current_parameter_spill_offsets.get(&index) {
            self.encoder.emit_ldr_x_sp(destination, *offset);
            return Ok(());
        }

        self.emit_unspilled_parameter_word_to_x(index, destination)
    }

    pub(super) fn emit_unspilled_parameter_word_to_x(
        &mut self,
        index: usize,
        destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        if let Some(source) = XReg::argument(index) {
            if source != destination {
                self.encoder.emit_mov_x(destination, source);
            }
            return Ok(());
        }

        let offset = self.stack_parameter_word_offset(index)?;
        self.encoder.emit_ldr_x_sp(destination, offset);
        Ok(())
    }

    pub(super) fn emit_parameter_word_to_w(
        &mut self,
        index: usize,
        destination: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        if let Some(offset) = self.current_parameter_spill_offsets.get(&index) {
            self.encoder.emit_ldr_w_sp(destination, *offset);
            return Ok(());
        }

        if let Some(source) = WReg::argument(index) {
            if source != destination {
                self.encoder.emit_mov_w(destination, source);
            }
            return Ok(());
        }

        let offset = self.stack_parameter_word_offset(index)?;
        self.encoder.emit_ldr_w_sp(destination, offset);
        Ok(())
    }

    pub(super) fn stack_parameter_word_offset(&self, index: usize) -> Result<u32, Vec<Diagnostic>> {
        let Some(stack_word_index) = index.checked_sub(ARGUMENT_REGISTER_COUNT) else {
            return Err(vec![Diagnostic::error(
                "E9003",
                format!("parameter word {index} is passed in a register"),
            )]);
        };
        let frame_size = self.current_frame_size.ok_or_else(|| {
            vec![Diagnostic::error(
                "E9003",
                "stack parameter access requires an active function frame context",
            )]
        })?;
        let stack_word_offset = u32::try_from(stack_word_index)
            .ok()
            .and_then(|index| index.checked_mul(ABI_WORD_SIZE as u32))
            .ok_or_else(|| stack_parameter_offset_diagnostic(index))?;
        frame_size
            .checked_add(stack_word_offset)
            .ok_or_else(|| stack_parameter_offset_diagnostic(index))
    }

    pub(super) fn i32_location_register(
        &self,
        location: I32Location,
    ) -> Result<WReg, Vec<Diagnostic>> {
        match location {
            I32Location::Return => Ok(WReg::W0),
            I32Location::Parameter(index) => WReg::argument(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9003",
                    format!("codegen supports at most 8 i32 parameters, got parameter {index}"),
                )]
            }),
            I32Location::Local(index) => WReg::local(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9004",
                    format!("codegen supports at most 7 i32 locals, got local {index}"),
                )]
            }),
        }
    }

    pub(super) fn usize_location_register(
        &self,
        location: UsizeLocation,
    ) -> Result<XReg, Vec<Diagnostic>> {
        match location {
            UsizeLocation::Return => Ok(XReg::X0),
            UsizeLocation::Parameter(index) => XReg::argument(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9003",
                    format!("codegen supports at most 8 usize parameters, got parameter {index}"),
                )]
            }),
            UsizeLocation::Local(index) => XReg::local(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9004",
                    format!("codegen supports at most 7 usize locals, got local {index}"),
                )]
            }),
        }
    }

    pub(super) fn u8_location_register(
        &self,
        location: U8Location,
    ) -> Result<WReg, Vec<Diagnostic>> {
        match location {
            U8Location::Return => Ok(WReg::W0),
            U8Location::Parameter(index) => WReg::argument(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9003",
                    format!("codegen supports at most 8 u8 parameters, got parameter {index}"),
                )]
            }),
            U8Location::Local(index) => WReg::local(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9004",
                    format!("codegen supports at most 7 u8 locals, got local {index}"),
                )]
            }),
        }
    }

    pub(super) fn bool_location_register(
        &self,
        location: BoolLocation,
    ) -> Result<WReg, Vec<Diagnostic>> {
        match location {
            BoolLocation::Return => Ok(WReg::W0),
            BoolLocation::Parameter(index) => WReg::argument(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9003",
                    format!("codegen supports at most 8 bool parameters, got parameter {index}"),
                )]
            }),
            BoolLocation::Local(index) => WReg::local(index).ok_or_else(|| {
                vec![Diagnostic::error(
                    "E9003",
                    format!("codegen supports at most 7 local scalar bindings, got local {index}"),
                )]
            }),
        }
    }

    pub(super) fn str_location_registers(
        &self,
        location: StrLocation,
    ) -> Result<(XReg, XReg), Vec<Diagnostic>> {
        match location {
            StrLocation::Return => Ok((XReg::X0, XReg::X1)),
            StrLocation::Parameter(index) => {
                let ptr = XReg::argument(index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9003",
                        format!("parameter word {index} is stack-passed and must be loaded through a frame-aware helper"),
                    )]
                })?;
                let len_index = index + 1;
                let len = XReg::argument(len_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9003",
                        format!("parameter word {len_index} is stack-passed and must be loaded through a frame-aware helper"),
                    )]
                })?;
                Ok((ptr, len))
            }
            StrLocation::Local(index) => {
                let ptr = XReg::local(index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9004",
                        format!(
                            "codegen supports at most 7 local ABI words, got local word {index}"
                        ),
                    )]
                })?;
                let len_index = index + 1;
                let len = XReg::local(len_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9004",
                        format!(
                            "codegen supports at most 7 local ABI words, got local word {len_index}"
                        ),
                    )]
                })?;
                Ok((ptr, len))
            }
        }
    }

    pub(super) fn slice_location_registers(
        &self,
        location: SliceLocation,
    ) -> Result<(XReg, XReg), Vec<Diagnostic>> {
        match location {
            SliceLocation::Return => Ok((XReg::X0, XReg::X1)),
            SliceLocation::Parameter(index) => {
                let ptr = XReg::argument(index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9003",
                        format!("parameter word {index} is stack-passed and must be loaded through a frame-aware helper"),
                    )]
                })?;
                let len_index = index + 1;
                let len = XReg::argument(len_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9003",
                        format!("parameter word {len_index} is stack-passed and must be loaded through a frame-aware helper"),
                    )]
                })?;
                Ok((ptr, len))
            }
            SliceLocation::Local(index) => {
                let ptr = XReg::local(index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9004",
                        format!(
                            "codegen supports at most 7 local ABI words, got local word {index}"
                        ),
                    )]
                })?;
                let len_index = index + 1;
                let len = XReg::local(len_index).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "E9004",
                        format!(
                            "codegen supports at most 7 local ABI words, got local word {len_index}"
                        ),
                    )]
                })?;
                Ok((ptr, len))
            }
        }
    }
}

fn stack_parameter_offset_diagnostic(index: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E9003",
        format!("stack parameter word {index} offset overflows"),
    )]
}
