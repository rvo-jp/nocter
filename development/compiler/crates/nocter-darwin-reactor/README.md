# nocter-darwin-reactor

## Responsibility

Implement and exercise the Darwin readiness boundary accepted by `nocter-task-runtime` without
owning task lifecycle or computation frames.

## Contract

The adapter projects generation-qualified logical registrations onto kqueue-backed descriptor
events and one monotonic timer domain. More than one logical registration may observe one native
descriptor. Native tokens are never reused, so a queued event cannot acquire the meaning of a
replacement descriptor.

This host implementation closes and tests native event semantics. Generated Nocter executables do
not link this Rust crate; ARM64 runtime generation must implement the same `Reactor` contract and
is qualified independently before v0.41.0 closes.

## Invariants

- Registering a logical interest either completes fully or retains no registration.
- Removing a registration invalidates its logical and native-token mapping before returning.
- Native deregistration failure can leave only harmless surplus observation; current logical
  registrations still filter every delivered event.
- Read closure and descriptor errors wake readable waiters; write closure and descriptor errors
  wake writable waiters so operation code can observe the actual result.
- Timer deadlines are fixed observations in the adapter clock domain and never restart after an
  interrupted or spurious poll.
