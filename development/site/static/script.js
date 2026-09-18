const heroTabs = [...document.querySelectorAll(".hero-code-tab")];

function selectHeroExample(example, focus = false) {
    heroTabs.forEach(tab => {
        const selected = tab.dataset.example === example;
        tab.setAttribute("aria-selected", String(selected));
        tab.tabIndex = selected ? 0 : -1;
        if (selected && focus) tab.focus();
    });

    document.querySelectorAll("[data-example-panel]").forEach(panel => {
        panel.hidden = panel.dataset.examplePanel !== example;
    });

    try {
        sessionStorage.setItem("nocter-hero-example", example);
    } catch {
        // Storage is an optional convenience; tab behavior remains complete without it.
    }
}

if (heroTabs.length > 0) {
    let selected = heroTabs[0].dataset.example;
    try {
        const stored = sessionStorage.getItem("nocter-hero-example");
        if (heroTabs.some(tab => tab.dataset.example === stored)) selected = stored;
    } catch {
        // Keep the first authored example when storage is unavailable.
    }
    selectHeroExample(selected);

    heroTabs.forEach((tab, index) => {
        tab.addEventListener("click", () => selectHeroExample(tab.dataset.example));
        tab.addEventListener("keydown", event => {
            if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
            event.preventDefault();
            let target = index;
            if (event.key === "ArrowLeft") target = (index - 1 + heroTabs.length) % heroTabs.length;
            if (event.key === "ArrowRight") target = (index + 1) % heroTabs.length;
            if (event.key === "Home") target = 0;
            if (event.key === "End") target = heroTabs.length - 1;
            selectHeroExample(heroTabs[target].dataset.example, true);
        });
    });
}

const documentTreeToggle = document.querySelector(".document-tree-toggle");
if (documentTreeToggle) {
    documentTreeToggle.addEventListener("click", () => {
        const expanded = documentTreeToggle.getAttribute("aria-expanded") === "true";
        documentTreeToggle.setAttribute("aria-expanded", String(!expanded));
    });
}

document.querySelectorAll(".markdown-body > pre").forEach(pre => {
    const wrapper = document.createElement("div");
    wrapper.className = "code-block";
    pre.parentNode.insertBefore(wrapper, pre);
    wrapper.appendChild(pre);

    const button = document.createElement("button");
    let resetTimer = null;
    button.className = "code-copy";
    button.type = "button";
    button.setAttribute("aria-live", "polite");
    setCopyButtonState(button, "copy");
    button.addEventListener("click", async () => {
        try {
            await navigator.clipboard.writeText(pre.textContent);
            setCopyButtonState(button, "copied");
        } catch {
            setCopyButtonState(button, "unavailable");
        }
        if (resetTimer) clearTimeout(resetTimer);
        resetTimer = setTimeout(() => {
            setCopyButtonState(button, "copy");
            resetTimer = null;
        }, 1400);
    });
    wrapper.appendChild(button);
});

function setCopyButtonState(button, state) {
    const labels = {
        copy: "Copy code",
        copied: "Code copied",
        unavailable: "Copy unavailable"
    };
    const paths = {
        copy: '<rect x="8" y="8" width="10" height="10" rx="1.5"></rect><path d="M14 8V6.5A1.5 1.5 0 0 0 12.5 5h-6A1.5 1.5 0 0 0 5 6.5v6A1.5 1.5 0 0 0 6.5 14H8"></path>',
        copied: '<path d="m5.5 12 4 4 9-10"></path>',
        unavailable: '<path d="m7 7 10 10M17 7 7 17"></path>'
    };

    button.setAttribute("aria-label", labels[state]);
    button.title = labels[state];
    button.dataset.state = state;
    button.innerHTML = `<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">${paths[state]}</svg>`;
}

