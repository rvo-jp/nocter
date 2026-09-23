use nocter_machine::{
    MachineAggregate, MachineAggregateWrite, MachineBody, MachineValueId,
    MachineValueRepresentation,
};

use crate::{Arm64NocterAbi, Arm64ValueStorage, Arm64VirtualRegister};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Arm64DirectAggregateLane {
    Immediate(u64),
    General(Arm64VirtualRegister),
    Floating {
        register: Arm64VirtualRegister,
        bytes: u8,
    },
}

/// One aggregate construction proven expressible entirely in its result registers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Arm64DirectAggregatePlan {
    lanes: Box<[Arm64DirectAggregateLane]>,
    lane_bytes: Box<[u8]>,
}

impl Arm64DirectAggregatePlan {
    pub(crate) fn build(
        body: &MachineBody,
        aggregate: &MachineAggregate,
        result: MachineValueId,
        values: &[Arm64ValueStorage],
    ) -> Option<Self> {
        let result_registers = values.get(result.index())?.direct_registers()?;
        let lane_bytes = lane_bytes(aggregate.size(), result_registers.len())?;
        let mut lanes = vec![LaneBuilder::default(); result_registers.len()];
        for write in aggregate.writes() {
            apply_write(body, values, aggregate.size(), &mut lanes, *write)?;
        }
        Some(Self {
            lanes: lanes
                .into_iter()
                .map(LaneBuilder::finish)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            lane_bytes: lane_bytes.into_boxed_slice(),
        })
    }

    pub(crate) fn lanes(
        &self,
    ) -> impl ExactSizeIterator<Item = (Arm64DirectAggregateLane, u8)> + '_ {
        self.lanes
            .iter()
            .copied()
            .zip(self.lane_bytes.iter().copied())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct LaneBuilder {
    source: Option<Arm64DirectAggregateLane>,
    immediate: u64,
    written_bytes: u8,
}

impl LaneBuilder {
    fn write_tag(&mut self, byte: u8, value: u8) -> Option<()> {
        let word_bytes = u8::try_from(Arm64NocterAbi::word_size()).ok()?;
        if self.source.is_some() || byte >= word_bytes {
            return None;
        }
        let bit = 1_u8.checked_shl(u32::from(byte))?;
        if self.written_bytes & bit != 0 {
            return None;
        }
        self.written_bytes |= bit;
        self.immediate |= u64::from(value) << (u32::from(byte) * 8);
        Some(())
    }

    fn write_full(&mut self, source: Arm64DirectAggregateLane) -> Option<()> {
        if self.source.is_some() || self.written_bytes != 0 {
            return None;
        }
        self.source = Some(source);
        self.written_bytes = u8::MAX;
        Some(())
    }

    fn finish(self) -> Arm64DirectAggregateLane {
        self.source
            .unwrap_or(Arm64DirectAggregateLane::Immediate(self.immediate))
    }
}

fn apply_write(
    body: &MachineBody,
    values: &[Arm64ValueStorage],
    aggregate_size: u64,
    lanes: &mut [LaneBuilder],
    write: MachineAggregateWrite,
) -> Option<()> {
    match write {
        MachineAggregateWrite::Tag { offset, value } => {
            if offset >= aggregate_size {
                return None;
            }
            let lane = usize::try_from(offset / Arm64NocterAbi::word_size()).ok()?;
            let byte = u8::try_from(offset % Arm64NocterAbi::word_size()).ok()?;
            lanes.get_mut(lane)?.write_tag(byte, value)
        }
        MachineAggregateWrite::Value { offset, value } => {
            apply_value(body, values, aggregate_size, lanes, offset, value)
        }
        MachineAggregateWrite::RepeatedValue {
            offset,
            stride,
            count,
            value,
        } => {
            for index in 0..count {
                let offset = index.checked_mul(stride)?.checked_add(offset)?;
                apply_value(body, values, aggregate_size, lanes, offset, value)?;
            }
            Some(())
        }
    }
}

fn apply_value(
    body: &MachineBody,
    values: &[Arm64ValueStorage],
    aggregate_size: u64,
    lanes: &mut [LaneBuilder],
    offset: u64,
    value: MachineValueId,
) -> Option<()> {
    let MachineValueRepresentation::Stored { size, .. } = body.value(value)?.representation()
    else {
        return None;
    };
    if size == 0 {
        return Some(());
    }
    if !offset.is_multiple_of(Arm64NocterAbi::word_size())
        || offset.checked_add(size)? > aggregate_size
    {
        return None;
    }
    let first_lane = usize::try_from(offset / Arm64NocterAbi::word_size()).ok()?;
    match values.get(value.index())? {
        Arm64ValueStorage::Direct(registers) => {
            let source_bytes = lane_bytes(size, registers.len())?;
            for (lane_offset, (register, bytes)) in
                registers.iter().copied().zip(source_bytes).enumerate()
            {
                let lane = first_lane.checked_add(lane_offset)?;
                if destination_lane_bytes(aggregate_size, lane, lanes.len())? != bytes {
                    return None;
                }
                lanes
                    .get_mut(lane)?
                    .write_full(Arm64DirectAggregateLane::General(register))?;
            }
            Some(())
        }
        Arm64ValueStorage::Floating { register, bytes } => {
            if destination_lane_bytes(aggregate_size, first_lane, lanes.len())? != *bytes {
                return None;
            }
            lanes
                .get_mut(first_lane)?
                .write_full(Arm64DirectAggregateLane::Floating {
                    register: *register,
                    bytes: *bytes,
                })
        }
        Arm64ValueStorage::Omitted if size == 0 => Some(()),
        Arm64ValueStorage::Omitted | Arm64ValueStorage::Memory { .. } => None,
    }
}

fn lane_bytes(size: u64, count: usize) -> Option<Vec<u8>> {
    let expected = usize::try_from(size.div_ceil(Arm64NocterAbi::word_size())).ok()?;
    if expected != count {
        return None;
    }
    (0..count)
        .map(|lane| destination_lane_bytes(size, lane, count))
        .collect()
}

fn destination_lane_bytes(size: u64, lane: usize, count: usize) -> Option<u8> {
    if lane >= count {
        return None;
    }
    let offset = u64::try_from(lane)
        .ok()?
        .checked_mul(Arm64NocterAbi::word_size())?;
    u8::try_from((size - offset).min(Arm64NocterAbi::word_size())).ok()
}
