# Documentation Site Generator

This directory owns the build mechanism and static inputs for the Nocter website. The repository
`docs/` directory is generated output only: no contributor instruction or manually maintained
website asset belongs there.

## Responsibility

- `build-docs.js` validates authored documentation and generates the complete `docs/` tree.
- `document-tree.js` owns the immutable hierarchy derived from the complete published-source set.
- `highlight.js` provides build-time syntax highlighting for Nocter and shell code blocks.
- `markdown-table.js` owns table-cell boundaries across code spans and escaped pipe characters.
- `output-transaction.js` owns temporary output, complete publication, and failure restoration.
- `static/` owns files copied verbatim to the website, including styles, runtime JavaScript,
  images, and `CNAME`.
- `test-generation.js` exercises determinism, complete output replacement, publication boundaries,
  and structural navigation in isolated repository copies.

The generator writes into a temporary sibling of `docs/` and replaces the previous output only
after every page has rendered successfully. A failed build therefore cannot leave a partly updated
website, and files removed from authored inputs cannot survive as stale generated pages.

## Publication Boundary

The website publishes:

- user-facing Markdown from the repository root, `spec/`, `examples/`, and `releases/`;
- current contributor Markdown under `development/`, except internal handoff and historical
  records;
- every runnable Nocter source under `examples/`;
- public standard-library READMEs and exact checked `index.nct` module-contract sources under
  `development/std/`, excluding the `internal/` subtree.

Standard-library documentation is projected from its repository location into the public `/std/`
tree; the site does not present it as contributor documentation. A contract page preserves its
complete canonical source, including visibly restricted declarations and module-assembly edges; it
does not relabel those source forms as public API. Private standard-library implementation sources
and package-only modules remain available in the repository but do not become website pages.
Markdown links to an existing non-published repository file resolve to its GitHub source page.
Compiler diagnostic fixtures and `development/history/` are not published; milestone, review, and
release-audit links are still validated as historical records.

## Navigation Authority

The filtered published-document set is the sole authority for navigation membership and hierarchy.
Each document receives one public path before `document-tree.js` derives the immutable directory
tree; rendering, reachability validation, output paths, and the sitemap consume that same
projection. Most public paths equal repository paths. Standard-library sources have the single
explicit `development/std/` to `std/` projection described above. A `README.md` is the preferred
landing page for its directory; when none exists, a published `index.nct` is the landing page.
Navigation lists the landing page, remaining files, and child directories in deterministic name
order. Directories without a landing page remain structural groups and expose their descendants
through the nearest navigable ancestor. Display labels may use a document's first heading, but
labels never affect membership or structure.

Navigation never parses links from README prose. README catalogs may explain a recommended reading
order and ordinary links may connect related concepts, but neither determines whether a page exists
or appears in navigation. Every published page must be reachable from the repository `README.md`
through generated structural navigation. Private sources and history are removed by the publication
filter before the tree is built, so filesystem discovery cannot make them public accidentally.

## Build

Run from the repository root:

```sh
node development/site/build-docs.js
```

Generation fails when:

- two authored sources claim one output path;
- a local Markdown link or heading anchor is unresolved or escapes the repository;
- the syntax highlighter keyword set differs from the lexical specification;
- the normative named-built-in list differs from checked standard-library `primitive type`
  declarations;
- the public diagnostic catalog differs from the compiler's registered-code inventory;
- a compiler workspace crate lacks its colocated responsibility README;
- a standard-library behavior README lacks its checked `index.nct` contract or fails to link it;
- a standard-library behavior README has no subject assignment in the standard-library catalog;
- a published source cannot be represented uniquely or reached through structural navigation;
- a Markdown table row has a different cell count from its header.

The generator enforces structural authority, not language meaning. It does not claim that a regular
expression can distinguish a duplicated standard-library declaration from a valid user example.
Standard-library READMEs must not restate exact signatures; review checks that content rule, while
the compiler checks the sole declarations in `index.nct`.

The generator derives output only from authored file contents and paths. Filesystem timestamps do
not enter HTML metadata or `sitemap.xml`. Publication dates require explicit authored metadata.

Run the adversarial generator test after changing build inputs or validation boundaries:

```sh
node development/site/test-generation.js
```

It builds source trees with different timestamps and compares every output byte. It also proves
that stale generated files are removed, private standard-library implementation sources stay
private, newly discovered public pages enter navigation without README registration, historical
records remain excluded, every standard-library behavior guide links its checked contract,
named built-in declarations cannot drift from the language catalog, unrelated Rust text cannot
register diagnostics, diagnostic drift is rejected, every behavior guide remains assigned by the
standard-library catalog, and table pipes inside code spans or escapes cannot corrupt the generated
columns.

## Editing Rule

Edit public Markdown in the repository root, `examples/`, `releases/`, and `spec/`. Standard-library
API documentation is the deliberate exception colocated under `development/std/`. Edit other
compiler and contributor documentation under `development/`. Edit website-only assets in
`static/`. Never edit `docs/` directly; regenerate it and commit the resulting output with the
authored change.
