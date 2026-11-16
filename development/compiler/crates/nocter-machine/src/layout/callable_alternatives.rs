//! Inline finite callable choices. This layout supplies no active-case or execution proof.
use super::{
    LayoutBuilder, MachineLayout, MachineLayoutError, MachineLayoutKind, MachineScalar, align_up,
};
use nocter_runtime_contract::RuntimeDataId;

impl LayoutBuilder<'_> {
    pub(super) fn callable_alternatives(
        &mut self,
        owner: RuntimeDataId,
        cases: &[RuntimeDataId],
    ) -> Result<MachineLayout, MachineLayoutError> {
        self.require_layouts(cases.iter().copied())?;
        let bits = tag_bits(cases.len());
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

fn tag_bits(count: usize) -> u8 {
    let largest = count.saturating_sub(1);
    if u8::try_from(largest).is_ok() {
        8
    } else if u16::try_from(largest).is_ok() {
        16
    } else if u32::try_from(largest).is_ok() {
        32
    } else {
        64
    }
}

#[cfg(test)]
mod tests {
    use super::tag_bits;
    #[test]
    fn tag_width_uses_case_indices_including_the_largest_host_domain() {
        for (count, bits) in [(2, 8), (256, 8), (257, 16), (65_536, 16), (65_537, 32)] {
            assert_eq!(tag_bits(count), bits);
        }
        if usize::BITS == 64 {
            assert_eq!(tag_bits(usize::MAX), 64);
            let boundary = usize::try_from(u64::from(u32::MAX) + 1).unwrap();
            assert_eq!(tag_bits(boundary), 32);
            assert_eq!(tag_bits(boundary + 1), 64);
        }
    }
}
