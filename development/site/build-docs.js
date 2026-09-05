#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const { PublishedDocumentTree, directoryPath, flattenEntries } = require("./document-tree");
const { NOCTER_RESERVED_KEYWORDS, highlightCode } = require("./highlight");
const { OutputTransaction } = require("./output-transaction");

const SITE_ORIGIN = "https://nocter.dev";
const SOURCE_ORIGIN = "https://github.com/rvo-jp/nocter/blob/main";
const PROJECT_ROOT = path.resolve(__dirname, "../..");
const FINAL_OUTPUT_ROOT = path.join(PROJECT_ROOT, "docs");
const STATIC_ROOT = path.join(__dirname, "static");
const outputTransaction = new OutputTransaction(PROJECT_ROOT, STATIC_ROOT, FINAL_OUTPUT_ROOT);
const OUTPUT_ROOT = outputTransaction.directory;
const SKIP_DIRS = new Set([".git", ".github", "dist", "target", "node_modules"]);
const SKIP_SOURCE_PATHS = new Set(["development/TODO.md"]);
const SKIP_PUBLICATION_PREFIXES = [
    "development/history/",
    "development/compiler/tests/fixtures/"
];
const SKIP_LINK_VALIDATION_PREFIXES = [
    "development/history/legacy-design/",
    "development/compiler/tests/fixtures/"
];
const OG_IMAGE_WIDTH = 1200;
const OG_IMAGE_HEIGHT = 630;

const PAGE_META = {
    "README.md": {
        title: "Nocter - Self-contained systems language",
        description: "Nocter is a statically typed systems programming language focused on self-contained native executables, explicit contracts, and simple toolchain distribution."
    },
    "spec/README.md": {
        title: "Nocter Specification",
        description: "The Nocter specification for language semantics, supported platforms, diagnostics, command-line tools, and editor behavior."
    },
    "releases/README.md": {
        title: "Nocter Releases",
        description: "Published Nocter downloads, release notes, supported targets, and version history."
    },
    "examples/README.md": {
        title: "Nocter Examples",
        description: "Complete Nocter packages demonstrating canonical source style, package execution, and practical standard-library use."
    },
    "development/README.md": {
        title: "Contributor Documentation",
        description: "Development documentation for the Nocter compiler, implementation status, backend, packaging, and release workflow."
    },
    "development/std/README.md": {
        title: "Nocter Standard Library",
        description: "Compiler-checked public standard-library declarations and their observable module contracts."
    }
};

const sourceFiles = collectSourceFiles(PROJECT_ROOT);
const sourceSet = new Set(sourceFiles.map(file => normalizePath(path.relative(PROJECT_ROOT, file))));
const sourceContents = new Map(sourceFiles.map(file => [path.resolve(file), fs.readFileSync(file, "utf8")]));
const publishedDocuments = sourceFiles.map(file => ({
    sourcePath: file,
    publicPath: publishedPathForSource(file)
}));
const documentTree = new PublishedDocumentTree(PROJECT_ROOT, publishedDocuments);
const documentLabels = new Map(sourceFiles.map(file => [path.resolve(file), sourceDocumentLabel(file)]));

// Hero panels consume complete runnable examples instead of maintaining a second set of Nocter
// snippets inside the documentation generator. Release qualification checks these same sources.
const codeExamples = Object.fromEntries(Object.entries({
    hello: "examples/hello.nct",
    recovery: "examples/recovery.nct",
    format: "examples/custom-format.nct",
    equality: "examples/equality.nct",
    indexing: "examples/indexing.nct"
}).map(([name, relative]) => [
    name,
    sourceContents.get(path.join(PROJECT_ROOT, relative)).trimEnd()
]));

validateNocterLexicon();
validatePrimitiveTypeCatalog();
validateDiagnosticCatalog();
validateCrateDocumentation();
validateStandardLibraryDocumentation();
validateOutputPaths(sourceFiles);
validateSourceLinks(collectDocumentationLinkSources(PROJECT_ROOT));
validateDocumentTreeNavigation();

try {
    outputTransaction.prepare();

    for (const file of sourceFiles) {
        const html = renderPage(file);
        const output = outputPathForSource(file);

        fs.mkdirSync(path.dirname(output), { recursive: true });
        fs.writeFileSync(output, html);
    }

    writeRobots();
    writeSitemap(sourceFiles);
    outputTransaction.publish();
    console.log(`Generated ${sourceFiles.length} HTML pages in docs/`);
} finally {
    outputTransaction.cleanup();
}

function validateNocterLexicon() {
    const lexicalPath = path.join(PROJECT_ROOT, "spec/language/lexical-grammar.md");
    const lexicalSource = fs.readFileSync(lexicalPath, "utf8");
    const match = lexicalSource.match(/Reserved keyword tokens:\n\n```text\n([\s\S]*?)\n```/);

    if (!match) {
        throw new Error("Cannot find the normative reserved-keyword block in spec/language/lexical-grammar.md");
    }

    const specificationKeywords = new Set(match[1].split("\n").filter(Boolean));
    const missing = [...specificationKeywords].filter(keyword => !NOCTER_RESERVED_KEYWORDS.has(keyword));
    const extra = [...NOCTER_RESERVED_KEYWORDS].filter(keyword => !specificationKeywords.has(keyword));

    if (missing.length > 0 || extra.length > 0) {
        throw new Error(`Nocter highlighter keyword drift (missing: ${missing.join(", ") || "none"}; extra: ${extra.join(", ") || "none"})`);
    }
}

