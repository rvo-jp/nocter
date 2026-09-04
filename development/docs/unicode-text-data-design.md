# Static Unicode Data Boundary

This document owns the cross-crate implementation boundary for v0.35.0 static data and generated
Unicode 17.0.0 tables. Public behavior belongs only in
[`spec/35-static-unicode-text.md`](../../spec/35-static-unicode-text.md).

## Why Static Data Is a Prerequisite

Unicode properties and full case mappings are data, not parser rules or backend semantics. Emitting
thousands of generated source branches would make every standard-library analysis pay for an
implementation-shaped syntax tree. Adding Unicode-specific compiler primitives would instead make
the compiler a second standard-library authority. Immutable typed static data is the reusable
missing contract: it represents the tables directly and also serves future protocol, parser, and
encoding tables without Unicode exceptions.

## Authority Chain

```text
final Unicode 17.0.0 data files + recorded SHA-256 digests
    -> deterministic development-only generator
    -> generated Nocter fixed-array definitions
    -> checked immutable-static product
    -> target-owned layout and bytes
    -> read-only Mach-O data
    -> std/internal/unicode lookup contract
    -> char/str/String public source implementations
```

Each arrow transports a closed product. The generator does not change the specification. The
compiler does not know Unicode properties. The backend does not evaluate source expressions. Public
standard modules do not inspect generated encoding details.

## Pinned Inputs

The generator accepts only the final Unicode 17.0.0 forms of `UnicodeData.txt`,
`DerivedCoreProperties.txt`, `PropList.txt`, `SpecialCasing.txt`, and the accompanying Unicode data
license. A checked manifest records the version, canonical source URL, file name, byte length, and
SHA-256 of every input. Any mismatch stops generation before output publication.

[Unicode 18.0.0 remains beta](https://www.unicode.org/versions/beta-18.0.0.html) at the v0.35.0
design boundary and is intentionally rejected. A future Unicode upgrade replaces the manifest and
regenerated output in one reviewed release change.

## Generated Product

Generated files contain sorted, non-overlapping scalar ranges and parallel fixed-width mapping
arrays. They carry no public declarations and are visible only through `std/internal/unicode`.
Lookup validation proves range ordering, scalar validity, mapping bounds, and exact property counts
before generated files are committed.

The generator writes temporary output, verifies it completely, and atomically replaces the tracked
product only when bytes differ. Running it twice over the same inputs must produce identical bytes.
Repository builds consume the committed product and never download Unicode data.

## Compiler Boundary

Declaration lowering and constant evaluation admit the closed static initializer domain and publish
one typed frozen aggregate. Checked ownership exposes a readonly static place; there is no move,
drop, mutable loan, allocator, or runtime initializer. Target lowering converts that aggregate to
layout-owned bytes exactly once. Machine and Mach-O layers receive data objects with fixed alignment
and relocation requirements and cannot access syntax or semantic stores.

The static product is generic infrastructure. No type, field, or enum variant in it mentions
Unicode. Unicode tables are ordinary clients owned by the standard library.

## Required Tests

- static source cannot acquire mutation, move, destruction, runtime initialization, or local
  provenance;
- source ordering cannot change static identity or output bytes;
- malformed or unsupported initializer shapes fail before target lowering;
- generated tables reproduce exactly and reject altered inputs;
- public property results match every pinned UCD entry and representative complements;
- full casing covers one-to-many and contextual mappings without locale-specific entries;
- UTF-8 results remain valid and allocation failure publishes no partial String;
- ordinary programs that do not reach Unicode tables do not retain their target data.
