# Nocter Website Docs

This directory contains the static website output for Nocter.

## Files

- `build-docs.js` generates static HTML from Markdown files in the project.
- `highlight.js` provides build-time syntax highlighting for Nocter and shell code blocks.
- `style.css` contains the shared site styles.
- `script.js` contains the small runtime behavior for hero code example tabs.
- `assets/` contains shared images and icons.
- Generated `index.html` files mirror the project Markdown structure.

## Build

Run from the repository root:

```sh
node docs/build-docs.js
```

The build script recursively reads project Markdown outside `docs/`, writes static HTML into this directory, refreshes `robots.txt` and `sitemap.xml`, and keeps shared files such as `style.css`, `script.js`, `highlight.js`, `build-docs.js`, and `assets/`.

## Editing Rule

Edit source Markdown in the project root, `spec/`, and `development/`. Do not manually edit generated HTML; regenerate it with `node docs/build-docs.js` after Markdown changes.
