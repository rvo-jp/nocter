# Nocter Development Handoff

## Current State

Nocter v0.57.0 is published and externally audited. v0.58.0 is qualified for publication after
completion of its binary scalar codecs, cursor and storage composition, practical binary
application, editor qualification, and whole-repository review. Release-content commit
`7ee9b8701852483547563660e14b74165d722bc6` passed the complete compiler gate and deterministic
two-build installed-home qualification. The retained archive has SHA-256
`332018f053c18d07d696ce34476c9b68d64c74bffbf53ca2f61ad7c1297de2eb`. The completed
implementation scope lives in
[`development/history/milestones/v0.58.0.md`](history/milestones/v0.58.0.md).

## Next Work

After explicit publication authorization, publish the retained qualified archive without
rebuilding it, then record and verify the immutable tag, GitHub Release asset, public latest-release
endpoint, downloaded installed home, and source-identified Pages deployment.

Preserve the v0.57.0 tag, release asset, public notes, specification snapshot, and publication audit
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
