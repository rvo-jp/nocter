# JSON Implementation Boundary

## Purpose

This document owns the cross-responsibility implementation boundary for the v0.22.0 JSON module.
Public behavior belongs only to [JSON Values and Text](../../spec/28-json.md). Work order and
completion evidence belong to the [v0.22.0 milestone](../milestones/v0.22.0.md).

JSON remains standard-library source. No compiler stage may branch on the identity or source names
of the `std/json` module or its public `Value` and `Number` declarations, nor on RFC tokens or JSON
error codes. The compiler's unrelated JSON protocol-envelope crate remains a generic tooling
serialization mechanism and is not a standard-library implementation dependency.

## Responsibility Split

```text
UTF-8 input view
    -> JSON byte cursor
        -> one on-demand token decision
            -> owning parser state
                -> Value / Number / String / Vec / Map

Value
    -> one owning traversal plan
        -> one escaping and token-emission engine
            -> String sink or Writer sink
```

The byte cursor owns the only source offset. Number scanning, string escape decoding, parser
errors, and trailing-input validation consume that cursor rather than maintaining parallel byte
positions. The parser asks for the next grammatical item on demand; it does not first allocate a
token list or construct a second syntax tree.

The parser state owns the only partial JSON construction. An explicit `Vec<Frame>` contains open
array and object frames. A frame owns every accepted child and any pending object name. Closing a
frame transfers one completed `Value` either to its parent or to the root slot. Failure destroys
the current builder, frame stack, and root through ordinary ownership. No initialized-entry bitmap,
recursive host stack, caller-provided count, or recovery-side ownership table is allowed.

The generation side likewise has one traversal authority. Both String generation and Writer
generation consume the same scalar, number, container, separator, and string-escaping decisions.
A sink boundary chooses where emitted UTF-8 bytes go and classifies sink failure; it cannot choose
JSON spelling. A new output target must implement that sink contract instead of copying the JSON
traversal or escaping algorithm.

## Value and Number Boundary

`Value` is an ordinary enum whose recursive storage crosses existing owning-container boundaries.
`Vec<Value>` and `Map<String, Value>` have finite stored layouts because their element storage is
indirect. Their ordinary drop bodies remain responsible for dynamic child destruction. JSON does
not add a compiler-recognized recursive-layout or recursive-drop rule.

`Number` alone owns JSON number validity and exact token storage. The cursor may identify a
candidate byte range, but it cannot publish a Number until the complete JSON number grammar and
following-token boundary are valid. Integer conversion consumes the validated decimal components;
it does not re-run the lexical grammar or convert through host or target floating point.

`std/num` continues to own general integer APIs. JSON-specific exponent, fraction, and exact-token
rules stay with Number rather than expanding general numeric parsing around one serialization
format.

## Unicode Boundary

The input `&str` contract already proves UTF-8 validity. JSON owns escape syntax and UTF-16
surrogate-pair rules. The existing standard-internal UTF-8 responsibility owns scalar-to-UTF-8
encoding, so JSON must call one package-internal scalar encoder instead of introducing another
encoding table. That encoder accepts only Unicode scalar values and returns a fixed-size byte
encoding plus initialized length; it knows nothing about JSON escapes, strings, or allocation.

Decoded strings append the resulting bytes through String's validated mutation contract. Object
duplicate detection operates on the final decoded String, never on source escape spelling.

## Allocation and Failure Boundary

One recoverable core implements each allocating operation. Ordinary APIs adapt that core by
terminating only allocation-class failures; recoverable APIs flatten the private classification to
the public built-in `error` payload. Ordinary wrappers must not distinguish allocation from syntax
or destination failure by comparing public error-code text.

Private parser failure has two cases:

- JSON input failure, which must remain recoverable from ordinary `parse`;
- allocation failure, which ordinary `parse` terminates and `try_parse` returns.

Private writer failure has two cases:

- destination failure, which both `write` and `try_write` return;
- traversal-stack allocation failure, which `write` terminates and `try_write` returns.

String generation has only allocation-class failure after accepting a valid Value. The ordinary
wrapper terminates; the recoverable wrapper returns it.

The recoverable cores require one package-internal adapter from the active allocation context to a
`TryAllocator`. This adapter remains owned by `std/mem`, is unavailable outside the standard
package, and exists only so an ordinary aborting API can share a recoverable implementation. It
does not change the public rule that explicit recoverable allocation uses named `try_*` APIs.

