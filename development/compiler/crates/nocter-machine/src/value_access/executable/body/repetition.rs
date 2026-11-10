//! Fill already supplied Copy data through the same finite reverse traversal as array cleanup.
use super::{Builder, Failure};
use crate::{MachineLayoutKind, MachineOperationKind};
use nocter_mir::value_access as mir;

impl Builder<'_> {
    pub(super) fn repeat(
        &mut self,
        source: &mir::Place,
        destination: &mir::Place,
    ) -> Result<(), Failure> {
        let layout = self
            .context
            .layouts
            .get(destination.data)
            .ok_or_else(Self::invalid)?;
        if layout.size() == 0 {
            return Ok(());
        }
        let MachineLayoutKind::FixedArray {
            element,
            length,
            stride,
        } = layout.kind()
        else {
            return Err(Self::invalid());
        };
        if *element != source.data || *length <= 1 {
            return Err(Self::invalid());
        }
        let (length, stride) = (*length, *stride);
        let payload = self.load(source)?;
        let base = self.place(destination)?;
        let pointer = self.value(
            destination.data,
            true,
            MachineOperationKind::AddressOf { source: base },
        )?;
        let traversal = self.begin_array(length)?;
        let address = self.array_address(source.data, pointer, traversal.index, stride)?;
        self.emit(MachineOperationKind::Store {
            destination: address,
            value: payload,
        });
        self.end_array(traversal)
    }
}
