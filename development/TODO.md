# Nocter Development Handoff

## Current State

Nocter v0.65.0 Practical Stateful Services is implementation-complete. Phases 0 through 6 carry
page-backed shared ownership and descriptor notification through mutexes, bounded channels,
cooperative cancellation, structured service ownership, process-wide termination observation,
bounded HTTP application data, explicitly stateful routing, opaque sessions, and structured
redacted operational output in one complete installed-standard-library service. The previous
v0.64.0 Application Encoding and Identity release is published and externally audited.

## Next Work

Prepare the exact v0.65.0 release inputs. Bump the sole authored version and standard-package
version together, write public release notes, commit the release content, then run deterministic
artifact construction, fresh-install, installed-home, public-example, interactive-LSP, and
immutability qualification from that clean commit. Do not rebuild between qualification and
publication.

Preserve the v0.64.0 tag, release asset, public notes, specification snapshot, and publication
audit without replacement. Any correction requires a new version and a newly qualified artifact.

Preserve every published tag and asset, including v0.49.0.

## Blockers

No external blocker is known. Release preparation must preserve the reviewed shared ownership,
notification, parser, router, session, logging, and service-lifecycle authorities.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