All nested strings and collections created by `try_parse` inherit the supplied allocator. Existing
owning-buffer affinity then routes later growth and destruction without JSON inspecting allocator
fields. A parser or generator never reconstructs allocator identity from provenance text.

## Map Boundary

The parser uses only Map's semantic construction, lookup, and insertion contract. It checks a
decoded name before insertion and reports a duplicate rather than relying on replacement behavior.
It cannot inspect table buckets, hashes, seeds, capacity policy, or iteration placement.

Generation consumes a package-restricted semantic ordinal projection owned by Map. The projection
returns the same unspecified semantic entry sequence as readonly iteration but exposes no bucket,
hash, capacity, or dense-storage field. Keeping the container loan separate from the ordinal lets a
suspended JSON frame retain a child value loan without also retaining and moving the cursor that
created it. A future canonical JSON API must own a separate ordering contract.

## Existing Capabilities and Exact Gaps

| Need | Existing owner |
| --- | --- |
| valid UTF-8 input and byte views | `str` |
| owning UTF-8 construction and recoverable growth | `String` |
| owning nested sequences and explicit traversal stacks | `Vec<T>` |
| decoded-name lookup and owning object storage | `Map<String, Value>` and `Hash` |
| current and recoverable allocation policies | `std/mem` |
| recoverable destination output | `io.Writer` |
| move-only partial state and once-only cleanup | language ownership and drop |

Only two new standard-internal prerequisites are admitted:

1. a package-internal Unicode-scalar encoder in the existing UTF-8 responsibility;
2. a package-internal active-context `TryAllocator` adapter in the existing memory responsibility.

The numeric specification already declares `u8.checked(u64)` and `u8.truncate(u64)`. Phase 1 found
that the authored standard library and backend had not implemented that existing contract. Its one
source-private primitive role is owned by the general numeric/runtime boundary and is exercised
independently of JSON. It is not a JSON declaration, parser operation, or ABI.

JSON number scanning, parser frames, errors, traversal, and emission are module implementation,
not language or compiler gaps. `char`, `f32`, `f64`, reflection, derive metadata, a parser primitive,
and a JSON-specific ABI are not prerequisites.

## Phase 1 Realization

`Cursor` owns the only byte offset. Number scanning publishes one private `NumberShape` containing
sign, decimal digit, trailing-zero, and normalized scale facts. Exact integer projection consumes
that shape and the retained token; it does not scan number grammar again or use floating point.

The private `Attempt<T>` enum is the only lexical failure channel. Input and allocation failures
remain distinct until `Number.parse` or `Number.try_parse` applies public policy. The input-error
factory also returns `Attempt<T>`: constructing its offset-bearing message uses the operation's
`TryAllocator`, so a recoverable parse cannot escape into the current allocation context on an
error path.

The UTF-8 owner returns an opaque fixed-capacity scalar encoding and a borrow of only its
initialized prefix. JSON owns UTF-16 escape and surrogate decisions, then passes that proven UTF-8
view through String's existing validated mutation contract. Neither module reads the other's
representation.

## Phase 2 Realization

The parser has one active `ParserState` and one `Vec<Continuation>`. Active state owns the current
root value, array, or object plus its next grammatical obligation. A continuation owns only a
parent array or a parent object together with its decoded pending name. It is therefore impossible
for a stack entry to claim both “waiting for a child” and a separator phase, or for a child to have
two partial-container owners.

Starting a scalar produces a complete `Value`. Starting a container produces a new active state.
When a value completes, the parser pops exactly one continuation, transfers the value into its
container, and changes that container to its separator state. An empty continuation stack means the
complete value is the root; only trailing JSON whitespace may remain. Parsing never calls itself,
materializes a token sequence, or stores a per-node source range.

Arrays begin with a zero-capacity Vec bound to the selected allocator, objects begin with a Map
bound to that allocator, and the continuation stack uses the same allocator. Number text, decoded
strings, object names, error detail, child growth, and Map growth all receive or retain that
selection. Native qualification returns an explicitly page-allocated parse result and input error
from inside a different current region, proving that neither path silently captures current
storage.

Decoded names are checked through Map's semantic `contains_key` contract before insertion. JSON
does not inspect a hash, bucket, dense index, or replacement implementation. Once absence has been
established, the normal Map insertion contract owns allocation and publication.

Phase 2 also corrected a general checker boundary exposed by the implementation: recovery operands
now pass their expected success-payload type into direct generic calls, just as propagation operands
already did. This permits `generic_call() catch` and `generic_call() otherwise` to infer result-only
type parameters. The correction belongs to outcome-expression call planning and contains no JSON
knowledge.

