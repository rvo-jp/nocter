# Nocter Repository Rules

## Public Documentation Language

Write all public-facing documentation in English. This includes the repository `README.md`,
`spec/`, `development/README.md`, and `development/docs/`, plus release notes and generated website
content.

Internal agent instructions and handoff files that are excluded from the public documentation build,
such as `AGENTS.md` and `development/TODO.md`, may use another language when it improves team
communication.

Edit the source Markdown rather than generated HTML. After changing public Markdown, run
`node docs/build-docs.js` and commit the corresponding generated website changes.
