use super::{
    Action, Address, AddressBase, Alternative, BitValue, Body, Effect, FLAG_WORD_BYTES, Failure,
    Frame, SlotLayout, StorageProgram,
};
use crate::identity::MachineId;
use crate::{MachineLayoutKind, MachineLayoutPlan, MachineValueCallableAbi};
use nocter_mir::value_access as mir;
use nocter_model::AccessInstanceId;
use nocter_model::ArenaBuilder;
use std::collections::BTreeMap;

pub(super) fn program(program: &mir::StorageProgram) -> Result<StorageProgram, Failure> {
    let roots = program.bodies().values().flat_map(|body| {
        body.slots()
            .iter()
            .flat_map(|(_, slot)| slot.representations.iter().copied())
            .chain(body.signature().ordered().map(|input| input.data))
            .chain(std::iter::once(body.signature().result()))
    });
    let layouts =
        MachineLayoutPlan::from_runtime(program.runtime(), roots).map_err(Failure::Layout)?;
    let bodies = program
        .bodies()
        .iter()
        .map(|(&id, source)| Ok((id, body(program, &layouts, id, source)?)))
        .collect::<Result<_, Failure>>()?;
    Ok(StorageProgram {
        layouts: layouts.finish(),
        bodies,
    })
}

fn aligned(value: u64, alignment: u64) -> Result<u64, Failure> {
    crate::transport::align_up(value, alignment).map_err(Failure::Abi)
}

fn frame(
    layouts: &MachineLayoutPlan,
    id: AccessInstanceId,
    source: &mir::Body,
) -> Result<Frame, Failure> {
    let invalid = Failure::InvalidBody(id);
    let overflow = Failure::FrameOverflow(id);
    let target = layouts.target();
    let mut cursor = 0;
    let mut alignment = target.stack_alignment();
    let mut zero = false;
    let mut slots = ArenaBuilder::new();
    for (expected, slot) in source.slots().iter() {
        if slot.representations.is_empty() {
            return Err(invalid);
        }
        let (size, slot_alignment) = match slot.storage {
            mir::Storage::Address => (target.pointer_size(), target.pointer_alignment()),
            mir::Storage::Payload => {
                let mut size = 0;
                let mut alignment = 1;
                for &data in &slot.representations {
                    let layout = layouts.get(data).ok_or(invalid)?;
                    size = size.max(layout.size());
                    alignment = alignment.max(layout.alignment());
                }
                (size, alignment)
            }
        };
        let offset = if size == 0 {
            zero = true;
            0
        } else {
            aligned(cursor, slot_alignment)?
        };
        if size != 0 {
            cursor = offset.checked_add(size).ok_or(overflow)?;
        }
        alignment = alignment.max(slot_alignment);
        let actual = slots.insert(SlotLayout {
            offset,
            size,
            alignment: slot_alignment,
            storage: slot.storage,
        });
        if actual != expected {
            return Err(invalid);
        }
    }
    let flags_offset = if source.flags().is_empty() {
        None
    } else {
        let offset = aligned(cursor, FLAG_WORD_BYTES)?;
        let size = u64::try_from(source.flags().len())
            .ok()
            .and_then(|length| length.checked_mul(FLAG_WORD_BYTES))
            .ok_or(overflow)?;
        cursor = offset.checked_add(size).ok_or(overflow)?;
        alignment = alignment.max(FLAG_WORD_BYTES);
        Some(offset)
    };
    if zero {
        cursor = cursor.max(1);
    }
    Ok(Frame {
        slots: slots.finish(),
        size: aligned(cursor, alignment)?,
        alignment,
        flags_offset,
        initial_flags: source.flags().into(),
    })
}

