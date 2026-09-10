# Nocter Development Handoff

## Current State

Nocter v0.44.0 is published and externally audited. It separates the `async` producer modifier from
the structural `future T` value type without retaining the old `async T` spelling or result-driven
execution inference. The public asset is byte-identical to the retained qualified archive built
from release-content commit `2409fb55d11eccb00d82f655128df024fdcbcda4`.

## Next Work

Plan the next milestone only when requested. A later asynchronous API phase may evaluate `_async`
naming and a separately enforceable `noblock` guarantee, but neither should be changed without a
public source-design decision and practical standard-library evidence.

Preserve every published tag and asset, including v0.44.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
