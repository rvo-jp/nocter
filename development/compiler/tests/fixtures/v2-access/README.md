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

No production compiler test consumes this manifest yet. Future consumers must load it explicitly;
do not add these files to a released-language glob, mark current rejection as success, or implement
a second parser/checker in test support. Expected acceptance is a requirement, not a passing
compiler result. Extend cases/snapshots at the owning phase as implementation closes each grammar
edge; this inventory does not establish exhaustive grammar or diagnostic coverage.

Check inventory and range integrity with:

```sh
python3 development/compiler/tests/fixtures/v2-access/check-inventory.py
```

That command checks file registration, schema, expectation classes, and source projections only.
It does not parse source, infer liveness, or validate an expected semantic result.