function validatePrimitiveTypeCatalog() {
    const specificationPath = path.join(PROJECT_ROOT, "spec/language/values-and-types.md");
    const specification = fs.readFileSync(specificationPath, "utf8");
    const match = specification.match(/Named built-in types:\n\n```text\n([\s\S]*?)\n```/);

    if (!match) {
        throw new Error("Cannot find the normative named-built-in-type block in spec/language/values-and-types.md");
    }

    const specificationEntries = match[1].split(/\s+/).filter(Boolean);
    const specificationTypes = new Set(specificationEntries);
    const duplicateSpecificationTypes = specificationEntries.filter(
        (name, index) => specificationEntries.indexOf(name) !== index
    );
    const declarationEntries = sourceFiles
        .filter(file => {
            const relative = normalizePath(path.relative(PROJECT_ROOT, file));
            return relative.startsWith("development/std/") && path.basename(file) === "index.nct";
        })
        .flatMap(file => [...sourceContents.get(path.resolve(file)).matchAll(/^pub primitive type ([A-Za-z_][A-Za-z0-9_]*)$/gm)])
        .map(match => match[1]);
    const declarationTypes = new Set(declarationEntries);
    const duplicateDeclarationTypes = declarationEntries.filter(
        (name, index) => declarationEntries.indexOf(name) !== index
    );
    const missingDeclarations = specificationEntries.filter(name => !declarationTypes.has(name));
    const undocumentedDeclarations = declarationEntries.filter(name => !specificationTypes.has(name));

    if (
        duplicateSpecificationTypes.length > 0
        || duplicateDeclarationTypes.length > 0
        || missingDeclarations.length > 0
        || undocumentedDeclarations.length > 0
    ) {
        throw new Error(
            `Named built-in type drift (`
            + `duplicate specification: ${[...new Set(duplicateSpecificationTypes)].join(", ") || "none"}; `
            + `duplicate declarations: ${[...new Set(duplicateDeclarationTypes)].join(", ") || "none"}; `
            + `missing declarations: ${missingDeclarations.join(", ") || "none"}; `
            + `undocumented declarations: ${undocumentedDeclarations.join(", ") || "none"})`
        );
    }
}

function validateDiagnosticCatalog() {
    const specificationPath = path.join(PROJECT_ROOT, "spec/tooling/diagnostics.md");
    const specification = fs.readFileSync(specificationPath, "utf8");
    const entries = [...specification.matchAll(/^- `(E\d{4})`:/gm)].map(match => match[1]);
    const catalog = new Set(entries);
    const duplicates = [...new Set(entries.filter((code, index) => entries.indexOf(code) !== index))];

    if (duplicates.length > 0) {
        throw new Error(`Duplicate diagnostic catalog entries: ${duplicates.join(", ")}`);
    }

    const referenced = new Set(specification.match(/E\d{4}/g) || []);
    referenced.delete("E0000");
    const uncataloguedReferences = [...referenced].filter(code => !catalog.has(code)).sort();
    if (uncataloguedReferences.length > 0) {
        throw new Error(`Diagnostic examples use uncatalogued codes: ${uncataloguedReferences.join(", ")}`);
    }

    const compilerCatalogPath = path.join(
        PROJECT_ROOT,
        "development/compiler/crates/nocter-language/diagnostic-codes.txt"
    );
    const compilerEntries = fs.readFileSync(compilerCatalogPath, "utf8").trimEnd().split("\n");
    const compilerCodes = new Set(compilerEntries);
    const invalidCompilerCodes = compilerEntries.filter(code => !/^E\d{4}$/.test(code));
    const duplicateCompilerCodes = [...new Set(compilerEntries.filter((code, index) => compilerEntries.indexOf(code) !== index))];
    const sortedCompilerCodes = [...compilerEntries].sort();
    if (invalidCompilerCodes.length > 0 || duplicateCompilerCodes.length > 0 || compilerEntries.some((code, index) => code !== sortedCompilerCodes[index])) {
        throw new Error("Compiler diagnostic catalog must contain unique E0000 codes in lexical order");
    }
    const undocumented = [...compilerCodes].filter(code => !catalog.has(code)).sort();
    const unregistered = [...catalog].filter(code => !compilerCodes.has(code)).sort();
    if (undocumented.length > 0 || unregistered.length > 0) {
        throw new Error(`Diagnostic catalog drift (undocumented: ${undocumented.join(", ") || "none"}; unregistered: ${unregistered.join(", ") || "none"})`);
    }
}

