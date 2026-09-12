# nocter-darwin-event-queue

## Responsibility

Own the host compiler's unsafe Darwin `kqueue`/`kevent64` FFI boundary and expose a small safe
event-registration API to `nocter-darwin-reactor`.

## Contract

The crate accepts validated descriptor or process subjects, an opaque native token, and an optional
relative timeout. It owns the queue descriptor, initialized native records, pointer lifetimes,
count conversion, registration interruption retry, and operating-system errors. An interrupted
timed wait is returned so the absolute-deadline owner can recompute its duration. Event constants
and record layout are validated against `nocter-runtime-contract::DarwinEventAbiSchema`.

No task, registration-generation, future, process-owner, exit-status, or source-language concept
crosses this boundary. Generated Nocter executables do not link this crate; ARM64 lowering consumes
the same ABI schema independently.

## Safety Boundary

Workspace Rust forbids unsafe code except in this crate. Every unsafe operation is confined to one
native call or one owned-descriptor construction whose preconditions are established immediately
before it. Safe callers cannot supply pointers, buffer lengths, or native record fields.

## Invariants

- `EventQueue` exclusively owns and closes its queue descriptor.
- Safe callers select a typed filter and opaque token; invalid descriptor and process subjects
  return errors rather than reaching an unchecked conversion.
- Output storage is initialized for the exact native capacity before a wait and decoded only up to
  the returned count.
- Process-exit filters are one-shot observations. Missing-process and interrupted-wait
  classification is exposed without assigning either result a scheduler meaning.
- The runtime-contract schema is the sole numeric record authority; the local SDK binding is
  tested against every size, alignment, offset, filter, and flag consumed here.
