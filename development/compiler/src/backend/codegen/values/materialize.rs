use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_trap(&mut self) {
        self.encoder.emit_brk(0);
    }

    pub(in crate::backend::codegen) fn emit_i32_value_to_w(
        &mut self,
        value: &I32Value,
        destination: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            I32Value::Const(value) => emit_mov_i32_to_w(&mut self.encoder, destination, *value),
            I32Value::Location(location) => {
                if let I32Location::Parameter(index) = location {
                    self.emit_parameter_word_to_w(*index, destination)?;
                    return Ok(());
                }
                if let I32Location::Local(index) = location {
                    self.emit_local_word_to_w(*index, destination, LocalScalarWidth::I32)?;
                } else {
                    let source = self.i32_location_register(*location)?;
                    if source != destination {
                        self.encoder.emit_mov_w(destination, source);
                    }
                }
            }
            I32Value::U8ZeroExtend(value) => {
                self.emit_u8_value_to_w(value, destination)?;
            }
            I32Value::SliceIndex { source, index } => {
                if let SliceLocation::Parameter(parameter_index) = *source {
                    self.emit_checked_parameter_i32_load(destination, parameter_index, index)?;
                    return Ok(());
                }
                if let SliceLocation::Local(local_index) = *source {
                    self.emit_checked_local_i32_load(destination, local_index, index)?;
                    return Ok(());
                }
                let (ptr, len) = self.slice_location_registers(*source)?;
                self.emit_checked_i32_load(destination, ptr, len, index)?;
            }
        }

        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_usize_value_to_x(
        &mut self,
        value: &UsizeValue,
        destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            UsizeValue::Const(value) => emit_mov_u64_to_x(&mut self.encoder, destination, *value),
            UsizeValue::ProcessArgCount => {
                self.encoder.emit_ldr_x_imm(destination, XReg::X19, 0);
            }
            UsizeValue::ProcessEnvironmentCount => {
                self.emit_process_environment_count_to_x(destination)?;
            }
            UsizeValue::CurrentAllocationState => {
                self.encoder.emit_mov_x(destination, XReg::X20);
            }
            UsizeValue::CurrentAllocationKind => {
                self.encoder.emit_mov_x(destination, XReg::X21);
            }
            UsizeValue::Location(location) => {
                if let UsizeLocation::Parameter(index) = location {
                    self.emit_parameter_word_to_x(*index, destination)?;
                    return Ok(());
                }
                if let UsizeLocation::Local(index) = location {
                    self.emit_local_word_to_x(*index, destination)?;
                } else {
                    let source = self.usize_location_register(*location)?;
                    if source != destination {
                        self.encoder.emit_mov_x(destination, source);
                    }
                }
            }
            UsizeValue::U8ZeroExtend(value) => {
                self.emit_u8_value_to_w(value, WReg::W16)?;
                if destination != XReg::X16 {
                    self.encoder.emit_mov_x(destination, XReg::X16);
                }
            }
            UsizeValue::StrLen(location) => {
                if let StrLocation::Parameter(index) = *location {
                    let len_index = pair_len_index(index, "parameter str")?;
                    self.emit_parameter_word_to_x(len_index, destination)?;
                    return Ok(());
                }
                if let StrLocation::Local(index) = *location {
                    let len_index = pair_len_index(index, "local str")?;
                    self.emit_local_word_to_x(len_index, destination)?;
                } else {
                    let (_, source) = self.str_location_registers(*location)?;
                    if source != destination {
                        self.encoder.emit_mov_x(destination, source);
                    }
                }
            }
            UsizeValue::SliceLen(location) => {
                if let SliceLocation::Parameter(index) = *location {
                    let len_index = pair_len_index(index, "parameter slice")?;
                    self.emit_parameter_word_to_x(len_index, destination)?;
                    return Ok(());
                }
                if let SliceLocation::Local(index) = *location {
                    let len_index = pair_len_index(index, "local slice")?;
                    self.emit_local_word_to_x(len_index, destination)?;
                } else {
                    let (_, source) = self.slice_location_registers(*location)?;
                    if source != destination {
                        self.encoder.emit_mov_x(destination, source);
                    }
                }
            }
            UsizeValue::SliceIndex { source, index } => {
                if let SliceLocation::Parameter(parameter_index) = *source {
                    self.emit_checked_parameter_usize_load(
                        destination,
                        parameter_index,
                        index.as_ref(),
                    )?;
                    return Ok(());
                }
                if let SliceLocation::Local(local_index) = *source {
                    self.emit_checked_local_usize_load(destination, local_index, index.as_ref())?;
                    return Ok(());
                }
                let (ptr, len) = self.slice_location_registers(*source)?;
                self.emit_checked_usize_load(destination, ptr, len, index.as_ref())?;
            }
        }

        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_u8_value_to_w(
        &mut self,
        value: &U8Value,
        destination: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            U8Value::Const(value) => {
                emit_mov_i32_to_w(&mut self.encoder, destination, i32::from(*value));
            }
            U8Value::Location(location) => {
                if let U8Location::Parameter(index) = location {
                    self.emit_parameter_word_to_w(*index, destination)?;
                    return Ok(());
                }
                if let U8Location::Local(index) = location {
                    self.emit_local_word_to_w(*index, destination, LocalScalarWidth::Byte)?;
                } else {
                    let source = self.u8_location_register(*location)?;
                    if source != destination {
                        self.encoder.emit_mov_w(destination, source);
                    }
                }
            }
            U8Value::StrIndex { source, index } => {
                if let StrLocation::Parameter(parameter_index) = *source {
                    self.emit_checked_parameter_byte_load(destination, parameter_index, index)?;
                    return Ok(());
                }
                if let StrLocation::Local(local_index) = *source {
                    self.emit_checked_local_byte_load(destination, local_index, index)?;
                    return Ok(());
                }
                let (ptr, len) = self.str_location_registers(*source)?;
                self.emit_checked_byte_load(destination, ptr, len, index)?;
            }
            U8Value::StaticStrIndex { bytes, index } => {
                self.emit_usize_value_to_x(index, XReg::X16)?;
                emit_mov_u64_to_x(&mut self.encoder, XReg::X17, bytes.len() as u64);
                self.emit_index_in_bounds_check(XReg::X16, XReg::X17)?;
                self.emit_static_data_address(XReg::X17, bytes);
                self.encoder
                    .emit_ldrb_w_reg(destination, XReg::X17, XReg::X16);
            }
            U8Value::SliceIndex { source, index } => {
                if let SliceLocation::Parameter(parameter_index) = *source {
                    self.emit_checked_parameter_byte_load(destination, parameter_index, index)?;
                    return Ok(());
                }
                if let SliceLocation::Local(local_index) = *source {
                    self.emit_checked_local_byte_load(destination, local_index, index)?;
                    return Ok(());
                }
                let (ptr, len) = self.slice_location_registers(*source)?;
                self.emit_checked_byte_load(destination, ptr, len, index)?;
            }
        }

        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_checked_parameter_byte_load(
        &mut self,
        destination: WReg,
        ptr_word_index: usize,
        index: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let len_word_index = ptr_word_index
            .checked_add(1)
            .ok_or_else(|| indexed_load_diagnostic("parameter length word index overflows"))?;
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.emit_parameter_word_to_x(len_word_index, XReg::X17)?;
        self.emit_index_in_bounds_check(XReg::X16, XReg::X17)?;
        self.emit_parameter_word_to_x(ptr_word_index, XReg::X17)?;
        self.encoder
            .emit_ldrb_w_reg(destination, XReg::X17, XReg::X16);
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_checked_local_byte_load(
        &mut self,
        destination: WReg,
        ptr_word_index: usize,
        index: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let len_word_index = ptr_word_index
            .checked_add(1)
            .ok_or_else(|| indexed_load_diagnostic("local length word index overflows"))?;
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.emit_local_word_to_x(len_word_index, XReg::X17)?;
        self.emit_index_in_bounds_check(XReg::X16, XReg::X17)?;
        self.emit_local_word_to_x(ptr_word_index, XReg::X17)?;
        self.encoder
            .emit_ldrb_w_reg(destination, XReg::X17, XReg::X16);
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_checked_byte_load(
        &mut self,
        destination: WReg,
        ptr: XReg,
        len: XReg,
        index: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.emit_index_in_bounds_check(XReg::X16, len)?;
        self.encoder.emit_ldrb_w_reg(destination, ptr, XReg::X16);
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_checked_parameter_i32_load(
        &mut self,
        destination: WReg,
        ptr_word_index: usize,
        index: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let len_word_index = ptr_word_index
            .checked_add(1)
            .ok_or_else(|| indexed_load_diagnostic("parameter length word index overflows"))?;
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.emit_parameter_word_to_x(len_word_index, XReg::X17)?;
        self.emit_index_in_bounds_check(XReg::X16, XReg::X17)?;
        self.emit_parameter_word_to_x(ptr_word_index, XReg::X17)?;
        self.emit_indexed_i32_load(destination, XReg::X17);
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_checked_local_i32_load(
        &mut self,
        destination: WReg,
        ptr_word_index: usize,
        index: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let len_word_index = ptr_word_index
            .checked_add(1)
            .ok_or_else(|| indexed_load_diagnostic("local length word index overflows"))?;
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.emit_local_word_to_x(len_word_index, XReg::X17)?;
        self.emit_index_in_bounds_check(XReg::X16, XReg::X17)?;
        self.emit_local_word_to_x(ptr_word_index, XReg::X17)?;
        self.emit_indexed_i32_load(destination, XReg::X17);
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_checked_i32_load(
        &mut self,
        destination: WReg,
        ptr: XReg,
        len: XReg,
        index: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.emit_index_in_bounds_check(XReg::X16, len)?;
        self.emit_indexed_i32_load(destination, ptr);
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_indexed_i32_load(
        &mut self,
        destination: WReg,
        ptr: XReg,
    ) {
        self.encoder.emit_lsl_x_imm(XReg::X16, XReg::X16, 2);
        self.encoder.emit_ldr_w_reg(destination, ptr, XReg::X16);
    }

    pub(in crate::backend::codegen::values) fn emit_checked_parameter_usize_load(
        &mut self,
        destination: XReg,
        ptr_word_index: usize,
        index: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let len_word_index = ptr_word_index
            .checked_add(1)
            .ok_or_else(|| indexed_load_diagnostic("parameter length word index overflows"))?;
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.emit_parameter_word_to_x(len_word_index, XReg::X17)?;
        self.emit_index_in_bounds_check(XReg::X16, XReg::X17)?;
        self.emit_parameter_word_to_x(ptr_word_index, XReg::X17)?;
        self.emit_indexed_usize_load(destination, XReg::X17);
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_checked_local_usize_load(
        &mut self,
        destination: XReg,
        ptr_word_index: usize,
        index: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let len_word_index = ptr_word_index
            .checked_add(1)
            .ok_or_else(|| indexed_load_diagnostic("local length word index overflows"))?;
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.emit_local_word_to_x(len_word_index, XReg::X17)?;
        self.emit_index_in_bounds_check(XReg::X16, XReg::X17)?;
        self.emit_local_word_to_x(ptr_word_index, XReg::X17)?;
        self.emit_indexed_usize_load(destination, XReg::X17);
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_checked_usize_load(
        &mut self,
        destination: XReg,
        ptr: XReg,
        len: XReg,
        index: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.emit_index_in_bounds_check(XReg::X16, len)?;
        self.emit_indexed_usize_load(destination, ptr);
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_indexed_usize_load(
        &mut self,
        destination: XReg,
        ptr: XReg,
    ) {
        self.encoder.emit_lsl_x_imm(XReg::X16, XReg::X16, 3);
        self.encoder.emit_adds_x(XReg::X17, ptr, XReg::X16);
        self.encoder.emit_ldr_x_imm(destination, XReg::X17, 0);
    }

    pub(in crate::backend::codegen::values) fn emit_checked_str_slice_index_to_x_pair(
        &mut self,
        source: SliceLocation,
        index: &UsizeValue,
        ptr_destination: XReg,
        len_destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_checked_str_slice_element_address(source, index)?;
        self.encoder.emit_ldr_x_imm(ptr_destination, XReg::X8, 0);
        self.encoder.emit_ldr_x_imm(len_destination, XReg::X8, 8);
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_checked_str_slice_element_address(
        &mut self,
        source: SliceLocation,
        index: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_value_to_x(index, XReg::X16)?;
        match source {
            SliceLocation::Parameter(ptr_word_index) => {
                let len_word_index = ptr_word_index.checked_add(1).ok_or_else(|| {
                    indexed_load_diagnostic("parameter slice length word index overflows")
                })?;
                self.emit_parameter_word_to_x(len_word_index, XReg::X17)?;
                self.emit_index_in_bounds_check(XReg::X16, XReg::X17)?;
                self.emit_parameter_word_to_x(ptr_word_index, XReg::X17)?;
            }
            SliceLocation::Local(ptr_word_index) => {
                let len_word_index = ptr_word_index.checked_add(1).ok_or_else(|| {
                    indexed_load_diagnostic("local slice length word index overflows")
                })?;
                self.emit_local_word_to_x(len_word_index, XReg::X17)?;
                self.emit_index_in_bounds_check(XReg::X16, XReg::X17)?;
                self.emit_local_word_to_x(ptr_word_index, XReg::X17)?;
            }
            SliceLocation::Return => {
                let (ptr, len) = self.slice_location_registers(source)?;
                if len != XReg::X17 {
                    self.encoder.emit_mov_x(XReg::X17, len);
                }
                self.emit_index_in_bounds_check(XReg::X16, XReg::X17)?;
                if ptr != XReg::X17 {
                    self.encoder.emit_mov_x(XReg::X17, ptr);
                }
            }
        }
        self.encoder.emit_lsl_x_imm(XReg::X16, XReg::X16, 4);
        self.encoder.emit_adds_x(XReg::X8, XReg::X17, XReg::X16);
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_index_in_bounds_check(
        &mut self,
        index: XReg,
        len: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_cmp_x(index, len);
        let in_bounds = self.emit_cond_branch_placeholder(BranchCondition::Cc);
        self.emit_trap();
        self.patch_branch_placeholder_to_current(in_bounds, "index in-bounds target")?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_bool_value_to_w(
        &mut self,
        value: &BoolValue,
        destination: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            BoolValue::Const(value) => {
                emit_mov_i32_to_w(&mut self.encoder, destination, i32::from(*value));
            }
            BoolValue::Location(location) => {
                if let BoolLocation::Parameter(index) = location {
                    self.emit_parameter_word_to_w(*index, destination)?;
                    return Ok(());
                }
                if let BoolLocation::Local(index) = location {
                    self.emit_local_word_to_w(*index, destination, LocalScalarWidth::Byte)?;
                } else {
                    let source = self.bool_location_register(*location)?;
                    if source != destination {
                        self.encoder.emit_mov_w(destination, source);
                    }
                }
            }
            BoolValue::SliceIndex { source, index } => {
                if let SliceLocation::Parameter(parameter_index) = *source {
                    self.emit_checked_parameter_byte_load(destination, parameter_index, index)?;
                    return Ok(());
                }
                if let SliceLocation::Local(local_index) = *source {
                    self.emit_checked_local_byte_load(destination, local_index, index)?;
                    return Ok(());
                }
                let (ptr, len) = self.slice_location_registers(*source)?;
                self.emit_checked_byte_load(destination, ptr, len, index)?;
            }
            BoolValue::StrComparison {
                operator,
                left,
                right,
            } => self.emit_str_comparison_to_w(*operator, left, right, destination)?,
            BoolValue::Not(_)
            | BoolValue::Logical { .. }
            | BoolValue::I32Comparison { .. }
            | BoolValue::UsizeComparison { .. }
            | BoolValue::BoolComparison { .. } => {
                let branches_to_false = self.emit_bool_false_branch_placeholders(value)?;
                emit_mov_i32_to_w(&mut self.encoder, destination, 1);
                let branch_to_end = self.emit_branch_placeholder();
                self.patch_branch_placeholders_to_current(
                    branches_to_false,
                    "bool false materialization target",
                )?;
                emit_mov_i32_to_w(&mut self.encoder, destination, 0);
                self.patch_branch_placeholder_to_current(
                    branch_to_end,
                    "bool materialization end target",
                )?;
            }
        }

        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_str_comparison_to_w(
        &mut self,
        operator: BoolComparisonOperator,
        left: &StrValue,
        right: &StrValue,
        destination: WReg,
    ) -> Result<(), Vec<Diagnostic>> {
        let (equal_result, not_equal_result) = match operator {
            BoolComparisonOperator::Equal => (true, false),
            BoolComparisonOperator::NotEqual => (false, true),
        };

        self.emit_str_len_to_x(left, XReg::X16)?;
        self.emit_str_len_to_x(right, XReg::X17)?;
        self.encoder.emit_cmp_x(XReg::X16, XReg::X17);
        let mut not_equal_branches = vec![self.emit_cond_branch_placeholder(BranchCondition::Ne)];

        self.encoder.emit_cmp_x_zero(XReg::X16);
        let equal_branch = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        emit_mov_u64_to_x(&mut self.encoder, XReg::X8, 0);

        let loop_start = self.encoder.position();
        self.emit_str_pointer_to_x(left, XReg::X16)?;
        self.encoder.emit_ldrb_w_reg(WReg::W16, XReg::X16, XReg::X8);
        self.emit_str_pointer_to_x(right, XReg::X17)?;
        self.encoder.emit_ldrb_w_reg(WReg::W17, XReg::X17, XReg::X8);
        self.encoder.emit_cmp_w(WReg::W16, WReg::W17);
        not_equal_branches.push(self.emit_cond_branch_placeholder(BranchCondition::Ne));

        self.encoder.emit_add_x_imm(XReg::X8, XReg::X8, 1);
        self.emit_str_len_to_x(left, XReg::X16)?;
        self.encoder.emit_cmp_x(XReg::X8, XReg::X16);
        let has_more = self.emit_cond_branch_placeholder(BranchCondition::Cc);
        self.patch_branch_placeholder_to_offset(
            has_more,
            loop_start,
            "string comparison loop target",
        )?;

        self.patch_branch_placeholder_to_current(equal_branch, "empty string equality target")?;
        emit_mov_i32_to_w(&mut self.encoder, destination, i32::from(equal_result));
        let end_branch = self.emit_branch_placeholder();

        self.patch_branch_placeholders_to_current(not_equal_branches, "string inequality target")?;
        emit_mov_i32_to_w(&mut self.encoder, destination, i32::from(not_equal_result));
        self.patch_branch_placeholder_to_current(end_branch, "string comparison end target")
    }

    pub(in crate::backend::codegen::values) fn emit_str_pointer_to_x(
        &mut self,
        value: &StrValue,
        destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            StrValue::StaticBytes(bytes) => {
                self.emit_static_data_address(destination, bytes);
            }
            StrValue::Location(StrLocation::Return) => {
                if destination != XReg::X0 {
                    self.encoder.emit_mov_x(destination, XReg::X0);
                }
            }
            StrValue::Location(StrLocation::Parameter(index)) => {
                self.emit_parameter_word_to_x(*index, destination)?;
            }
            StrValue::Location(StrLocation::Local(index)) => {
                self.emit_local_word_to_x(*index, destination)?;
            }
            StrValue::SliceIndex { source, index } => {
                let len_scratch = if destination == XReg::X17 {
                    XReg::X16
                } else {
                    XReg::X17
                };
                self.emit_checked_str_slice_index_to_x_pair(
                    *source,
                    index,
                    destination,
                    len_scratch,
                )?;
            }
            StrValue::ProcessArg { index } => {
                let len_scratch = if destination == XReg::X8 {
                    XReg::X17
                } else {
                    XReg::X8
                };
                self.emit_process_arg_to_x_pair(index, destination, len_scratch)?;
            }
            StrValue::ProcessEnvironmentName { index } => {
                let len_scratch = if destination == XReg::X8 {
                    XReg::X17
                } else {
                    XReg::X8
                };
                self.emit_process_environment_name_to_x_pair(index, destination, len_scratch)?;
            }
            StrValue::ProcessEnvironmentValue { index } => {
                let len_scratch = if destination == XReg::X8 {
                    XReg::X17
                } else {
                    XReg::X8
                };
                self.emit_process_environment_value_to_x_pair(index, destination, len_scratch)?;
            }
        }
        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_str_len_to_x(
        &mut self,
        value: &StrValue,
        destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            StrValue::StaticBytes(bytes) => {
                emit_mov_u64_to_x(&mut self.encoder, destination, bytes.len() as u64);
            }
            StrValue::Location(StrLocation::Return) => {
                if destination != XReg::X1 {
                    self.encoder.emit_mov_x(destination, XReg::X1);
                }
            }
            StrValue::Location(StrLocation::Parameter(index)) => {
                let len_index = pair_len_index(*index, "parameter str")?;
                self.emit_parameter_word_to_x(len_index, destination)?;
            }
            StrValue::Location(StrLocation::Local(index)) => {
                let len_index = pair_len_index(*index, "local str")?;
                self.emit_local_word_to_x(len_index, destination)?;
            }
            StrValue::SliceIndex { source, index } => {
                self.emit_checked_str_slice_index_to_x_pair(
                    *source,
                    index,
                    XReg::X16,
                    destination,
                )?;
            }
            StrValue::ProcessArg { index } => {
                self.emit_process_arg_to_x_pair(index, XReg::X16, destination)?;
            }
            StrValue::ProcessEnvironmentName { index } => {
                self.emit_process_environment_name_to_x_pair(index, XReg::X16, destination)?;
            }
            StrValue::ProcessEnvironmentValue { index } => {
                self.emit_process_environment_value_to_x_pair(index, XReg::X16, destination)?;
            }
        }
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_str_value_to_x_pair(
        &mut self,
        value: &StrValue,
        ptr_destination: XReg,
        len_destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            StrValue::StaticBytes(bytes) => {
                self.emit_static_data_address(ptr_destination, bytes);
                emit_mov_u64_to_x(&mut self.encoder, len_destination, bytes.len() as u64);
            }
            StrValue::Location(location) => {
                if let StrLocation::Parameter(index) = *location {
                    self.emit_parameter_word_pair_to_x_pair(
                        index,
                        ptr_destination,
                        len_destination,
                    )?;
                    return Ok(());
                }
                if let StrLocation::Local(index) = *location {
                    self.emit_local_word_pair_to_x_pair(index, ptr_destination, len_destination)?;
                } else {
                    let (ptr_source, len_source) = self.str_location_registers(*location)?;
                    self.emit_x_pair_to_x_pair(
                        ptr_source,
                        len_source,
                        ptr_destination,
                        len_destination,
                    )?;
                }
            }
            StrValue::SliceIndex { source, index } => {
                self.emit_checked_str_slice_index_to_x_pair(
                    *source,
                    index,
                    ptr_destination,
                    len_destination,
                )?;
            }
            StrValue::ProcessArg { index } => {
                self.emit_process_arg_to_x_pair(index, ptr_destination, len_destination)?;
            }
            StrValue::ProcessEnvironmentName { index } => {
                self.emit_process_environment_name_to_x_pair(
                    index,
                    ptr_destination,
                    len_destination,
                )?;
            }
            StrValue::ProcessEnvironmentValue { index } => {
                self.emit_process_environment_value_to_x_pair(
                    index,
                    ptr_destination,
                    len_destination,
                )?;
            }
        }

        Ok(())
    }

    pub(in crate::backend::codegen::values) fn emit_process_arg_to_x_pair(
        &mut self,
        index: &UsizeValue,
        ptr_destination: XReg,
        len_destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_value_to_x(index, XReg::X16)?;
        self.encoder.emit_ldr_x_imm(XReg::X17, XReg::X19, 0);
        self.emit_index_in_bounds_check(XReg::X16, XReg::X17)?;
        self.encoder.emit_lsl_x_imm(XReg::X16, XReg::X16, 3);
        self.encoder.emit_adds_x(XReg::X17, XReg::X19, XReg::X16);
        self.encoder.emit_ldr_x_imm(XReg::X16, XReg::X17, 8);

        emit_mov_u64_to_x(&mut self.encoder, XReg::X8, 0);
        let loop_start = self.encoder.position();
        self.encoder.emit_ldrb_w_reg(WReg::W3, XReg::X16, XReg::X8);
        self.encoder.emit_cmp_w_zero(WReg::W3);
        let done = self.emit_cond_branch_placeholder(BranchCondition::Eq);
        self.encoder.emit_add_x_imm(XReg::X8, XReg::X8, 1);
        let branch_to_loop = self.emit_branch_placeholder();
        self.patch_branch_placeholder_to_offset(
            branch_to_loop,
            loop_start,
            "process argument length loop target",
        )?;
        self.patch_branch_placeholder_to_current(done, "process argument length done target")?;

        self.emit_x_pair_to_x_pair(XReg::X16, XReg::X8, ptr_destination, len_destination)
    }

    pub(in crate::backend::codegen) fn emit_slice_value_to_x_pair(
        &mut self,
        value: &SliceValue,
        ptr_destination: XReg,
        len_destination: XReg,
    ) -> Result<(), Vec<Diagnostic>> {
        match value {
            SliceValue::StrBytes(text) => {
                self.emit_str_value_to_x_pair(text, ptr_destination, len_destination)?;
            }
            SliceValue::Location(location) => {
                if let SliceLocation::Parameter(index) = *location {
                    self.emit_parameter_word_pair_to_x_pair(
                        index,
                        ptr_destination,
                        len_destination,
                    )?;
                    return Ok(());
                }
                if let SliceLocation::Local(index) = *location {
                    self.emit_local_word_pair_to_x_pair(index, ptr_destination, len_destination)?;
                } else {
                    let (ptr_source, len_source) = self.slice_location_registers(*location)?;
                    self.emit_x_pair_to_x_pair(
                        ptr_source,
                        len_source,
                        ptr_destination,
                        len_destination,
                    )?;
                }
            }
        }

        Ok(())
    }
}
