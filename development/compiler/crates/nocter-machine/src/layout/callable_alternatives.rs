//! Inline finite callable choices. This layout supplies no active-case or execution proof.
use super::{
    LayoutBuilder, MachineLayout, MachineLayoutError, MachineLayoutKind, MachineScalar, align_up,
};
use nocter_runtime_contract::{RuntimeDataId, RuntimePrimitive};

impl LayoutBuilder<'_> {
    pub(super) fn callable_alternatives(
        &mut self,
        owner: RuntimeDataId,
        cases: &[RuntimeDataId],
    ) -> Result<MachineLayout, MachineLayoutError> {
        self.require_layouts(cases.iter().copied())?;
        let Some(RuntimePrimitive::Unsigned(bits)) = RuntimePrimitive::callable_tag(cases.len())
        else {
            return Err(MachineLayoutError::InvalidRepresentation(owner));
        };
        let bits =
            u8::try_from(bits).map_err(|_| MachineLayoutError::InvalidRepresentation(owner))?;
        let tag_bytes = u64::from(bits / 8);
        let mut alignment = tag_bytes;
        let mut payload_size = 0;
        for case in cases {
            let layout = self.layout(*case)?;
            alignment = alignment.max(layout.alignment());
            payload_size = payload_size.max(layout.size());
        }
        let payload_offset = align_up(tag_bytes, alignment, owner)?;
        let size = align_up(
            payload_offset
                .checked_add(payload_size)
                .ok_or(MachineLayoutError::LayoutOverflow(owner))?,
            alignment,
            owner,
        )?;
        Ok(MachineLayout {
            size,
            alignment,
            kind: MachineLayoutKind::CallableAlternatives {
                tag: MachineScalar::Integer {
                    bits,
                    signed: false,
                },
                payload_offset,
                cases: cases.into(),
            },
        })
    }
}
