const path = require("path");
const { indexNocterSource } = require("./nocter-source-index");

// Owns presentation metadata for every published source. Navigation, page rendering, search,
// structured data, and source outlines receive the same immutable document records instead of
// rediscovering titles or source meaning independently.
class PublishedDocumentCatalog {
    #documents;
    #bySource;
    #byRelativeSource;

    constructor(projectRoot, sources, pageOverrides = {}) {
        const root = path.resolve(projectRoot);
        this.#documents = sources.map(source => createDocument(root, source, pageOverrides));
        this.#bySource = new Map(this.#documents.map(document => [document.sourcePath, document]));
        this.#byRelativeSource = new Map(this.#documents.map(document => [document.relativeSourcePath, document]));

        if (this.#bySource.size !== this.#documents.length) {
            throw new Error("Published document catalog contains duplicate source paths");
        }
        Object.freeze(this.#documents);
    }

    all() {
        return this.#documents;
    }

    document(sourcePath) {
        const document = this.#bySource.get(path.resolve(sourcePath));
        if (!document) throw new Error(`Published document catalog has no source ${sourcePath}`);
        return document;
    }

    hasRelativeSource(relativeSourcePath) {
        return this.#byRelativeSource.has(normalizePath(relativeSourcePath));
    }

    findRelativeSource(relativeSourcePath) {
        return this.#byRelativeSource.get(normalizePath(relativeSourcePath)) || null;
    }
}

function createDocument(projectRoot, source, pageOverrides) {
    const sourcePath = path.resolve(source.sourcePath);
    const relativeSourcePath = normalizePath(path.relative(projectRoot, sourcePath));
    const publicPath = normalizePath(source.publicPath);
    const kind = sourcePath.endsWith(".nct") ? "nocter-source" : "markdown";
    const override = pageOverrides[relativeSourcePath] || {};
    const navigationTitle = kind === "markdown"
        ? firstMarkdownHeading(source.source) || path.basename(sourcePath)
        : path.basename(sourcePath);
    const pageTitle = override.title || defaultPageTitle(kind, navigationTitle, publicPath);
    const symbols = kind === "nocter-source" ? indexNocterSource(source.source) : Object.freeze([]);
    const description = override.description || (kind === "markdown"
        ? markdownDescription(source.source)
        : nocterDescription(source.source, publicPath));

    return Object.freeze({
        sourcePath,
        relativeSourcePath,
        publicPath,
        source: source.source,
        kind,
        navigationTitle,
        pageTitle,
        searchTitle: searchTitle(kind, navigationTitle, relativeSourcePath),
        description,
        section: documentSection(relativeSourcePath),
        searchText: searchText(source.source, kind),
        symbols
    });
}

function defaultPageTitle(kind, navigationTitle, publicPath) {
    if (kind === "nocter-source") return `${publicPath} - Nocter`;
    return navigationTitle === "Nocter"
        ? "Nocter - Self-contained systems language"
        : `${navigationTitle} - Nocter`;
}

function searchTitle(kind, navigationTitle, relativeSourcePath) {
    if (kind !== "nocter-source" || path.basename(relativeSourcePath) !== "index.nct") {
        return navigationTitle;
    }
    const modulePath = normalizePath(path.dirname(relativeSourcePath));
    return `${modulePath} Module Contract`;
}

function documentSection(relativeSourcePath) {
    const root = relativeSourcePath.split("/")[0];
    return ({
        spec: "specification",
        std: "standard-library",
        examples: "examples",
        development: "contributors",
        releases: "releases"
    })[root] || "home";
}

function firstMarkdownHeading(markdown) {
    const match = String(markdown).match(/^#\s+(.+)$/m);
    return match ? stripMarkdown(match[1]).trim() : "";
}

function markdownDescription(markdown) {
    const text = String(markdown)
        .replace(/```[\s\S]*?```/g, "")
        .split(/\n{2,}/)
        .map(block => block.trim())
        .filter(block => block && !block.startsWith("#") && !block.startsWith("<"))
        .map(stripMarkdown)
        .find(Boolean);
    return (text || fallbackDescription()).slice(0, 155);
}

function nocterDescription(source, publicPath) {
    const moduleDocs = [];
    for (const line of String(source).split("\n")) {
        const trimmed = line.trim();
        if (trimmed.startsWith("//!")) moduleDocs.push(trimmed.slice(3).trim());
        else if (moduleDocs.length > 0 && trimmed) break;
    }
    return (moduleDocs.join(" ") || `Nocter source code for ${publicPath}.`).slice(0, 155);
}

function searchText(source, kind) {
    if (kind === "markdown") {
        return stripMarkdown(String(source).replace(/```[\s\S]*?```/g, " ")).slice(0, 6000);
    }
    return String(source).split("\n")
        .map(line => line.trim())
        .filter(line => line.startsWith("pub ") || line.startsWith("///") || line.startsWith("//!"))
        .join(" ")
        .slice(0, 6000);
}

function stripMarkdown(text) {
    return removeHtml(text)
        .replace(/!\[([^\]]*)\]\([^)]+\)/g, "$1")
        .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
        .replace(/[`*_>#]/g, "")
        .replace(/\s+/g, " ");
}

function removeHtml(text) {
    return String(text)
        .replace(/<!--[\s\S]*?-->/g, "")
        .replace(/<[^>]*>/g, "");
}

function fallbackDescription() {
    return "Nocter is a self-contained systems language built around simplicity, encapsulation, and foolproof design.";
}

function normalizePath(value) {
    return value.split(path.sep).join("/");
}

module.exports = {
    PublishedDocumentCatalog,
    stripMarkdown
};