function validateCrateDocumentation() {
    const compilerRoot = path.join(PROJECT_ROOT, "development/compiler");
    const manifestPath = path.join(compilerRoot, "Cargo.toml");
    const manifest = fs.readFileSync(manifestPath, "utf8");
    const members = manifest.match(/members\s*=\s*\[([\s\S]*?)\]/)?.[1]
        .match(/"[^"]+"/g)
        ?.map(value => value.slice(1, -1)) || [];

    if (members.length === 0) {
        throw new Error("Cannot find workspace members in development/compiler/Cargo.toml");
    }

    const documentedMembers = new Set();
    for (const member of members) {
        const memberRoot = path.join(compilerRoot, member);
        const readme = path.join(memberRoot, "README.md");

        if (!fs.existsSync(path.join(memberRoot, "Cargo.toml"))) {
            throw new Error(`Workspace member ${member} has no Cargo.toml`);
        }
        if (!fs.existsSync(readme)) {
            throw new Error(`Workspace member ${member} has no colocated README.md`);
        }
        const readmeSource = fs.readFileSync(readme, "utf8");
        const crateName = path.basename(memberRoot);
        const requiredSections = [
            `# ${crateName}`,
            "## Responsibility",
            "## Contract",
            "## Invariants"
        ];
        for (const section of requiredSections) {
            if (!readmeSource.split("\n").includes(section)) {
                throw new Error(`Workspace member ${member} README.md is missing ${section}`);
            }
        }
        documentedMembers.add(path.resolve(memberRoot));
    }

    const cratesRoot = path.join(compilerRoot, "crates");
    for (const entry of fs.readdirSync(cratesRoot, { withFileTypes: true })) {
        if (!entry.isDirectory()) {
            continue;
        }
        const crateRoot = path.join(cratesRoot, entry.name);
        if (fs.existsSync(path.join(crateRoot, "Cargo.toml")) && !documentedMembers.has(crateRoot)) {
            throw new Error(`Documented compiler crate crates/${entry.name} is absent from the workspace manifest`);
        }
    }
}

function validateStandardLibraryDocumentation() {
    const readmes = sourceFiles.filter(file => {
        const relative = normalizePath(path.relative(PROJECT_ROOT, file));
        return relative.startsWith("development/std/") && path.basename(file) === "README.md";
    });

    for (const readme of readmes) {
        const relative = normalizePath(path.relative(PROJECT_ROOT, readme));
        const publicContract = path.join(path.dirname(readme), "index.nct");
        const relativeContract = normalizePath(path.relative(PROJECT_ROOT, publicContract));
        if (!sourceSet.has(relativeContract)) {
            throw new Error(`Standard-library documentation has no public contract: ${relative}`);
        }
        const source = sourceContents.get(path.resolve(readme));
        if (!source.includes("(index.nct)")) {
            throw new Error(`Standard-library documentation does not link its public contract: ${relative}`);
        }
    }
}

function validateOutputPaths(files) {
    const ownersByOutput = new Map();

    for (const file of files) {
        const output = normalizePath(path.relative(OUTPUT_ROOT, outputPathForSource(file)));
        const owner = normalizePath(path.relative(PROJECT_ROOT, file));
        const existingOwner = ownersByOutput.get(output);

        if (existingOwner) {
            throw new Error(`Documentation sources ${existingOwner} and ${owner} both generate docs/${output}`);
        }

        ownersByOutput.set(output, owner);
    }
}

function validateDocumentTreeNavigation() {
    const rootSource = path.join(PROJECT_ROOT, "README.md");
    const pending = [rootSource];
    const reachable = new Set();

    while (pending.length > 0) {
        const sourcePath = pending.pop();
        if (reachable.has(sourcePath)) {
            continue;
        }
        reachable.add(sourcePath);

        const navigation = documentTree.navigation(sourcePath);
        const targets = [
            ...navigation.ancestors.map(entry => entry.page),
            ...flattenEntries(navigation.entries)
        ];
        for (const target of targets) {
            if (!reachable.has(target.sourcePath)) {
                pending.push(target.sourcePath);
            }
        }
    }

    const unreachable = documentTree.sourcePaths()
        .filter(sourcePath => !reachable.has(sourcePath))
        .map(sourcePath => normalizePath(path.relative(PROJECT_ROOT, sourcePath)))
        .sort();
    if (unreachable.length > 0) {
        throw new Error(`Published documentation is unreachable from README.md: ${unreachable.join(", ")}`);
    }
}

