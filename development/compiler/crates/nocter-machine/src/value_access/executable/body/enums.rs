//! Enum tag operations use the same physical field for conditions, dispatch and cleanup.
use super::{Builder, Failure, mir};
use crate::{
    MachineAddressId, MachineConstant, MachineLayout, MachineLayoutKind, MachineOperationKind,
    MachineScalar, MachineValueId,
};
use nocter_runtime_contract::{RuntimeDataId, RuntimePrimitive};
#[cfg(test)]
mod tests;

impl Builder<'_> {
    pub(super) fn construct_enum(
        &mut self,
        destination: &mir::Place,
        tag: u64,
        fields: &[mir::Place],
    ) -> Result<(), Failure> {
        let data = destination.data;
        let base = self.place(destination)?;
        let layout = self.context.layouts.get(data).ok_or_else(Self::invalid)?;
        let members = match layout.kind() {
            MachineLayoutKind::ValueEnum { variants, .. } => variants
                .get(usize::try_from(tag).map_err(|_| Self::invalid())?)
                .ok_or_else(Self::invalid)?
                .fields(),
            MachineLayoutKind::Scalar(_)
                if self
                    .context
                    .runtime
                    .scalar_enum_variant(data, tag)
                    .is_some() =>
            {
                &[]
            }
            _ => return Err(Self::invalid()),
        };
        if members.len() != fields.len() {
            return Err(Self::invalid());
        }
        for ((_, member), source) in members.iter().zip(fields) {
            let field = self
                .context
                .layouts
                .get(member.ty())
                .ok_or_else(Self::invalid)?;
            if field.size() != 0 {
                let value = self.load(source)?;
                let destination = self.offset_address(base, member.ty(), member.offset())?;
                self.emit(MachineOperationKind::Store { destination, value });
            }
        }

        let (tag_data, destination) = self.enum_tag_address(base, data)?;
        let value = self.value(
            tag_data,
            false,
            MachineOperationKind::Constant(MachineConstant::Integer(i128::from(tag))),
        )?;
        self.emit(MachineOperationKind::Store { destination, value });
        Ok(())
    }

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
