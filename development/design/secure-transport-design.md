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

## Darwin Provider Evaluation

The Darwin SDK exposes two materially different choices. Neither currently satisfies the complete
contract without changing one accepted release requirement.

### Secure Transport

Security.framework's Secure Transport API can operate over an already connected descriptor through
application-supplied read and write callbacks. It therefore composes with the existing TCP owner and
reactor. It provides system trust, explicit peer-domain verification, custom roots, and ALPN.

Its public protocol-version API admits TLS 1.2 as the highest legal version and the complete API is
deprecated on modern macOS. A closed callback bridge is also required: its C callbacks have an
in/out byte-count and `OSStatus` contract that is not equivalent to POSIX `read` or `write`. Passing
those functions directly would be ABI-incorrect. A bridge may be compiler-owned, but it must expose
only a fixed TLS service operation and fixed connection record—not general callback construction.

### Network.framework

Network.framework provides the supported modern TLS stack and TLS 1.3. Its public connection API
owns endpoint resolution, connection establishment, transport progress, dispatch scheduling, and
completion callbacks together with TLS. It cannot wrap Nocter's existing connected descriptor
through a public SDK contract. Adopting it only for HTTPS would therefore create a second DNS, TCP,
deadline, cancellation, and reactor model.

Replacing the complete network substrate with Network.framework would avoid that duplication, but
is a separate network architecture migration rather than a TLS provider addition. It would also
replace the already qualified descriptor-based TCP, UDP, listener, and structured-async boundary.

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

## Decision Gate

Implementation beyond the loader foundation requires one explicit product decision:

1. ship authenticated TLS 1.2 over the existing descriptor architecture using the deprecated but
   still available Secure Transport provider;
2. replace the broader Darwin network substrate with Network.framework before implementing HTTPS;
   or
3. first add a general native-object/static-archive link boundary and adopt a maintained embedded
   TLS provider.

The compiler must not hide this conflict behind a TLS-specific wrapper or claim TLS 1.3 from an API
whose public contract stops at TLS 1.2.

