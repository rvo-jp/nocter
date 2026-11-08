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

### Selecting Evidence for a Change

Select tests by the behavior changed and the consumers reached, not by the number of passing
cases. Record direct feature evidence separately from existing-product regression evidence.

- For a private future front-end change, run the owning model/declaration/checking tests and
  source cases that exercise its actual production path. Workspace compilation and
  warnings-denied Clippy check shared API compatibility. A private resource test proves its
  resource operation, not source parsing, projected initialization, access safety or execution.
- CLI, native-session and language-server tests directly apply when their active pipeline or
  observable contracts can be affected. When that pipeline does not consume the future product,
  those suites provide broad regression evidence; they are not evidence that the future feature
  works. Do not repeatedly run them for each small internal checkpoint merely because they are
  part of the complete gate.
- Keep the complete gate for phase/integration closure, changes crossing the active compiler and
  its consumers, release qualification and scheduled regression runs. A coherent internal commit
  need not be declared a completed phase. Use focused checks during its implementation and
  state the scope of any remaining verification explicitly.

For a language feature, the source acceptance/rejection boundary is the integration target.
Internal schemas and resource kernels can be necessary milestones, but passing their tests alone
does not close that feature. Add source cases that demonstrate the new behavior and the unsafe
counterexamples; then select broader checks from the affected dependency path. No focused tier
or test double may substitute different semantic rules or expected behavior.

Compiler workspace crates use test-profile optimization level 1 because the integration suite
executes the compiler itself as its dominant workload. Third-party dependencies remain
unoptimized, bounding clean compilation cost. The language-server and native-session test binaries
run separately and cap concurrency at four workers (or the available processor count when lower).
Both suites perform independent full semantic compilations; native-session additionally launches
generated programs and local service fixtures. Higher concurrency makes these tests compete for CPU
and process resources, and can let an otherwise healthy local operation reach the deadline whose
behavior the test is measuring. Other workspace tests retain Cargo's normal concurrency. Public
examples cross native compilation and actual process execution once at the command boundary; lower
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
