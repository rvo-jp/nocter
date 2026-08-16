# Grammar Conformance Plan

This document derives parser tests from the normative
[Syntactic Grammar](../../spec/25-syntactic-grammar.md). It does not define accepted source. If a
case and the specification disagree, the specification wins and this plan must be corrected before
implementation.

## Test Layers

The source and syntax workspace keeps three distinct expectations:

- **parse**: the token sequence has one grammar path and produces no syntax diagnostic
- **syntax reject**: no complete production consumes the token sequence
- **semantic boundary**: parsing succeeds, but the later declaration or checked-program phase must
  reject the program under the linked topical rule

Parser tests must not resolve a name, inspect a type, or invoke a semantic fallback to decide any
of these expectations. Each accepted case also records a lossless syntax-tree snapshot so later
error recovery cannot silently change its shape.

The implemented G001-G033 boundary has grouped accepted node-shape snapshots plus rejected and
semantic-boundary source fixtures under `development/compiler/tests/fixtures/syntax/`. Focused
unit cases exercise the individual optional, repeated, newline, ambiguity, and recovery branches
inside those groups. Every future grammar production must add the same three fixture classes when
it is introduced.

## Conformance Matrix

| ID | Production coverage | Parse case | Syntax rejection | Semantic-boundary case |
| --- | --- | --- | --- | --- |
| G001 | `PackageFile`, `PackageDirective`, directive records and fields | `#name: "p"` | `#name: true` | duplicate `#name` directives |
| G002 | `ModuleSource`, use/item sequencing | `use ./helper` before `func run(): void {}` | a `use` after the function | a public item in an implementation source |
| G003 | visibility scopes | `pub(../../) func run(): void {}` | `pub(parser) func run(): void {}` | visibility wider than the declaration can expose |
| G004 | module paths, selections, aliases, and re-exports | `use /parser.{Parser, parse as parse_value,}` | `use ./parser.{}` | an unresolved imported name |
| G005 | target attachment and targetable items | `#target: "arm64-darwin"` followed by a function | `#target: "arm64-darwin"` before `test smoke {}` | an unsupported target name |
| G006 | functions, primitives, aliases, parameters, callable tails, provenance | `func choose<T>(left: &T, right: &T): &T from left | right` | a function without `: Result` | an ineligible bodyless private function |
| G007 | structs, fields, enums, variants, payloads | `enum Maybe<T> { some(value: T) missing }` with newline-separated variants | comma-separated struct fields | an empty enum body |
| G008 | interfaces and associated declarations | `interface Source { pub type Item pub method &+self.next(): Self.Item? }` with member newlines | a non-public interface member | a duplicate associated name |
| G009 | construction declarations, functions, and literals | `construct Vec<T> { pub default func new(): Self { ... } }` | a construction member without visibility | two default members |
| G010 | instances, methods, coercions, and the four operator families | an instance containing a method, coercion, equality, ordering, index, and expansion member | `operator (&self != other: &Self): bool { ... }` | an instance for a type outside its ownership boundary |
| G011 | conformances and associated bindings | `conform Source for Input { type Item = u8 method &+self.next(): Self.Item? { ... } }` | `pub method` inside a conformance | a method signature that disagrees with the interface |
| G012 | drop and native test declarations | `drop Buffer(&+self) {}` and `test empty {}` as separate items | visibility on either declaration | drop for a non-owning or foreign type |
| G013 | scalar, named, selected, slice, fixed-array, grouped, pointer, and borrow types | `&parser.Buffer<T>.Item?` | `[T; size]` | type arguments on an associated projection |
| G014 | outcome suffixes and nesting | `T?!` and `(T!)?` | `T!?` | an outcome whose eventual payload is `never` |
| G015 | callable capabilities, named parameters, and provenance | `&+func(input: &T): &T from input` | a callable type without a result annotation | a provenance name that is not a parameter |
| G016 | opaque callable results and bindings | `func values(): some Source<Item = u8>? { ... }` | `func values(): some { ... }` | opaque syntax in a bodyless requirement |
| G017 | generic parameters, nested arguments, and split closers | `Outer<Inner<T>>` | an empty generic parameter list | duplicate binders in `struct Pair<T, T> {}` |
| G018 | all requirement predicates | one clause containing capability, copy, equality, operator, coercion, and expansion predicates | a newline used instead of a predicate comma | equality without an associated projection |
| G019 | blocks, block imports, executable sequences, and body results | `{ use std/io.print` then a blank line then `value }` | a block import after an executable | a non-final non-`void` expression statement |
| G020 | bindings, annotations, assignment, and compound assignment | `var value: i32 = 1` followed by `value += 2` | `var value: i32` without an initializer | assignment through an immutable place |
| G021 | return, break, continue, and explicit drop statements | `return value`, `break`, `continue`, and `drop value` in their legal containers | `drop owner.field` | `break` outside a loop |
| G022 | while, loop, for-range, for-collection, region, and allocator places | `for index in 0..<count {}` and `region temp using allocators.arena {}` | a call as the allocator place | iteration over a value without expansion capability |
| G023 | if, `if is`, match, enum patterns, payload slots, and fallback | `if value is Maybe.some(item) { item } else { fallback }` | a nested payload pattern | duplicate or non-final fallback arms |
| G024 | recovery, logical operators, and short-circuit levels | `load() catch error { recover(error) } otherwise { fallback() }` | bare `catch {}` | `catch` on a non-fallible value |
| G025 | equality, ordering, shifts, arithmetic, and conversions | `(left == right) == expected` and `&value as &View` | `left < middle < right` | an operator without an applicable built-in or instance declaration |
| G026 | unary, move-place, and outcome expressions | `!ready`, `-count`, `&&value`, and `(move result?)?` | `move make_value()` | moving a borrowed or already moved place |
| G027 | calls, members, and joint indexing | `factory().field[index]` | `value??` | calling a non-callable value |
| G028 | references, struct construction, fixed arrays, and qualified generic owners | `parser.Buffer<T> { value: item }` and `Option<T>.some(item)` | explicit callable type arguments at an ordinary call | constructing a type with no accessible entry |
| G029 | typed sequence/string literals, spread, and allocation overrides | `Vec [1, ...&source, ...move owned] using arenas.temp` | `Vec [...&+source]` | an override place that is not an allocation context |
| G030 | plain strings, interpolation, and literal-only strings | `"value: ${value}"` | `String "value: ${value}"` | interpolating a value without formatting support |
| G031 | closures, captures, parameters, results, and grouping | `(&limit; value: i32): bool { value < limit }` | `(; value) { value }` | an unlisted outer binding used by the closure |
| G032 | control-header brace boundary | `if (Flags { ready: true }).ready {}` | `if Flags { ready: true } {}` as a struct-literal condition | a grouped closure used where `bool` is required |
| G033 | contextual spellings and removed forms | `func some(): i32 { 1 }` and `let copy = 1` | legacy `alloc func` and top-level `trait` forms | `Self` outside a type-owned semantic context |

Ellipses in this planning table stand for an ordinary valid block body; fixture files will replace
them with concrete tokens. A parser fixture must never use an ellipsis unless it is testing the
actual expansion operator production.

## Coverage Rule

Every grammar production belongs to the narrowest row that names it or its enclosing family.
Before a parser production is implemented, that row is expanded into independent accepted,
rejected, and semantic-boundary fixtures. Helper productions such as `List`, `LineSequence`,
`NamedPlace`, and `TypeSelectionSuffix` receive direct boundary assertions inside every consuming
family rather than one artificial top-level fixture.

The suite is complete only when every production has at least one successful edge, every optional
or repeated branch has zero/one/many coverage as applicable, every joint/gap boundary is tested
with both spaces and newlines, and every contextual spelling is tested both in and outside its
special position.
