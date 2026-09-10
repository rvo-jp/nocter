# Nocter Development Handoff

## Current State

Nocter v0.43.0 is qualified on the development branch with no open practical finding. Its retained
archive was built reproducibly from release-content commit
`de4932a08a123b274f8b44df39f706511f549255`; public latest-release references remain at v0.42.0.
Network.framework now supplies plain TCP and authenticated TLS
through one owner/event model. System trust, custom-root augmentation, hostname authentication,
synchronous TLS, asynchronous TLS, HTTP/1.1 ALPN, and custom-trust HTTPS have deterministic local
native coverage.

## Next Work

Commit the public latest-release surfaces, integrate them into `main`, create and push one annotated
`v0.43.0` tag, upload the retained archive as the release's only asset, and verify the public tag,
latest-release endpoint, asset bytes, extracted installation, and remote `main`. Record that
evidence and stop. Publication must not rebuild the candidate.

Preserve every published tag and asset, including v0.42.0.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
