# Future Access Conformance Inputs

These inputs target the future
[first access validation fragment](../../../../../spec/language/syntactic-grammar.md#first-access-validation-fragment).
They are not released-language programs or runnable packages. A test harness supplies one private
module and uses `V2CoreUnit`; the filenames do not make these bodies public root contracts.

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
rejects eight at structural typing and produces typed-only evidence for 30: 19 eventual acceptance
cases and eleven still awaiting flow rejection. Each `body_typing` expectation describes that intermediate result. The shared flow query now
reports named-slot initialization failures in seven; 23 have no such failure, including four still
requiring lifetime/conflict rejection. `compiler_status: slot-initialization-only` and
`slot_initialization` record this additional boundary, not checked-program acceptance or execution. The nineteen `check-reject` cases remain eventual rejections; passing typing does not
change their expectation.

Do not add these files to a released-language glob, mark current rejection as success, or implement
a second parser/checker in test support. Referent validity, access-liveness, escape, conflict, and validated cleanup remain pending.
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
