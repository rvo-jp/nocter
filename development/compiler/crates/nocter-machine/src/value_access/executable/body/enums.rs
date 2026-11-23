//! Enum tag operations use the same physical field for conditions, dispatch and cleanup.
use super::{Builder, Failure};
use crate::{
    MachineAddressId, MachineLayout, MachineLayoutKind, MachineOperationKind, MachineScalar,
    MachineValueId,
};
use nocter_runtime_contract::{RuntimeDataId, RuntimePrimitive};

impl Builder<'_> {
    pub(super) fn enum_tag_address(
        &mut self,
        base: MachineAddressId,
        data: RuntimeDataId,
    ) -> Result<(RuntimeDataId, MachineAddressId), Failure> {
        if self.context.runtime.scalar_enum_variants(data).is_some() {
            return Ok((data, base));
        }
        let Some(MachineLayoutKind::ValueEnum {
            tag:
                MachineScalar::Integer {
                    bits,
                    signed: false,
                },
            tag_offset,
            ..
        }) = self.context.layouts.get(data).map(MachineLayout::kind)
        else {
            return Err(Self::invalid());
        };
        let tag_data = self.primitive(RuntimePrimitive::Unsigned(u16::from(*bits)))?;
        Ok((tag_data, self.offset_address(base, tag_data, *tag_offset)?))
    }

    pub(super) fn load_enum_tag(
        &mut self,
        base: MachineAddressId,
        data: RuntimeDataId,
    ) -> Result<MachineValueId, Failure> {
        let (tag_data, source) = self.enum_tag_address(base, data)?;
        self.value(tag_data, false, MachineOperationKind::Load { source })
    }
}
