# Nocter Development Handoff

## Current State

Nocter v0.50.0 is published and externally audited. The v0.51.0 Unified Execution Contracts
candidate is complete and qualified. It separates authored promises, invocation facts,
deferred-drive facts, and destruction facts under one checking-owned relation authority. Its
completion definition and phases live in
[`development/history/milestones/v0.51.0.md`](history/milestones/v0.51.0.md).

## Next Work

Publish the retained v0.51.0 candidate. Release-content commit
`c9f3b6bf367619c6ba009e99dbda255b16629fc1` passed every compiler and installed-toolchain gate.
The publication transaction must reuse the retained archive without rebuilding it, update public
latest-version entry points, create the annotated tag, push `main` and the tag, upload exactly one
asset, and verify the public release and downloaded artifact.

Preserve the v0.50.0 tag, release asset, public notes, specification snapshot, and publication audit
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
