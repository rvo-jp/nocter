use std::collections::BTreeSet;
use std::fmt;
use std::ops::Bound;

use crate::{Arm64FloatRegister, Arm64NocterAbi, Arm64Register};

/// The physical ARM64 register bank required by one selected value lane.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Arm64RegisterClass {
    General,
    Floating,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Arm64VirtualRegister {
    index: usize,
    class: Arm64RegisterClass,
}

impl Arm64VirtualRegister {
    const fn new(index: usize, class: Arm64RegisterClass) -> Self {
        Self { index, class }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }

    #[must_use]
    pub const fn class(self) -> Arm64RegisterClass {
        self.class
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Arm64SpillSlotId(usize);

impl Arm64SpillSlotId {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64AllocatedLocation {
    GeneralRegister(Arm64Register),
    FloatRegister(Arm64FloatRegister),
    Spill(Arm64SpillSlotId),
}

/// Dense physical or spill assignment for one selected ARM64 function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64RegisterAllocation {
    locations: Box<[Arm64AllocatedLocation]>,
    preserved_registers: Box<[Arm64Register]>,
    spill_count: usize,
}

impl Arm64RegisterAllocation {
    #[must_use]
    pub fn location(&self, register: Arm64VirtualRegister) -> Option<Arm64AllocatedLocation> {
        self.locations.get(register.index).copied()
    }

    #[must_use]
    pub const fn preserved_registers(&self) -> &[Arm64Register] {
        &self.preserved_registers
    }

    #[must_use]
    pub const fn spill_count(&self) -> usize {
        self.spill_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LiveRange {
    start: usize,
    end: usize,
    class: Arm64RegisterClass,
}

/// Collects virtual-register definitions, CFG-liveness endpoints, and call positions after
/// selection. The selector must extend a range through every block where CFG liveness keeps it
/// alive; allocation deliberately does not reconstruct control flow from instruction order.
#[derive(Default)]
pub struct Arm64RegisterAllocationBuilder {
    ranges: Vec<LiveRange>,
    calls: BTreeSet<usize>,
    call_crossing: BTreeSet<Arm64VirtualRegister>,
}

impl Arm64RegisterAllocationBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ranges: Vec::new(),
            calls: BTreeSet::new(),
            call_crossing: BTreeSet::new(),
        }
    }

    /// Defines one virtual register at the selected-instruction position.
    #[must_use]
    pub fn define(&mut self, position: usize, class: Arm64RegisterClass) -> Arm64VirtualRegister {
        let register = Arm64VirtualRegister::new(self.ranges.len(), class);
        self.ranges.push(LiveRange {
            start: position,
            end: position,
            class,
        });
        register
    }

    /// Extends a register's interval through one use or CFG live-through endpoint.
    ///
    /// # Errors
    ///
    /// Rejects an unknown register or a use positioned before its definition.
    pub fn use_at(
        &mut self,
        register: Arm64VirtualRegister,
        position: usize,
    ) -> Result<(), Arm64RegisterAllocationError> {
        let range = self.ranges.get_mut(register.index).ok_or(
            Arm64RegisterAllocationError::UnknownVirtualRegister(register),
        )?;
        if range.class != register.class {
            return Err(Arm64RegisterAllocationError::UnknownVirtualRegister(
                register,
            ));
        }
        if position < range.start {
            return Err(Arm64RegisterAllocationError::UseBeforeDefinition {
                register,
                definition: range.start,
                usage: position,
            });
        }
        range.end = range.end.max(position);
        Ok(())
    }

    /// Records a call instruction. A range defined before and used after this position must survive
    /// caller-saved clobbers.
    pub fn record_call(&mut self, position: usize) {
        self.calls.insert(position);
    }

    /// Marks a register as live across a call from CFG dataflow. This is more precise than
    /// deriving the fact from one flattened instruction interval because unrelated branch arms
    /// may occupy positions between the interval endpoints.
    ///
    /// # Errors
    ///
    /// Rejects an unknown virtual register.
    pub fn mark_call_crossing(
        &mut self,
        register: Arm64VirtualRegister,
    ) -> Result<(), Arm64RegisterAllocationError> {
        if self
            .ranges
            .get(register.index)
            .is_none_or(|range| range.class != register.class)
        {
            return Err(Arm64RegisterAllocationError::UnknownVirtualRegister(
                register,
            ));
        }
        self.call_crossing.insert(register);
        Ok(())
    }

    #[must_use]
    pub fn finish(self) -> Arm64RegisterAllocation {
        allocate(self.ranges, &self.calls, &self.call_crossing)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Interval {
    register: Arm64VirtualRegister,
    start: usize,
    end: usize,
    crosses_call: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActiveInterval {
    interval: Interval,
    physical: Arm64PhysicalRegister,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Arm64PhysicalRegister {
    General(Arm64Register),
    Floating(Arm64FloatRegister),
}

fn allocate(
    ranges: Vec<LiveRange>,
    calls: &BTreeSet<usize>,
    call_crossing: &BTreeSet<Arm64VirtualRegister>,
) -> Arm64RegisterAllocation {
    let mut intervals = ranges
        .into_iter()
        .enumerate()
        .map(|(index, range)| Interval {
            register: Arm64VirtualRegister::new(index, range.class),
            start: range.start,
            end: range.end,
            crosses_call: call_crossing.contains(&Arm64VirtualRegister::new(index, range.class))
                || calls
                    .range((Bound::Excluded(range.start), Bound::Excluded(range.end)))
                    .next()
                    .is_some(),
        })
        .collect::<Vec<_>>();
    intervals.sort_unstable_by_key(|interval| (interval.start, interval.register));

    let mut locations = vec![None; intervals.len()];
    let mut active = Vec::<ActiveInterval>::new();
    let mut spill_count = 0;
    for interval in intervals {
        active.retain(|active| active.interval.end >= interval.start);
        if let Some(physical) = available_register(&active, interval) {
            locations[interval.register.index] = Some(allocated_location(physical));
            active.push(ActiveInterval { interval, physical });
            continue;
        }

        let victim = active
            .iter()
            .enumerate()
            .filter(|(_, active)| eligible(active.physical, interval))
            .max_by_key(|(_, active)| (active.interval.end, active.interval.register));
        if let Some((victim_index, victim)) = victim
            && victim.interval.end > interval.end
        {
            let victim = *victim;
            locations[victim.interval.register.index] =
                Some(Arm64AllocatedLocation::Spill(Arm64SpillSlotId(spill_count)));
            spill_count += 1;
            locations[interval.register.index] = Some(allocated_location(victim.physical));
            active[victim_index] = ActiveInterval {
                interval,
                physical: victim.physical,
            };
        } else {
            locations[interval.register.index] =
                Some(Arm64AllocatedLocation::Spill(Arm64SpillSlotId(spill_count)));
            spill_count += 1;
        }
    }

    let locations = locations
        .into_iter()
        .map(|location| location.expect("every dense interval receives one location"))
        .collect::<Vec<_>>();
    let preserved_registers = locations
        .iter()
        .filter_map(|location| match location {
            Arm64AllocatedLocation::GeneralRegister(register)
                if Arm64NocterAbi::is_callee_saved(*register) =>
            {
                Some(*register)
            }
            Arm64AllocatedLocation::GeneralRegister(_)
            | Arm64AllocatedLocation::FloatRegister(_)
            | Arm64AllocatedLocation::Spill(_) => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Arm64RegisterAllocation {
        locations: locations.into_boxed_slice(),
        preserved_registers: preserved_registers.into_boxed_slice(),
        spill_count,
    }
}

fn available_register(
    active: &[ActiveInterval],
    interval: Interval,
) -> Option<Arm64PhysicalRegister> {
    let is_available = |candidate| active.iter().all(|active| active.physical != candidate);
    match (interval.register.class(), interval.crosses_call) {
        (Arm64RegisterClass::General, crosses_call) => (0..31)
            .filter_map(Arm64Register::new)
            .filter(|register| {
                Arm64NocterAbi::is_allocatable(*register)
                    && (!crosses_call || Arm64NocterAbi::is_callee_saved(*register))
            })
            .map(Arm64PhysicalRegister::General)
            .find(|candidate| is_available(*candidate)),
        // The current ABI deliberately has no partially preserved SIMD register model. A
        // floating interval that survives a call therefore receives a spill slot.
        (Arm64RegisterClass::Floating, true) => None,
        (Arm64RegisterClass::Floating, false) => (0..32)
            .filter_map(Arm64FloatRegister::new)
            .filter(|register| Arm64NocterAbi::is_floating_allocatable(*register))
            .map(Arm64PhysicalRegister::Floating)
            .find(|candidate| is_available(*candidate)),
    }
}

fn eligible(register: Arm64PhysicalRegister, interval: Interval) -> bool {
    match (register, interval.register.class(), interval.crosses_call) {
        (Arm64PhysicalRegister::General(register), Arm64RegisterClass::General, crosses_call) => {
            !crosses_call || Arm64NocterAbi::is_callee_saved(register)
        }
        (Arm64PhysicalRegister::Floating(_), Arm64RegisterClass::Floating, false) => true,
        _ => false,
    }
}

const fn allocated_location(register: Arm64PhysicalRegister) -> Arm64AllocatedLocation {
    match register {
        Arm64PhysicalRegister::General(register) => {
            Arm64AllocatedLocation::GeneralRegister(register)
        }
        Arm64PhysicalRegister::Floating(register) => {
            Arm64AllocatedLocation::FloatRegister(register)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64RegisterAllocationError {
    UnknownVirtualRegister(Arm64VirtualRegister),
    UseBeforeDefinition {
        register: Arm64VirtualRegister,
        definition: usize,
        usage: usize,
    },
}

impl fmt::Display for Arm64RegisterAllocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 register allocation failed: {self:?}")
    }
}

impl std::error::Error for Arm64RegisterAllocationError {}
