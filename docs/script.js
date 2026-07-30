document.querySelectorAll(".hero-code-tab").forEach(tab => {
    tab.addEventListener("click", () => {
        const example = tab.dataset.example;

        document
            .querySelectorAll(".hero-code-tab")
            .forEach(item => item.setAttribute("aria-selected", String(item === tab)));

        document
            .querySelectorAll("[data-example-panel]")
            .forEach(panel => {
                panel.hidden = panel.dataset.examplePanel !== example;
            });
    });
});
