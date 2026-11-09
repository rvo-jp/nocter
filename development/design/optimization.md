# Optimization Experiments

## Objective and Boundaries

Remove unnecessary runtime work through coherent source semantics, high-level information and
checked transformations. Nocter owns its optimization pipeline without depending on LLVM.
Implementation effort and short-term delivery do not set the language model. Relative speed
against another language is neither a semantic rule nor a required development gate.
No v2.0.0 runtime advantage or general optimality is established.

The [storage contract](../spec/language/memory-and-regions.md#storage-optimization) and adopted
[memory](../spec/library/memory.md#storage-optimization) and
[vector](../spec/library/vectors.md#storage-optimization) contracts own observable obligations.
Physical registers, stack/frame slots and backing blocks are an internal placement plan, not
another source permission. Preserve R/D/S, required resource requests, validation, budgets,
allocator affinity, supported address observations and semantic cleanup. An ordinary API name
is not an optimization certificate. Unknown effects remain conservative.

## Research Candidates

Use primary research to identify conditional transformations, not to copy syntax or promise a
published numerical gain. The following are candidate experiments, not reproduced artifacts or
implemented Nocter passes:

| Research mechanism | Nocter experiment | Mandatory evidence |
| --- | --- | --- |
| [Partial evaluation](https://pldi25.sigplan.org/details/pldi-2025-papers/14/Partial-Evaluation-Whole-Program-Compilation) | Specialize decoding/dispatch against a known schema or table while input bytes remain dynamic | Eliminate interpretation/dispatch rather than precompute test answers; preserve validation and failure order |
| [MEMOIR collection enumeration](https://2026.cgo.org/details/cgo-2026-papers/16/Automatic-Data-Enumeration-for-Fast-Collections) | Retain logical collection operations, introduce dense IDs, and select a bitset/array where profitable | Stable equality, legal reconstruction, preserved ordering/identity/effects, and end-to-end initialization cost |
| [Futhark consumption analysis](https://futhark-lang.org/blog/2022-06-13-uniqueness-types.html) and [Perceus reuse](https://www.microsoft.com/en-us/research/uploads/prod/2021/06/perceus-pldi21.pdf) | Reuse proven-dead buffers, construct into destinations, and fuse intermediates under access/effect evidence | No hidden clone, premature destruction, invalid alias, or adoption of RC/GC merely because those systems use it |
| [Equality saturation](https://popl21.sigplan.org/details/POPL-2021-research-papers/23/egg-Fast-and-Extensible-Equality-Saturation) | Search equivalent plans for a bounded high-value region, using target-aware costs | Valid rewrite side conditions, compile/memory/code-size budgets, and measured extraction quality |
| [Translation validation](https://pldi21.sigplan.org/details/pldi-2021-papers/5/Alive2-Bounded-Translation-Validation-for-LLVM) | Validate changes against Nocter's own checked arithmetic, effects, and cleanup semantics | A validator for Nocter's IR; Alive2 is LLVM-specific and bounded, not a drop-in general proof |

Retain logical type, collection, shape, access and effect information before raw-address lowering.
An independent backend still needs instruction selection, register allocation, loop scheduling
and target costs. Search cannot rediscover dispatch or assume a method named map has known
collection semantics. Safety or async-drive failure is a validity problem, not a reason to invent
a pure summary, hidden worker, clone, RC/GC scheme or application modifier.

## Storage and Effect Certificates

Derive each certificate from selected public contracts, checked bodies/callees and conditional
witness evidence. Separate logical value/resource obligations, observable requests and physical
backing alternatives. Record byte uses, cleanup needs, materialization deadlines and allowed
ordering changes. Cache validity depends on body, callee/witness, contract, target and backend
facts. A classifier treating a complete callable as removable can lose its independent validation
or I/O even when its private allocation disappears.

Complete actual required backing before invocation commit or another promised deadline. Do not
move a possible failure/wait into reserved append, a getter, future drive, cancellation or cleanup
that excludes it. Reuse needs a valid size/alignment/domain plan and no remaining access to old
bytes, including guards, foreign observations and byte-reading destruction. Preserve allocator
release/growth ownership even when backing is virtualized.

Fusion needs a checked order/commutation argument. For inputs [0, -1], let the producer fail on
negative input and otherwise yield 2*x+1; let the consumer fail on 1. Materializing the complete
producer fails at index 1, whereas naive streaming fails in the consumer at index 0. Both can be
free of I/O while exposing different failures. Intermediate cleanup likewise retains its semantic
schedule. An advanced fused plan may preserve that schedule, but must account for saved data.

Implement certificate derivation, path-sensitive byte liveness, materialization and translation
validation against Nocter's own checked operations. Finite tests of assumed certificates cannot
prove their derivation sound. A failed optional optimization retains a valid conservative plan.
Profiler counters do not turn private backing into a source-required allocator event; keep logical
requests, potential materialization sites and actual backend events separate.

## Vertical Experiments and Evidence

Start with dynamic-input decoding under a static schema, collection/graph processing with dense-ID
or layout choices, and async/callback transport with frame/buffer reuse. Include numeric and byte
controls to expose backend quality. Connect ordinary source through checked operations to native
code; do not time an internal interpreter and claim native execution.

Use the [measurement boundary](../benchmarks/methodology.md). Preserve source/binary/toolchain and
installed-home identities, options, host, dynamic inputs, outputs, algorithms, safety/accuracy,
parallel resources, initialization cost, peak memory, allocations/copies where measurable, code
size and compilation resources. Test errors and cancellation separately from timing. Alternate
samples, keep repeated distributions, and separate startup from steady execution where relevant.
Use independent evaluation inputs when tuning. Save reproducibility details with the experiment's
Git commit rather than retaining historical result catalogs in the working tree.

Use ablations to attribute changes to specialization, representation, reuse, safety proofs or the
backend. Include losing workloads and preprocessing costs. A schema-specific decoder must show
saved work beyond what existing typed decoding already avoids. Do not multiply isolated pass
speedups into a total or change validation, overflow, floating-point accuracy, durability or error
behavior to improve timings. Improvements are local experimental evidence, not a language-wide
performance guarantee.
