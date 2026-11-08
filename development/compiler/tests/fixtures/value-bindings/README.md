# Value Binding Conformance Inputs

These sources derive from the owning future
[bounded value-binding grammar](../../../../../spec/language/syntactic-grammar.md#bounded-value-binding-fragment)
and [input/value rules](../../../../../spec/language/value-access.md). Each file is one supplied
private module, not a runnable package or a contract-only `index.nct`.

The production `V2CoreUnit` syntax, declaration and closed accepted-unit tests consume
`manifest.json` as the sole per-source expectation catalog. All 189 inputs have compiler assertions:
6 syntax rejections, 6 header rejections, 81 checking rejections and 96 whole-unit acceptances.
Rejections assert their diagnostic codes as well as rejection. Declaration tests freeze all 177
header-valid units without claiming body acceptance and reject the six invalid headers.
The 183 syntax-valid inputs have reviewed lossless `.shape` parser snapshots. Parser tests
also check token/source projection. Neither shape equality nor manifest registration proves
memory safety; the closed checker owns the semantic verdicts.

The compiler tests retain their assertions. The manifest identifies the source and distinguishing
requirement; the owning specification remains normative. No second parser/checker or source-text
semantic oracle belongs in corpus tooling. The [callable input review](../../../../history/reviews/v2.0.0-callable-input-witnesses.md),
[function witness review](../../../../history/reviews/v2.0.0-function-witnesses.md)
and earlier [record review](../../../../history/reviews/v2.0.0-record-source-integration.md)
record qualification and limits. The [input-default decision](../../../../history/reviews/v2.0.0-parameter-defaults.md)
records design rationale; its generic/closure examples require later extensions.

The corpus covers default-read and taking inputs, local values/aliases, Copy/move, fixed/mutable
state, RHS-first replacement, unit, branch/loop initialization, direct reservations and evaluation
cleanup. Closed records add nested and Copy schemas, selected fields, partial move/restoration/drop,
branch/loop availability, disjoint versus overlapping calls/aliases and field replacement.
Structural values add empty/nested/reordered construction, component Copy-or-move, scalar snapshot
order, replacement, partial construction interrupted by return/break/continue, and missing/duplicate/
unknown/wrong-type/reused components. Opaque error inputs supply move-only data under an exact
independent snapshot contract. Computed fields add call/literal/conditional roots, complete
nested paths, Copy snapshots, read/take supply, replacement, control headers, loop cleanup,
interrupted known types, exact unknown-field errors and rejection of hidden alias/update owners.
Additional checker integration tests inspect field-specific cleanup order and traverse 64 levels
of a shared record schema for both named and computed selections without expanding its
conceptual payload tree.

Named-function cases add value receipt, exact local constraints, Copy/read aliases, all invocation
views, recursive/computed calls, same-witness joins/replacement, environment-first preparation,
taking-input indices, interrupted calls and Copy-before-drop. Rejections distinguish invocation,
input operation/data, result/count mismatches, non-writable environments, distinct witnesses,
invalid aliases, unavailable values, live referents, invalid callees and unsupported callback headers.
Formal-callback cases add binding and forwarding, finite recursive edges, independent input
positions, higher-order named values, record/error contracts, non-Copy supply, symbolic cleanup,
invocation adapters and interrupted input evaluation. Invalid unused bodies still reject even
when a known caller supplies a Copy named function. Availability and atomic conflict diagnostics
remain separate; terminating unknown-shaped inputs do not hide known contract mismatches.
These passes do not implement indexing, dependent aggregates, structural callback results,
captured environments, erased callables, generics, outcomes, async, public modules, ABI or native execution. No CLI version switch or released-program
adapter is added. The incompatible pre-S12 inputs are retired, rather than kept as another mode.

Run focused production tests from the repository root:

```sh
cargo test --locked --manifest-path development/compiler/Cargo.toml -p nocter-syntax parser::access
cargo test --locked --manifest-path development/compiler/Cargo.toml -p nocter-declaration-lowering access_declarations
cargo test --locked --manifest-path development/compiler/Cargo.toml -p nocter-checking value_access
```

To update structural snapshots deliberately, set `NOCTER_UPDATE_ACCESS_SHAPES=1` for the syntax
test, then review every changed shape. Do not regenerate snapshots to conceal semantic failures.
