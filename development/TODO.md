# Nocter Development Handoff

## Current State

Nocter v0.53.0 is published and externally audited. v0.54.0 Phases 0–5 are complete. Incoming
requests and client responses share one canonical body cursor. Server output freezes one validated
fixed-length or chunked plan and streams through a linear `ResponseWriter`; successful completion
returns the same connection only when the selected HTTP/1.1 policy permits reuse. Graceful shutdown
stops listener admission before one deadline-bounded drain of the application-owned handler group.
The complete public service streams 32 KiB input and output through 1 KiB transfer buffers, reuses
one sequential connection, forces terminal policy, and covers malformed framing plus idle timeout.

## Next Work

Prepare v0.54.0 as a separate release change. Advance the sole release-version input, update the
public release record and release selectors, run the clean-tree compiler and deterministic package
qualification, retain exactly one candidate archive, and audit the published tag, asset, installed
home, and documentation deployment. Do not alter the completed streaming or lifecycle contracts
during release preparation unless qualification exposes a product defect.

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
