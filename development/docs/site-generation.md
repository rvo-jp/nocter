# Documentation Site Generation

The repository `docs/` directory contains the static website output for Nocter. Website build
instructions live here with the rest of the compiler-development documentation so generated output
does not also own contributor guidance.

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

The build script recursively reads project `.md` and `.nct` files outside generated or transient
directories, writes static HTML into `docs/`, refreshes `robots.txt` and `sitemap.xml`, and keeps the
site source files listed above. Markdown becomes documentation pages; Nocter files become
syntax-highlighted source pages. Links from Markdown to either source type stay within the website.
The Examples directory navigation also discovers descendant `.nct` files, so a newly added public
example remains reachable from the website even before its overview gains a dedicated section.

## Editing Rule

Edit user-facing source Markdown in the repository root, `examples/`, `releases/`, and `spec/`.
Edit compiler-developer documentation under `development/`. Do not manually edit generated HTML;
regenerate it with `node docs/build-docs.js` after Markdown changes.
