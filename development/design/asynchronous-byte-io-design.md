# Canonical Asynchronous Byte I/O Boundary

This document owns the cross-module design for generic asynchronous byte streams. Exact public
declarations and observable operation details remain in the checked `std/io` contracts and their
behavior guide. Future ownership and drive safety remain language contracts. Transport, HTTP, and
target implementation details remain with their existing owners.

## Problem

Nocter has executor-safe TCP, TLS, and HTTP body reads, but their common asynchronous behavior is
available only as concrete methods. Generic byte algorithms can name only the older synchronous
`Reader` and `Writer` interfaces. Those names no longer match the language-wide rule that an
unqualified operation name denotes the canonical asynchronous surface while a synchronous twin
ends in `_blocking`.

The absence of one async contract also makes `Response` own a private copy of whole-stream
collection. Adding another async stream would otherwise require another copy or a helper that knows
concrete transport types.

## Adopted Contract

`std/io.Reader` and `std/io.Writer` become the canonical asynchronous interfaces:

```nct
pub interface Reader {
    pub async method &+self.read(buffer: &+[u8]): usize!
    pub async default method &+self.read_to_end(): Vec<u8>!
    pub async default method &+self.read_to_string(): String!
}

pub interface Writer {
    pub async method &+self.write(value: &[u8]): void!
    pub async default method &+self.flush(): void!
    pub async default method &+self.write_text(text: &str): void!
    pub async default method &+self.write_line(text: &str): void!
}
```

The current synchronous contracts are renamed to `BlockingReader` and `BlockingWriter`; their
methods retain the explicit `_blocking` suffix. No aliases preserve the old interface meanings.
`File` and file-backed buffers initially implement only the blocking contracts. `TcpStream` and
`TlsStream` implement both execution surfaces. HTTP `Response` implements both reader contracts.

Execution kind remains part of each method declaration. `Reader` does not mean that any method
returning `future T` happens to qualify, and `BlockingReader` does not permit an async body to call
its methods. Interface witness validation remains the sole authority for matching execution kind,
receiver access, parameter and result types, effects, and provenance.

## Generic Defaults

The async `Reader` defaults own one whole-stream collection algorithm and UTF-8 validation step.
The async `Writer` defaults own text encoding, line termination, and the stateless flush default.
They dispatch only through their declared interface requirements and await every deferred
operation. They cannot know a transport, descriptor, timeout, reactor, HTTP framing state, or
concrete owner representation.

Transport-specific timeout methods remain inherent APIs. A generic `Reader` operation has no
portable authority to invent a relative or idle timeout, and a timeout wrapper must not repeatedly
restart one duration without an explicit contract. `Response` therefore retains its explicit
timeout collection methods but delegates ordinary collection to the interface default.

## Information and Ownership Flow

The dependency direction is:

1. the language defines async interface contracts, lazy futures, awaiting, and nonblocking drive;
2. `std/io` defines generic byte semantics and default algorithms;
3. concrete transports implement the exact interface requirements using their existing checked
   methods;
4. callers dispatch through frozen interface evidence and receive one owned future;
5. MIR and native lowering consume that dispatch and future lifecycle without rediscovering a
   stream kind.

A reader exclusively borrows its cursor and mutable output for the complete pending operation. A
writer exclusively borrows its cursor and immutably borrows input bytes until completion or
cancellation. Default methods keep scratch and result storage inside their own future frame and
destroy it exactly once on success, failure, or cancellation.

No `std/io` algorithm may branch on a concrete type or module path. No transport may reimplement
whole-stream collection. No compiler or editor layer may infer async behavior from the `Reader`
name or a method spelling.

## Rejected Alternatives

- Adding `AsyncReader` and `AsyncWriter` while retaining synchronous `Reader` and `Writer` would
  preserve an older naming rule opposite to every canonical network API.
- Making one interface effect-polymorphic would require call-site effect specialization that the
  language does not define and would weaken the universal nonblocking future invariant.
- Giving `File` an async witness backed by ordinary blocking reads would make a valid `future T`
  capable of blocking an executor thread.
- Putting timeout methods on the base interfaces would conflate whole-operation, idle, configured,
  and relative deadline policies.
- Keeping response-specific ordinary collection would leave two authorities for EOF, invalid read
  counts, allocation, and UTF-8 validation.

## Completion Boundary

The boundary is complete when synchronous contracts use only their explicit names, canonical async
interfaces drive TCP, TLS, and HTTP bodies through ordinary witness dispatch, generic defaults are
the sole ordinary collection and text-adapter implementations, native and editor integration cover
that dispatch, and a whole-area review finds no compatibility alias, concrete-type branch, blocking
future path, or duplicate collection policy.
