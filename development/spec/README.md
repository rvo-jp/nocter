# Nocter v2.0.0 Development Specification

This directory is the sole normative source for the adopted, unreleased Nocter v2.0.0 contracts
within their explicitly defined scope. It is intended for Nocter developers. The published
[language specification](../../spec/README.md), [standard library](../../std/README.md), and
[runnable examples](../../examples/README.md) describe the released product.

These chapters specify selected behavior, not complete v2.0.0 support. An omitted subject has no
implicit fallback to a released production. Shared behavior is inherited only where a chapter
explicitly selects it. The implemented private prototype predates mandatory postfix transfer; its
[recorded grammar](../history/reviews/v2.0.0-pre-explicit-transfer-fragment.md) is not current
language conformance or a compatibility mode. It grants no public CLI, ABI or native admission.
Implementation coverage and next work belong to the [handoff](../TODO.md) and linked qualification
records; they do not change the rules here.

## Reading Order

- [Language](language/README.md): values, source grammar, callables, origins and storage contracts.
- [Library](library/README.md): adopted changes to observable standard-library behavior.
- [Tooling](tooling/README.md): adopted diagnostic, formatting and editor contracts.
- [Current development](../README.md): architecture, implementation evidence and work order.

The catalogs recommend reading order and ownership; they are not website page registries.
This tree is excluded from the documentation site's pages, navigation, sitemap and search index.
Developer links resolve to the repository source at the site's selected revision.

## Ownership and Changes

Each adopted rule has one owning chapter in this tree. Published behavior remains owned by
`spec/` and `std/`; a development chapter does not amend that released contract. Proposals,
alternatives and superseded decisions stay under [development history](../history/reviews/README.md)
until adopted. Compiler mechanisms stay in architecture documents and crate READMEs.

Write these contracts in English and change the owning chapter before accepting new behavior in
the v2.0.0 compiler. Update links and conformance references when moving a rule. Keep source
semantics under `language/`, library behavior under `library/`, and observable tooling contracts
under `tooling/`; add platform contracts when selected. File names describe concepts, not release
order. Do not copy the complete released specification here to fill unclosed subjects.

## Promotion to a Public Specification

Before publishing v2.0.0, close the intended release's language, library, platform and tooling
scope and verify it against the compiler, examples and distribution. Move the qualifying
contracts into `spec/` and the corresponding product-owned `std/` documentation; remove their
editable development copies. Keep private grammar subsets and compiler implementation evidence
under `development/`. Promotion is a coherent release transition, not a blanket file copy.

Previous public contracts remain fixed by their release tags. Do not maintain two editable
copies of the same version's rule or replace qualified release artifacts during documentation
work. Publication still requires its own authorization and release qualification.
