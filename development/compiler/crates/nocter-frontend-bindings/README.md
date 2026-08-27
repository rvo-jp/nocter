# nocter-frontend-bindings

## Responsibility

Carry the exact mapping from compiler-selected frontend roles to authored declaration identities.

## Contract

Declaration lowering constructs the binding set from the selected toolchain package. Checking and
target setup consume role-to-identity accessors and never infer builtin or standard authority from a
name, path, or source location.

## Invariants

- Each required role has at most one exact declaration identity.
- Named builtin fallback and standard roles remain distinct authorities.
- Source projection is not an input to role selection.
