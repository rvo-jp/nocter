# Asynchronous Datagram I/O Boundary

This document owns the cross-crate design for adding executor-safe UDP operations without a second
socket representation or a source-level nonblocking mode. Exact public declarations remain owned
by `development/std/net/index.nct`, and observable behavior remains owned by the network guide.

## One Socket Owner

`UdpSocket` remains the sole public datagram owner. Synchronous and asynchronous operations borrow
the same descriptor and share one datagram-attempt model, error normalization, timeout model, and
terminal close path. There is no async socket wrapper, duplicated descriptor state, or hidden
process-global executor.

Socket creation, numeric bind, numeric peer selection, and address observation do not wait for
external progress. They remain immediate operations. Datagram send and receive can wait for kernel
buffer readiness, so their asynchronous forms return drive-safe futures and their synchronous
twins carry `blocking`.

The public naming rule is:

- `UdpSocket.bind`, `connect`, `local_address`, and `peer_address` are immediate;
- `send`, `send_to`, and `receive` are the canonical asynchronous names;
- `send_blocking`, `send_to_blocking`, and `receive_blocking` are synchronous twins;
- asynchronous `_with_timeout` methods take one explicit relative timeout;
- configured read and write timeouts affect only the synchronous twins.

An empty datagram is a successful message in both execution modes. Cancellation while waiting does
not consume a datagram or change descriptor ownership. A successful send remains atomic at the
public boundary; partial datagram success is never reported.

## Closed Target Operations

Generic `SyscallN` primitives remain conservatively `blocking` because a runtime syscall number
cannot prove whether invocation waits. The async substrate must not bypass that rule with a module
path, constant-number check, or trusted wrapper name.

v0.46.0 introduces closed semantic roles for the finite immediate datagram operations currently
implemented through generic syscall entry points. Each role fixes its source declaration,
signature, target support, Darwin syscall identity, fixed invocation flags, and carry/error
normalization. The target-specific standard adapter remains the sole owner of Darwin socket-address,
message-header, and option-value records; it passes typed pointers across the closed primitive
boundary, so the compiler never learns `NetworkAddress` or reconstructs standard-library data.
The initial role set covers:

- UDP descriptor opening and configuration as distinct ownership steps;
- numeric bind;
- numeric peer selection and completion observation;
- one nonwaiting connected or addressed send attempt;
- one nonwaiting receive attempt with source and truncation;
- local and peer address observation.

These roles perform one immediate attempt and return explicit ready, interrupted, would-block,
in-progress, failed, or malformed data as appropriate. They never poll, retry for readiness, or
construct a future. `nocter-runtime-contract` owns their invocation classification;
`nocter-target-program` validates exact source contracts; Machine retains only selected roles and
typed operands; the ARM64 backend owns fixed kernel-entry details. The target-specific standard
adapter owns native record construction and validation. Target-independent standard source owns
retry, deadline, and public error policy without knowing syscall numbers or record layouts.

## Shared Descriptor Policy

One package-internal datagram policy consumes the closed attempt results. Immediate setup retries
only interruption. A synchronous transfer waits through the existing blocking descriptor wait and
one fixed `OperationDeadline`. An asynchronous transfer uses the existing compiler-owned
descriptor-readiness future, retries the same immediate attempt, and never invokes the blocking
wait adapter.

The ordinary async methods use no deadline. Each `_with_timeout` method creates exactly one
monotonic deadline before its first attempt and publishes descriptor readiness and that deadline in
one wait set. A zero timeout still permits one immediate attempt. Waking, interruption, and retry do
not restart a relative duration.

Dropping a suspended send or receive removes its wait registration through the existing future
cancellation contract. The future contains no independent native operation owner, so cancellation
does not need a cleanup worker and cannot close or duplicate the borrowed socket. The socket remains
usable after cancellation.

## Information Flow

The responsibility direction is strictly downstream:

1. the runtime contract defines closed datagram operation identities and effects;
2. target closure binds exact standard declarations to those identities;
3. standard source interprets typed attempt results into retry, deadline, and public failure policy;
4. checked async bodies prove that only drive-safe readiness futures and nonblocking attempts are
   reachable;
5. MIR, Machine, ARM64 lowering, native execution, and editor presentation consume frozen choices
   without reclassifying effects or rediscovering operations from names.

No consumer may reconstruct a target role from a source path, primitive spelling, syscall number,
or emitted instruction sequence. No backend may decide public timeout or datagram semantics.

## Rejected Designs

- Marking the existing generic syscall wrappers nonblocking would make unrelated runtime syscall
  arguments silently inherit an invalid guarantee.
- Wrapping blocking UDP methods in `async` would block the executor while preserving a misleading
  future type.
- A second `AsyncUdpSocket` would duplicate ownership, timeout configuration, address observation,
  and close policy.
- Making bind or numeric peer selection lazy would add future ownership where no external wait
  exists.
- Inheriting mutable socket timeouts in async methods would make a future's deadline depend on
  unrelated earlier mutation; async deadlines remain explicit inputs.

## Completion Boundary

The boundary is complete only when generic syscalls are absent from datagram adapter source,
blocking and asynchronous policy share one attempt model, cancellation and timeout behavior cross
native execution, LSP features consume the checked public contracts, public examples exercise UDP
without raw nonblocking state, and a full review finds no duplicate ABI or effect authority.
