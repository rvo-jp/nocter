# Private Value Binding Inputs

Each source is one private module, not a runnable package or contract-only index.nct. The
[private handoff](../../../../architecture/pipeline/value-access-declarations.md) owns current
implementation scope and its source gaps. These inputs exercise the private frontend's current
behavior; they do not qualify full v2.0.0 syntax, ABI or native execution.

`manifest.json` is the sole per-source expectation catalog. Syntax, declaration, checking and
private-instance tests consume it and own their assertions. Shape files are reviewed lossless
parser projections, not a semantic oracle or memory-safety proof. Derive actual counts from the
manifest rather than maintaining a separate inventory.

The corpus exercises value/alias bindings, input contracts, control and initialization, record
components, partial cleanup, static callable witnesses/results and bounded owned environments.
Do not infer a broader source capability from parser acceptance or a concrete named caller.

Run focused tests from the repository root:

```sh
cargo test --locked --manifest-path development/compiler/Cargo.toml -p nocter-syntax parser::access
cargo test --locked --manifest-path development/compiler/Cargo.toml -p nocter-declaration-lowering access_declarations
cargo test --locked --manifest-path development/compiler/Cargo.toml -p nocter-checking value_access
cargo test --locked --manifest-path development/compiler/Cargo.toml -p nocter-target-program value_access
```

For an intentional parser change, set NOCTER_UPDATE_ACCESS_SHAPES=1 for the syntax test and
review every changed shape. Preserve each fixture's intended availability and transfer timing;
never regenerate snapshots to conceal semantic failures. Source and semantic admission must
migrate together under the [realization requirements](../../../../design/compiler-realization.md).
