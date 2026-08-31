# nocter-diagnostics

## Responsibility

Own the phase-neutral diagnostic envelope and deterministic human and JSON presentation of
source-backed compiler failures.

## Contract

Each compiler stage selects its rule, supplies typed subjects through narrow origin contracts, and
may attach a typed repair capability while it still owns the failed semantic evidence. The crate
retains that phase-selected capability beside the diagnostic envelope and projects source for
rendering; it does not rerun lookup, typing, target selection, or recovery policy.

## Internal Responsibilities

- stable codes, severity, labels, and notes
- syntax and semantic origin projection
- phase-selected semantic repair capabilities
- human-readable rendering
- machine-readable JSON rendering

## Invariants

- One authored violation has one owning stage and stable diagnostic identity.
- Rendering cannot change whether compilation succeeds.
- Missing source projection is an integrity failure, not permission to guess a range.
- A repair consumer cannot infer eligibility or authored evidence from a code, rendered message,
  or source substring.
