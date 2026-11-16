//! Two-phase physical preservation; sparse arrays never become whole initialized loads.
use super::{Builder, Failure, mir};
use crate::identity::MachineId;
use crate::{
    MachineAddressId, MachineStackId, MachineStackObject, MachineStackPurpose, MachineValueId,
};

enum Snapshot<'a> {
    Complete {
        value: MachineValueId,
        destination: &'a mir::Place,
    },
    Sparse {
        scratch: MachineAddressId,
        part: &'a mir::Preserved,
    },
}

impl Builder<'_> {
    pub(super) fn repack(
        &mut self,
        source: &mir::Place,
        destination: &mir::Place,
        preserved: &[mir::Preserved],
        missing: &[Box<[mir::Projection]>],
    ) -> Result<(), Failure> {
        let mut snapshots = Vec::new();
        for part in preserved {
            match part.kind {
                mir::Preservation::Complete => snapshots.push(Snapshot::Complete {
                    value: self.load(&part.source)?,
                    destination: &part.destination,
                }),
                mir::Preservation::ArrayRemainder => {
                    let layout = self
                        .context
                        .layouts
                        .get(part.source.data)
                        .ok_or_else(Self::invalid)?;
                    let stack = MachineStackId::new(self.stack.len());
                    self.stack.push(MachineStackObject::new(
                        part.source.data,
                        layout.size(),
                        layout.alignment(),
                        MachineStackPurpose::Temporary,
                    ));
                    let scratch = self.slot_address(stack);
                    let from = self.place(&part.source)?;
                    self.copy_remaining_fields(&part.source, from, scratch, missing)?;
                    snapshots.push(Snapshot::Sparse { scratch, part });
                }
            }
        }
        for snapshot in snapshots {
            match snapshot {
                Snapshot::Complete { value, destination } => self.store(destination, value)?,
                Snapshot::Sparse { scratch, part } => {
                    let to = self.place(&part.destination)?;
                    self.copy_remaining_fields(&part.source, scratch, to, missing)?;
                }
            }
        }
        self.copy(source, destination)
    }
}