const outlineLinks = [...document.querySelectorAll("[data-outline-link]")];
if (outlineLinks.length > 0) {
    const linksByTarget = new Map(outlineLinks.map(link => [decodeURIComponent(link.hash.slice(1)), link]));
    const targets = [...linksByTarget.keys()].map(id => document.getElementById(id)).filter(Boolean);

    function selectOutlineTarget(id, reveal = false) {
        document.querySelectorAll(".source-outline details[data-active]").forEach(group => {
            group.removeAttribute("data-active");
        });
        outlineLinks.forEach(link => {
            if (decodeURIComponent(link.hash.slice(1)) === id) {
                link.setAttribute("aria-current", "location");
                const group = link.closest("details");
                if (group) {
                    group.dataset.active = "true";
                    if (reveal) group.open = true;
                }
            } else link.removeAttribute("aria-current");
        });
    }

    const requestedTarget = decodeURIComponent(location.hash.slice(1));
    selectOutlineTarget(linksByTarget.has(requestedTarget) ? requestedTarget : targets[0]?.id, true);

    if ("IntersectionObserver" in window) {
        const visible = new Set();
        const observer = new IntersectionObserver(entries => {
            entries.forEach(entry => {
                if (entry.isIntersecting) visible.add(entry.target);
                else visible.delete(entry.target);
            });
            const current = [...visible].sort((left, right) => left.offsetTop - right.offsetTop).at(-1);
            if (current) selectOutlineTarget(current.id);
        }, { rootMargin: "-12% 0px -72% 0px" });
        targets.forEach(target => observer.observe(target));
    }

    window.addEventListener("hashchange", () => {
        const id = decodeURIComponent(location.hash.slice(1));
        if (linksByTarget.has(id)) selectOutlineTarget(id, true);
    });
}

