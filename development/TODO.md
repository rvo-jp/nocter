# Nocter Development Handoff

## Current State

Nocter v0.72.0 Flow-Proven Safe Operations is published and externally audited. Checking owns
path-sensitive bounds and integer-arithmetic dispositions; MIR and Machine transport them without
re-proof; ARM64 consumes the closed Machine contract. Practical checked slice observers publish
guarantees validated by their complete source bodies. Linux target work remains deliberately
deferred.

## Next Work

Define the next milestone from a concrete language or standard-library usability boundary before
implementation begins. Do not reopen v0.72.0 for unrelated work; preserve its checked safety
dispositions, qualified artifact, public tag, and external audit as one immutable boundary.

Preserve the v0.72.0 release-content commit, publication tag, retained asset, release notes,
specification snapshot, and audit without replacement. Any correction to a published artifact
requires a new version and a newly qualified archive.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No blocker remains. `notrap async` is deliberately rejected because `future T` does not retain a
drive-time trap contract.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
