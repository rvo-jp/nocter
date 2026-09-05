const path = require("path");

class PublishedDocumentTree {
    #projectRoot;
    #root;
    #pagesBySource;

    constructor(projectRoot, sourceFiles) {
        this.#projectRoot = path.resolve(projectRoot);
        this.#root = directoryNode("", null);
        this.#pagesBySource = new Map();

        for (const sourcePath of sourceFiles) {
            this.#addSource(sourcePath);
        }

        freezeDirectory(this.#root);
    }

    page(sourcePath) {
        const page = this.#pagesBySource.get(path.resolve(sourcePath));
        if (!page) {
            throw new Error(`Published document tree has no source ${sourcePath}`);
        }
        return page;
    }

    navigation(sourcePath) {
        const page = this.page(sourcePath);
        let scope = page.parent;

        while (scope.parent && navigationTargets(scope).length <= 1) {
            scope = scope.parent;
        }

        return {
            scope,
            ancestors: ancestorLandings(scope),
            entries: navigationEntries(scope)
        };
    }

    sourcePaths() {
        return [...this.#pagesBySource.keys()];
    }

    #addSource(sourcePath) {
        const absoluteSource = path.resolve(sourcePath);
        const relative = path.relative(this.#projectRoot, absoluteSource);
        if (relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
            throw new Error(`Published documentation source is outside the repository: ${sourcePath}`);
        }

        const segments = relative.split(path.sep);
        const fileName = segments.pop();
        let directory = this.#root;
        for (const segment of segments) {
            let child = directory.directories.find(entry => entry.name === segment);
            if (!child) {
                child = directoryNode(segment, directory);
                directory.directories.push(child);
            }
            directory = child;
        }

        if (directory.pages.some(page => page.name === fileName)) {
            throw new Error(`Published document tree contains duplicate source ${relative}`);
        }

        const page = {
            kind: "page",
            name: fileName,
            relativePath: normalizePath(relative),
            sourcePath: absoluteSource,
            parent: directory
        };
        directory.pages.push(page);
        this.#pagesBySource.set(absoluteSource, page);
    }
}

function directoryNode(name, parent) {
    return {
        kind: "directory",
        name,
        parent,
        directories: [],
        pages: []
    };
}

function freezeDirectory(directory) {
    directory.directories.sort((left, right) => left.name.localeCompare(right.name));
    directory.pages.sort((left, right) => pageSortKey(left).localeCompare(pageSortKey(right)));
    for (const child of directory.directories) {
        freezeDirectory(child);
    }
    for (const page of directory.pages) {
        Object.freeze(page);
    }
    Object.freeze(directory.directories);
    Object.freeze(directory.pages);
    Object.freeze(directory);
}

function pageSortKey(page) {
    if (page.name === "README.md") return "0";
    if (page.name === "index.nct") return "1";
    return `2:${page.name}`;
}

function landingPage(directory) {
    return directory.pages.find(page => page.name === "README.md")
        || directory.pages.find(page => page.name === "index.nct")
        || null;
}

function navigationTargets(directory) {
    return flattenEntries(navigationEntries(directory));
}

function navigationEntries(directory) {
    const entries = [];
    const landing = landingPage(directory);

    if (landing) {
        entries.push({ kind: "page", page: landing });
    }

    for (const page of directory.pages) {
        if (page !== landing) {
            entries.push({ kind: "page", page });
        }
    }

    for (const child of directory.directories) {
        const childLanding = landingPage(child);
        if (childLanding) {
            entries.push({
                kind: "directory",
                directory: child,
                page: childLanding,
                children: []
            });
        } else {
            entries.push({
                kind: "directory",
                directory: child,
                page: null,
                children: navigationEntries(child)
            });
        }
    }

    return entries;
}

function flattenEntries(entries) {
    const pages = [];
    for (const entry of entries) {
        if (entry.kind === "page") {
            pages.push(entry.page);
        } else if (entry.page) {
            pages.push(entry.page);
        } else {
            pages.push(...flattenEntries(entry.children));
        }
    }
    return pages;
}

function ancestorLandings(directory) {
    const directories = [];
    let current = directory.parent;
    while (current) {
        directories.push(current);
        current = current.parent;
    }
    directories.reverse();

    return directories
        .map(entry => ({ directory: entry, page: landingPage(entry) }))
        .filter(entry => entry.page);
}

function directoryPath(directory) {
    const segments = [];
    let current = directory;
    while (current && current.parent) {
        segments.push(current.name);
        current = current.parent;
    }
    return segments.reverse().join("/");
}

function normalizePath(value) {
    return value.split(path.sep).join("/");
}

module.exports = {
    PublishedDocumentTree,
    directoryPath,
    flattenEntries,
    landingPage
};
