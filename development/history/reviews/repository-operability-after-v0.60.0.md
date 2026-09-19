# Repository Operability Review after v0.60.0

**Result: remediated (2026-09-19).** The repository had strong compiler authority boundaries but
weak automated change feedback and unbounded local generated output. Both operational defects are
closed. The source-size audit found maintainability candidates, not a second semantic pipeline or
an immediate reason for mechanical file splitting.

## Automated Verification

Before this review, GitHub Actions built only the documentation site. Compiler correctness depended
on a developer manually running the complete gate, whose clean end-to-end suites take long enough
to discourage use as change feedback.

`development/verification/verify-compiler.sh` now remains the sole command authority and accepts
two explicit tiers:

- `--fast` runs repository metadata, benchmark-helper and Unicode reproducibility checks,
  formatting, warnings-denied workspace Clippy, and every test except the CLI, command,
  native-session, and language-server end-to-end suites;
- `--full` is the default and adds those four suites, no-default-feature checking, and
  warnings-denied Rust documentation.

The compiler workflow invokes those modes instead of copying their Cargo commands. Pull requests
and `main` pushes receive fast feedback. Weekly and manually dispatched ARM64 Darwin jobs exercise
the complete supported-target boundary. The local fast gate passed from a fresh disposable target;
its loopback conformance case also passed under ordinary host permissions. The complete gate then
passed after the formatter change, including all 60 command tests, 106 native-session tests, 118
language-server tests, no-default-feature checking, and warnings-denied Rust documentation.

## Generated-output Lifecycle

The checkout occupied approximately 4.4 GiB before remediation. About 4.1 GiB was an ignored Cargo
target containing obsolete focused-build generations, while `dist/` retained every generated
archive from v0.26.0 through v0.60.0.

`cargo clean` removed 17,838 files and 4.0 GiB. Obsolete local archives were removed while the
current v0.60.0 candidate and installed image were retained. Subsequent focused verification has
created a bounded active Cargo cache; that cache remains disposable developer state.

Successful release qualification now removes older versioned archives only after the replacement
archive, installed home, and checksum have all been published into `dist/`. This makes `dist/`
current-candidate staging. GitHub Releases owns published binaries, and release-audit records own
their immutable identities.

## Source-size Audit

The audit found 44 Rust source files over 1,000 lines: 17 test-focused files and 27 production
files. Two website implementation files and the generated Unicode table also exceed that threshold.
The threshold is evidence for review, not an architectural rule.

The test-focused files mostly collect feature matrices around one shared harness. Splitting them by
line count would move assertions without changing authority. Partitioning is justified when a
feature gains an independent fixture or setup contract; it is not required merely to lower a line
counter.

Most production files are closed representations or single-pass authorities: checked-node
algebras, declaration-program freezing, type storage, concrete dispatch, ownership analysis, MIR
validation, ARM64 selection, and executable closure. Their consumers see exported products rather
than their internal branches. Arbitrary physical splits would increase private coupling without
removing a decision or recomputation.

Two files remain deliberate maintainability candidates:

- `nocter-command/src/arguments.rs` contains typed parsed/prepared command values, option parsing,
  diagnostic errors, and its focused tests. These can be separated in one command-boundary
  refactor when argument behavior next changes.
- `nocter-analysis/src/query/presentation.rs` contains one canonical stateful renderer but many
  entity and type families. It should be split only after a narrow internal rendering contract can
  avoid bidirectional access to formatter state.

Neither candidate currently duplicates authority, leaks an internal representation across crates,
or causes observable failure. They therefore do not justify a disruptive refactor in this review.

## User-visible Defect Found by the Audit

The formatter rejected every source containing comments. The lexer already owned exact comment
spans and kinds, so rejection represented an incomplete tooling responsibility rather than a
language limitation.

The formatter now has an independent comment-layout component. It merges comments with the ordered
syntax-token stream, removes lexer newline events physically contained inside block-comment text,
and records whether surrounding significant elements shared the authored line. Token layout does
not interpret comment kinds. Before publication, reparsing proves structural syntax equality,
exact comment kind/text/order, and unchanged documentation attachments. This removes `E0601` from
normal behavior without reusing its published code.

## Conclusion

The repository now has automatic compiler feedback, one verification command authority, bounded
release staging, and complete single-file formatting for comments. Large files remain visible
maintenance risks, but no reviewed file currently warrants responsibility-free splitting. The next
architectural decision is the verified-installation trust model, not another compiler-internal
reorganization.
