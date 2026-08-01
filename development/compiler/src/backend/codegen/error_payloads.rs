use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_static_error_payload(
        &mut self,
        payload: StaticErrorPayload,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_str_value_to_x_pair(
            &StrValue::StaticBytes(payload.code.to_vec()),
            XReg::X1,
            XReg::X2,
        )?;
        self.emit_str_value_to_x_pair(
            &StrValue::StaticBytes(payload.message.to_vec()),
            XReg::X3,
            XReg::X4,
        )?;
        emit_mov_i32_to_w0(&mut self.encoder, 1);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_error_payload_from_errno(
        &mut self,
        mappings: &[DarwinErrnoPayload],
        fallback: StaticErrorPayload,
        done_target_description: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut done_branches = Vec::new();

        for mapping in mappings {
            emit_mov_i32_to_w(&mut self.encoder, WReg::W17, mapping.errno);
            self.encoder.emit_cmp_w(WReg::W0, WReg::W17);
            let next_mapping = self.emit_cond_branch_placeholder(BranchCondition::Ne);
            self.emit_static_error_payload(mapping.payload)?;
            done_branches.push(self.emit_branch_placeholder());
            self.patch_branch_placeholder_to_current(
                next_mapping,
                "errno error payload next mapping target",
            )?;
        }

        self.emit_static_error_payload(fallback)?;
        self.patch_branch_placeholders_to_current(done_branches, done_target_description)
    }

    pub(in crate::backend::codegen) fn emit_open_failure_payload_from_errno(
        &mut self,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_error_payload_from_errno(
            OPEN_ERRNO_PAYLOADS,
            OPEN_FAILURE_PAYLOAD,
            "open failure payload end target",
        )
    }

    pub(in crate::backend::codegen) fn emit_read_failure_payload_from_errno(
        &mut self,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_error_payload_from_errno(
            READ_ERRNO_PAYLOADS,
            READ_FAILURE_PAYLOAD,
            "read failure payload end target",
        )
    }

    pub(in crate::backend::codegen) fn emit_write_failure_payload_from_errno(
        &mut self,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_error_payload_from_errno(
            WRITE_ERRNO_PAYLOADS,
            WRITE_FAILURE_PAYLOAD,
            "write failure payload end target",
        )
    }
}
