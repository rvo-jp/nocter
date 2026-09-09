# Nocter Development Handoff

## Current State

Publication of the qualified v0.40.0 candidate is authorized and in progress. The retained archive
was built from release-content commit `a99f1e2f364dc562cfc5c98fd3c2380834432c75`; its identity must
remain unchanged through the public audit.

## Next Work

Commit the public latest-release surfaces, create and push one annotated `v0.40.0` tag, upload the
retained archive as the release's only asset, and verify the public tag, latest-release endpoint,
asset bytes, extracted installation, and remote `main`. Record that evidence and stop.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