const searchRoot = document.querySelector("[data-search-root]");
if (searchRoot) {
    const input = searchRoot.querySelector("input[type=search]");
    const results = searchRoot.querySelector(".site-search-results");
    const matchesRoot = searchRoot.querySelector("[data-search-matches]");
    const scopeButtons = [...searchRoot.querySelectorAll("[data-search-scope]")];
    let documentsPromise = null;
    let searchRequest = 0;
    let selectedScope = "all";

    function closeSearch() {
        results.hidden = true;
        input.setAttribute("aria-expanded", "false");
    }

    function cancelSearch() {
        searchRequest += 1;
        closeSearch();
    }

    function openSearch() {
        results.hidden = false;
        input.setAttribute("aria-expanded", "true");
    }

    async function searchDocuments() {
        const request = ++searchRequest;
        const query = input.value.trim().toLocaleLowerCase();
        if (query.length < 2) {
            matchesRoot.replaceChildren();
            const prompt = document.createElement("p");
            prompt.className = "site-search-empty";
            prompt.textContent = "Type at least 2 characters to search.";
            matchesRoot.appendChild(prompt);
            openSearch();
            return;
        }

        documentsPromise ||= fetch(searchRoot.dataset.searchIndex)
            .then(response => {
                if (!response.ok) throw new Error(`search index returned ${response.status}`);
                return response.json();
            })
            .then(index => {
                if (index.version !== 2 || !Array.isArray(index.documents)) {
                    throw new Error("unsupported search index");
                }
                return index.documents;
            });

        let documents;
        try {
            documents = await documentsPromise;
        } catch {
            if (request !== searchRequest) return;
            matchesRoot.replaceChildren();
            const unavailable = document.createElement("p");
            unavailable.className = "site-search-empty";
            unavailable.textContent = "Search is unavailable.";
            matchesRoot.appendChild(unavailable);
            openSearch();
            return;
        }
        if (request !== searchRequest) return;

        const matches = documents
            .filter(entry => selectedScope === "all" || entry.section === selectedScope)
            .map(entry => {
                const documentScore = scoreDocument(entry, query);
                const symbol = bestSymbolMatch(entry.symbols, query);
                if (symbol && symbol.score > documentScore) {
                    return {
                        entry,
                        score: symbol.score,
                        title: symbol.symbol.qualified_name,
                        detail: `${symbol.symbol.kind} · ${entry.path}`,
                        url: symbol.symbol.url
                    };
                }
                return {
                    entry,
                    score: documentScore,
                    title: entry.title,
                    detail: `${sectionLabel(entry.section)} · ${entry.path}`,
                    url: entry.url
                };
            })
            .filter(match => match.score > 0)
            .sort((left, right) => right.score - left.score || left.entry.path.localeCompare(right.entry.path))
            .slice(0, 8);

        matchesRoot.replaceChildren();
        if (matches.length === 0) {
            const empty = document.createElement("p");
            empty.className = "site-search-empty";
            empty.textContent = "No matching documentation.";
            matchesRoot.appendChild(empty);
        } else {
            matches.forEach(match => {
                const link = document.createElement("a");
                link.href = match.url;
                const title = document.createElement("strong");
                title.textContent = match.title;
                const detail = document.createElement("span");
                detail.textContent = match.detail;
                link.append(title, detail);
                matchesRoot.appendChild(link);
            });
        }
        openSearch();
    }

    input.addEventListener("input", searchDocuments);
    input.addEventListener("focus", searchDocuments);
    scopeButtons.forEach(button => {
        button.addEventListener("click", () => {
            selectedScope = button.dataset.searchScope;
            scopeButtons.forEach(candidate => {
                candidate.setAttribute("aria-pressed", String(candidate === button));
            });
            searchDocuments();
        });
    });
    input.addEventListener("keydown", event => {
        if (event.key === "Escape") {
            cancelSearch();
            input.blur();
        } else if (event.key === "ArrowDown" && !results.hidden) {
            const first = matchesRoot.querySelector("a");
            if (first) {
                event.preventDefault();
                first.focus();
            }
        }
    });
    document.addEventListener("click", event => {
        if (!searchRoot.contains(event.target)) cancelSearch();
    });
    document.addEventListener("keydown", event => {
        const target = event.target;
        const editing = target instanceof HTMLInputElement
            || target instanceof HTMLTextAreaElement
            || target.isContentEditable;
        if (event.key === "/" && !editing && !event.metaKey && !event.ctrlKey && !event.altKey) {
            event.preventDefault();
            input.focus();
        }
    });
}

function scoreDocument(entry, query) {
    const title = entry.title.toLocaleLowerCase();
    const entryPath = entry.path.toLocaleLowerCase();
    const headings = entry.headings.join(" ").toLocaleLowerCase();
    const text = entry.text.toLocaleLowerCase();
    let score = 0;
    if (title === query) score += 140;
    else if (title.startsWith(query)) score += 100;
    else if (title.includes(query)) score += 70;
    if (entryPath.includes(query)) score += 35;
    if (headings.includes(query)) score += 25;
    if (text.includes(query)) score += 10;
    return score;
}

function bestSymbolMatch(symbols, query) {
    return symbols.map(symbol => {
        const name = symbol.name.toLocaleLowerCase();
        const qualified = symbol.qualified_name.toLocaleLowerCase();
        const documentation = symbol.documentation.toLocaleLowerCase();
        let score = 0;
        if (qualified === query) score = 220;
        else if (name === query) score = 200;
        else if (qualified.startsWith(query)) score = 150;
        else if (name.startsWith(query)) score = 140;
        else if (qualified.includes(query)) score = 100;
        else if (name.includes(query)) score = 90;
        if (documentation.includes(query)) score += 20;
        return { symbol, score };
    }).sort((left, right) => right.score - left.score)[0] || null;
}

function sectionLabel(section) {
    return ({
        home: "Home",
        specification: "Specification",
        "standard-library": "Standard Library",
        examples: "Examples",
        contributors: "Contributors",
        releases: "Releases"
    })[section] || section;
}
