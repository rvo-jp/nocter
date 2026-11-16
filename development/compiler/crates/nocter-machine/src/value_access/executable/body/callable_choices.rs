//! Stored tags select complete callable cases; no source or ownership inference happens here.
use super::{Builder, Failure};
use crate::{
    MachineAddress, MachineAddressId, MachineAddressRoot, MachineLayoutKind, MachineOperationKind,
    MachineValueId,
};
use nocter_runtime_contract::{RuntimeDataId, RuntimePrimitive};

impl Builder<'_> {
    pub(super) fn callable_tag(
        &mut self,
        base: MachineAddressId,
        data: RuntimeDataId,
    ) -> Result<MachineValueId, Failure> {
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
        let source = self.address(MachineAddress::new(
            tag,
            layout.size(),
            layout.alignment(),
            MachineAddressRoot::Pointer { value: pointer },
            [],
        ));
        self.value(tag, false, MachineOperationKind::Load { source })
    }
}
