# Time

This chapter defines the `std/time` API for durations, monotonic elapsed-time measurement, Unix
wall-clock observations, and blocking sleep. It does not expose a target clock identifier,
hardware tick, syscall number, or raw target time structure.

## Duration

`Duration` is the copyable, non-negative span declared by the compiler-checked
[`std/time` contract](index.nct).

The value is normalized. `subsecond_nanoseconds` is always less than 1,000,000,000.
`from_milliseconds`, `from_microseconds`, and `from_nanoseconds` split their input before scaling,
so every `u64` input is representable without intermediate overflow.

`checked_add` returns absence when the mathematical result exceeds `u64::MAX` whole seconds plus
999,999,999 nanoseconds. `checked_sub` returns absence when `other` is greater than `self`.
Neither operation traps for an arithmetic boundary. Equality and ordering compare mathematical
duration rather than private field layout.

## Instant

`Instant` is the opaque value declared by the compiler-checked
[`std/time` contract](index.nct) for one target monotonic-clock domain.

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

## System Time

`SystemTime` is a copyable wall-clock instant represented by signed whole seconds relative to
1970-01-01T00:00:00Z and a normalized non-negative nanosecond component smaller than one second.
The signed seconds use the mathematical floor around the epoch: one nanosecond before the epoch is
`seconds == -1` and `nanoseconds == 999999999`.

`from_unix_seconds` creates an instant with no fractional component. `from_unix_parts` rejects a
nanosecond component of one second or greater with `std.time.invalid_system_time`; it does not
silently carry excess nanoseconds into the seconds field.

`SystemTime.now` observes the target wall clock and returns `std.time.wall_clock_failed` when that
observation fails. Successive observations may be equal or move backward when the target clock is
adjusted. Programs measuring elapsed time must use `Instant` instead. Equality and ordering compare
the represented instant, not the private field layout.

## UTC Calendar

`UtcDateTime` represents UTC components in the proleptic Gregorian calendar. Its supported years
are 0000 through 9999 inclusive. Month and day validation follows Gregorian leap-year rules; hours
use the 24-hour clock, and seconds range from 00 through 59. Leap seconds are not values in this
calendar domain. Nanoseconds range from zero through 999,999,999.

`UtcDateTime.new` rejects an invalid component combination with
`std.time.invalid_utc_datetime`. Conversion to `SystemTime` is exact. Conversion from `SystemTime`
preserves nanoseconds and returns `std.time.utc_datetime_out_of_range` when the instant lies before
0000-01-01T00:00:00Z or after 9999-12-31T23:59:59.999999999Z. Dates before the Unix epoch use the
same Gregorian rules; there is no separate historical-calendar transition.

Equality and ordering compare the represented UTC instant. Component accessors expose the
validated calendar fields without reparsing or consulting a target clock.

## Blocking Sleep

The module namespace owns the blocking sleep operation declared in [`index.nct`](index.nct).

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

The compiler target contract provides only the closed facts needed to read a monotonic counter,
read its fixed frequency, compute a wrap-aware counter delta, and perform generic target syscalls.
It does not construct `Duration`, `SystemTime`, or `UtcDateTime`; implement sleep policy or calendar
arithmetic; classify public errors; or expose target time structures to user code.

Target-specific standard-library adapters own raw wall-clock and wait ABI layouts plus one target
operation. The target-independent `std/time` implementation owns normalization,
counter-to-duration conversion, rounding, chunking, interruption retry, and public failure policy.
Neither layer may rediscover the other layer's facts from source spelling or machine instructions.

## Non-goals

This contract does not add local time zones, daylight-saving rules, locale-dependent presentation,
async timers, scheduler integration, deadlines as a public type, periodic timers, `noblock`, or
`realtime`. RFC 3339 interchange is added by a later v0.36.0 phase.
