# Release Packaging Inputs

This directory owns the metadata, assembly, and installed-artifact qualification boundary for a
local release candidate.

- `VERSION` is the sole authored release-version identity. `RELEASE.json` owns version-independent
  host, target, license, and archive-layout metadata.
- `render-manifest.js` validates those inputs, derives the versioned archive name, and combines them
  with the compiler file digest and standard-library tree digest to create the installed
  `MANIFEST.json` v2.
- `package-local-release.sh` builds the optimized compiler in a fresh temporary Cargo target,
  computes both artifact identities through the shared Rust content-integrity implementation,
  validates the assembled home through its own compiler, normalizes archive metadata, and
  atomically writes one host archive. The temporary target is removed on exit.
- `qualify-local-release.sh` requires a clean release-content commit, refuses to reuse a version
  already tagged at another commit, creates the archive twice, compares both compressed archives
  and extracted homes, and exercises the installed compiler.
- `verify-lsp.js` owns the framed installed-LSP lifecycle check used by qualification.

From the repository root, create and qualify the candidate with:

```sh
development/packaging/qualify-local-release.sh
```

Qualification covers manifest-bound compiler and standard-library content, version, installation
diagnosis, help, package initialization, locked and
offline checking, native tests, deterministic JSON graphs, native run and build, direct execution,
every public example, and LSP analysis of installed standard-library contract and implementation
sources. It also proves that these commands do not mutate the installed home and that changing
either the installed compiler or one standard-library source invalidates the home. Only after every
check passes does it replace the generated candidate outputs in `dist/`.

Generated repository-root `dist/.nocter/`, `dist/SHA256SUMS`, and
`dist/nocter-v<version>-arm64-darwin.tar.gz` outputs are not committed to git. Packaging and
qualification do not tag, push, upload, or publish anything.
