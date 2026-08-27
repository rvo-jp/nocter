# nocter-package

## Responsibility

Parse package declarations, identify package roots, resolve dependencies and locks, and freeze one
deterministic package graph from an immutable filesystem view.

## Contract

The crate reads package metadata and installed exact packages but does not download, mutate lock
state, choose compiler semantics, or publish files. Acquisition and mutation use separate crates.

## Internal Responsibilities

- package declaration and lock decoding
- root probing and canonical package identity
- dependency graph resolution
- package-store and lock overlays for validation

## Invariants

- `index.nct` with `#package` is the sole package-root declaration.
- Dependency aliases never replace canonical package identities.
- Resolution is deterministic and independent of filesystem enumeration order.
- An overlay is an input view, not authority to publish persistent state.
