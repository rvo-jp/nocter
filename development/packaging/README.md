# Release Packaging Inputs

This directory owns the metadata, assembly, and installed-artifact qualification boundary for a
local release candidate.

- `VERSION` and `MANIFEST.json` are the release-identity inputs.
- `validate-manifest.js` enforces the exact v1 metadata schema and agreement with `VERSION`.
- `package-local-release.sh` builds the optimized compiler in a fresh temporary Cargo target,
  copies only tracked standard-library files, normalizes archive metadata, and atomically writes
  one host archive. The temporary target is removed on exit.
- `qualify-local-release.sh` requires a clean release-content commit, creates the archive twice,
  compares both compressed archives and extracted homes, and exercises the installed compiler.
- `verify-lsp.js` owns the framed installed-LSP lifecycle check used by qualification.

From the repository root, create and qualify the candidate with:

```sh
development/packaging/qualify-local-release.sh
```

Qualification covers version, installation diagnosis, help, package initialization, locked and
offline checking, native tests, deterministic JSON graphs, native run and build, direct execution,
and LSP analysis of installed standard-library contract and implementation sources. It also proves
that these commands do not mutate the installed home. Only after every check passes does it replace
the generated candidate outputs in `dist/`.

Generated repository-root `dist/.nocter/`, `dist/SHA256SUMS`, and
`dist/nocter-v<version>-arm64-darwin.tar.gz` outputs are not committed to git. Packaging and
qualification do not tag, push, upload, or publish anything.
