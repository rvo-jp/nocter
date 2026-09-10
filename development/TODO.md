# Nocter Development Handoff

## Current State

Nocter v0.43.0 is published and externally audited. v0.44.0 implementation and qualification are
complete on `develop-v0.44.0`. The old result-type-driven asynchronous producer model has been
replaced by an explicit `async` declaration modifier and a separate `future T` structural type
throughout the frontend, semantic pipeline, standard library, examples, and editor presentation.

## Next Work

Begin v0.44.0 release preparation only when requested. Recheck release identity, public version
references, changelog and release notes, archive contents, installation smoke tests, and publication
state without weakening the completed asynchronous model.

Preserve every published tag and asset, including v0.43.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
