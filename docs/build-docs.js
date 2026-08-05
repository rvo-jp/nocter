#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const { highlightCode } = require("./highlight");

const SITE_ORIGIN = "https://nocter.dev";
const SOURCE_ORIGIN = "https://github.com/rvo-jp/nocter/blob/main";
const PROJECT_ROOT = path.resolve(__dirname, "..");
const DOCS_DIR = __dirname;
const OUTPUT_ROOT = DOCS_DIR;
const SKIP_DIRS = new Set([".git", ".github", "target", "node_modules"]);
const SKIP_MARKDOWN_PATHS = new Set(["development/TODO.md"]);
const OG_IMAGE_WIDTH = 1200;
const OG_IMAGE_HEIGHT = 630;

const PAGE_META = {
    "README.md": {
        title: "Nocter - Self-contained systems language",
        description: "Nocter is a statically typed systems programming language focused on self-contained native executables, explicit contracts, and simple toolchain distribution."
    },
    "spec/README.md": {
        title: "Nocter Language Specification",
        description: "Language specification for Nocter v0.4.0, covering syntax, types, packages, interfaces, ownership, diagnostics, tooling, and historical release contracts."
    },
    "development/README.md": {
        title: "Nocter Development Documentation",
        description: "Development documentation for the Nocter compiler, implementation status, backend, packaging, and release workflow."
    }
};

const codeExamples = {
    greet: `use std/io.print

func greet(user: User): void! {
    let name = user.display.name() otherwise { "guest" }

    let tone = match user.presence {
        Presence.online { "hello" }
        Presence.away { "welcome back" }
    }

    print("\${tone}, \${name}\\n")?
}`,
    writer: `interface Writer {
    pub method &+self.write(bytes: &[u8]): void!
}

func save_log(output: &+Writer, events: &[Event]): void! {
    for event in events {
        output.write(event.message().bytes())?
        output.write("\\n".bytes())?
    }
}`,
    session: `pub struct Session {
    id: SessionId
    user: User
    expires_at: Time
}

pub func Session.start(user: User, clock: &Clock): Session {
    return Session {
        id: SessionId.new(),
        user: move user,
        expires_at: clock.now().plus_minutes(30),
    }
}`,
    profile: `struct Profile {
    ...UserSummary
    ...ActivityStats
    visits: u32
}

func profile(user: &User, stats: ActivityStats): Profile {
    return Profile {
        ...user.summary(),
        ...move stats,
        visits: 0,
    }
}`,
    literal: `literal NonEmptyList<T> [first: T, ...rest: [T]]: Self {
    let list = Self.with_first(move first)

    for item in rest {
        list.push(move item)
    }

    return list
}`
};

const markdownFiles = collectMarkdownFiles(PROJECT_ROOT);
const markdownSet = new Set(markdownFiles.map(file => normalizePath(path.relative(PROJECT_ROOT, file))));

cleanGeneratedHtml();

for (const file of markdownFiles) {
    const html = renderPage(file);
    const output = outputPathForMarkdown(file);

    fs.mkdirSync(path.dirname(output), { recursive: true });
    fs.writeFileSync(output, html);
}

writeRobots();
writeSitemap(markdownFiles);

console.log(`Generated ${markdownFiles.length} HTML pages in ${path.relative(PROJECT_ROOT, OUTPUT_ROOT)}/`);

function cleanGeneratedHtml() {
    for (const entry of fs.readdirSync(OUTPUT_ROOT, { withFileTypes: true })) {
        if (entry.name.startsWith(".")) {
            continue;
        }

        const fullPath = path.join(OUTPUT_ROOT, entry.name);

        if (entry.isFile() && entry.name === "index.html") {
            fs.rmSync(fullPath);
            continue;
        }

        if (entry.isDirectory() && entry.name !== "assets") {
            fs.rmSync(fullPath, { recursive: true, force: true });
        }
    }
}

function collectMarkdownFiles(directory) {
    const files = [];

    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
        if (entry.name.startsWith(".")) {
            continue;
        }

        const fullPath = path.join(directory, entry.name);
        const relative = normalizePath(path.relative(PROJECT_ROOT, fullPath));

        if (entry.isDirectory()) {
            if (!SKIP_DIRS.has(entry.name) && relative !== "docs") {
                files.push(...collectMarkdownFiles(fullPath));
            }

            continue;
        }

        if (
            entry.isFile()
            && entry.name.endsWith(".md")
            && entry.name !== "AGENTS.md"
            && !relative.startsWith("docs/")
            && !SKIP_MARKDOWN_PATHS.has(relative)
        ) {
            files.push(fullPath);
        }
    }

    return files.sort((a, b) => normalizePath(path.relative(PROJECT_ROOT, a)).localeCompare(normalizePath(path.relative(PROJECT_ROOT, b))));
}

