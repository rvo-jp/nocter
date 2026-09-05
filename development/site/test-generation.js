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

    const unindexedReview = path.join(early, "development/reviews/unindexed-review.md");
    fs.writeFileSync(unindexedReview, "# Unindexed Review\n");
    const unindexedReviewResult = runBuild(early);
    if (
        unindexedReviewResult.status === 0
        || !combinedOutput(unindexedReviewResult).includes("does not catalog")
    ) {
        throw new Error("documentation generation accepted an unindexed development review");
    }
    fs.rmSync(unindexedReview);

    const unindexedAudit = path.join(early, "development/release-audits/unindexed-audit.md");
    fs.writeFileSync(unindexedAudit, "# Unindexed Release Audit\n");
    const unindexedAuditResult = runBuild(early);
    if (
        unindexedAuditResult.status === 0
        || !combinedOutput(unindexedAuditResult).includes("does not catalog")
    ) {
        throw new Error("documentation generation accepted an unindexed release audit");
    }
    fs.rmSync(unindexedAudit);

    const catalog = path.join(
        early,
        "development/compiler/crates/nocter-language/diagnostic-codes.txt"
    );
    fs.appendFileSync(catalog, "E9999\n");
    const rejected = runBuild(early);
    if (rejected.status === 0 || !combinedOutput(rejected).includes("Diagnostic catalog drift")) {
        throw new Error("documentation generation accepted a compiler catalog absent from the specification");
    }

    console.log("documentation generation is deterministic and enforces publication boundaries and catalog authority");
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
        "docs/development/std/str/index/index.html"
    ];
    for (const relative of required) {
        if (!fs.existsSync(path.join(root, relative))) {
            throw new Error(`documentation generation omitted published input ${relative}`);
        }
    }

    const privateSources = [
        "docs/development/std/str/text/index.html",
        "docs/development/std/internal/utf8/index/index.html"
    ];
    for (const relative of privateSources) {
        if (fs.existsSync(path.join(root, relative))) {
            throw new Error(`documentation generation published private standard-library source ${relative}`);
        }
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
