#!/usr/bin/env node

const childProcess = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { splitTableRow } = require("./markdown-table");

const PROJECT_ROOT = path.resolve(__dirname, "../..");
const TEMP_ROOT = fs.mkdtempSync(path.join(os.tmpdir(), "nocter-doc-generation-"));
const SOURCE_REVISION = "0123456789abcdef0123456789abcdef01234567";
const SKIP_NAMES = new Set([".git", "dist", "target"]);

try {
    assertMarkdownTableTokenizer();
    const early = prepareTree("early", new Date("2001-01-01T00:00:00Z"));
    const late = prepareTree("late", new Date("2037-12-31T23:59:59Z"));

    assertOutputIsolation(early);
    build(early);
    build(late);
    assertEqualTrees(generatedRoot(early), generatedRoot(late));
    assertPublicationBoundary(early);
    assertDocumentTreeNavigation(early);
    assertMarkdownTableRendering(early);
    assertDeploymentManifest(early);
    assertRepositoryLinksUseDeploymentRevision(early);

    const staleOutput = path.join(generatedRoot(early), "stale-output.txt");
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

    const standardLibraryCatalog = path.join(early, "development/std/README.md");
    const originalStandardLibraryCatalog = fs.readFileSync(standardLibraryCatalog, "utf8");
    fs.writeFileSync(
        standardLibraryCatalog,
        originalStandardLibraryCatalog.replace("](map/README.md)", "](map/index.nct)")
    );
    const unassignedStandardLibraryGuide = runBuild(early);
    if (
        unassignedStandardLibraryGuide.status === 0
        || !combinedOutput(unassignedStandardLibraryGuide).includes("has no authority assignment")
    ) {
        throw new Error("documentation generation accepted an unassigned standard-library behavior guide");
    }
    fs.writeFileSync(standardLibraryCatalog, originalStandardLibraryCatalog);

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
    const generatedGuideFixtureDirectory = path.join(early, "spec/guides");
    const unindexedGuide = path.join(generatedGuideFixtureDirectory, "unindexed-guide.md");
    fs.mkdirSync(generatedGuideFixtureDirectory, { recursive: true });
    fs.writeFileSync(unindexedGuide, "# Unindexed Guide\n");
    build(early);
    for (const relative of [
        "spec/language/unindexed-language-rule/index.html",
        "spec/guides/unindexed-guide/index.html"
    ]) {
        if (!fs.existsSync(path.join(generatedRoot(early), relative))) {
            throw new Error(`directory navigation omitted automatically discovered page ${relative}`);
        }
    }
    assertDocumentTreeNavigation(early);
    fs.rmSync(unindexedSpecification);
    fs.rmSync(unindexedGuide);

    const unindexedReview = path.join(early, "development/history/reviews/unindexed-review.md");
    fs.writeFileSync(unindexedReview, "# Unindexed Review\n");
    build(early);
    if (fs.existsSync(path.join(generatedRoot(early), "development/history/reviews/unindexed-review/index.html"))) {
        throw new Error("directory navigation published an excluded historical record");
    }
    fs.rmSync(unindexedReview);

    const malformedTable = path.join(early, "spec/guides/malformed-table.md");
    fs.writeFileSync(
        malformedTable,
        "# Malformed Table\n\n| first | second |\n| --- | --- |\n| one | two | three |\n"
    );
    const malformedTableResult = runBuild(early);
    if (
        malformedTableResult.status === 0
        || !combinedOutput(malformedTableResult).includes("has 3 cells; expected 2")
    ) {
        throw new Error("documentation generation accepted a Markdown table with inconsistent columns");
    }
    fs.rmSync(malformedTable);
    fs.rmSync(generatedGuideFixtureDirectory, { recursive: true });

    const unsupportedStaticEntry = path.join(early, "development/site/static/style-link.css");
    fs.symlinkSync("style.css", unsupportedStaticEntry);
    const unsupportedArtifact = runBuild(early);
    if (
        unsupportedArtifact.status === 0
        || !combinedOutput(unsupportedArtifact).includes("unsupported entry")
    ) {
        throw new Error("documentation generation accepted a symbolic link in its Pages artifact");
    }
    fs.rmSync(unsupportedStaticEntry);

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
    return childProcess.spawnSync(process.execPath, [
        "development/site/build-docs.js",
        "--output",
        generatedRoot(root),
        "--source-revision",
        SOURCE_REVISION
    ], {
        cwd: root,
        encoding: "utf8"
    });
}

function generatedRoot(root) {
    return path.join(TEMP_ROOT, `${path.basename(root)}-output`);
}