function renderPage(markdownPath) {
    const relativeMarkdownPath = normalizePath(path.relative(PROJECT_ROOT, markdownPath));
    const markdown = fs.readFileSync(markdownPath, "utf8");
    const body = markdownToHtml(markdown, markdownPath);
    const title = firstHeading(markdown) || "Nocter";
    const pageMeta = PAGE_META[relativeMarkdownPath] || {};
    const description = pageMeta.description || pageDescription(markdown);
    const outputPath = outputPathForMarkdown(markdownPath);
    const outputDir = path.dirname(outputPath);
    const styleHref = relativeUrl(outputDir, path.join(OUTPUT_ROOT, "style.css"));
    const scriptHref = relativeUrl(outputDir, path.join(OUTPUT_ROOT, "script.js"));
    const logoHref = relativeUrl(outputDir, path.join(OUTPUT_ROOT, "assets/logo.svg"));
    const specHref = relativeUrl(outputDir, outputPathForMarkdown(path.join(PROJECT_ROOT, "spec/README.md"))) + "#content";
    const canonical = `${SITE_ORIGIN}${publicPathForOutput(outputPath)}`;
    const toc = renderDirectoryToc(markdownPath, outputDir);
    const bodyClass = toc ? ' class="has-directory-toc"' : "";
    const isHomePage = relativeMarkdownPath === "README.md";

    const pageTitle = pageMeta.title || (title === "Nocter" ? "Nocter - Self-contained systems language" : `${title} - Nocter`);
    const lastModified = fileLastModifiedDate(markdownPath);

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
    <meta property="og:image:height" content="${OG_IMAGE_HEIGHT}">${isHomePage ? "" : `
    <meta property="article:modified_time" content="${lastModified}">`}

    <meta name="twitter:card" content="summary_large_image">
    <meta name="twitter:title" content="${escapeAttribute(pageTitle)}">
    <meta name="twitter:description" content="${escapeAttribute(description)}">
    <meta name="twitter:image" content="${SITE_ORIGIN}/assets/og-image.png">

    <script type="application/ld+json">${structuredData(pageTitle, description, canonical, outputPath, lastModified)}</script>

    <link rel="stylesheet" href="${styleHref}">
</head>
<body${bodyClass}>
    ${renderHero(logoHref, specHref)}

    <div class="docs-shell">
        ${toc || '<aside class="directory-toc" aria-label="Directory table of contents"></aside>'}
        <main id="content">
            <div class="markdown-path">
                <span class="markdown-path-text">/${escapeHtml(relativeMarkdownPath)}</span>
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

function renderDirectoryToc(markdownPath, outputDir) {
    const tocSource = findDirectoryTocSource(markdownPath);

    if (!tocSource) {
        return "";
    }

    const markdown = fs.readFileSync(tocSource, "utf8");
    const links = [...markdown.matchAll(/\[([^\]]+)\]\(([^)]+\.md)\)/g)];
    const items = [];

    for (const [, label, href] of links) {
        const targetMarkdown = path.resolve(path.dirname(tocSource), href);
        const relativeTarget = normalizePath(path.relative(PROJECT_ROOT, targetMarkdown));

        if (!markdownSet.has(relativeTarget)) {
            continue;
        }

        const targetOutput = outputPathForMarkdown(targetMarkdown);
        const current = normalizePath(path.relative(PROJECT_ROOT, targetMarkdown)) === normalizePath(path.relative(PROJECT_ROOT, markdownPath));
        items.push(`<li><a href="${relativeUrl(outputDir, targetOutput)}#content"${current ? ' aria-current="page"' : ""}>${escapeHtml(label)}</a></li>`);
    }

    if (!items.length) {
        return "";
    }

    return `<aside class="directory-toc" aria-label="Directory table of contents">
            <p class="directory-toc-title">${escapeHtml(directoryTocTitle(tocSource))}</p>
            <ul class="directory-toc-list">
                ${items.join("\n                ")}
            </ul>
        </aside>`;
}

function findDirectoryTocSource(markdownPath) {
    let directory = path.dirname(markdownPath);
    const rootReadme = path.join(PROJECT_ROOT, "README.md");

    while (directory.startsWith(PROJECT_ROOT)) {
        const readme = path.join(directory, "README.md");

        if (path.resolve(readme) === rootReadme) {
            return null;
        }

        if (fs.existsSync(readme)) {
            return readme;
        }

        const parent = path.dirname(directory);
        if (parent === directory) return null;
        directory = parent;
    }

    return null;
}

