# Nocter Development Handoff

## Current State

Nocter v0.54.0 is a qualified release candidate. The exact release-content commit passed the
whole-repository compiler gate, deterministic two-build packaging, fresh installed-home
qualification, interactive LSP verification, and complete HTTP service execution. The retained
archive and measured identities are recorded in the release-preparation record.

## Next Work

Commit the qualification evidence and public version selectors, create annotated tag `v0.54.0`,
fast-forward `main`, and publish the retained archive as the release's single asset. Then audit the
public tag, asset, latest-release endpoint, downloaded installed home, and source-identified Pages
deployment. Do not rebuild or replace the qualified archive.

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
