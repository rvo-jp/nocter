# Nocter Development Handoff

## Current State

Nocter v0.48.0 is published and externally audited. v0.49.0 Phase 6 is complete on
`develop-v0.49.0`. A complete subprocess pipeline now composes generic byte transfer, concurrent
stream capture, exact child observation, and one structured timeout through public APIs. Native
qualification covers early stdin closure and proves that timeout cancellation terminates and reaps
the exact owned child. LSP qualification consumes the same checked source and interface evidence.

## Next Work

Begin v0.49.0 Phase 7 with a whole-area review and release-readiness pass. Review blocking future
paths, periodic probes, duplicate launch or session policy, raw-PID ownership, repeated status
decoding, hidden descriptor copies, reverse dependencies, source-name recognition, and
caller-required cleanup. Then run the complete compiler, standard-library, native, LSP, examples,
documentation, packaging, formatting, and repository gates. Stop before changing release identity
or publishing.

Preserve every published tag and asset, including v0.48.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
