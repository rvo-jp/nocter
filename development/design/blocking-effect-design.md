# Blocking Effect Boundary

This document owns the compiler boundary that carries the public
[`blocking` effect and universal future drive invariant](../../spec/language/asynchronous-computations.md)
from declarations through checking. It does not redefine which source programs are valid, list
crate internals, or assign standard-library signatures.

## Independent Facts

One callable has three orthogonal facts:

| Fact | Meaning | Initial owner |
|---|---|---|
| execution | invocation is immediate or creates deferred work | declaration lowering |
| allocation guarantee | invocation is proven not to request Nocter allocator storage | declaration contract plus checked effect proof |
| blocking effect | invocation may synchronously wait for external progress | authored contract plus checked effect proof |

Result provenance and computation-capture provenance remain separate from all three. No stage may
infer one fact from another, from a result type, or from a source name such as `_async`.

`future T` has one representation and one drive-safety invariant. The invariant is not an optional
fact attached to a particular future value. This prevents a local binding, aggregate field,
generic parameter, return, or task combinator from erasing whether the executor may safely drive
the computation.

## Closed Effect Flow

Declaration lowering records authored `blocking` separately from `CallableExecution`. Checking
builds one body-relation graph from already selected calls, interface dispatch, callable witnesses,
and ownership-owned cleanup dependencies. A least fixed point classifies each callable, closure,
and drop body as nonblocking or possibly blocking.

The fixed point has only positive propagation: a blocking primitive, blocking bodyless contract,
or edge to a possibly blocking target makes its owner possibly blocking. Recursive groups are
therefore independent of traversal order. The result validates:

- every public unqualified callable contract;
- every asynchronous body;
- every literal, operator, coercion, expansion, and drop body;
- every callable witness viewed through a nonblocking structural contract;
- every interface implementation against its selected requirement.

Every named callable, including a private source-backed helper, exposes blocking behavior in its
contract. Checking proves an unqualified body nonblocking, while `blocking` admits either result.
This makes direct calls and stored callable values consume the same declaration fact instead of
making a declaration type depend on later body inference. Closure bodies remain inferred against
their required structural callable contracts. A public contract/private implementation pair
preserves the authored effect exactly, while a nonblocking inherent method may safely implement a
`blocking` interface requirement.

The immutable checked effect table is the last blocking-classification product. MIR, Machine,
ARM64, the executor, LSP, and documentation presentation cannot inspect operations or names to
recompute it. MIR receives only accepted call and future edges. Editor features receive authored
contract facts and, when explicitly requested, checked implementation evidence through semantic
queries.

## Primitive Authority

`nocter-runtime-contract` owns two distinct facts for every closed primitive role:

1. whether invoking the primitive may synchronously wait;
2. for a role that constructs a future, whether every resume and cancellation entry satisfies the
   universal drive invariant.

Target contract validation compares source modifiers with the first fact and requires the second
fact for every primitive result whose outer structural type is `future`. Later target and native
lowering consume the selected role without reopening either decision.

Closed target services carry the same invocation fact. The target-service catalog marks Darwin
`getaddrinfo` as possibly blocking and `freeaddrinfo` as nonblocking; target closure rejects a
source declaration whose `blocking` modifier disagrees. The source resolver therefore propagates
one catalog-owned fact rather than inferring behavior from a foreign symbol or module path.

The v0.45.0 migration inventory classifies current roles as follows:

| Classification | Current roles | Required action |
|---|---|---|
| generic possibly-blocking entry | `Syscall0` through `Syscall6`, `SyscallPair0` | mark invocation blocking; never infer safety from the runtime syscall number |
| synchronous external barrier | `NetworkConnectionReleaseBarrier`, `NetworkListenerReleaseBarrier` | mark blocking and remove from any asynchronous drive/cancellation path before Phase 2 closes |
| synchronous callback receive | `NetworkConnectionReceiveEvent`, `NetworkListenerReceiveEvent` | retain for explicitly blocking policy only; asynchronous policy uses the distinct nonblocking try-receive roles |
| drive-safe future constructor | `DescriptorReadiness`, `DescriptorReadinessOrDeadline`, `MonotonicDeadline`, `TaskJoin` | certify constructor-call effect and future-drive safety separately |
| instantiated cleanup | `DropValueAtPointer` | consume the ownership-selected drop dependency rather than assigning a universal primitive blocking fact |
| closed nonwaiting operation | every remaining current role | certify nonblocking invocation; adding a role requires an explicit classification |

The generic syscall classification is intentionally conservative. If a public API needs a precise
nonblocking contract for a target operation currently hidden behind `SyscallN`, it must gain a
closed semantic primitive role or be expressed through an already certified operation. Checking
must not special-case constant syscall numbers, standard-module paths, or wrapper names.

