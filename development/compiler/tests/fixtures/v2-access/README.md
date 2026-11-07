# Future Access Conformance Inputs

These inputs target the future
[first access validation fragment](../../../../../spec/language/syntactic-grammar.md#first-access-validation-fragment).
They are not released-language programs or runnable packages. A test harness supplies one private
module and uses `V2CoreUnit`; the filenames do not make these bodies public root contracts.

`manifest.json` records expected acceptance, syntax rejection (including the direct-marker
structural restriction), or checked-program rejection. Each rejected case identifies one unique
primary source substring and its distinguishing rule. Exact diagnostic wording/codes remain
owned by the eventual diagnostic specification and consuming test; the manifest does not invent them.

The corpus covers explicit update, delegated reads, Copy versus transfer, immediate U versus call
reservations, normal return/handoff, RHS-first replacement, conditional initialization, alternative
U roots, joins, backedges, break/continue, short-circuit flow, unit, discard, and retired spellings.
It uses opaque `error` parameters to test move-only values without relying on new nominal
construction or the released standard library. The
[first-slice review](../../../../history/reviews/v2.0.0-first-access-slice.md) owns implementation
entry and completion gates.

The `nocter-syntax` tests explicitly consume this manifest through the production `V2CoreUnit`
parse goal. All 38 inputs requiring later checking have lossless node-shape snapshots; the six
syntax rejections retain their source tokens and point at the required region. Additional tests
cover precedence, grouping restrictions, boundaries, recovery, token subdivision, depth limits,
and reusable-source rebinding. `compiler_status: syntax-only` does not claim semantic acceptance
or execution. The 19 `check-reject` cases must parse successfully before later checking rejects them.

Do not add these files to a released-language glob, mark current rejection as success, or implement
a second parser/checker in test support. Declaration/body-checking consumers remain pending.
Extend cases/snapshots at the owning phase as implementation closes each grammar edge; this
inventory alone does not establish exhaustive grammar or diagnostic coverage.

Check inventory and range integrity with:

```sh
python3 development/compiler/tests/fixtures/v2-access/check-inventory.py
```

That command checks file/shape registration, schema, expectation classes, and source projections only.
The actual parser tests run with:

```sh
cargo test --locked --manifest-path development/compiler/Cargo.toml -p nocter-syntax parser::access
```

To deliberately regenerate node-shape snapshots, run that focused command with
`NOCTER_UPDATE_ACCESS_SHAPES=1`, then review the changed structures before accepting them.
The inventory command does not parse source, infer liveness, or validate a semantic result.