function directoryTocTitle(readmePath) {
    return normalizePath(path.relative(PROJECT_ROOT, path.dirname(readmePath))) + "/";
}

function resolveLinkUrl(markdownPath, href) {
    if (/^[a-z]+:/i.test(href) || href.startsWith("#")) {
        return href;
    }

    const [rawPath, hash = ""] = href.split("#");
    const targetMarkdown = path.resolve(path.dirname(markdownPath), rawPath);
    const relativeTarget = normalizePath(path.relative(PROJECT_ROOT, targetMarkdown));

    if (rawPath.endsWith(".md") && markdownSet.has(relativeTarget)) {
        const targetOutput = outputPathForMarkdown(targetMarkdown);
        const currentOutputDir = path.dirname(outputPathForMarkdown(markdownPath));
        return relativeUrl(currentOutputDir, targetOutput) + (hash ? `#${hash}` : "#content");
    }

    const targetReadme = path.join(targetMarkdown, "README.md");
    const relativeTargetReadme = normalizePath(path.relative(PROJECT_ROOT, targetReadme));
    if (fs.existsSync(targetReadme) && markdownSet.has(relativeTargetReadme)) {
        const targetOutput = outputPathForMarkdown(targetReadme);
        const currentOutputDir = path.dirname(outputPathForMarkdown(markdownPath));
        return relativeUrl(currentOutputDir, targetOutput) + (hash ? `#${hash}` : "#content");
    }

    if (rawPath.endsWith(".md") && fs.existsSync(targetMarkdown) && !relativeTarget.startsWith("docs/")) {
        return `${SOURCE_ORIGIN}/${relativeTarget}${hash ? `#${hash}` : ""}`;
    }

    if (fs.existsSync(targetMarkdown) && !relativeTarget.startsWith("docs/")) {
        return `${SOURCE_ORIGIN}/${relativeTarget}${hash ? `#${hash}` : ""}`;
    }

    return href;
}

function resolveAssetUrl(markdownPath, src) {
    if (/^[a-z]+:/i.test(src) || src.startsWith("#")) {
        return src;
    }

    const target = path.resolve(path.dirname(markdownPath), src);
    const currentOutputDir = path.dirname(outputPathForMarkdown(markdownPath));
    return relativeUrl(currentOutputDir, target);
}

function outputPathForMarkdown(markdownPath) {
    const relative = normalizePath(path.relative(PROJECT_ROOT, markdownPath));

    if (relative === "README.md") {
        return path.join(OUTPUT_ROOT, "index.html");
    }

    if (path.basename(markdownPath) === "README.md") {
        return path.join(OUTPUT_ROOT, path.dirname(relative), "index.html");
    }

    const parsed = path.parse(relative);
    return path.join(OUTPUT_ROOT, parsed.dir, parsed.name, "index.html");
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

function structuredData(title, description, canonical, outputPath, lastModified) {
    const isHome = normalizePath(path.relative(OUTPUT_ROOT, outputPath)) === "index.html";
    const schemas = [breadcrumbStructuredData(outputPath)];

    if (isHome) {
        schemas.unshift(softwareSourceCodeStructuredData(title, description, canonical));
    } else {
        schemas.unshift(techArticleStructuredData(title, description, canonical, lastModified));
    }

    return JSON.stringify(schemas);
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

function techArticleStructuredData(title, description, canonical, lastModified) {
    return {
        "@context": "https://schema.org",
        "@type": "TechArticle",
        headline: title,
        description,
        url: canonical,
        mainEntityOfPage: canonical,
        dateModified: lastModified,
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
    const urls = files.map(file => `  <url>\n    <loc>${SITE_ORIGIN}${publicPathForOutput(outputPathForMarkdown(file))}</loc>\n    <lastmod>${fileLastModifiedDate(file)}</lastmod>\n    <changefreq>weekly</changefreq>\n    <priority>${normalizePath(path.relative(PROJECT_ROOT, file)) === "README.md" ? "1.0" : "0.7"}</priority>\n  </url>`);
    fs.writeFileSync(path.join(OUTPUT_ROOT, "sitemap.xml"), `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls.join("\n")}\n</urlset>\n`);
}

function fileLastModifiedDate(file) {
    return fs.statSync(file).mtime.toISOString().slice(0, 10);
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
