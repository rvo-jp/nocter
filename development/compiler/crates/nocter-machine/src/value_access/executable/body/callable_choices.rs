//! Shared selected-case control for moving, invoking and destroying callable environments.
use super::{Builder, Failure};
use crate::{
    MachineAddress, MachineAddressId, MachineAddressRoot, MachineConstant, MachineLayoutKind,
    MachineOperationKind, MachineValueId,
};
use nocter_runtime_contract::{RuntimeDataId, RuntimePrimitive};

impl Builder<'_> {
    fn callable_tag_address(
        &mut self,
        base: MachineAddressId,
        data: RuntimeDataId,
    ) -> Result<(RuntimeDataId, MachineAddressId), Failure> {
        let Some(MachineLayoutKind::CallableAlternatives { cases, .. }) = self
            .context
            .layouts
            .get(data)
            .map(crate::MachineLayout::kind)
        else {
            return Err(Self::invalid());
        };
        let role = RuntimePrimitive::callable_tag(cases.len()).ok_or_else(Self::invalid)?;
        let tag = self.primitive(role)?;
        let layout = self.context.layouts.get(tag).ok_or_else(Self::invalid)?;
        let pointer = self.value(data, true, MachineOperationKind::AddressOf { source: base })?;
        let address = self.address(MachineAddress::new(
            tag,
            layout.size(),
            layout.alignment(),
            MachineAddressRoot::Pointer { value: pointer },
            [],
        ));
        Ok((tag, address))
    }
    pub(super) fn callable_tag(
        &mut self,
        base: MachineAddressId,
        data: RuntimeDataId,
    ) -> Result<MachineValueId, Failure> {
        let (tag, source) = self.callable_tag_address(base, data)?;
        self.value(tag, false, MachineOperationKind::Load { source })
    }
    pub(super) fn store_callable_tag(
        &mut self,
        base: MachineAddressId,
        data: RuntimeDataId,
        index: u64,
    ) -> Result<(), Failure> {
        let Some(MachineLayoutKind::CallableAlternatives { cases, .. }) = self
            .context
            .layouts
            .get(data)
            .map(crate::MachineLayout::kind)
        else {
            return Err(Self::invalid());
        };
        if usize::try_from(index)
            .ok()
            .is_none_or(|index| index >= cases.len())
        {
            return Err(Self::invalid());
        }
        let (tag, destination) = self.callable_tag_address(base, data)?;
        let value = self.value(
            tag,
            false,
            MachineOperationKind::Constant(MachineConstant::Integer(i128::from(index))),
        )?;
        self.emit(MachineOperationKind::Store { destination, value });
        Ok(())
    }
    pub(super) fn select_callable_case(
        &mut self,
        base: MachineAddressId,
        data: RuntimeDataId,
        action: impl FnMut(&mut Self, usize) -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        let Some(MachineLayoutKind::CallableAlternatives { cases, .. }) = self
            .context
            .layouts
            .get(data)
            .map(crate::MachineLayout::kind)
        else {
            return Err(Self::invalid());
        };
        let tags = (0_u64..).take(cases.len()).map(i128::from);
        let subject = self.callable_tag(base, data)?;
        self.select_integer_cases(subject, tags, action)
    }
}
