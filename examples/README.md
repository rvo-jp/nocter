# Nocter Examples

These runnable programs demonstrate current Nocter source style and public standard-library use.
The smallest example uses explicit single-file mode; the larger example uses package mode and a
directory module split across physical sources.

Examples illustrate the qualified compiler candidate in the current repository. For exact behavior
of a published release, use the examples from that release's repository tag.

## Single-File Examples

[hello.nct](hello.nct) is the smallest executable example. Passing its filename explicitly selects
single-file mode without a package declaration or inferred entry filename.

```sh
nocter check examples/hello.nct
nocter run examples/hello.nct
```

[custom-format.nct](custom-format.nct) defines `Format` for a project-owned `Point` and uses that
ordinary static conformance from string interpolation.

```sh
nocter check examples/custom-format.nct
nocter run examples/custom-format.nct
```

[equality.nct](equality.nct) defines instance-owned equality for a project type, compares an
indexed `Vec` element, and exercises readonly plus readwrite Vec indexing.

```sh
nocter check examples/equality.nct
nocter run examples/equality.nct
```

[indexing.nct](indexing.nct) defines readonly and readwrite index operators for a project-owned
collection, assigns through the resulting place, and satisfies a generic structural index
requirement.

```sh
nocter check examples/indexing.nct
nocter run examples/indexing.nct
```

[recovery.nct](recovery.nct) turns a fallible result into a local fallback value and continues the
surrounding function. The caught `error` remains available while the fallback is computed.

```sh
nocter check examples/recovery.nct
nocter run examples/recovery.nct
```

[mutable-iteration.nct](mutable-iteration.nct) updates aggregate elements through `&+Vec<T>`, then
observes the same values through readonly and consuming expansion.

```sh
nocter check examples/mutable-iteration.nct
nocter run examples/mutable-iteration.nct
```

[ordering.nct](ordering.nct) defines strict ordering for a project type, uses the same structural
requirement from generic code, and compares `String` plus `Vec` values through standard source
declarations and readonly coercions.

```sh
nocter check examples/ordering.nct
nocter run examples/ordering.nct
```

## Package Examples

[file-summary/index.nct](file-summary/index.nct) reads a UTF-8 path from the first command-line
argument and reports the number of newline bytes. Its private
[summary.nct](file-summary/summary.nct) helper demonstrates a direct source include without
creating another namespace. The package also demonstrates process arguments, owned paths,
`std/fs` whole-file UTF-8 input, borrowed byte views, and numeric formatting.

```sh
cd examples/file-summary
nocter check
nocter build
./file-summary ../../README.md
```

[text-report/index.nct](text-report/index.nct) is a larger command-line application. It reads a
UTF-8 file, counts logical lines and lines containing a requested string, and renders a small
report. Its [report/index.nct](text-report/report/index.nct) child module is a short public API
contract; [analysis.nct](text-report/report/analysis.nct) contains the reciprocal private
representation and implementation. The example combines directory-module imports, direct source
includes, opaque public types, borrowed line iteration, string search, owned string construction,
numeric formatting, process arguments, and fallible file I/O.

```sh
cd examples/text-report
nocter check
nocter build
./text-report ../../README.md Nocter
```

## Contract

- Examples are complete user programs, not compiler fixtures.
- Single-file examples must pass explicit file-mode `nocter check`; package examples must pass
  package-mode `nocter check` with the distributed standard library.
- Runnable examples must remain buildable and executable on the implemented target.
- Source uses the canonical formatter style.
- Examples demonstrate behavior defined by the [language specification](../spec/README.md); they do
  not define additional language rules.
- Deliberately invalid compiler inputs belong under
  `development/compiler/tests/fixtures/source_corpus/`, not in this directory.
