# Nocter Examples

These complete packages demonstrate current Nocter source style and public standard-library use.
Each directory contains a package file (`nocter.nct`) and a root module source
(`index.nct`), so commands run in package mode without an inferred source filename.

Examples illustrate the qualified compiler candidate in the current repository. For exact behavior
of a published release, use the examples from that release's repository tag.

## Hello

[hello/index.nct](hello/index.nct) is the smallest executable package's root source. Its package
metadata and executable source have separate responsibilities.

```sh
cd examples/hello
nocter check
nocter run
```

## File Summary

[file-summary/index.nct](file-summary/index.nct) reads a UTF-8 path from the first command-line
argument and reports the number of newline bytes. Its private
[summary.nct](file-summary/summary.nct) helper demonstrates a same-module source import without
creating another namespace. The package also demonstrates process arguments, owned paths,
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
