use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_return_outcome_success(
        &mut self,
        return_type: &Type,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        match return_type {
            Type::Optional(payload_type) | Type::Fallible(payload_type) => {
                self.emit_outcome_success_payload(payload_type)?;
                emit_mov_i32_to_w0(&mut self.encoder, 0);
            }
            Type::ComposedOutcome { payload, .. } => {
                self.emit_composed_outcome_success_payload(payload)?;
                emit_mov_i32_to_w(&mut self.encoder, WReg::W1, 0);
                emit_mov_i32_to_w0(&mut self.encoder, 0);
            }
            _ => {
                return Err(vec![Diagnostic::error(
                    "E9002",
                    "`ReturnOutcomeSuccess` requires an outcome function return type",
                )]);
            }
        }
        self.emit_return(frame);
        Ok(())
    }

    fn emit_composed_outcome_success_payload(
        &mut self,
        payload_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        validate_supported_outcome_payload_abi(payload_type)?;
        match payload_type {
            Type::I32 | Type::U8 | Type::Bool => {
                self.encoder.emit_mov_w(WReg::W2, WReg::W0);
            }
            Type::Usize | Type::Integer(_) | Type::Borrow { .. } => {
                self.encoder.emit_mov_x(XReg::X2, XReg::X0);
            }
            Type::Str | Type::Slice { .. } => {
                self.encoder.emit_mov_x(XReg::X3, XReg::X1);
                self.encoder.emit_mov_x(XReg::X2, XReg::X0);
            }
            Type::Aggregate { .. } | Type::Void => {}
            Type::DirectAggregate { words, .. } => match words {
                0 => {}
                1 => self.encoder.emit_mov_x(XReg::X2, XReg::X0),
                2 => {
                    self.encoder.emit_mov_x(XReg::X3, XReg::X1);
                    self.encoder.emit_mov_x(XReg::X2, XReg::X0);
                }
                _ => {
                    return Err(vec![Diagnostic::error(
                        "E9002",
                        "invalid direct aggregate composed outcome payload width",
                    )]);
                }
            },
            Type::Error
            | Type::Never
            | Type::Optional(_)
            | Type::Fallible(_)
            | Type::ComposedOutcome { .. } => {
                return Err(vec![Diagnostic::error(
                    "E9002",
                    "invalid composed outcome payload type for codegen",
                )]);
            }
        }
        Ok(())
    }

    fn emit_outcome_success_payload(&mut self, payload_type: &Type) -> Result<(), Vec<Diagnostic>> {
        validate_supported_outcome_payload_abi(payload_type)?;
        match payload_type {
            Type::I32 | Type::U8 | Type::Bool => {
                self.encoder.emit_mov_w(WReg::W1, WReg::W0);
            }
            Type::Usize | Type::Integer(_) | Type::Borrow { .. } => {
                self.encoder.emit_mov_x(XReg::X1, XReg::X0);
            }
            Type::Str | Type::Slice { .. } => {
                self.encoder.emit_mov_x(XReg::X2, XReg::X1);
                self.encoder.emit_mov_x(XReg::X1, XReg::X0);
            }
            Type::Aggregate { .. } | Type::Void => {}
            Type::DirectAggregate { words, .. } => match words {
                0 => {}
                1 => self.encoder.emit_mov_x(XReg::X1, XReg::X0),
                2 => {
                    self.encoder.emit_mov_x(XReg::X2, XReg::X1);
                    self.encoder.emit_mov_x(XReg::X1, XReg::X0);
                }
                _ => {
                    return Err(vec![Diagnostic::error(
                        "E9002",
                        "invalid direct aggregate outcome payload width",
                    )]);
                }
            },
            Type::Error
            | Type::Never
            | Type::Optional(_)
            | Type::Fallible(_)
            | Type::ComposedOutcome { .. } => {
                return Err(vec![Diagnostic::error(
                    "E9002",
                    "invalid outcome payload type for codegen",
                )]);
            }
        }

        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_return_optional_none(
        &mut self,
        frame: Option<&FrameLayout>,
        return_type: &Type,
    ) -> Result<(), Vec<Diagnostic>> {
        match return_type {
            Type::Optional(_) => emit_mov_i32_to_w0(&mut self.encoder, 1),
            Type::ComposedOutcome { outer, inner, .. } => match (outer, inner) {
                (
                    crate::outcomes::OutcomeLayer::Fallible,
                    crate::outcomes::OutcomeLayer::Optional,
                ) => {
                    emit_mov_i32_to_w(&mut self.encoder, WReg::W1, 1);
                    emit_mov_i32_to_w0(&mut self.encoder, 0);
                }
                (crate::outcomes::OutcomeLayer::Optional, _) => {
                    emit_mov_i32_to_w0(&mut self.encoder, 1);
                }
                _ => {
                    return Err(vec![Diagnostic::error(
                        "E9002",
                        "composed outcome has no optional return layer",
                    )]);
                }
            },
            Type::Fallible(_) => {
                return Err(vec![Diagnostic::error(
                    "E9002",
                    "optional none return requires an optional outcome layer",
                )]);
            }
            _ => {
                return Err(vec![Diagnostic::error(
                    "E9002",
                    "optional absence requires an optional return layer",
                )]);
            }
        }
        self.emit_return(frame);
        Ok(())
    }
}
