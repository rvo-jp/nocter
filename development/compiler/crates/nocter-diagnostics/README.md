# nocter-diagnostics

## Responsibility

Own the phase-neutral diagnostic envelope and deterministic human and JSON presentation of
source-backed compiler failures.

## Contract

Each compiler stage selects its rule and supplies typed subjects through narrow origin contracts.
The crate projects those subjects to source and renders them; it does not rerun lookup, typing,
target selection, or recovery policy.

## Internal Responsibilities

- stable codes, severity, labels, and notes
- syntax and semantic origin projection
- human-readable rendering
- machine-readable JSON rendering

## Invariants

- One authored violation has one owning stage and stable diagnostic identity.
- Rendering cannot change whether compilation succeeds.
- Missing source projection is an integrity failure, not permission to guess a range.