struct Materializer<'a> {
    id: AccessInstanceId,
    source: &'a mir::Body,
    runtime: &'a nocter_runtime_contract::RuntimeEnvironment,
    layouts: &'a MachineLayoutPlan,
    frame: &'a Frame,
    addresses: BTreeMap<mir::Place, Address>,
}
impl Materializer<'_> {
    fn invalid(&self) -> Failure {
        Failure::InvalidBody(self.id)
    }
    fn address(&mut self, place: &mir::Place) -> Result<Address, Failure> {
        if let Some(address) = self.addresses.get(place) {
            return Ok(*address);
        }
        let invalid = self.invalid();
        let slot = self.source.slots().get(place.slot).ok_or(invalid)?;
        let home = self.frame.slots.get(place.slot).ok_or(invalid)?;
        if !slot.representations.contains(&place.root) {
            return Err(invalid);
        }
        let mut data = place.root;
        let mut relative = 0u64;
        for projection in &place.projections {
            match *projection {
                mir::Projection::Field(field) => {
                    let field = self.layouts.value_field(data, field).ok_or(invalid)?;
                    relative = relative.checked_add(field.offset()).ok_or(invalid)?;
                    data = field.ty();
                }
                mir::Projection::Environment => {
                    let Some(nocter_runtime_contract::RuntimeType::StaticCallable {
                        environment: Some(environment),
                    }) = self.runtime.types().get(data)
                    else {
                        return Err(invalid);
                    };
                    data = *environment;
                }
            }
        }
        if data != place.data {
            return Err(invalid);
        }
        let layout = self.layouts.get(data).ok_or(invalid)?;
        let root = self.layouts.get(place.root).ok_or(invalid)?;
        if relative
            .checked_add(layout.size())
            .is_none_or(|end| end > root.size())
            || !relative.is_multiple_of(layout.alignment())
        {
            return Err(invalid);
        }
        let (base, offset) = match home.storage {
            mir::Storage::Payload => {
                if root.size() > home.size || root.alignment() > home.alignment {
                    return Err(invalid);
                }
                (
                    AddressBase::Frame,
                    home.offset.checked_add(relative).ok_or(invalid)?,
                )
            }
            mir::Storage::Address => (
                AddressBase::Referent {
                    pointer_offset: home.offset,
                },
                relative,
            ),
        };
        let address = Address {
            identity: crate::MachineAddressId::new(self.addresses.len()),
            slot: place.slot,
            base,
            offset,
            data,
            size: layout.size(),
            alignment: layout.alignment(),
        };
        self.addresses.insert(place.clone(), address);
        Ok(address)
    }

    fn effects(&mut self, effects: &[mir::Effect]) -> Result<Box<[Effect]>, Failure> {
        effects
            .iter()
            .map(|effect| {
                Ok(match effect {
                    mir::Effect::Initialize(place) => Effect::Initialize(self.address(place)?),
                    mir::Effect::Transfer {
                        source,
                        destination,
                    }
                    | mir::Effect::Reinterpret {
                        source,
                        destination,
                    } => {
                        let reinterpret = matches!(effect, mir::Effect::Reinterpret { .. });
                        let source = self.address(source)?;
                        let destination = self.address(destination)?;
                        if source.size != destination.size
                            || (reinterpret && source.alignment != destination.alignment)
                            || (!reinterpret && source.data != destination.data)
                        {
                            return Err(self.invalid());
                        }
                        Effect::CopyBytes {
                            source,
                            destination,
                            bytes: source.size,
                        }
                    }
                    mir::Effect::Dispose { source, disposal } => {
                        let source = self.address(source)?;
                        let kind = self
                            .layouts
                            .get(source.data)
                            .ok_or_else(|| self.invalid())?
                            .kind();
                        let matches = match disposal {
                            mir::Disposal::Trivial => true,
                            mir::Disposal::Error => matches!(kind, MachineLayoutKind::ErrorHandle),
                            mir::Disposal::Record => {
                                matches!(kind, MachineLayoutKind::Struct { .. })
                            }
                            mir::Disposal::Capture => matches!(
                                kind,
                                MachineLayoutKind::StaticCallable {
                                    environment: Some(_)
                                }
                            ),
                        };
                        if !matches {
                            return Err(self.invalid());
                        }
                        Effect::Dispose {
                            source,
                            disposal: *disposal,
                        }
                    }
                    mir::Effect::End(place) => Effect::End(self.address(place)?),
                    mir::Effect::BindAddress {
                        destination,
                        source,
                    } => {
                        let source = self.address(source)?;
                        let declaration = self
                            .source
                            .slots()
                            .get(*destination)
                            .ok_or_else(|| self.invalid())?;
                        if !declaration.representations.contains(&source.data) {
                            return Err(self.invalid());
                        }
                        let destination = self
                            .frame
                            .slots
                            .get(*destination)
                            .ok_or_else(|| self.invalid())?;
                        if destination.storage != mir::Storage::Address {
                            return Err(self.invalid());
                        }
                        Effect::BindAddress {
                            destination: destination.offset,
                            source,
                        }
                    }
                })
            })
            .collect()
    }
    fn bits(&self, bits: &[mir::BitValue]) -> Result<Box<[BitValue]>, Failure> {
        bits.iter()
            .map(|bit| {
                if bit.word >= self.frame.initial_flags.len() || bit.mask.count_ones() != 1 {
                    return Err(self.invalid());
                }
                let offset = u64::try_from(bit.word)
                    .ok()
                    .and_then(|word| word.checked_mul(FLAG_WORD_BYTES))
                    .and_then(|offset| self.frame.flags_offset?.checked_add(offset))
                    .ok_or_else(|| self.invalid())?;
                Ok(BitValue {
                    offset,
                    mask: bit.mask,
                    value: bit.value,
                })
            })
            .collect()
    }
}

fn body(
    program: &mir::StorageProgram,
    layouts: &MachineLayoutPlan,
    id: AccessInstanceId,
    source: &mir::Body,
) -> Result<Body, Failure> {
    let frame = frame(layouts, id, source)?;
    let abi = MachineValueCallableAbi::build(source.signature().clone(), layouts)
        .map_err(Failure::Abi)?;
    let mut materializer = Materializer {
        id,
        source,
        runtime: program.runtime(),
        layouts,
        frame: &frame,
        addresses: BTreeMap::new(),
    };
    let actions = source
        .actions()
        .iter()
        .map(|(&at, action)| {
            let alternatives = action
                .alternatives
                .iter()
                .map(|alternative| {
                    Ok(Alternative {
                        tests: materializer.bits(&alternative.tests)?,
                        before: materializer.effects(&alternative.before)?,
                        before_writes: materializer.bits(&alternative.before_writes)?,
                        after: materializer.effects(&alternative.after)?,
                        after_writes: materializer.bits(&alternative.after_writes)?,
                    })
                })
                .collect::<Result<Box<[_]>, Failure>>()?;
            Ok((
                at,
                Action {
                    call_bindings: materializer.effects(&action.call_bindings)?,
                    alternatives,
                },
            ))
        })
        .collect::<Result<_, Failure>>()?;
    Ok(Body {
        frame,
        abi,
        actions,
    })
}
