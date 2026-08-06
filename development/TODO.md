# Nocter Development Handoff

## Current Task

Restructure documentation so each fact has one owner and each directory serves one audience. The
ownership map is [docs/README.md](docs/README.md#information-ownership); the qualified candidate
record is [milestones/v0.5.0.md](milestones/v0.5.0.md).

## Completed Checkpoint

- public release notes moved to repository-root `releases/`
- published-release qualification moved to `development/releases/`
- the active candidate record moved to `development/milestones/`
- specification indexes no longer delegate normative behavior to development records
- developer website instructions moved out of generated `docs/` Markdown
- standard-library and LSP development documents now describe implementation architecture rather
  than public API inventories

## Next Work

Normalize the remaining specification chapters around the current v0.5.0 candidate contract.
Remove release-phase archaeology from unqualified rules, preserve only explicit current non-goals,
and keep old version semantics in repository tags rather than the current specification.

Do not publish, tag, merge, or change compiler behavior as part of this documentation migration.
