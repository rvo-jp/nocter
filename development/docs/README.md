# Nocter Development Documents

This directory contains the active implementation policy for the specification-first compiler
rewrite. Public language and standard-library behavior belongs exclusively in
[`spec/`](../../spec/README.md).

## Active Documents

- [Compiler Rewrite Architecture](architecture.md)
- [Maintenance](maintenance.md)
- [Documentation Site Generation](site-generation.md)
- [Current Handoff](../TODO.md)
- [v0.14.0 Rewrite Milestone](../milestones/v0.14.0.md)

Historical implementation design was removed from the active tree when the compiler rewrite
started. Git history and published release records preserve it for archival purposes, but it must
not be consulted to determine new compiler structure or unspecified language behavior.

## Information Ownership

| Information | Sole owner |
|---|---|
| Public language and standard-library behavior | `spec/` |
| New compiler dependency and authority boundaries | `architecture.md` |
| Rewrite scope and completion gates | `../milestones/v0.14.0.md` |
| Next concrete work and blockers | `../TODO.md` |
| Published qualification evidence | `../releases/` |
| Documentation generation | `site-generation.md` |

Do not duplicate normative rules in development documents. Link to the owning specification rule.
