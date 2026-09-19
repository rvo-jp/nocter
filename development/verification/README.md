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

Run the fast change-feedback gate from any directory:

```sh
development/verification/verify-compiler.sh --fast
```

It verifies repository metadata, benchmark helpers, pinned Unicode data, formatting, warnings-denied
Clippy, and every workspace test except the CLI, command, native-session, and language-server
end-to-end suites. Those suites repeatedly compile or execute complete Nocter programs and dominate
test latency; excluding them from the fast tier does not create alternate assertions or
implementations.

Run the complete compiler gate before closing a phase or qualifying a release:

```sh
development/verification/verify-compiler.sh
```

The full tier is the same entry point with its default `--full` behavior. It adds the CLI, command,
native-session, and language-server suites, no-default-features checking, and warnings-denied Rust
documentation. Both tiers first verify that the machine-readable release license, shipped legal
files, and all Cargo package metadata agree. They check the performance-runner helpers without
executing timing scenarios, then verify the pinned Unicode-data manifest, its mutation guard, and
the exact generated standard-library tables without network access.

Every invocation creates one target under `/tmp`, shares it across all selected Cargo operations,
and removes it on exit. Verification therefore cannot add another Cargo hash generation to
`development/compiler/target/`. The fast and full tiers differ only in selected consumers of the
same source, workspace, and tests; neither tier owns independent expected behavior.

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

## Automation

The compiler workflow runs the fast tier for pull requests and pushes to `main`. A weekly scheduled
job and a manually dispatchable job run the complete tier on an ARM64 macOS runner, matching the
only currently supported native target. The workflow invokes this script rather than copying Cargo
commands into YAML, so this directory remains the sole verification-command authority.
