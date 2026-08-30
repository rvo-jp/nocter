# nocter-package-state

## Responsibility

Own recoverable root dependency-source mutation and validated append-only exact-package cache
publication.

## Contract

The crate stages and validates the complete intended dependency graph before making persistent
changes. It then publishes each validated exact package as an immutable cache entry, revalidates
the graph, and commits generated `commit` or `sha256` fields with compare-before-write protection.
Exact package cache publication and root-source commit are deliberately separate responsibilities:
cache entries carry no dependency-selection authority and may safely survive a later root-source
rejection. The exact field inside each dependency declaration is the sole persistent selection
authority.

Package resolution supplies domain values; acquisition supplies staged content. A caller injects
the read-only package resolver so command parsing uses its compiler-computation source authority.
Editor overlays cannot enter this mutation boundary. Each resolver attempt receives an
operation-owned filesystem revision that advances only after cache publication, an observed
concurrent publication race, or root-source commit; lock and store overlays do not impersonate
filesystem changes.

## Internal Responsibilities

- root dependency-source compare-before-write authority
- staging directories and destination validation
- immutable exact-package cache publication
- exact-selection/source transition assembly and staging cleanup
- post-commit package-graph revalidation through the injected resolver

## Invariants

- A rejected root-source transition never publishes partial root source or returns a selected graph.
- A validated exact cache entry may remain after a later root-source rejection, but cannot select a
  dependency without an authored exact-selection field.
- Every exact cache destination is one immutable package identity; an existing physical directory
  wins a concurrent publication race for that identity.
- Concurrent root-source changes are rejected instead of overwritten.
- Every destination is canonical and inside the authorized package state root.
- One package-state operation object can run only once.
- A transaction never returns a package snapshot captured before its own source commit.
- Resolver retries over in-memory overlays retain one filesystem revision and cannot invalidate
  disk-backed source queries by attempt count.
- Every transaction requires an injected resolution driver; package state cannot silently select a
  direct parser outside the compiler-computation source authority.