Phase 2 has moved allocator page mapping, page release, descriptor close, hash-seed filling,
wall-clock observation, and timeout waiting to the closed `MemoryMap`, `MemoryUnmap`,
`DescriptorClose`, `EntropySeedFill`, `WallClockRead`, and `TimeoutWait` roles. Their Darwin
numbers, fixed native layouts, and result normalization now have one target-backend owner; standard
source no longer uses runtime syscall values to identify them. This migration is intentionally
separate from the remaining network lifecycle work: a narrower primitive role cannot make a
synchronous release barrier safe inside future cancellation.

Network callback receipt follows the same rule. The blocking receive roles require one complete
ownership-bearing datagram. The distinct `NetworkConnectionTryReceiveEvent` and
`NetworkListenerTryReceiveEvent` roles use a target-owned nonblocking receive flag and normalize an
empty channel to explicit result availability. Reactor readiness is therefore a wake signal and
optimization, not an unchecked precondition required to keep future drive nonblocking.

Every terminal connection and listener path uses one ownership-consuming disposal role. Explicit
close, failed setup or transfer, observed cancellation, and implicit destruction atomically remove
the owner from its source value before invoking that role. The invocation copies the closed owner
record into runtime storage, submits exactly one private cleanup worker to a global concurrent
queue, and returns. Only that worker performs blocking event drain and the serial callback-queue
barrier. Accepted connections already queued for a listener are transferred to the same disposal
boundary, so no terminal path can forget retained provider ownership or accidentally wait on an
executor thread. Public wrappers use ordinary structural field destruction instead of repeating
the provider lifecycle.

Asynchronous host setup also stays inside the provider lifecycle. Standard source owns validation
and NUL-terminated host/service storage, while distinct closed primitive roles select numeric or
host endpoint creation. Network.framework owns DNS progress and address selection after the future
starts. TLS extends the same host constructor with authentication parameters; it does not run a
second resolver or reproduce connection fallback policy in source.

The explicitly synchronous resolver remains a separate API and calls the blocking `getaddrinfo`
target service. `net.resolve`, `net.try_resolve`, synchronous host-based TCP and TLS constructors,
and synchronous HTTP sends propagate that effect. None of those annotations are reused to
classify the provider-backed asynchronous path.

## Standard-Library Migration Inventory

The following current families contain synchronous external waits and must expose or propagate
`blocking` before their public names are normalized:

| Family | Waiting boundary |
|---|---|
| `std/fs` and file-backed `std/io` | file open, metadata, directory enumeration, read, and write |
| terminal and stream `std/io` | descriptor read/write and input waits |
| `std/time.sleep` | clock progress |
| `std/process` | fork/exec reporting, pipe progress, polling, and child completion |
| synchronous `std/net` | resolution, connect, accept, descriptor transfer, and configured timeout waits |
| synchronous `std/tls` and `std/http` | the underlying resolver, connection, transfer, and timeout paths |

Pure formatting, in-memory buffering, URL parsing, HTTP codec work, calendar conversion,
monotonic-counter reads, and collection operations do not intrinsically wait. Interface
abstractions such as `BlockingReader` and `BlockingWriter` must nevertheless state the effect
admitted by their requirement. Without effect polymorphism, a generic algorithm checked through such a requirement
conservatively retains `blocking`, even when one later concrete witness is memory-only. The
declaration contract remains authoritative; a call site does not specialize or recompute it.

The Network.framework release barrier synchronously waits, but no future drive or cancellation
path invokes it. Every terminal path transfers the owner to the nonwaiting disposal primitive;
only the private cleanup worker drains callbacks and crosses the barrier. The barrier's source
declaration remains `blocking`, so a future path cannot regain it without failing effect checking.

## Rejected Models

- `noblock async func` loses its guarantee when invocation produces plain `future T`.
- `noblock future T` creates two future kinds and makes generic task composition carry an avoidable
  effect dimension.
- treating every `async` spelling or `_async` name as proof lets body and primitive behavior bypass
  checking.
- classifying emitted syscalls in ARM64 would move a semantic decision downstream and disagree with
  editor analysis.
- assuming implicit destruction is nonblocking would leave cancellation safety dependent on callers
  remembering an unstated precondition.

## Phase Boundary

Phase 0 closed the public contract and inventory. Phase 1 changed syntax, model, lowering,
checking, and presentation contracts together and established the effect table before any
standard-library normalization. Phase 2 validates every registered primitive declaration against
the catalog's exact blocking classification and propagates synchronous waits through standard
contracts and their private helper chains. This order prevents names from claiming nonblocking
behavior before the compiler can prove it.
