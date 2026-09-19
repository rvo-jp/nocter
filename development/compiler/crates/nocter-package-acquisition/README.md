# nocter-package-acquisition

## Responsibility

Fetch and validate exact Git or archive package content under the public acquisition policy.

## Contract

The crate consumes a resolved acquisition request and produces staged exact content for a
`nocter-package-state` transaction. It owns HTTPS, Git object traversal, archive decoding, digest
verification, transport limits, and the package-root requirement. Physical tar entry safety and
bounded materialization come from the `nocter-archive-extraction` contract. This crate does not
choose dependency versions or publish package state directly.

## Invariants

- Every transport remains authenticated HTTPS under the declared policy.
- Locked Git commits and archive digests are verified before publication.
- Symlinks, traversal, devices, duplicate destinations, and resource-limit violations are rejected.
- A rejected download cannot leave an installable partial package.
- Git and archive roots are accepted only when the language-owned module-root source is a regular
  file; acquisition does not maintain a second root-file convention.
