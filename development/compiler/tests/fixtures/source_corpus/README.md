# Source Corpus Fixtures

This directory contains external source inputs for `tests/source_corpus.rs`. They exercise the
real command-line frontend without making test cases part of the public example collection.

- `valid/` contains small source files that must pass human and JSON `check` modes.
- `invalid/` contains one intended language error per file and an expected stable diagnostic code
  in the Rust test table.
- Imported companion modules remain next to the valid root that loads them.

These files are compiler-development fixtures. Public, runnable packages belong in the repository
root `examples/` directory, and normative behavior belongs in `spec/`.
