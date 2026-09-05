# Documentation Site Generator

This directory owns the build mechanism and static inputs for the Nocter website. The repository
`docs/` directory is generated output only: no contributor instruction or manually maintained
website asset belongs there.

## Responsibility

- `build-docs.js` validates authored documentation and generates the complete `docs/` tree.
- `highlight.js` provides build-time syntax highlighting for Nocter and shell code blocks.
- `output-transaction.js` owns temporary output, complete publication, and failure restoration.
- `static/` owns files copied verbatim to the website, including styles, runtime JavaScript,
  images, and `CNAME`.
- `test-generation.js` exercises determinism, complete output replacement, publication boundaries,
  and documentation catalogs in isolated repository copies.

The generator writes into a temporary sibling of `docs/` and replaces the previous output only
after every page has rendered successfully. A failed build therefore cannot leave a partly updated
website, and files removed from authored inputs cannot survive as stale generated pages.

## Publication Boundary

The website publishes:

- user-facing Markdown from the repository root, `spec/`, `examples/`, and `releases/`;
- current contributor Markdown under `development/`, except internal handoff and historical
  records;
- every runnable Nocter source under `examples/`;
- `index.nct` public-contract files under `development/std/`, excluding the `internal/` subtree.

Private standard-library implementation sources remain available in the repository but do not
become website pages. Markdown links to an existing non-published repository file resolve to its
GitHub source page. Compiler diagnostic fixtures and `development/history/` are not published;
milestone, review, and release-audit links are still validated as historical records.

## Build

Run from the repository root:

```sh
node development/site/build-docs.js
```

Generation fails when:

- two authored sources claim one output path;
- a local Markdown link or heading anchor is unresolved or escapes the repository;
- the syntax highlighter keyword set differs from the lexical specification;
- the public diagnostic catalog differs from the compiler's registered-code inventory;
- a compiler workspace crate lacks its colocated responsibility README;
- a specification chapter, milestone, review, or release-audit record is absent from its directory
  catalog.

The generator derives output only from authored file contents and paths. Filesystem timestamps do
not enter HTML metadata or `sitemap.xml`. Publication dates require explicit authored metadata.

Run the adversarial generator test after changing build inputs or validation boundaries:

```sh
node development/site/test-generation.js
```

It builds source trees with different timestamps and compares every output byte. It also proves
that stale generated files are removed, private standard-library implementation sources stay
private, unrelated Rust text cannot register diagnostics, and uncataloged specification or
development records and diagnostic drift are rejected.

## Editing Rule

Edit public Markdown in the repository root, `examples/`, `releases/`, and `spec/`. Edit compiler
and contributor documentation under `development/`. Edit website-only assets in `static/`. Never
edit `docs/` directly; regenerate it and commit the resulting output with the authored change.
