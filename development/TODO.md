# Nocter Development Handoff

## Current State

Nocter v0.39.0 is published and externally audited. The v0.40.0 synchronous Internet client
implementation is complete and reviewed. Trusted target services, canonical URLs, system name
resolution, bounded HTTP/1.1 framing, the owned synchronous client lifecycle, examples, installed-
home execution, editor tooling, and source-tree qualification are closed.

## Next Work

Begin v0.40.0 release preparation only when requested. Set the exact release identity, write public
release notes, rerun the complete disposable-target verification and documentation gates, build two
independent optimized packages, compare archives and installed homes, qualify a fresh installation,
and retain but do not publish the candidate without explicit authorization. Preserve the immutable
v0.39.0 tag and asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