function validateSourceLinks(files) {
    const headingIdsBySource = new Map();

    for (const file of files.filter(file => file.endsWith(".md"))) {
        const markdown = fs.readFileSync(file, "utf8");
        const searchable = markdown
            .replace(/```[\s\S]*?```/g, fenced => "\n".repeat((fenced.match(/\n/g) || []).length))
            .replace(/`[^`\n]*`/g, "");

        for (const match of searchable.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
            const href = match[1].trim();

            if (!href || /^[a-z]+:/i.test(href) || href.startsWith("#")) {
                continue;
            }

            const [encodedPath, encodedHash = ""] = href.split("#");
            let rawPath;
            let hash;

            try {
                rawPath = decodeURIComponent(encodedPath);
                hash = decodeURIComponent(encodedHash);
            } catch {
                throw new Error(`Documentation source ${normalizePath(path.relative(PROJECT_ROOT, file))} has an invalid encoded link: ${href}`);
            }

            let target = validationTarget(path.resolve(path.dirname(file), rawPath));

            if (!target.startsWith(`${PROJECT_ROOT}${path.sep}`) && target !== PROJECT_ROOT) {
                throw new Error(`Documentation source ${normalizePath(path.relative(PROJECT_ROOT, file))} links outside the repository: ${href}`);
            }

            if (fs.existsSync(target) && fs.statSync(target).isDirectory()) {
                target = path.join(target, "README.md");
            }

            if (!fs.existsSync(target)) {
                throw new Error(`Documentation source ${normalizePath(path.relative(PROJECT_ROOT, file))} has an unresolved local link: ${href}`);
            }

            if (hash && target.endsWith(".md")) {
                let headingIds = headingIdsBySource.get(target);

                if (!headingIds) {
                    headingIds = markdownHeadingIds(fs.readFileSync(target, "utf8"));
                    headingIdsBySource.set(target, headingIds);
                }

                if (!headingIds.has(hash)) {
                    throw new Error(`Documentation source ${normalizePath(path.relative(PROJECT_ROOT, file))} has an unresolved heading link: ${href}`);
                }
            }
        }
    }
}

function markdownHeadingIds(markdown) {
    const counts = new Map();
    const ids = new Set();
    const withoutFences = markdown.replace(/```[\s\S]*?```/g, "");

    for (const match of withoutFences.matchAll(/^#{1,6}\s+(.+)$/gm)) {
        ids.add(uniqueHeadingId(match[1].trim(), counts));
    }

    return ids;
}

function collectDocumentationLinkSources(directory) {
    const files = [];

    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
        if (entry.name.startsWith(".")) {
            continue;
        }

        const fullPath = path.join(directory, entry.name);
        const relative = normalizePath(path.relative(PROJECT_ROOT, fullPath));

        if (entry.isDirectory()) {
            if (
                !SKIP_DIRS.has(entry.name)
                && relative !== "docs"
                && !SKIP_LINK_VALIDATION_PREFIXES.some(prefix => `${relative}/`.startsWith(prefix))
            ) {
                files.push(...collectDocumentationLinkSources(fullPath));
            }

            continue;
        }

        if (entry.isFile() && entry.name.endsWith(".md")) {
            files.push(fullPath);
        }
    }

    return files.sort((a, b) => normalizePath(path.relative(PROJECT_ROOT, a)).localeCompare(normalizePath(path.relative(PROJECT_ROOT, b))));
}

function collectSourceFiles(directory) {
    const files = [];

    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
        if (entry.name.startsWith(".")) {
            continue;
        }

        const fullPath = path.join(directory, entry.name);
        const relative = normalizePath(path.relative(PROJECT_ROOT, fullPath));

        if (entry.isDirectory()) {
            if (
                !SKIP_DIRS.has(entry.name)
                && relative !== "docs"
                && !SKIP_PUBLICATION_PREFIXES.some(prefix => `${relative}/`.startsWith(prefix))
            ) {
                files.push(...collectSourceFiles(fullPath));
            }

            continue;
        }

        if (
            entry.isFile()
            && isPublishedSource(relative)
            && entry.name !== "AGENTS.md"
            && !relative.startsWith("docs/")
            && !SKIP_SOURCE_PATHS.has(relative)
            && !SKIP_PUBLICATION_PREFIXES.some(prefix => relative.startsWith(prefix))
        ) {
            files.push(fullPath);
        }
    }

    return files.sort((a, b) => normalizePath(path.relative(PROJECT_ROOT, a)).localeCompare(normalizePath(path.relative(PROJECT_ROOT, b))));
}

function isPublishedSource(relative) {
    if (relative.startsWith("development/std/internal/")) {
        return false;
    }

    if (relative.endsWith(".md")) {
        return true;
    }

    if (!relative.endsWith(".nct")) {
        return false;
    }

    return relative.startsWith("examples/")
        || relative === "development/std/index.nct"
        || (
            relative.startsWith("development/std/")
            && !relative.startsWith("development/std/internal/")
            && path.basename(relative) === "index.nct"
        );
}

function validationTarget(target) {
    const relativeOutput = path.relative(FINAL_OUTPUT_ROOT, target);
    if (!relativeOutput.startsWith("..") && !path.isAbsolute(relativeOutput)) {
        return path.join(STATIC_ROOT, relativeOutput);
    }

    return target;
}

function renderPage(sourcePath) {
    const relativeSourcePath = normalizePath(path.relative(PROJECT_ROOT, sourcePath));
    const publishedSourcePath = publishedPathForSource(sourcePath);
    const source = sourceContents.get(path.resolve(sourcePath));
    const isNocterSource = sourcePath.endsWith(".nct");
    const body = isNocterSource ? nocterSourceToHtml(source, sourcePath) : markdownToHtml(source, sourcePath);
    const title = isNocterSource ? publishedSourcePath : firstHeading(source) || "Nocter";
    const pageMeta = PAGE_META[relativeSourcePath] || {};
    const description = pageMeta.description || (isNocterSource ? nocterSourceDescription(publishedSourcePath) : pageDescription(source));
    const outputPath = outputPathForSource(sourcePath);
    const outputDir = path.dirname(outputPath);
    const styleHref = relativeUrl(outputDir, path.join(OUTPUT_ROOT, "style.css"));
    const scriptHref = relativeUrl(outputDir, path.join(OUTPUT_ROOT, "script.js"));
    const logoHref = relativeUrl(outputDir, path.join(OUTPUT_ROOT, "assets/logo.svg"));
    const specHref = relativeUrl(outputDir, outputPathForSource(path.join(PROJECT_ROOT, "spec/README.md"))) + "#content";
    const canonical = `${SITE_ORIGIN}${publicPathForOutput(outputPath)}`;
    const navigation = renderDocumentTreeNavigation(sourcePath, outputDir);
    const bodyClass = navigation ? ' class="has-document-tree"' : "";
    const isHomePage = relativeSourcePath === "README.md";

    const pageTitle = pageMeta.title || (title === "Nocter" ? "Nocter - Self-contained systems language" : `${title} - Nocter`);
    return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>${escapeHtml(pageTitle)}</title>
    <meta name="description" content="${escapeAttribute(description)}">
    <meta name="robots" content="index, follow">
    <meta name="theme-color" content="#f7f8fc">
    <link rel="canonical" href="${canonical}">
    <link rel="icon" href="${logoHref}" type="image/svg+xml">

    <meta property="og:type" content="${isHomePage ? "website" : "article"}">
    <meta property="og:site_name" content="Nocter">
    <meta property="og:title" content="${escapeAttribute(pageTitle)}">
    <meta property="og:description" content="${escapeAttribute(description)}">
    <meta property="og:url" content="${canonical}">
    <meta property="og:image" content="${SITE_ORIGIN}/assets/og-image.png">
    <meta property="og:image:width" content="${OG_IMAGE_WIDTH}">
    <meta property="og:image:height" content="${OG_IMAGE_HEIGHT}">

    <meta name="twitter:card" content="summary_large_image">
    <meta name="twitter:title" content="${escapeAttribute(pageTitle)}">
    <meta name="twitter:description" content="${escapeAttribute(description)}">
    <meta name="twitter:image" content="${SITE_ORIGIN}/assets/og-image.png">

    <script type="application/ld+json">${structuredData(pageTitle, description, canonical, outputPath, isNocterSource)}</script>

    <link rel="stylesheet" href="${styleHref}">
</head>
<body${bodyClass}>
    ${renderHero(logoHref, specHref)}

    <div class="docs-shell">
        ${navigation || '<aside class="document-tree" aria-label="Documentation tree"></aside>'}
        <main id="content">
            <div class="markdown-path">
                <span class="markdown-path-text">/${escapeHtml(publishedSourcePath)}</span>
            </div>
            <div class="markdown-body">
                ${body}
            </div>
        </main>
    </div>

    ${renderFooter()}
    <script src="${scriptHref}" defer></script>
</body>
</html>
`;
}

function nocterSourceToHtml(source, sourcePath) {
    return `<h1>${escapeHtml(path.basename(sourcePath))}</h1><pre><code class="language-nocter">${highlightCode(source, "nocter")}</code></pre>`;
}

function nocterSourceDescription(relativeSourcePath) {
    return `Nocter source code for ${relativeSourcePath}.`;
}

function renderHero(logoHref, specHref) {
    return `<header class="hero">
        <div class="hero-inner">
            <div class="hero-copy">
                <div class="hero-mark">
                    <img class="hero-logo" src="${logoHref}" alt="Nocter Logo" width="72" height="72">
                    <p class="hero-kicker">Programming Language</p>
                </div>

                <h1 class="hero-title">Nocter</h1>

                <p class="hero-description">
                    A self-contained systems language built around simplicity, encapsulation, and foolproof design.
                </p>

                <div class="hero-actions" aria-label="Nocter links">
                    <a class="hero-action hero-action-primary" href="${specHref}">Documentation</a>
                    <a class="hero-action" href="https://github.com/rvo-jp/nocter/" target="_blank" rel="noreferrer">GitHub</a>
                </div>
            </div>

            <aside class="hero-code" aria-label="Nocter code examples">
                <div class="hero-code-tabs" role="tablist" aria-label="Code example">
                    ${Object.keys(codeExamples).map((name, index) => `<button class="hero-code-tab" type="button" role="tab" aria-selected="${index === 0 ? "true" : "false"}" data-example="${name}">${name}</button>`).join("\n                    ")}
                </div>

                <div class="hero-code-panels">
                    ${Object.entries(codeExamples).map(([name, code], index) => `<pre class="hero-code-panel" data-example-panel="${name}"${index === 0 ? "" : " hidden"}><code class="language-nocter">${highlightCode(code, "nocter")}</code></pre>`).join("\n                    ")}
                </div>
            </aside>
        </div>
    </header>`;
}

function renderFooter() {
    return `<footer class="site-footer">
        <div class="site-footer-inner">
            <p>© 2026 Rvo JP</p>

            <nav class="site-footer-links" aria-label="Footer links">
                <a href="mailto:contact@rvo.jp">contact@rvo.jp</a>
                <a href="https://github.com/rvo-jp/nocter/" target="_blank" rel="noreferrer">GitHub</a>
                <span>Apache License 2.0</span>
            </nav>
        </div>
    </footer>`;
}

function markdownToHtml(markdown, markdownPath, headingIds = new Map()) {
    return markdown
        .replace(/\r/g, "")
        .split(/(```[\s\S]*?```)/)
        .map(block => {
            if (block.startsWith("```")) {
                const [, lang = "", code = ""] = block.match(/^```(\w*)\n?([\s\S]*?)```$/);
                const language = lang ? ` class="language-${escapeAttribute(lang)}"` : "";
                const highlighted = highlightCode(code, lang);
                return `<pre><code${language}>${highlighted}</code></pre>`;
            }

            return block.split(/\n{2,}/).map(part => parseBlock(part, markdownPath, headingIds)).join("");
        })
        .join("");
}

function parseBlock(block, markdownPath, headingIds) {
    block = block.trim();

    if (!block || block.startsWith("<")) {
        return "";
    }

    if (block.startsWith("#")) {
        const level = Math.min(block.match(/^#+/)[0].length, 6);
        const text = block.slice(level).trim();
        const id = uniqueHeadingId(text, headingIds);
        return `<h${level} id="${escapeAttribute(id)}">${inline(text, markdownPath)}</h${level}>`;
    }

    const lines = block.split("\n");

    if (lines.every(line => line.startsWith(">") || line.trim() === "")) {
        const quoted = lines.map(line => line.replace(/^>\s?/, "")).join("\n").trim();
        return `<blockquote>${markdownToHtml(quoted, markdownPath, headingIds)}</blockquote>`;
    }

    if (/^(-{3,}|\*{3,}|_{3,})$/.test(block)) {
        return "<hr>";
    }

    if (isTableBlock(lines)) {
        return parseTable(lines, markdownPath);
    }

    if (block.startsWith("- ")) {
        return `<ul>${block.slice(2).split("\n- ").map(line => `<li>${inline(line, markdownPath)}</li>`).join("")}</ul>`;
    }

    if (block.startsWith("1. ")) {
        return `<ol>${block.split(/\n\d+\.\s*/).map(line => `<li>${inline(line, markdownPath)}</li>`).join("")}</ol>`;
    }

    return `<p>${inline(block.replace(/\n/g, " "), markdownPath)}</p>`;
}

function isTableBlock(lines) {
    return lines.length >= 2
        && lines[0].includes("|")
        && /^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$/.test(lines[1]);
}

function parseTable(lines, markdownPath) {
    const headers = splitTableRow(lines[0]);
    const alignments = splitTableRow(lines[1]).map(cell => {
        const left = cell.startsWith(":");
        const right = cell.endsWith(":");
        if (left && right) return "center";
        if (right) return "right";
        return left ? "left" : "";
    });
    const rows = lines.slice(2).filter(line => line.trim()).map(splitTableRow);
    const head = `<thead><tr>${headers.map((cell, index) => tableCell("th", cell, alignments[index], markdownPath)).join("")}</tr></thead>`;
    const body = `<tbody>${rows.map(row => `<tr>${row.map((cell, index) => tableCell("td", cell, alignments[index], markdownPath)).join("")}</tr>`).join("")}</tbody>`;
    return `<table>${head}${body}</table>`;
}

function splitTableRow(line) {
    return line.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map(cell => cell.trim());
}

function tableCell(tag, cell, alignment, markdownPath) {
    const style = alignment ? ` style="text-align: ${alignment}"` : "";
    return `<${tag}${style}>${inline(cell, markdownPath)}</${tag}>`;
}

function uniqueHeadingId(text, headingIds) {
    const base = slugifyHeading(text) || "section";
    const count = headingIds.get(base) || 0;
    headingIds.set(base, count + 1);
    return count === 0 ? base : `${base}-${count + 1}`;
}

function slugifyHeading(text) {
    return removeHtml(text)
        .replace(/!\[([^\]]*)\]\(([^)]+)\)/g, "$1")
        .replace(/\[([^\]]+)\]\(([^)]+)\)/g, "$1")
        .replace(/[`*_~]/g, "")
        .toLowerCase()
        .trim()
        .replace(/&[a-z0-9#]+;/g, "")
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-+|-+$/g, "");
}

function removeHtml(text) {
    return String(text)
        .replace(/<!--[\s\S]*?-->/g, "")
        .replace(/<[^>]*>/g, "");
}

function inline(text, markdownPath) {
    const codeSpans = [];
    const protectedText = String(text).replace(/`([^`]+)`/g, (_, code) => {
        const index = codeSpans.push(code) - 1;
        return `CODE_SPAN_${index}_PLACEHOLDER`;
    });

    return escapeHtml(removeHtml(protectedText))
        .replace(/\*\*(.*?)\*\*/g, "<strong>$1</strong>")
        .replace(/!\[([^\]]*)\]\(([^)]+)\)/g, (_, alt, src) => `<img src="${escapeAttribute(resolveAssetUrl(markdownPath, src))}" alt="${escapeAttribute(removeHtml(alt))}">`)
        .replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_, label, href) => `<a href="${escapeAttribute(resolveLinkUrl(markdownPath, href))}">${removeHtml(label)}</a>`)
        .replace(/CODE_SPAN_(\d+)_PLACEHOLDER/g, (_, index) => `<code>${escapeHtml(codeSpans[Number(index)])}</code>`);
}

