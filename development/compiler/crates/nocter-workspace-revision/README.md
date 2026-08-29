# nocter-workspace-revision

## Responsibility

Own accepted open-document state and emit one complete immutable source revision for every
causally valid editor transition.

## Contract

The crate consumes canonical source paths, document versions, and normalized source bytes. It
publishes linear `WorkspaceSourceRevision` values containing the exact open-document domain,
overlay, primary document, and changed-source set. It does not select packages, discover modules,
invoke compiler stages, or answer semantic queries.

## Internal Responsibilities

- open/change/save/close state transitions
- stale-version rejection
- opaque revision-sequence ownership
- monotonic generation assignment
- complete source-overlay publication

## Invariants

- Only this owner constructs a workspace source revision.
- A revision carries its complete overlay and open-document domain; consumers do not reconstruct
  either value from earlier generations.
- Foreign, duplicate, and non-increasing revision sequences cannot be fabricated through the
  public API.
- Filesystem paths end at the workspace-planning boundary and never become semantic query keys.
