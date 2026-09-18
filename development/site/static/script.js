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
    button.className = "code-copy";
    button.type = "button";
    button.textContent = "Copy";
    button.setAttribute("aria-label", "Copy code");
    button.addEventListener("click", async () => {
        try {
            await navigator.clipboard.writeText(pre.textContent);
            button.textContent = "Copied";
            setTimeout(() => { button.textContent = "Copy"; }, 1400);
        } catch {
            button.textContent = "Unavailable";
            setTimeout(() => { button.textContent = "Copy"; }, 1400);
        }
    });
    wrapper.appendChild(button);
});

const searchRoot = document.querySelector("[data-search-root]");
if (searchRoot) {
    const input = searchRoot.querySelector("input[type=search]");
    const results = searchRoot.querySelector(".site-search-results");
    let documentsPromise = null;

    function closeSearch() {
        results.hidden = true;
        input.setAttribute("aria-expanded", "false");
    }

    function openSearch() {
        results.hidden = false;
        input.setAttribute("aria-expanded", "true");
    }

    async function searchDocuments() {
        const query = input.value.trim().toLocaleLowerCase();
        if (query.length < 2) {
            closeSearch();
            return;
        }

        documentsPromise ||= fetch(searchRoot.dataset.searchIndex)
            .then(response => {
                if (!response.ok) throw new Error(`search index returned ${response.status}`);
                return response.json();
            })
            .then(index => index.documents);

        let documents;
        try {
            documents = await documentsPromise;
        } catch {
            results.replaceChildren();
            const unavailable = document.createElement("p");
            unavailable.className = "site-search-empty";
            unavailable.textContent = "Search is unavailable.";
            results.appendChild(unavailable);
            openSearch();
            return;
        }

        const matches = documents
            .map(entry => {
                const title = entry.title.toLocaleLowerCase();
                const entryPath = entry.path.toLocaleLowerCase();
                const headings = entry.headings.join(" ").toLocaleLowerCase();
                const text = entry.text.toLocaleLowerCase();
                let score = 0;
                if (title === query) score += 120;
                else if (title.startsWith(query)) score += 90;
                else if (title.includes(query)) score += 60;
                if (entryPath.includes(query)) score += 35;
                if (headings.includes(query)) score += 25;
                if (text.includes(query)) score += 10;
                return { entry, score };
            })
            .filter(match => match.score > 0)
            .sort((left, right) => right.score - left.score || left.entry.path.localeCompare(right.entry.path))
            .slice(0, 8);

        results.replaceChildren();
        if (matches.length === 0) {
            const empty = document.createElement("p");
            empty.className = "site-search-empty";
            empty.textContent = "No matching documentation.";
            results.appendChild(empty);
        } else {
            matches.forEach(({ entry }) => {
                const link = document.createElement("a");
                link.href = entry.url;
                const title = document.createElement("strong");
                title.textContent = entry.title;
                const entryPath = document.createElement("span");
                entryPath.textContent = entry.path;
                link.append(title, entryPath);
                results.appendChild(link);
            });
        }
        openSearch();
    }

    input.addEventListener("input", searchDocuments);
    input.addEventListener("focus", searchDocuments);
    input.addEventListener("keydown", event => {
        if (event.key === "Escape") {
            closeSearch();
            input.blur();
        } else if (event.key === "ArrowDown" && !results.hidden) {
            const first = results.querySelector("a");
            if (first) {
                event.preventDefault();
                first.focus();
            }
        }
    });
    document.addEventListener("click", event => {
        if (!searchRoot.contains(event.target)) closeSearch();
    });
}