function renderDocumentTreeNavigation(sourcePath, outputDir) {
    const navigation = documentTree.navigation(sourcePath);
    const title = directoryPath(navigation.scope) || "Documentation";
    const breadcrumbs = navigation.ancestors.length > 0
        ? `\n                <ol class="document-tree-breadcrumbs">${navigation.ancestors.map(entry => renderDocumentTreeLink(entry.page, sourcePath, outputDir, documentLabel(entry.page))).join("")}</ol>`
        : "";
    const entries = renderDocumentTreeEntries(navigation.entries, sourcePath, outputDir);

    if (!entries) {
        return "";
    }

    return `<aside class="document-tree">
            <nav aria-label="Documentation tree">${breadcrumbs}
                <p class="document-tree-title">${escapeHtml(title)}/</p>
                <ul class="document-tree-list">
                    ${entries}
                </ul>
            </nav>
        </aside>`;
}

function renderDocumentTreeEntries(entries, sourcePath, outputDir) {
    return entries.map(entry => {
        if (entry.kind === "page") {
            return renderDocumentTreeLink(entry.page, sourcePath, outputDir, documentLabel(entry.page));
        }

        const directoryLabel = entry.directory.name;
        if (entry.page) {
            return renderDocumentTreeLink(entry.page, sourcePath, outputDir, directoryLabel);
        }

        const children = renderDocumentTreeEntries(entry.children, sourcePath, outputDir);
        if (!children) {
            return "";
        }
        return `<li class="document-tree-group"><span>${escapeHtml(directoryLabel)}/</span><ul>${children}</ul></li>`;
    }).join("\n                    ");
}