function assertOutputIsolation(root) {
    const missingOutput = childProcess.spawnSync(
        process.execPath,
        ["development/site/build-docs.js"],
        { cwd: root, encoding: "utf8" }
    );
    if (
        missingOutput.status === 0
        || !combinedOutput(missingOutput).includes("requires --output <directory>")
    ) {
        throw new Error("documentation generation accepted an implicit repository output");
    }

    const repositoryOutput = childProcess.spawnSync(process.execPath, [
        "development/site/build-docs.js",
        "--output",
        path.join(root, "generated-site")
    ], { cwd: root, encoding: "utf8" });
    if (
        repositoryOutput.status === 0
        || !combinedOutput(repositoryOutput).includes("must be disjoint")
    ) {
        throw new Error(
            `documentation generation accepted output inside its source repository:\n${combinedOutput(repositoryOutput)}`
        );
    }

    const ancestorOutput = childProcess.spawnSync(process.execPath, [
        "development/site/build-docs.js",
        "--output",
        path.dirname(root)
    ], { cwd: root, encoding: "utf8" });
    if (
        ancestorOutput.status === 0
        || !combinedOutput(ancestorOutput).includes("must be disjoint")
    ) {
        throw new Error(
            `documentation generation accepted an ancestor of its source repository:\n${combinedOutput(ancestorOutput)}`
        );
    }

    const duplicateOutput = childProcess.spawnSync(process.execPath, [
        "development/site/build-docs.js",
        "--output",
        generatedRoot(root),
        "--output",
        `${generatedRoot(root)}-other`
    ], { cwd: root, encoding: "utf8" });
    if (
        duplicateOutput.status === 0
        || !combinedOutput(duplicateOutput).includes("may be specified only once")
    ) {
        throw new Error("documentation generation accepted two output authorities");
    }
}

function assertPublicationBoundary(root) {
    const required = [
        "assets/logo.svg",
        "examples/hello/index.html",
        "spec/language/index.html",
        "std/index.html",
        "std/str/index/index.html"
    ];
    for (const relative of required) {
        if (!fs.existsSync(path.join(generatedRoot(root), relative))) {
            throw new Error(`documentation generation omitted published input ${relative}`);
        }
    }
    const home = fs.readFileSync(path.join(generatedRoot(root), "index.html"), "utf8");
    if (!home.includes('src="./assets/logo.svg"')) {
        throw new Error("repository and website logo paths no longer share one public asset");
    }

    const privateSources = [
        "development/history/index.html",
        "development/history/milestones/index.html",
        "development/std/index.html",
        "std/str/text/index.html",
        "std/json/output/index/index.html",
        "std/internal/utf8/index/index.html"
    ];
    for (const relative of privateSources) {
        if (fs.existsSync(path.join(generatedRoot(root), relative))) {
            throw new Error(`documentation generation published private standard-library source ${relative}`);
        }
    }
}

function assertDocumentTreeNavigation(root) {
    const docsRoot = generatedRoot(root);
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

function assertMarkdownTableRendering(root) {
    const grammar = fs.readFileSync(
        path.join(generatedRoot(root), "development/design/grammar-conformance/index.html"),
        "utf8"
    );
    const row = grammar.match(/<tr><td>G006<\/td>([\s\S]*?)<\/tr>/)?.[0];
    if (!row) {
        throw new Error("generated grammar documentation has no G006 table row");
    }
    if ((row.match(/<td>/g) || []).length !== 5) {
        throw new Error("a pipe inside a Markdown code span changed the generated table width");
    }
    if (!row.includes("<code>func choose&lt;T&gt;(left: &amp;T, right: &amp;T): &amp;T from left | right</code>")) {
        throw new Error("a pipe inside a Markdown code span did not remain in its source cell");
    }
}

function assertDeploymentManifest(root) {
    const manifest = JSON.parse(
        fs.readFileSync(path.join(generatedRoot(root), "deployment.json"), "utf8")
    );
    if (
        manifest.schema !== "nocter.documentation-deployment"
        || manifest.version !== 1
        || manifest.source_repository !== "https://github.com/rvo-jp/nocter"
        || manifest.source_revision !== SOURCE_REVISION
    ) {
        throw new Error(`documentation deployment identity changed: ${JSON.stringify(manifest)}`);
    }
}

function assertRepositoryLinksUseDeploymentRevision(root) {
    const contributorPage = fs.readFileSync(
        path.join(generatedRoot(root), "development/index.html"),
        "utf8"
    );
    const expected = `https://github.com/rvo-jp/nocter/blob/${SOURCE_REVISION}/development/history/`;
    if (!contributorPage.includes(expected)) {
        throw new Error("generated repository links do not use the deployment source revision");
    }
}

function assertMarkdownTableTokenizer() {
    const cells = splitTableRow("| name | `left | right` | escaped \\| pipe |");
    const expected = ["name", "`left | right`", "escaped | pipe"];
    if (JSON.stringify(cells) !== JSON.stringify(expected)) {
        throw new Error(`Markdown table tokenizer produced ${JSON.stringify(cells)}`);
    }

    const evenEscape = splitTableRow("| first \\\\| second |");
    if (evenEscape.length !== 2) {
        throw new Error("an even backslash run incorrectly escaped a Markdown table delimiter");
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
