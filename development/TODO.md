# Nocter v0.3.0 Handoff

The completion criteria and implementation boundaries are owned by the
[v0.3.0 Release Record](docs/v0.3.0.md) and
[First-Class Outcome Values](docs/outcome-values.md). Git owns
chronological implementation history.

## Current Baseline

- branch: `develop`
- qualified release candidate: `v0.3.0`
- completed milestone gates: `v0.3.0 Phase 0`, `v0.3.0 Phase 1 Typed Literal Core`,
  `v0.3.0 Phase 2 Explicit Iteration and Collection Access`, and
  `v0.3.0 Phase 3 Owned String Interpolation and Formatting`,
  `v0.3.0 Phase 4 Public Provenance Contracts and Generic Interface Bounds`, and
  `v0.3.0 Phase 5 Nested Outcomes and Executable Process Context`, and
  `v0.3.0 Phase 6 First-Class Outcome Values`, and
  `v0.3.0 Phase 7 Protocol-Driven Collection Iteration`, and
  `v0.3.0 Phase 8 Explicit Sequence Spread and Composable Element Packs`, and
  `v0.3.0 Phase 9 Composable Iterators and Collection Builders`, and
  `v0.3.0 Phase 10 Callable Values and Interface Default Methods`
- active milestone gate: none
- target: `arm64-darwin`
- required Phase 0 items: none
- required Phase 1 items: none
- required Phase 2 items: none
- required Phase 3 items: none
- required Phase 4 items: none
- required Phase 5 items: none
- required Phase 6 items: none
- required Phase 7 items: none
- required Phase 8 items: none
- required Phase 9 items: none
- required Phase 10 items: none

Phase 9 added capability sets, conditional conformances, statically specialized generic iteration,
allocation-transparent standard adapters, unknown/exact-size vector builders, stored optional
ownership support, and capability-set LSP presentation and recovery.

Phase 10 added method-level generics, interface default methods, explicit-capture closure values,
static callable specialization, lazy callback-driven iterator defaults, recursive nested cleanup,
consuming-receiver ownership transfer, provenance and allocation propagation, and compiler-backed
editor integration.

## Current Objective

Preserve the qualified v0.3.0 release candidate without expanding its feature set. The next
operation is explicit merge, tag, and remote publication, not additional language or standard-
library work. Optional and fallible outcome identity is preserved through IR, including stored
borrow payloads and contextual generic specialization. User-facing compiler diagnostics no longer
describe implementation limits as a bare `v0` contract, and call-lowering failures distinguish
unavailable targets from borrow and scalar materialization failures.

The ownership, allocation, and region cleanup audit now enforces allocator restoration before
outer-owner destruction, reverse region release after destruction, and non-unwinding `never`
termination independently of tail-call eligibility. Native and packaged-home probes cover normal,
`return`, `break`, `continue`, propagation, recovery, and immediate termination edges.

The editor identity audit now gives destructor and generic-parameter declarations explicit name
spans instead of reconstructing editor ranges from larger syntax spans. Resolver diagnostics,
typecheck facts, specialization lookup, hover, semantic tokens, document symbols, and IR indexing
share those declaration identities. Sequence-spread hover likewise carries the parsed operator span
through typecheck facts instead of deriving three bytes from an expression span. Paired analysis and
JSON-RPC tests cover destructor keyword/receiver separation and protocol selection ranges.

Malformed-input recovery now preserves outline symbols, semantic tokens, go-to-definition, and
references while trailing function or member blocks are unclosed. The fallback appends only
syntactically missing braces, so original document offsets remain stable; collection-for recovery
also composes with trailing block recovery before its insertion map is applied. Supported
text-document requests now reject absent or ill-typed document/position parameters with JSON-RPC
`-32602` and continue serving later requests.

Structural resilience now separates completion context traversal and member-candidate resolution
from the completion coordinator. The coordinator is below 700 lines and each new responsibility is
below 450 lines. Completion, visible-local collection, and typed-literal specialization no longer
panic when partial editor facts violate an internal shape assumption. The parser now validates its
EOF token-stream contract at entry and derives provenance bounds without an impossible-state
`expect`.

The final diagnostic audit replaces every remaining user-facing bare `v0` boundary in compiler
source with a stable syntax, ownership, ABI, formatter, or native-backend capability statement.
Exact records such as `v0.2.0` and `v0.3.0 Phase 8` remain versioned intentionally.

Clean-worktree release qualification is complete. The full compiler verification suite, Clippy
with warnings denied, public documentation generation, local release packaging, installed-home
`doctor`, and archive inspection pass from `develop`. The distributed runtime suite passes all 199
Phase 0 through Phase 10 integration cases.

The stabilization gate is complete. Body-bearing interface implementations are implemented across
parser, formatter, JSON AST, resolver, typecheck, specialization, buildability, lowering, analysis,
LSP, the distributed standard library, and packaged execution. Brace-less conformance and
inherent-method satisfaction have no compatibility path. The full clean-worktree compiler matrix,
warnings-denied Clippy, documentation build, optimized local distribution, installed-home `doctor`,
packaged-home runtime suite, and archive inspection pass. Release preparation updated the compiler,
distribution, documentation, and release notes to v0.3.0 and reran the complete gate against that
exact artifact.

Type-owned construction-surface stabilization is also complete. `construct` is the sole AST owner
for public literal definitions and for associated functions that directly produce a nominal struct
or enum. Resolver-owned `ConstructionSurface` identities drive raw structural accessibility,
default selection, imports, hover, completion ordering, signature help, definition, references,
semantic tokens, and document symbols. `Vec<T>`, `String`, `Layout`, iterator constructors, and
`File` use this model in distributed std. Detached legacy declarations receive migration
diagnostics; the compiler does not synthesize a compatibility surface.

## Release Qualification Boundary

No feature work is admitted before the v0.3.0 release. Version coherence, generated documentation,
full compiler verification, warnings-denied Clippy, optimized local packaging, installed-home
`doctor`, packaged-home execution, and archive inspection all pass. The next operation is the
explicit merge, tag, and remote publication workflow.