function renderDocumentTreeLink(page, sourcePath, outputDir, label) {
    const current = path.resolve(page.sourcePath) === path.resolve(sourcePath);
    const href = `${relativeUrl(outputDir, outputPathForSource(page.sourcePath))}#content`;
    return `<li><a href="${href}"${current ? ' aria-current="page"' : ""}>${escapeHtml(label)}</a></li>`;
}

function documentLabel(page) {
    return documentLabels.get(path.resolve(page.sourcePath));
}

function sourceDocumentLabel(sourcePath) {
    const absoluteSource = path.resolve(sourcePath);
    if (absoluteSource.endsWith(".md")) {
        const heading = firstHeading(sourceContents.get(absoluteSource));
        if (heading) return heading;
    }

    const name = path.basename(absoluteSource);
    if (name === "index.nct") {
        const readme = path.join(path.dirname(absoluteSource), "README.md");
        const relativeReadme = normalizePath(path.relative(PROJECT_ROOT, readme));
        return sourceSet.has(relativeReadme) ? "Public API" : path.basename(path.dirname(absoluteSource));
    }
    return name;
}

function resolveLinkUrl(markdownPath, href) {
    if (/^[a-z]+:/i.test(href) || href.startsWith("#")) {
        return href;
    }

    const [rawPath, hash = ""] = href.split("#");
    const targetSource = path.resolve(path.dirname(markdownPath), rawPath);
    const relativeTarget = normalizePath(path.relative(PROJECT_ROOT, targetSource));

    if (/\.(?:md|nct)$/.test(rawPath) && sourceSet.has(relativeTarget)) {
        const targetOutput = outputPathForSource(targetSource);
        const currentOutputDir = path.dirname(outputPathForSource(markdownPath));
        return relativeUrl(currentOutputDir, targetOutput) + (hash ? `#${hash}` : "#content");
    }

    const targetReadme = path.join(targetSource, "README.md");
    const relativeTargetReadme = normalizePath(path.relative(PROJECT_ROOT, targetReadme));
    if (fs.existsSync(targetReadme) && sourceSet.has(relativeTargetReadme)) {
        const targetOutput = outputPathForSource(targetReadme);
        const currentOutputDir = path.dirname(outputPathForSource(markdownPath));
        return relativeUrl(currentOutputDir, targetOutput) + (hash ? `#${hash}` : "#content");
    }

    if (/\.(?:md|nct)$/.test(rawPath) && fs.existsSync(targetSource) && !relativeTarget.startsWith("docs/")) {
        return `${SOURCE_ORIGIN}/${relativeTarget}${hash ? `#${hash}` : ""}`;
    }

    if (fs.existsSync(targetSource) && !relativeTarget.startsWith("docs/")) {
        return `${SOURCE_ORIGIN}/${relativeTarget}${hash ? `#${hash}` : ""}`;
    }

    return href;
}

