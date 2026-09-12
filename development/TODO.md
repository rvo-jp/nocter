# Nocter Development Handoff

## Current State

Nocter v0.47.0 is published and externally audited. v0.48.0 implementation Phases 0 through 5 are
complete, and the release identity is fixed at `0.48.0` on `develop-v0.48.0`. Public latest-release
references remain at v0.47.0 until publication. Both execution surfaces have qualified generic
buffered byte adapters over their ordinary interfaces. `BlockingBufReader<R>` and
`BlockingBufWriter<W>` are explicit synchronous types; `BufReader<R>` and `BufWriter<W>` are
canonical executor-safe types with cancellation-stable reader scratch and terminal-before-await
writer transitions. No type is tied to `File`, and no old blocking name or close-shaped wrapper API
remains. Whole-area review passes with no open practical implementation finding.

## Next Work

Commit the v0.48.0 release-content identity, rerun the exact-content compiler and documentation
gates, produce two independent byte-identical archives, and qualify a fresh installed home. Then
reuse the retained archive for the authorized publication, update public latest-release surfaces,
integrate into `main`, create and push one annotated tag, upload exactly one asset, and record a
byte-for-byte public audit.

Preserve every published tag and asset, including v0.47.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
