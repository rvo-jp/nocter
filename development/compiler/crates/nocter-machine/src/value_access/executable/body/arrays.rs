//! Evaluated selections capture their checked numeric address at the selected operation.
use super::{Builder, Failure};
use crate::{
    MachineAddress, MachineAddressRoot, MachineAddressStep, MachineIndex, MachineIndexBound,
    MachineIndexCheck, MachineLayoutKind, MachineOperationKind,
};
use nocter_mir::value_access as mir;

impl Builder<'_> {
    /// The original normal selection proves last < length, so last + 1 <= length even when
    /// length is the maximum usize. This is a cleanup boundary, never an element address.
    pub(super) fn retained_successor(
        &mut self,
        id: nocter_model::MirLocalId,
        length: u64,
    ) -> Result<crate::MachineValueId, Failure> {
        let last = self.retained_index(id, length)?;
        let one = self.size_constant(1)?;
        self.size_binary(crate::MachineBinaryOperation::Add, last, one)
    }
    pub(super) fn index(
        &mut self,
        base: &mir::Place,
        index: &mir::Place,
        destination: &mir::Place,
        copy: bool,
        bounds: MachineIndexCheck,
    ) -> Result<(), Failure> {
        let Some(MachineLayoutKind::FixedArray {
            element,
            length,
            stride,
        }) = self
            .context
            .layouts
            .get(base.data)
            .map(crate::MachineLayout::kind)
        else {
            return Err(Self::invalid());
        };
        if *element != destination.data {
            return Err(Self::invalid());
        }
        let (length, stride) = (*length, *stride);
        let base_address = self.place(base)?;
        let pointer = self.value(
            base.data,
            true,
            MachineOperationKind::AddressOf {
                source: base_address,
            },
        )?;
        let index = self.load(index)?;
        let layout = self
            .context
            .layouts
            .get(destination.data)
            .ok_or_else(Self::invalid)?;
        let (size, alignment) = (layout.size(), layout.alignment());
        let selected = self.address(MachineAddress::new(
            destination.data,
            size,
            alignment,
            MachineAddressRoot::Pointer { value: pointer },
            [MachineAddressStep::Index {
                index: MachineIndex::Value(index),
                stride,
                bound: MachineIndexBound::Fixed(length),
                check: bounds,
            }],
        ));
        // Materialize even a zero-sized Copy selection: its required bounds effect survives.
        let pointer = self.value(
            destination.data,
            true,
            MachineOperationKind::AddressOf { source: selected },
        )?;
        if copy {
            if size != 0 {
                let source = self.address(MachineAddress::new(
                    destination.data,
                    size,
                    alignment,
                    MachineAddressRoot::Pointer { value: pointer },
                    [],
                ));
                let value = self.value(
                    destination.data,
                    false,
                    MachineOperationKind::Load { source },
                )?;
                self.store(destination, value)?;
            }
        } else {
            let stack = *self
                .slots
                .get(&destination.slot)
                .ok_or_else(Self::invalid)?;
            let destination = self.slot_address(stack);
            self.emit(MachineOperationKind::Store {
                destination,
                value: pointer,
            });
        }
        Ok(())
    }
    pub(super) fn retained_index(
        &mut self,
        id: nocter_model::MirLocalId,
        length: u64,
    ) -> Result<crate::MachineValueId, Failure> {
        let slot = self
            .source
            .ok_or_else(Self::invalid)?
            .slots()
            .get(id)
            .ok_or_else(Self::invalid)?;
        let bound = match slot.key {
            nocter_mir::value_access::SlotKey::IndexSnapshot(index) => index.bound(),
            nocter_mir::value_access::SlotKey::ArrayFrontier(frontier) => frontier.bound(),
            _ => return Err(Self::invalid()),
        };
        let data = self
            .context
            .program
            .runtime()
            .types()
            .primitive(nocter_runtime_contract::RuntimePrimitive::Usize)
            .ok_or_else(Self::invalid)?;
        if bound != length
            || slot.storage != nocter_mir::value_access::Storage::Payload
            || slot.representations.as_ref() != [data]
        {
            return Err(Self::invalid());
        }
        self.load(&nocter_mir::value_access::Place {
            slot: id,
            root: data,
            data,
            projections: Box::new([]),
        })
    }
}