function resolveAssetUrl(markdownPath, src) {
    if (/^[a-z]+:/i.test(src) || src.startsWith("#")) {
        return src;
    }

    let target = path.resolve(path.dirname(markdownPath), src);
    const relativeOutput = path.relative(FINAL_OUTPUT_ROOT, target);
    if (!relativeOutput.startsWith("..") && !path.isAbsolute(relativeOutput)) {
        target = path.join(OUTPUT_ROOT, relativeOutput);
    }
    const currentOutputDir = path.dirname(outputPathForSource(markdownPath));
    return relativeUrl(currentOutputDir, target);
}

function outputPathForSource(sourcePath) {
    const relative = publishedPathForSource(sourcePath);

    if (relative === "README.md") {
        return path.join(OUTPUT_ROOT, "index.html");
    }

    if (path.basename(sourcePath) === "README.md") {
        return path.join(OUTPUT_ROOT, path.dirname(relative), "index.html");
    }

    const parsed = path.parse(relative);
    return path.join(OUTPUT_ROOT, parsed.dir, parsed.name, "index.html");
}

function publishedPathForSource(sourcePath) {
    const relative = normalizePath(path.relative(PROJECT_ROOT, sourcePath));
    const standardLibraryPrefix = "development/std/";

    if (relative.startsWith(standardLibraryPrefix)) {
        return relative.slice("development/".length);
    }

    return relative;
}

