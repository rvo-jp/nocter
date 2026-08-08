use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_set_u8(
        &mut self,
        destination: U8Location,
        value: &U8Value,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination_register = self.u8_register_destination_or_scratch(destination)?;
        self.emit_u8_value_to_w(value, destination_register)?;
        self.emit_w_to_u8_location(destination_register, destination)
    }

    pub(in crate::backend::codegen) fn emit_set_bool(
        &mut self,
        destination: BoolLocation,
        value: &BoolValue,
    ) -> Result<(), Vec<Diagnostic>> {
        let destination_register = self.bool_register_destination_or_scratch(destination)?;
        self.emit_bool_value_to_w(value, destination_register)?;
        self.emit_w_to_bool_location(destination_register, destination)
    }

    pub(in crate::backend::codegen) fn emit_set_str(
        &mut self,
        destination: StrLocation,
        value: &StrValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_str_value_to_x_pair(value, XReg::X16, XReg::X17)?;
        self.emit_x_pair_to_str_location(XReg::X16, XReg::X17, destination)
    }

    pub(in crate::backend::codegen) fn emit_set_str_raw_parts(
        &mut self,
        destination: StrLocation,
        pointer: &UsizeValue,
        len: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_value_to_x(len, XReg::X17)?;
        self.emit_usize_value_to_x(pointer, XReg::X16)?;
        self.emit_x_pair_to_str_location(XReg::X16, XReg::X17, destination)
    }

    pub(in crate::backend::codegen) fn emit_set_str_subview(
        &mut self,
        destination: StrLocation,
        source: &StrValue,
        start: &UsizeValue,
        len: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_str_value_to_x_pair(source, XReg::X16, XReg::X17)?;
        self.emit_usize_value_to_x(start, XReg::X8)?;
        self.emit_usize_value_to_x(len, XReg::X17)?;
        self.encoder.emit_adds_x(XReg::X16, XReg::X16, XReg::X8);
        self.emit_x_pair_to_str_location(XReg::X16, XReg::X17, destination)
    }

    pub(in crate::backend::codegen) fn emit_set_slice(
        &mut self,
        destination: SliceLocation,
        value: &SliceValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_slice_value_to_x_pair(value, XReg::X16, XReg::X17)?;
        self.emit_x_pair_to_slice_location(XReg::X16, XReg::X17, destination)
    }

    pub(in crate::backend::codegen) fn emit_set_slice_raw_parts(
        &mut self,
        destination: SliceLocation,
        pointer: &UsizeValue,
        len: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_value_to_x(len, XReg::X17)?;
        self.emit_usize_value_to_x(pointer, XReg::X16)?;
        self.emit_x_pair_to_slice_location(XReg::X16, XReg::X17, destination)
    }
}
