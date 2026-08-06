# Documentation Site Generation

The repository `docs/` directory contains the static website output for Nocter. Website build
instructions live here with the rest of the compiler-development documentation so generated output
does not also own contributor guidance.

## Files

- `docs/build-docs.js` generates static HTML from Markdown files in the project.
- `docs/highlight.js` provides build-time syntax highlighting for Nocter and shell code blocks.
- `docs/style.css` contains the shared site styles.
- `docs/script.js` contains the small runtime behavior for hero code example tabs.
- `docs/assets/` contains shared images and icons.
- Generated `index.html` files mirror the project Markdown structure.

## Build

Run from the repository root:

```sh
node docs/build-docs.js
```

The build script recursively reads project Markdown outside `docs/`, writes static HTML into
`docs/`, refreshes `robots.txt` and `sitemap.xml`, and keeps the site source files listed above.

## Editing Rule

Edit user-facing source Markdown in the repository root, `releases/`, and `spec/`. Edit
compiler-developer documentation under `development/`. Do not manually edit generated HTML;
regenerate it with `node docs/build-docs.js` after Markdown changes.
