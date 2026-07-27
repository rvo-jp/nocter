# Release Packaging Inputs

This directory contains metadata inputs for the local release image generated
from the repository root by
`development/compiler/scripts/package-local-release.sh`.

- `VERSION` is copied to repository-root `dist/.nocter/VERSION`.
- `MANIFEST.json` is copied to repository-root `dist/.nocter/MANIFEST.json`.
- Standard-library source is tracked separately in `../std/` and copied to
  repository-root `dist/.nocter/std/`.

Generated repository-root `dist/.nocter/` output is not committed to git.
