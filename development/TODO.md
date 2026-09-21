# Nocter Development Handoff

## Current State

Nocter v0.65.0 Practical Stateful Services is published and externally audited. v0.66.0 Durable
Local Application State is active; Phases 0–4 established durable filesystem replacement, the
recoverable journal, the byte-oriented store, bounded compaction, and persistent HTTP service
shutdown/restart behavior.

## Next Work

Implement v0.66.0 Phase 5: complete editor coverage, adversarial interruption and corruption probes,
installed-home qualification, deterministic packaging, and the final authority review. Do not
publish or advance release identity until the complete compiler and documentation gates pass.

Preserve the v0.64.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains from v0.65.0.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
