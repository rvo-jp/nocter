# Nocter Development Handoff

## Current State

Publication of the qualified v0.47.0 candidate is authorized and in progress. The retained archive
was built from release-content commit `e191c63a881899e9e1df833184111d0ef4116026`; its identity must
remain unchanged through the public audit. The candidate makes
unqualified `Reader` and `Writer` canonical asynchronous byte-stream contracts, gives synchronous
contracts explicit `BlockingReader` and `BlockingWriter` names, removes duplicated concrete
collection logic through generic defaults, and closes compiler convention and source-snapshot
authorities.

## Next Work

Integrate the public latest-release surfaces into `main`, create and push one annotated `v0.47.0`
tag, upload the retained archive as the release's only asset, and verify the public tag,
latest-release endpoint, asset bytes, extracted installation, and remote `main`. Record that
evidence and stop.

Preserve every published tag and asset, including v0.46.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
