# Development Verification

## Responsibility

This directory owns repository-development verification entry points whose intermediate artifacts
must not become persistent repository caches. Release assembly and artifact qualification remain
under [`development/packaging/`](../packaging/).

`lsp-client.js` is the single development-side JSON-RPC process client used by installed-artifact
qualification and performance measurement. It owns framing, ordered requests, notifications,
server-to-client requests, timeouts, and clean process shutdown; individual scenarios own only
their observable editor workflow and assertions.

## Compiler Verification

Run the complete compiler gate from any directory:

```sh
development/verification/verify-compiler.sh
```

The script first verifies that the machine-readable release license, shipped legal files, and all
Cargo package metadata agree. It checks the performance-runner helpers without executing timing
scenarios, then verifies the pinned Unicode-data manifest, its mutation guard, and the exact
generated standard-library tables without network access. Finally, it creates one target under
`/tmp`, shares it across formatting, warnings-denied Clippy, workspace tests, feature checking, and
Rust documentation, and removes it on exit. A complete gate therefore cannot add another Cargo hash
generation to `development/compiler/target/`.

Compiler workspace crates use test-profile optimization level 1 because the integration suite
executes the compiler itself as its dominant workload. Third-party dependencies remain
unoptimized, bounding clean compilation cost. The language-server test binary runs separately and
caps concurrency at four workers (or the available processor count when lower); its independent
full semantic compilations otherwise contend for enough memory that eight workers increase both
CPU work and elapsed time. Other workspace tests retain Cargo's normal concurrency. Public examples
cross native compilation and actual process execution once at the command boundary; lower
native-session tests retain focused ABI and runtime contracts instead of compiling the same complete
example corpus again.

Release packaging invokes the same repository-metadata verifier before building an archive. This
keeps `development/packaging/RELEASE.json` authoritative for shipped license metadata instead of
allowing Cargo and release artifacts to drift independently.

Use the workspace target only for focused inner-loop commands. It is a disposable cache; reclaim it
without affecting source or release artifacts with:

```sh
cargo clean --manifest-path development/compiler/Cargo.toml
```

The script intentionally does not clean the workspace target. Verification must not destroy a
developer's active inner-loop cache as a side effect.
