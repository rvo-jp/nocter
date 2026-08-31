# nocter-package

## Responsibility

Parse package declarations, identify package roots, resolve dependency intent and exact selections,
and freeze one deterministic package graph from an immutable filesystem view.

## Contract

The crate reads package metadata and installed exact packages through the syntax-owned source
provider contract but does not know whether parse work is direct or reused. It does not download,
mutate lock state, choose compiler semantics, or publish files. Acquisition and mutation use
separate crates.

## Internal Responsibilities

- package declaration decoding, including source-specific exact-selection fields
- revision-local package-root source catalog and canonical package identity
- dependency graph resolution
- verified package-store and exact-selection overlays for validation
- retained package-root parse products shared by topology probing and graph loading

## Invariants

- `index.nct` with `#package` is the sole package-root declaration.
- Dependency aliases never replace canonical package identities.
- The compiler-selected standard package must declare `name: "std"` and the exact release carried
  by its `StandardPackage` input before its graph can close.
- Each dependency declaration is the sole syntax authority for both its source intent and optional
  exact selection; no parallel alias-to-lock map is decoded from source.
- Resolution is deterministic and independent of filesystem enumeration order.
- One root catalog retains the exact bytes and result behind every package-boundary decision.
  Package loading binds the same retained parse product to its semantic source identity, while
  discovery reuses the decision instead of reopening, reparsing, or reclassifying the root.
- An overlay is an input view, not authority to publish persistent state.
- A remote package root enters resolution only through the content-integrity capability produced by
  `nocter-package-cache`; a directory basename or `index.nct` alone cannot establish exactness.
- An effective provisional selection is checked against its dependency source before package lookup
  or acquisition can begin.
- Production graph loading and resolution require a caller-owned syntax provider and retained root
  catalog; convenience direct parsing is confined to tests.
