# Future Access Conformance Inputs

These inputs qualify the **pre-S12 private-access checkpoint**, not the subsequently adopted
value-binding language. The [checkpoint contract](../../../../history/reviews/v2.0.0-access-checkpoint-contracts.md)
identifies the former source rules. They are not released programs or runnable packages. A test
harness supplies one private module and uses `V2CoreUnit`; filenames do not make bodies public
root contracts. Retain current expectations until the complete parser/checker authority is
replaced under the [S12 implementation design](../../../../history/reviews/v2.0.0-value-binding-implementation.md).
Do not advertise these passes as new-language conformance or add an old-language compatibility flag.

`manifest.json` records expected acceptance, syntax rejection (including the direct-marker
structural restriction), declaration rejection, or checked-program rejection. Each rejected case
identifies one unique primary source substring and its distinguishing rule. Exact diagnostic wording/codes remain
owned by the eventual diagnostic specification and consuming test; the manifest does not invent them.

The corpus covers explicit update, delegated reads, Copy versus transfer, immediate U versus call
reservations, normal return/handoff, RHS-first replacement, conditional initialization, alternative
U roots, joins, backedges, break/continue, short-circuit flow, unit, discard, and retired spellings.
It uses opaque `error` parameters to test move-only values without relying on new nominal
construction or the released standard library. The
[first-slice review](../../../../history/reviews/v2.0.0-first-access-slice.md) owns implementation
entry and completion gates.

The `nocter-syntax` tests explicitly consume this manifest through the production `V2CoreUnit`
parse goal. All 44 inputs requiring declaration processing have lossless node-shape snapshots;
the six syntax rejections retain their source tokens and point at the required region. Additional tests
cover precedence, grouping restrictions, boundaries, recovery, token subdivision, depth limits,
and reusable-source rebinding. The production declaration query consumes all 50 entries: six fail
syntax, six fail header validation, and 38 produce declaration metadata. The body-typing query
rejects eight at structural typing and produces typed-only evidence for 30: 19 positive
cases and eleven requiring flow rejection. Each `body_typing` expectation describes that intermediate result. The shared flow query now
reports named-slot initialization failures in seven. The referent/permission query then rejects
the four remaining access/lifetime cases and retains all nineteen positive units. The
`referent_access` field distinguishes pass, rejection and a blocked initialization prerequisite.
`compiler_status: checked-private-unit` records complete front-end acceptance of the nineteen
positive units through the production whole-unit constructor, including exact first-fragment
payload contracts and ordered ownership/cleanup transitions. All nineteen `check-reject` units
are rejected; the earlier fields still describe their intermediate stages. This is not native
execution or the released `CheckedProgram`.

Do not add these files to a released-language glob, mark current rejection as success, or implement
a second parser/checker in test support. Dependency-bearing aggregates, user destructors,
outcomes/async cancellation, and executable target realization remain outside this closed fragment.
Extend cases/snapshots at the owning phase as implementation closes each grammar edge; this
inventory alone does not establish exhaustive grammar or diagnostic coverage.

Check inventory and range integrity with:

```sh
python3 development/compiler/tests/fixtures/v2-access/check-inventory.py
```

That command checks file/shape registration, schema, expectation classes, and source projections only.
The actual parser, header, typed-only body and flow/slot tests run with:

```sh
cargo test --locked --manifest-path development/compiler/Cargo.toml -p nocter-syntax parser::access
cargo test --locked --manifest-path development/compiler/Cargo.toml -p nocter-declaration-lowering access_declarations
cargo test --locked --manifest-path development/compiler/Cargo.toml -p nocter-checking value_access
```

To deliberately regenerate node-shape snapshots, run the parser command with
`NOCTER_UPDATE_ACCESS_SHAPES=1`, then review the changed structures before accepting them.
The inventory command does not parse source, infer liveness, or validate a semantic result.
