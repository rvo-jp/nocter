//! Shared selected-case control for moving and destroying complete callable environments.
use super::{Builder, Failure};
use crate::{
    MachineAddress, MachineAddressId, MachineAddressRoot, MachineBlockId, MachineBranchTarget,
    MachineConstant, MachineLayoutKind, MachineOperationKind, MachineSwitchCase,
    MachineSwitchValue, MachineTerminator, MachineValueId,
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
        mut action: impl FnMut(&mut Self, usize) -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        let Some(MachineLayoutKind::CallableAlternatives { cases, .. }) = self
            .context
            .layouts
            .get(data)
            .map(crate::MachineLayout::kind)
        else {
            return Err(Self::invalid());
        };
        let blocks: Vec<MachineBlockId> = cases.iter().map(|_| self.block()).collect();
        let subject = self.callable_tag(base, data)?;
        let fallback = self.block();
        let done = self.block();
        let cases = blocks
            .iter()
            .enumerate()
            .map(|(index, block)| {
                Ok(MachineSwitchCase::new(
                    MachineSwitchValue::Integer(
                        i128::try_from(index).map_err(|_| Self::invalid())?,
                    ),
                    MachineBranchTarget::new(*block, []),
                ))
            })
            .collect::<Result<Box<[_]>, Failure>>()?;
        self.terminate(MachineTerminator::SwitchValue {
            subject,
            cases,
            fallback: MachineBranchTarget::new(fallback, []),
        })?;
        for (index, block) in blocks.into_iter().enumerate() {
            self.current = block;
            action(self, index)?;
            self.go(done)?;
        }
        self.current = fallback;
        self.terminate(MachineTerminator::Unreachable)?;
        self.current = done;
        Ok(())
    }
}