function publicPathForOutput(outputPath) {
    const relative = normalizePath(path.relative(OUTPUT_ROOT, outputPath));

    if (relative === "index.html") {
        return "/";
    }

    return `/${relative.replace(/index\.html$/, "")}`;
}

function relativeUrl(fromDir, toPath) {
    let relative = normalizePath(path.relative(fromDir, toPath));

    if (!relative.startsWith(".")) {
        relative = `./${relative}`;
    }

    return relative;
}

function firstHeading(markdown) {
    const match = markdown.match(/^#\s+(.+)$/m);
    return match ? stripMarkdown(match[1]).trim() : "";
}

function pageDescription(markdown) {
    const text = markdown
        .replace(/```[\s\S]*?```/g, "")
        .split(/\n{2,}/)
        .map(block => block.trim())
        .filter(block => block && !block.startsWith("#") && !block.startsWith("<"))
        .map(stripMarkdown)
        .find(Boolean);

    return (text || "Nocter is a self-contained systems language built around simplicity, encapsulation, and foolproof design.").slice(0, 155);
}

function stripMarkdown(text) {
    return removeHtml(text)
        .replace(/!\[([^\]]*)\]\([^)]+\)/g, "$1")
        .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
        .replace(/[`*_>#]/g, "")
        .replace(/\s+/g, " ");
}

function structuredData(title, description, canonical, outputPath, isNocterSource) {
    const isHome = normalizePath(path.relative(OUTPUT_ROOT, outputPath)) === "index.html";
    const schemas = [breadcrumbStructuredData(outputPath)];

    if (isNocterSource) {
        schemas.unshift(nocterSourceCodeStructuredData(title, description, canonical));
    } else if (isHome) {
        schemas.unshift(softwareSourceCodeStructuredData(title, description, canonical));
    } else {
        schemas.unshift(techArticleStructuredData(title, description, canonical));
    }

    return JSON.stringify(schemas);
}

function nocterSourceCodeStructuredData(title, description, canonical) {
    return {
        "@context": "https://schema.org",
        "@type": "SoftwareSourceCode",
        name: title,
        description,
        programmingLanguage: "Nocter",
        codeRepository: "https://github.com/rvo-jp/nocter/",
        license: "https://www.apache.org/licenses/LICENSE-2.0",
        url: canonical,
        author: siteOrganization()
    };
}

function softwareSourceCodeStructuredData(title, description, canonical) {
    return {
        "@context": "https://schema.org",
        "@type": "SoftwareSourceCode",
        name: "Nocter",
        headline: title,
        description,
        programmingLanguage: "Nocter",
        codeRepository: "https://github.com/rvo-jp/nocter/",
        license: "https://www.apache.org/licenses/LICENSE-2.0",
        url: canonical,
        author: siteOrganization()
    };
}

function breadcrumbStructuredData(outputPath) {
    const publicPath = publicPathForOutput(outputPath);
    const parts = publicPath.split("/").filter(Boolean);
    const items = [
        {
            "@type": "ListItem",
            position: 1,
            name: "Home",
            item: `${SITE_ORIGIN}/`
        }
    ];

    let currentPath = "";
    parts.forEach((part, index) => {
        currentPath += `/${part}`;
        items.push({
            "@type": "ListItem",
            position: index + 2,
            name: breadcrumbName(part),
            item: `${SITE_ORIGIN}${currentPath}/`
        });
    });

    return {
        "@context": "https://schema.org",
        "@type": "BreadcrumbList",
        itemListElement: items
    };
}

function techArticleStructuredData(title, description, canonical) {
    return {
        "@context": "https://schema.org",
        "@type": "TechArticle",
        headline: title,
        description,
        url: canonical,
        mainEntityOfPage: canonical,
        author: siteOrganization(),
        publisher: siteOrganization()
    };
}

function siteOrganization() {
    return {
        "@type": "Organization",
        name: "Rvo JP",
        email: "contact@rvo.jp"
    };
}

function breadcrumbName(segment) {
    if (segment === "spec") return "Language Specification";
    if (segment === "development") return "Development";
    if (segment === "docs") return "Docs";
    return segment
        .split("-")
        .filter(Boolean)
        .map(word => word.charAt(0).toUpperCase() + word.slice(1))
        .join(" ");
}

function writeRobots() {
    fs.writeFileSync(path.join(OUTPUT_ROOT, "robots.txt"), `User-agent: *\nAllow: /\n\nSitemap: ${SITE_ORIGIN}/sitemap.xml\n`);
}

function writeSitemap(files) {
    const urls = files.map(file => `  <url>\n    <loc>${SITE_ORIGIN}${publicPathForOutput(outputPathForSource(file))}</loc>\n    <changefreq>weekly</changefreq>\n    <priority>${normalizePath(path.relative(PROJECT_ROOT, file)) === "README.md" ? "1.0" : "0.7"}</priority>\n  </url>`);
    fs.writeFileSync(path.join(OUTPUT_ROOT, "sitemap.xml"), `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls.join("\n")}\n</urlset>\n`);
}

function escapeHtml(text) {
    return String(text).replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

function escapeAttribute(text) {
    return escapeHtml(text).replaceAll('"', "&quot;").replaceAll("'", "&#39;");
}

function normalizePath(value) {
    return value.split(path.sep).join("/");
}
