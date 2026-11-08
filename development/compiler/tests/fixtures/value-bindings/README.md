# Value Binding Conformance Inputs

These sources derive from the owning future
[bounded value-binding grammar](../../../../../spec/language/syntactic-grammar.md#bounded-value-binding-fragment)
and [input/value rules](../../../../../spec/language/value-access.md). Each file is one supplied
private module, not a runnable package or a contract-only `index.nct`.

The production `V2CoreUnit` syntax, declaration and closed accepted-unit tests consume
`manifest.json` as the sole per-source expectation catalog. All 64 inputs have compiler assertions:
4 syntax rejections, 4 header rejections, 25 checking rejections and 31 whole-unit acceptances.
Rejections assert their diagnostic codes as well as rejection. Declaration tests freeze all 56
header-valid units without claiming body acceptance and reject the four invalid headers.
The 60 syntax-valid inputs have reviewed lossless `.shape` parser snapshots. Parser tests
also check token/source projection. Neither shape equality nor manifest registration proves
memory safety; the closed checker owns the semantic verdicts.

The compiler tests retain their assertions. The manifest identifies the source and distinguishing
requirement; the owning specification remains normative. No second parser/checker or source-text
semantic oracle belongs in corpus tooling. The [implementation review](../../../../history/reviews/v2.0.0-record-source-integration.md)
records qualification and limits. The [input-default decision](../../../../history/reviews/v2.0.0-parameter-defaults.md)
records design rationale; its generic/closure examples require later extensions.

The corpus covers default-read and taking inputs, local values/aliases, Copy/move, fixed/mutable
state, RHS-first replacement, unit, branch/loop initialization, direct reservations and evaluation
cleanup. Closed records add nested and Copy schemas, selected fields, partial move/restoration/drop,
branch/loop availability, disjoint versus overlapping calls/aliases and field replacement.
Opaque error inputs supply move-only data under an exact independent snapshot contract. Additional
checker integration tests inspect field-specific cleanup order and traverse 64 levels of a shared
record schema without expanding its conceptual payload tree.
These passes do not implement record construction, computed-value projections, dependent aggregates,
callable values, generics, outcomes, async, public modules, ABI or native execution. No CLI version switch or released-program
adapter is added. The incompatible pre-S12 inputs are retired, rather than kept as another mode.

Run focused production tests from the repository root:

```sh
cargo test --locked --manifest-path development/compiler/Cargo.toml -p nocter-syntax parser::access
cargo test --locked --manifest-path development/compiler/Cargo.toml -p nocter-declaration-lowering access_declarations
cargo test --locked --manifest-path development/compiler/Cargo.toml -p nocter-checking value_access
```

To update structural snapshots deliberately, set `NOCTER_UPDATE_ACCESS_SHAPES=1` for the syntax
test, then review every changed shape. Do not regenerate snapshots to conceal semantic failures.
