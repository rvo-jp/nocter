# Nocter Examples

These complete packages demonstrate current Nocter source style and public standard-library use.
Each directory contains its own `nocter.nct`, so commands run in package mode without an implicit
entry file.

Examples illustrate the qualified compiler candidate in the current repository. For exact behavior
of a published release, use the examples from that release's repository tag.

## Hello

[hello/nocter.nct](hello/nocter.nct) is the smallest executable package. It keeps package metadata
and `main` in the same file.

```sh
cd examples/hello
nocter check
nocter run
```

## File Summary

[file-summary/nocter.nct](file-summary/nocter.nct) reads a UTF-8 path from the first command-line
argument and reports the number of newline bytes. It demonstrates process arguments, owned paths,
whole-stream UTF-8 file input, borrowed byte views, and numeric formatting.

```sh
cd examples/file-summary
nocter check
nocter build
./file-summary ../../README.md
```

## Contract

- Examples are complete user packages, not compiler fixtures.
- Every example must pass package-mode `nocter check` with the distributed standard library.
- Runnable examples must remain buildable and executable on the implemented target.
- Source uses the canonical formatter style.
- Examples demonstrate behavior defined by the [language specification](../spec/README.md); they do
  not define additional language rules.
- Deliberately invalid compiler inputs belong under
  `development/compiler/tests/fixtures/source_corpus/`, not in this directory.
