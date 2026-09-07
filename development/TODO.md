# Nocter Development Handoff

## Current State

Nocter v0.38.0 is published and externally audited. v0.39.0 is active as the synchronous network
I/O foundation. Phases 0 through 5 are complete: `std/net` provides checked, native-tested numeric
addresses, synchronous TCP, boundary-preserving UDP, and finite monotonic operation timeouts over
one private descriptor/deadline policy and one Darwin ABI adapter, without introducing network
concepts into the compiler pipeline. Single-file and package examples, installed-home execution,
semantic editor features, and the complete compiler gate are qualified.

## Next Work

Prepare the completed v0.39.0 content for release. Assign release identity only in the dedicated
release-preparation change, qualify two independently built archives and fresh installed homes,
then stop for explicit publication authorization. Preserve the immutable v0.38.0 tag and asset.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
