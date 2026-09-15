# Nocter Development Handoff

## Current State

Nocter v0.53.0 is published and externally audited. Its tag, single release asset, downloaded
installed home, latest-release selection, and source-identified Pages deployment agree with the
qualified candidate. The immutable evidence lives in
[`development/history/release-audits/v0.53.0.md`](history/release-audits/v0.53.0.md).

## Next Work

Start v0.54.0 by defining the ownership and framing contract for bounded streaming HTTP bodies and
persistent HTTP/1.1 connections. The design must preserve one framing authority, explicit
connection reuse or closure, bounded buffering, backpressure, and structured cancellation before
adding public declarations or target operations.

Preserve the v0.52.0 tag, release asset, public notes, specification snapshot, and publication audit
without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
