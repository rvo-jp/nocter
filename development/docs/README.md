# Development Documentation Index

This directory is for engineers and AI coding agents working on Nocter
development: the Rust bootstrap compiler, standard library implementation, and
release packaging. It must not become a second language specification.
Normative user-facing language rules live in [../../spec](../../spec/README.md).

## Documents

- [Architecture](architecture.md): pipeline, module ownership, shared facts, and
  the buildability boundary.
- [Implementation Status](implementation-status.md): current implementation
  capability and known gaps.
- [v0 Closure Definition](v0-closure.md): the fixed completion checklist for
  Nocter v0 implementation work.
- [Backend V0](backend-v0.md): ARM64 Darwin backend, Nocter ABI lowering, frame
  and call model, aggregate handling, and backend non-goals.
- [Std Runtime Status](std-runtime-status.md): what the tracked standard
  library ships, rejects, or keeps check-only when packaged as distributed
  `std/`.
- [Interpolation Lowering](interpolation-lowering.md): design note for promoting
  bare string interpolation.
- [Roadmap](roadmap.md): near-term implementation order.
- [Maintenance Policy](maintenance.md): refactoring, testing, documentation, and
  commit hygiene rules.

## Ownership Rules

- Put source language rules in `spec/`.
- Use [Design Principles](../../spec/00-design-principles.md) when a compiler
  implementation choice exposes new source behavior.
- Put Rust compiler implementation in `development/compiler/`.
- Put compiler architecture, implementation status, and internal ABI work in
  `development/docs/`.
- Put short-lived handoff notes in `development/TODO.md`.
- Keep root `README.md` as the short Nocter language introduction,
  getting-started flow, and documentation map.
- Do not copy whole feature specifications into this directory. Link to the
  relevant spec chapter and describe only implementation status or compiler
  design.
- When a user-visible behavior changes, update the relevant `spec/` chapter
  first, then update implementation status and closure gates if needed.
