#!/usr/bin/env node

const childProcess = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");

const PROJECT_ROOT = path.resolve(__dirname, "../..");
const TEMP_ROOT = fs.mkdtempSync(path.join(os.tmpdir(), "nocter-doc-generation-"));
const SKIP_NAMES = new Set([".git", "dist", "docs", "target"]);

try {
    const early = prepareTree("early", new Date("2001-01-01T00:00:00Z"));
    const late = prepareTree("late", new Date("2037-12-31T23:59:59Z"));

    build(early);
    build(late);
    assertEqualTrees(path.join(early, "docs"), path.join(late, "docs"));
    assertPublicationBoundary(early);
    assertDocumentTreeNavigation(early);

    const staleOutput = path.join(early, "docs/stale-output.txt");
    fs.writeFileSync(staleOutput, "stale\n");
    build(early);
    if (fs.existsSync(staleOutput)) {
        throw new Error("documentation generation preserved output absent from its authored inputs");
    }

    const unrelatedRust = path.join(
        early,
        "development/compiler/crates/nocter-hash/src/lib.rs"
    );
    fs.appendFileSync(unrelatedRust, '\n#[cfg(test)]\nconst UNRELATED_TEXT: &str = "E9999";\n');
    build(early);

    const standardLibraryGuide = path.join(early, "development/std/map/README.md");
    const originalStandardLibraryGuide = fs.readFileSync(standardLibraryGuide, "utf8");
    fs.writeFileSync(standardLibraryGuide, originalStandardLibraryGuide.replaceAll("(index.nct)", "(README.md)"));
    const missingStandardLibraryContractLink = runBuild(early);
    if (
        missingStandardLibraryContractLink.status === 0
        || !combinedOutput(missingStandardLibraryContractLink).includes("does not link its public contract")
    ) {
        throw new Error("documentation generation accepted a standard-library guide without its contract link");
    }
    fs.writeFileSync(standardLibraryGuide, originalStandardLibraryGuide);

    const valueTypesSpecification = path.join(early, "spec/language/values-and-types.md");
    const originalValueTypesSpecification = fs.readFileSync(valueTypesSpecification, "utf8");
    fs.writeFileSync(
        valueTypesSpecification,
        originalValueTypesSpecification.replace("usize isize\nchar\nstr", "usize isize\nstr")
    );
    const missingNamedBuiltin = runBuild(early);
    if (missingNamedBuiltin.status === 0 || !combinedOutput(missingNamedBuiltin).includes("Named built-in type drift")) {
        throw new Error("documentation generation accepted a primitive type absent from the specification catalog");
    }
    fs.writeFileSync(valueTypesSpecification, originalValueTypesSpecification);

    const unindexedSpecification = path.join(
        early,
        "spec/language/unindexed-language-rule.md"
    );
    fs.writeFileSync(unindexedSpecification, "# Unindexed Language Rule\n");
    const unindexedGuide = path.join(early, "spec/guides/unindexed-guide.md");
    fs.writeFileSync(unindexedGuide, "# Unindexed Guide\n");
    build(early);
    for (const relative of [
        "docs/spec/language/unindexed-language-rule/index.html",
        "docs/spec/guides/unindexed-guide/index.html"
    ]) {
        if (!fs.existsSync(path.join(early, relative))) {
            throw new Error(`directory navigation omitted automatically discovered page ${relative}`);
        }
    }
    assertDocumentTreeNavigation(early);
    fs.rmSync(unindexedSpecification);
    fs.rmSync(unindexedGuide);

    const unindexedReview = path.join(early, "development/history/reviews/unindexed-review.md");
    fs.writeFileSync(unindexedReview, "# Unindexed Review\n");
    build(early);
    if (fs.existsSync(path.join(early, "docs/development/history/reviews/unindexed-review/index.html"))) {
        throw new Error("directory navigation published an excluded historical record");
    }
    fs.rmSync(unindexedReview);

    const catalog = path.join(
        early,
        "development/compiler/crates/nocter-language/diagnostic-codes.txt"
    );
    fs.appendFileSync(catalog, "E9999\n");
    const rejected = runBuild(early);
    if (rejected.status === 0 || !combinedOutput(rejected).includes("Diagnostic catalog drift")) {
        throw new Error("documentation generation accepted a compiler catalog absent from the specification");
    }

    console.log("documentation generation is deterministic and enforces publication boundaries and structural navigation");
} finally {
    fs.rmSync(TEMP_ROOT, { recursive: true, force: true });
}

