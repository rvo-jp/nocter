# Nocter Development Handoff

## Current State

Nocter v0.59.0 is published and externally audited. Canonical variable-width codecs, CRC-32,
integrity-checked binary records, native and editor integration, deterministic packaging, the
public asset, and the Pages deployment are complete. The immutable evidence is recorded in
[`development/history/release-audits/v0.59.0.md`](history/release-audits/v0.59.0.md).

The post-release repository and documentation authority reviews are complete. Compiler integration
tests and reusable Nocter corpora have explicit physical owners. The distributed standard library
now lives at root `std/`, current cross-responsibility contracts live under
`development/architecture/`, and local test and measurement policy lives with its mechanism.
Findings and evidence are recorded in the
[`Repository Structure Review after v0.59.0`](history/reviews/repository-structure-after-v0.59.0.md)
and the
[`Repository Documentation Authority Review after v0.59.0`](history/reviews/repository-documentation-authority-after-v0.59.0.md).

The generated documentation site retains the complete Nocter hero on fragment-free external entry
URLs. All generated site-internal navigation enters at the content boundary, while specific heading
links remain exact. Global sections, local search, page contents, adjacent-page links, narrow-screen
navigation, and accessible hero tabs are derived from the published document set without a second
page registry or hosted search service.

v0.60.0 development has started with the streaming-compression state foundation. The active scope,
responsibility boundaries, and completion gates are recorded in
[`v0.60.0: Streaming Compression and Safe Archives`](history/milestones/v0.60.0.md).

## Next Work

Continue v0.60.0 Phase 1 from the qualified bit-input and canonical-Huffman authorities. Add the
stored, fixed, dynamic-tree, and history-window states to one DEFLATE decoder without introducing
transport or archive policy. Do not reopen v0.59.0; any correction requires a new version,
implementation gate, qualified archive, tag, and publication audit.

Preserve the v0.59.0 tag, release asset, public notes, specification snapshot, and publication audit
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
