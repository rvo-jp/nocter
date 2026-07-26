# Compiler Documentation Index

This directory is for compiler engineers and AI coding agents working on the
Rust bootstrap compiler. It must not become a second language specification.
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
- [Std Runtime Status](std-runtime-status.md): what the distributed `.nocter/std`
  implementation ships, rejects, or keeps check-only.
- [Interpolation Lowering](interpolation-lowering.md): design note for promoting
  bare string interpolation.
- [Roadmap](roadmap.md): near-term implementation order.
- [Maintenance Policy](maintenance.md): refactoring, testing, documentation, and
  commit hygiene rules.

## Ownership Rules

- Put source language rules in `spec/`.
- Put compiler architecture, implementation status, internal ABI work, and
  handoff notes in `compiler/`.
- Keep root `README.md` as a short public entrance.
- Do not copy whole feature specifications into this directory. Link to the
  relevant spec chapter and describe only implementation status or compiler
  design.
- When a user-visible behavior changes, update the relevant `spec/` chapter
  first, then update implementation status and closure gates if needed.
