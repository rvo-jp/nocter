# nocter-package

## Responsibility

Parse package declarations, identify package roots, resolve dependencies and locks, and freeze one
deterministic package graph from an immutable filesystem view.

## Contract

The crate reads package metadata and installed exact packages through the syntax-owned source
provider contract but does not know whether parse work is direct or reused. It does not download,
mutate lock state, choose compiler semantics, or publish files. Acquisition and mutation use
separate crates.

## Internal Responsibilities

- package declaration and lock decoding
- revision-local package-root source catalog and canonical package identity
- dependency graph resolution
- package-store and lock overlays for validation
- retained package-root parse products shared by topology probing and graph loading

## Invariants

- `index.nct` with `#package` is the sole package-root declaration.
- Dependency aliases never replace canonical package identities.
- Resolution is deterministic and independent of filesystem enumeration order.
- One root catalog retains the exact bytes and result behind every package-boundary decision.
  Package loading binds the same retained parse product to its semantic source identity, while
  discovery reuses the decision instead of reopening, reparsing, or reclassifying the root.
- An overlay is an input view, not authority to publish persistent state.
- Production graph loading and resolution require a caller-owned syntax provider and retained root
  catalog; convenience direct parsing is confined to tests.