## Phase 3 Realization

`GenerationFrame` is the only suspended traversal state. A value frame borrows one `Value`; an
array or object frame borrows its source container and owns the next semantic ordinal. Processing a
container frame first pushes its resumed ordinal and then its child value, so the LIFO stack emits
depth-first compact JSON without recursive calls. Frames never own user values and therefore add no
parallel cleanup authority.

Map owns package-restricted ordinal projection and delegates it to the private table's semantic
entry projection. This is an inter-module contract, not a representation escape: JSON receives only
`MapEntryRef`, and Map remains free to replace hashing, buckets, or dense storage while preserving
the same unspecified iteration semantics. Separating source and ordinal also avoids an invalid
self-referential state in which a frame would move an iterator while retaining a result loan from
that iterator.

`ByteSink.emit` is the only destination operation consumed by traversal and escaping. `StringSink`
and `WriterSink` contain destination adaptation only; neither sees `Value`, punctuation, Number, or
escape decisions. String escaping batches complete UTF-8 chunks in fixed local storage. Before a
multibyte scalar crosses the local capacity boundary, the current chunk is flushed, so an owning
String never receives a partial scalar even though Writer accepts arbitrary bytes.

`GenerationAttempt` preserves destination failure and traversal-stack allocation failure until a
public wrapper applies policy. `write` returns destination failure and terminates on stack
allocation failure; `try_write` returns either. String-sink failure is allocation failure by
construction, so `stringify` terminates and `try_stringify` returns it. None of these wrappers
compare public error-code text to reconstruct private failure classes.

## Phase 4 Realization

The runnable `json-normalize` package is a consumer of the public standard library, not another
JSON implementation layer. It obtains one UTF-8 path from the process API, reads one String through
`std/fs`, parses that String through `std/json`, and writes the compact result through the public
`Writer` contract. Usage and JSON failures are rendered through the built-in error surface. The
application contains no parser state, JSON cursor, traversal frame, sink adapter, filesystem
primitive, or operating-system binding.

Process qualification executes the package through the complete target session. It fixes three
observable boundaries: invalid command usage, successful normalization, and malformed-input error
reporting. Formatter qualification discovers the package through the existing repository-wide
runnable-example contract, so JSON gains no private formatting path or fixture-only source form.

Editor qualification opens the same application source used by native execution. Hover,
definition, signature help, and receiver completion are projected from the ordinary semantic
snapshot and public declarations. Separate recoverable-source qualification covers `try_parse`,
`try_stringify`, and `try_write`. Invalid public calls prove that diagnostics select user-source
arguments rather than internal parser or Writer-adapter declarations. The LSP contains no JSON
token parser, API list, source-text fallback, or phase-specific feature path.

## Enforcement

Implementation and review must reject:

- compiler or backend branches on the `std/json` module or declaration identities;
- token arrays or source trees allocated before DOM construction;
- a second byte offset maintained outside the cursor;
- recursive JSON input parsing on the native call stack;
- String and Writer paths with independent escaping or traversal logic;
- public error-code string matching used to recover private failure classes;
- duplicate-name handling delegated to Map replacement;
- JSON code that reads Map, String, Vec, allocator, or Writer representation fields;
- floating-point conversion used as the Number storage authority;
- a fixed nesting limit introduced only to accommodate an implementation shortcut.

## Phase 5 Stabilization

The final source qualification extends the native matrix across integer range and exponent
boundaries, non-JSON whitespace, duplicate decoded names, maximum and invalid Unicode scalars,
every compact control-escape class, deep values, explicit allocator affinity, and Writer failure.
These cases exercise the same cursor, Number shape, parser state, traversal, and escaping authority;
no adversarial-only parser or serializer exists.

Standard capacity failures now come from one package-internal memory function with the public code
`std.mem.capacity_overflow`. Vec, String, Map, and Set retain their own representation and growth
algorithms but cannot invent representation-specific capacity error identities. Formatting and
single-item iteration use generic copy and replacement helpers from `std/internal/ptr`, so neither
module reconstructs raw pointer operations locally or depends directly on public pointer
primitives.

The exact standard dependency graph is executable review evidence. Phase 5 removes the direct
`fmt -> ptr` and `iter -> ptr` edges, records the intentional `vec -> internal/safety` bounds-abort
edge, and rejects any unreviewed future cross-module dependency. Artifact identity and installed
home qualification remain release-preparation responsibilities.
