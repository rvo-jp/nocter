# Documentation Site Generation

The repository `docs/` directory contains the static website output for Nocter. Website build
instructions live here with the rest of the compiler-development documentation so generated output
does not also own contributor guidance.

Markdown and Nocter source files outside `docs/` are the sole authored sources. Generated HTML,
`sitemap.xml`, and navigation are reproducible publication artifacts and never become a second
documentation authority.

Generation depends only on authored file contents and paths. Filesystem timestamps are not source
metadata and must not enter HTML structured data or `sitemap.xml`. If publication dates become a
user requirement, they require explicit authored metadata rather than checkout timestamps.

## Files

- `docs/build-docs.js` generates static HTML from Markdown documents and Nocter source files in the
  project.
- `docs/highlight.js` provides build-time syntax highlighting for Nocter and shell code blocks.
- `docs/style.css` contains the shared site styles.
- `docs/script.js` contains the small runtime behavior for hero code example tabs.
- `docs/assets/` contains shared images and icons.
- Generated `index.html` files mirror the project documentation and source structure.

## Build

Run from the repository root:

```sh
node docs/build-docs.js
```

The build script recursively reads project `.md` and `.nct` files outside generated, transient, and
compiler-fixture directories, writes static HTML into `docs/`, refreshes `robots.txt` and
`sitemap.xml`, and keeps the site source files listed above. Markdown becomes documentation pages;
Nocter files become syntax-highlighted source pages. Links from Markdown to either source type stay
within the website. Deliberately invalid compiler fixtures under
`development/compiler/tests/fixtures/` never become public pages. The Examples directory
navigation discovers descendant `.nct` files, so a newly added public example remains reachable
from the website even before its overview gains a dedicated section.

The public diagnostic catalog is compared with the compiler's explicit registered-code inventory
in `nocter-language/diagnostic-codes.txt`. The generator does not infer implementation state by
searching Rust comments, tests, or string literals.

Generation fails when two sources claim one output path, a local Markdown link or heading anchor is
unresolved, a local link escapes the repository, or the syntax highlighter's keyword set differs
from the lexical specification. It also requires one colocated `README.md` for every compiler
workspace member and rejects a crate manifest under `development/compiler/crates/` that is absent
from the workspace manifest. Each crate README must carry its exact crate heading plus
`Responsibility`, `Contract`, and `Invariants` sections. Links inside fenced and inline code remain
example text rather than navigation and are excluded from link validation. Every milestone and
review record must be linked from its directory `README.md`; a new record cannot silently bypass
the catalogs that define how contributors discover status and evidence.

Run the adversarial generation test after changing generator inputs or validation boundaries:

```sh
node docs/test-generation.js
```

It builds two copied source trees with different timestamps, compares every documentation output
byte, proves unrelated Rust diagnostic-like text has no authority, and proves diagnostic and
development-record catalog drift are rejected.

## Editing Rule

Edit user-facing source Markdown in the repository root, `examples/`, `releases/`, and `spec/`.
Edit compiler-developer documentation under `development/`. Do not manually edit generated HTML;
regenerate it with `node docs/build-docs.js` after Markdown changes.