function prepareTree(name, sourceTime) {
    const destination = path.join(TEMP_ROOT, name);
    fs.cpSync(PROJECT_ROOT, destination, {
        recursive: true,
        filter(source) {
            if (source === PROJECT_ROOT) return true;
            return !SKIP_NAMES.has(path.basename(source));
        }
    });
    for (const file of collectFiles(destination)) {
        if (file.endsWith(".md") || file.endsWith(".nct")) {
            fs.utimesSync(file, sourceTime, sourceTime);
        }
    }
    return destination;
}

function build(root) {
    const result = runBuild(root);
    if (result.status !== 0) {
        throw new Error(`documentation generation failed:\n${combinedOutput(result)}`);
    }
}

function runBuild(root) {
    return childProcess.spawnSync(process.execPath, ["development/site/build-docs.js"], {
        cwd: root,
        encoding: "utf8"
    });
}

function assertPublicationBoundary(root) {
    const required = [
        "docs/assets/logo.svg",
        "docs/examples/hello/index.html",
        "docs/spec/language/index.html",
        "docs/std/index.html",
        "docs/std/str/index/index.html"
    ];
    for (const relative of required) {
        if (!fs.existsSync(path.join(root, relative))) {
            throw new Error(`documentation generation omitted published input ${relative}`);
        }
    }

    const privateSources = [
        "docs/development/history/index.html",
        "docs/development/history/milestones/index.html",
        "docs/development/std/index.html",
        "docs/std/str/text/index.html",
        "docs/std/internal/utf8/index/index.html"
    ];
    for (const relative of privateSources) {
        if (fs.existsSync(path.join(root, relative))) {
            throw new Error(`documentation generation published private standard-library source ${relative}`);
        }
    }
}

function assertDocumentTreeNavigation(root) {
    const docsRoot = path.join(root, "docs");
    const pages = collectFiles(docsRoot).filter(file => file.endsWith("index.html"));
    const pageSet = new Set(pages.map(file => path.resolve(file)));
    const reachable = new Set();
    const pending = [path.join(docsRoot, "index.html")];

    while (pending.length > 0) {
        const page = path.resolve(pending.pop());
        if (reachable.has(page)) {
            continue;
        }
        reachable.add(page);

        const html = fs.readFileSync(page, "utf8");
        const navigation = html.match(/<aside class="document-tree">([\s\S]*?)<\/aside>/)?.[1];
        if (!navigation) {
            throw new Error(`generated page has no structural navigation: ${path.relative(docsRoot, page)}`);
        }
        for (const match of navigation.matchAll(/href="([^"]+)"/g)) {
            const href = match[1].split("#")[0];
            if (!href || /^[a-z]+:/i.test(href)) {
                continue;
            }
            const target = path.resolve(path.dirname(page), href);
            if (!pageSet.has(target)) {
                throw new Error(`structural navigation points to a missing page: ${path.relative(docsRoot, target)}`);
            }
            if (!reachable.has(target)) {
                pending.push(target);
            }
        }
    }

    const unreachable = pages
        .filter(page => !reachable.has(path.resolve(page)))
        .map(page => path.relative(docsRoot, page))
        .sort();
    if (unreachable.length > 0) {
        throw new Error(`generated pages are unreachable through structural navigation: ${unreachable.join(", ")}`);
    }
}

function combinedOutput(result) {
    return `${result.stdout || ""}${result.stderr || ""}`;
}

function assertEqualTrees(leftRoot, rightRoot) {
    const left = collectRelativeFiles(leftRoot);
    const right = collectRelativeFiles(rightRoot);
    if (left.length !== right.length || left.some((file, index) => file !== right[index])) {
        throw new Error("documentation generation produced different file sets");
    }
    for (const file of left) {
        const leftBytes = fs.readFileSync(path.join(leftRoot, file));
        const rightBytes = fs.readFileSync(path.join(rightRoot, file));
        if (!leftBytes.equals(rightBytes)) {
            throw new Error(`documentation generation depends on source metadata: ${file}`);
        }
    }
}

function collectRelativeFiles(root) {
    return collectFiles(root).map(file => path.relative(root, file)).sort();
}

function collectFiles(root) {
    const files = [];
    for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
        const child = path.join(root, entry.name);
        if (entry.isDirectory()) {
            files.push(...collectFiles(child));
        } else if (entry.isFile()) {
            files.push(child);
        }
    }
    return files;
}
