# Release Packaging Inputs

This directory owns the metadata, assembly, and installed-artifact qualification boundary for a
local release candidate.

- `VERSION` is the sole authored release-version identity. `RELEASE.json` owns version-independent
  host, target, license, and archive-layout metadata.
- Root [`std/`](../../std/README.md) is the sole authored standard-library package copied into the
  installed home; packaging rejects untracked or non-regular entries in that tree.
- `render-manifest.js` validates those inputs, derives the versioned archive name, and combines them
  with the compiler file digest and standard-library tree digest to create the installed
  `MANIFEST.json` v2.
- `package-local-release.sh` builds the optimized compiler in a fresh temporary Cargo target,
  first verifies that `RELEASE.json`, the shipped legal files, and every Cargo package agree on the
  license, then computes both artifact identities through the shared Rust content-integrity
  implementation, validates the assembled home through its own compiler, normalizes archive
  metadata, and atomically writes one host archive. The temporary target is removed on exit.
- `qualify-local-release.sh` requires a clean release-content commit, refuses to reuse a version
  already tagged at another commit, creates the archive twice, compares both compressed archives
  and extracted homes, and exercises the installed compiler.
- `verify-lsp.js` owns the installed-LSP qualification scenario. It drives the packaged process
  through the same ordered JSON-RPC boundary as an editor: initialization and dynamic capability
  registration, installed and user-source analysis, semantic queries, document edits, diagnostic
  recovery, and clean shutdown. Framing and bidirectional request handling are shared with the
  performance runner through `development/verification/lsp-client.js`.

From the repository root, create and qualify the candidate with:

```sh
development/packaging/qualify-local-release.sh
```

Qualification covers manifest-bound compiler and standard-library content, version, installation
diagnosis, help, package initialization, locked and
offline checking, native tests, deterministic JSON graphs, native run and build, direct execution,
every public example, exact child arguments with piped standard input, exact synchronous subprocess
status, captured-output behavior, and configured environment, working-directory, finite-input, and
captured-output behavior. It runs the structured subprocess pipeline against the installed
standard library to qualify concurrent generic transfer and observation under one timeout. It also
builds and runs the bounded asynchronous file report application, checks its exact recursively
discovered file and byte counts, and proves that its recoverable missing-root path leaves neither
final nor temporary output. It compiles and runs every declared integrity-checked binary-record
test, builds the application, and compares its exact 46-byte output. The remaining gates include
interactive LSP analysis of an initialized user package and installed standard-library contract,
implementation, fixed and variable byte-codec, checksum, cursor, dynamic-storage, generated
Unicode table, and Unicode casing sources. It also proves
that these commands do not mutate the installed home and that changing either the installed
compiler or one standard-library source invalidates the home.
Only after every check passes does it replace the generated candidate outputs in `dist/`.

Generated repository-root `dist/.nocter/`, `dist/SHA256SUMS`, and
`dist/nocter-v<version>-arm64-darwin.tar.gz` outputs are not committed to git. Packaging and
qualification do not tag, push, upload, or publish anything.
