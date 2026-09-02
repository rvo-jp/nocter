# Monotonic Time

This chapter defines the `std/time` API for durations, monotonic elapsed-time measurement, and
blocking sleep. It does not expose a target clock identifier, hardware tick, syscall number, or
calendar representation.

## Duration

`Duration` is a copyable, non-negative span of time:

```nct
pub copy struct Duration

construct Duration {
    pub noalloc func from_seconds(value: u64): Self
    pub noalloc func from_milliseconds(value: u64): Self
    pub noalloc func from_microseconds(value: u64): Self
    pub noalloc func from_nanoseconds(value: u64): Self
}

instance Duration {
    pub noalloc method &self.whole_seconds(): u64
    pub noalloc method &self.subsecond_nanoseconds(): u64
    pub noalloc method &self.is_zero(): bool
    pub noalloc method &self.checked_add(other: &Self): Self?
    pub noalloc method &self.checked_sub(other: &Self): Self?
    pub noalloc operator (&self == other: &Self): bool
    pub noalloc operator (&self < other: &Self): bool
}
```

The value is normalized. `subsecond_nanoseconds` is always less than 1,000,000,000.
`from_milliseconds`, `from_microseconds`, and `from_nanoseconds` split their input before scaling,
so every `u64` input is representable without intermediate overflow.

`checked_add` returns absence when the mathematical result exceeds `u64::MAX` whole seconds plus
999,999,999 nanoseconds. `checked_sub` returns absence when `other` is greater than `self`.
Neither operation traps for an arithmetic boundary. Equality and ordering compare mathematical
duration rather than private field layout.

## Instant

`Instant` is an opaque value from one target monotonic-clock domain:

```nct
pub copy struct Instant

construct Instant {
    pub noalloc func now(): Self
}

instance Instant {
    pub noalloc method &self.elapsed(): Duration
}
```

`Instant.now` cannot fail and does not allocate. Successive observations from one running process
never move backward. The clock may pause while the machine is suspended; callers must not infer
calendar time or time spent powered off.

`elapsed` returns the non-negative duration from the receiver to a fresh observation. The target
owns wrap-aware subtraction in its private counter domain. The result remains defined across one
counter wrap, provided less than one complete counter cycle elapsed. Every implemented target must
make one complete cycle longer than one hundred years.

Clock resolution may be coarser than one nanosecond. Conversion to `Duration` rounds down to the
greatest representable nanosecond value not later than the measured counter delta. No target tick
or frequency is observable through `Instant` or `Duration`.

## Blocking Sleep

The module namespace owns blocking sleep:

```nct
pub noalloc func sleep(duration: &Duration): void!
```

`sleep` returns only after the monotonic elapsed time since entry is at least `duration`, unless an
OS failure other than interruption is returned. A zero duration returns immediately. A positive
duration below the target wait resolution is rounded up so that it cannot become a zero wait.
Oversleep is permitted.

An interrupted target wait is not a public failure. The implementation remeasures monotonic elapsed
time, subtracts it from the requested duration, and waits for the remainder. It does not trust a
target-mutated timeout structure as the remaining-time authority. Another target error returns the
built-in error code `std.time.sleep_failed`.

`noalloc` guarantees only the absence of Nocter allocator requests. `sleep` blocks the current
thread and may perform target operations. This API does not imply `noblock`, `notrap`, `realtime`,
or another undeclared guarantee.

## Responsibility Boundaries

The compiler target contract may provide only the closed facts needed to read a monotonic counter,
read its fixed frequency, and compute a wrap-aware counter delta. It does not construct
`Duration`, implement sleep policy, classify public errors, or expose target values to user code.

The target-specific standard-library adapter owns raw wait ABI layout and one wait attempt. The
target-independent `std/time` implementation owns normalization, counter-to-duration conversion,
rounding, chunking, interruption retry, and public failure policy. Neither layer may rediscover the
other layer's facts from source spelling or machine instructions.

## Non-goals

v0.26.0 does not add wall-clock time, Unix timestamps, calendar dates, time zones, parsing or
formatting, async timers, scheduler integration, deadlines as a public type, periodic timers,
`noblock`, or `realtime`. Those contracts require separate milestones.
