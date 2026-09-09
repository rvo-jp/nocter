# Secure Transport Boundary

This document records the cross-responsibility constraints and provider evaluation for the secure
transport planned for v0.43.0. Exact public declarations will remain owned by the relevant
`development/std/**/index.nct`, and observable behavior will remain in the single standard-library
guide assigned by `development/std/README.md`.

## Outcome Boundary

Secure transport must authenticate the remote server, preserve the existing unique stream owner,
and drive synchronous and asynchronous progress without creating a second HTTP implementation.
HTTPS will reuse the URL, request policy, response decoder, body cursor, and response owner already
shared by the synchronous and asynchronous HTTP clients.

The public layer must not expose a native TLS context, certificate object, trust result, Darwin
status code, callback pointer, dispatch object, or loader symbol. The compiler must not expose a
general foreign-function or callback facility merely to reach a platform TLS provider.

## Required Provider Contract

A provider is acceptable only if it can uphold all of these requirements:

- server authentication against the operating-system trust store;
- hostname verification using the exact requested DNS host;
- a separately owned custom-root configuration without silently disabling system validation;
- TLS 1.2 at minimum, with any TLS 1.3 claim backed by an API contract and a native test;
- ALPN configuration and inspection sufficient to require `http/1.1`;
- one terminal release of every native TLS and trust object;
- logical progress results that distinguish complete, need-readable, need-writable, clean EOF, and
  stable failure without leaking provider status codes;
- cancellation that removes any outstanding readiness interest before releasing the stream; and
- composition with Nocter's existing descriptor, monotonic-deadline, reactor, and HTTP ownership
  authorities.

The provider owns record protection, handshake state, certificate-path evaluation, hostname
verification, and provider error classification. `std/tls` owns public values and stable Nocter
errors. TCP owns the descriptor. The reactor owns readiness registration. HTTP owns protocol
syntax and response framing.

## Darwin Provider Selection

The Darwin SDK exposes two materially different system providers. v0.43.0 selects
Network.framework and expands the release boundary to replace the existing Darwin TCP
implementation rather than retaining two connection engines.

### Secure Transport

Security.framework's Secure Transport API can operate over an already connected descriptor through
application-supplied read and write callbacks. It therefore composes with the existing TCP owner and
reactor. It provides system trust, explicit peer-domain verification, custom roots, and ALPN.

Its public protocol-version API admits TLS 1.2 as the highest legal version and the complete API is
deprecated on modern macOS. A closed callback bridge is also required: its C callbacks have an
in/out byte-count and `OSStatus` contract that is not equivalent to POSIX `read` or `write`. Passing
those functions directly would be ABI-incorrect. A bridge may be compiler-owned, but it must expose
only a fixed TLS service operation and fixed connection record—not general callback construction.

### Network.framework: selected

Network.framework provides the supported modern TLS stack and TLS 1.3. Its public connection API
owns endpoint resolution, connection establishment, transport progress, dispatch scheduling, and
completion callbacks together with TLS. It cannot wrap Nocter's existing connected descriptor
through a public SDK contract. Adopting it only for HTTPS would therefore create a second DNS, TCP,
deadline, cancellation, and reactor model.

Network.framework will therefore replace the complete public TCP stream and listener substrate
before HTTPS is enabled. The descriptor implementation remains qualification evidence until the
migration is complete, then is removed rather than retained as a compatibility path. Numeric
address values and the explicit resolver remain independent value services. UDP remains on its
datagram-specific descriptor substrate because it neither constructs nor backs a TCP stream.

This choice preserves Nocter's low-dependency distribution contract. Network.framework,
Security.framework, CoreFoundation.framework, libdispatch, and the Blocks runtime are operating-
system components on the supported Darwin target. Compilation still invokes no external compiler
or linker, and a generated executable requires no adjacent Nocter or third-party runtime.

### Embedded TLS

An embedded modern TLS implementation could preserve the current transport boundary and support
TLS 1.3. The current runtime-free executable pipeline has no native-object or static-archive link
contract, however. Adding one solely for TLS would introduce a bundled native runtime, new object
relocation and licensing responsibilities, and a security update channel. Implementing cryptography
directly in Nocter before the required numeric and constant-time foundations exist is not an
acceptable alternative.

## Executable Dependency Authority

The provider evaluation exposed an independent executable defect: Mach-O images previously encoded
one unconditional libSystem load and assigned every imported function dylib ordinal 1. That model
could not represent any framework-backed target service.

The runtime contract now gives each trusted import a logical library identity. The Mach-O writer is
the sole authority mapping those identities to concrete install names. It freezes one canonical
loaded-library sequence from the completed ARM64 program, and the same sequence drives:

