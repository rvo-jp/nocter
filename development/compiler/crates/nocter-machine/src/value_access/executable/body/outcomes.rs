//! The common layout owns outcome tag encoding and payload offsets.
use super::{Builder, Failure};
use crate::{
    MachineAddress, MachineAddressId, MachineAddressRoot, MachineAddressStep,
    MachineArithmeticCheck, MachineBinaryOperation, MachineBranchTarget, MachineConstant,
    MachineLayoutKind, MachineOperationKind, MachineOutcomeKind, MachineTerminator, MachineValueId,
};
use nocter_mir::value_access::Place;
use nocter_runtime_contract::RuntimeDataId;
use nocter_runtime_contract::RuntimePrimitive;
impl Builder<'_> {
    fn stored_outcome_tag(
        &mut self,
        base: MachineAddressId,
        ty: RuntimeDataId,
    ) -> Result<(MachineAddressId, MachineOutcomeKind), Failure> {
        let Some(MachineLayoutKind::Outcome {
            kind, tag_offset, ..
        }) = self.context.layouts.get(ty).map(crate::MachineLayout::kind)
        else {
            return Err(Self::invalid());
        };
        let (kind, offset) = (*kind, *tag_offset);
        let pointer = self.value(ty, true, MachineOperationKind::AddressOf { source: base })?;
        let data = self.primitive(RuntimePrimitive::Unsigned(8))?;
        let layout = self.context.layouts.get(data).ok_or_else(Self::invalid)?;
        let address = self.address(MachineAddress::new(
            data,
            layout.size(),
            layout.alignment(),
            MachineAddressRoot::Pointer { value: pointer },
            [MachineAddressStep::Offset(offset)],
        ));
        Ok((address, kind))
    }
    pub(super) fn variant_tag(&mut self, place: &Place, primary: bool) -> Result<(), Failure> {
        let base = self.place(place)?;
        self.store_outcome_tag(base, place.data, primary)
    }
    pub(super) fn store_outcome_tag(
        &mut self,
        base: MachineAddressId,
        data: RuntimeDataId,
        primary: bool,
    ) -> Result<(), Failure> {
        let (destination, kind) = self.stored_outcome_tag(base, data)?;
        let tag = if primary {
            kind.primary_tag()
        } else {
            kind.alternate_tag()
        };
        let value = self.value(
            self.primitive(RuntimePrimitive::Unsigned(8))?,
            false,
            MachineOperationKind::Constant(MachineConstant::Integer(tag.into())),
        )?;
        self.emit(MachineOperationKind::Store { destination, value });
        Ok(())
    }
    pub(super) fn primary_condition(
        &mut self,
        base: MachineAddressId,
        ty: RuntimeDataId,
    ) -> Result<MachineValueId, Failure> {
        let (source, kind) = self.stored_outcome_tag(base, ty)?;
        let data = self.primitive(RuntimePrimitive::Unsigned(8))?;
        self.tag_matches(source, data, u64::from(kind.primary_tag()))
    }
    pub(super) fn tag_matches(
        &mut self,
        source: MachineAddressId,
        data: RuntimeDataId,
        expected: u64,
    ) -> Result<MachineValueId, Failure> {
        let tag = self.value(data, false, MachineOperationKind::Load { source })?;
        let expected = self.value(
            data,
            false,
            MachineOperationKind::Constant(MachineConstant::Integer(expected.into())),
        )?;
        self.value(
            self.primitive(RuntimePrimitive::Bool)?,
            false,
            MachineOperationKind::Binary {
                operation: MachineBinaryOperation::Equal,
                left: tag,
                right: expected,
                check: MachineArithmeticCheck::NotRequired,
            },
        )
    }
    pub(super) fn require_variant(&mut self, place: &Place, primary: bool) -> Result<(), Failure> {
        let base = self.place(place)?;
        let condition = self.primary_condition(base, place.data)?;
        let next = self.block();
        let trap = self.block();
        self.terminate(MachineTerminator::Branch {
            condition,
            then_target: MachineBranchTarget::new(if primary { next } else { trap }, []),
            else_target: MachineBranchTarget::new(if primary { trap } else { next }, []),
        })?;
        self.current = trap;
        self.terminate(MachineTerminator::Unreachable)?;
        self.current = next;
        Ok(())
    }
}