- `LC_LOAD_DYLIB` command order;
- each dyld bind ordinal;
- command-count and command-size layout; and
- deterministic image identity.

ARM64 retains logical imports and pointer slots only. It does not know framework paths or ordinals.
A native generated-image test loads Security.framework and CoreFoundation.framework, creates a TLS
context, releases it, and exits without an external linker or bundled runtime.

## Selected Migration Boundary

The migration proceeds through one compiler-owned adapter with four closed responsibilities:

1. retain typed function and data imports required by the public Network.framework ABI;
2. materialize only the fixed block signatures used by the adapter;
3. translate callback completion into one fixed event sent through an owned reactor-visible
   datagram socketpair;
4. publish logical connection, transfer, cancellation, and release results to the standard
   library.

No source declaration may construct a block, import a native symbol, choose a dispatch queue, or
inspect an `nw_*` object. The adapter will migrate plain TCP before it exposes TLS so HTTPS cannot
introduce a second connection engine. Secure Transport and embedded TLS remain rejected
alternatives for this milestone.

The first adapter foundation now models the Darwin Blocks ABI once in the runtime contract. ARM64
can construct only a typed, one-pointer capture block whose descriptor size and signature pointer
are fixed by that schema. A generated executable passes such a block to Network.framework; the
framework invokes it and the callback updates its captured mailbox. This proves the actual block
calling convention without exposing general blocks or foreign callbacks to Nocter source. The
asynchronous boundary consumes a closed adapter-operation vocabulary before any connection object
becomes public.

The callback transport does not use a shared-memory mailbox plus a separate wake byte. Each
callback writes one fixed 40-byte record to an `AF_UNIX/SOCK_DGRAM` socketpair owned by its native
adapter object. The kernel datagram queue preserves complete message boundaries and callback order,
applies bounded backpressure, and exposes the read descriptor directly to the existing reactor.
Consequently no mutex, event-node allocation, pointer publication, or second readiness model exists.
A serial dispatch queue is mandatory. Any provider object placed in an event must be retained before
the send and becomes the consumer's responsibility only after a complete receive. Receiving the
final cancelled state does not by itself permit release: the callback sends the event before it
returns. The consumer must subsequently complete a synchronous barrier on the same serial dispatch
queue. Only this two-step fence proves that the final callback has returned, after which blocks,
queue, channel, and connection can be released.

A generated executable now copies a one-pointer block onto a serial dispatch queue, sends one event
from the callback thread, receives it as one datagram, and observes its payload. A second generated
executable creates and cancels a real secure TCP connection, retains callback error objects across
the channel, releases them after receipt, waits for the final cancelled state, completes the
same-queue barrier, and only then releases every connection, endpoint, parameter, queue, and channel
owner. Loader symbols, symbol kinds, libraries, and Block signatures now come from one closed
runtime catalog. ARM64 event transfer retries only interruption and aborts on EOF, short records, or
permanent channel failure because continuing after a lost ownership-bearing event cannot be safe.
The production operation surface consumes this policy rather than reproducing it or exposing it to
source code.

## Adapter Operation Authority

The runtime contract owns one checked lifecycle for both connection and listener owners. A native
owner progresses through `initialized`, `running`, `cancel requested`, `final state observed`,
`quiesced`, and `released`. The final provider state and the same-queue dispatch barrier are
separate transitions; neither can independently manufacture release authority. Connection
transfer operations are rejected for listener owners, and provider-state finality is derived from
the typed connection or listener state rather than supplied as a Boolean by a caller.

The closed operation vocabulary admits only owner creation or accepted-owner adoption, start,
event-descriptor observation, receive/send initiation, complete event receipt, cancellation, the
release barrier, and final release. It does not admit independent handler installation, arbitrary
queue selection, Block construction, or native retain/release from source code. The target adapter
applies this state machine while it materializes those operations.

The runtime import catalog contains the complete plain-TCP dependency families needed by that
surface: address endpoints, plain-TCP parameters, connections, listeners, dispatch data, effective
path endpoints, provider errors, and fixed message contexts. Exact Block signatures for connection
state, receive, send, listener state, and accepted-connection callbacks are likewise closed runtime
roles. A source-level primitive cannot choose a different loader symbol or callback signature.

Connection and listener owners share one fixed five-word native record: provider object, serial
queue, event reader, event writer, and lifecycle tag. Endpoint, parameter, Block, dispatch-data,
path, and error objects are operation-local and cannot silently acquire a second owner lifetime.
ARM64 initializes the record only after all resources exist, derives every legal transition from
the runtime operation authority, checks the current tag in generated code, and permits terminal
cleanup only from `quiesced`. Cleanup clears each resource slot and ends in `released`, so a second
release fails before it can touch native storage.
