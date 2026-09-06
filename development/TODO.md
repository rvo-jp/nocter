# Nocter Development Handoff

## Current State

Nocter v0.36.0 is published and externally audited. v0.37.0 performance work is complete. The final
candidate comparison reduces single-file and package check medians by 23.6 percent, first checked
hover by 28.0 percent, body-edit hover by 32.7 percent, and the fifty-edit session by 33.2 percent.
Compiler verification is 46.3 percent faster. The final review found no semantic shortcut,
duplicated authority, editor divergence, or confirmed major-path regression.

## Next Work

Enter v0.37.0 release preparation: assign the 0.37.0 release identity, update user-facing release
metadata, run reproducible packaging and the complete installed-home qualification, record the
release audit, and stop before publication.

## Blockers

None.

## Non-negotiable Boundaries

- `spec/` is the sole source of public language behavior.
- A crate knows another responsibility only through its exported contract.
- A later phase cannot revisit an earlier representation to repeat a decision.
- Source projection cannot affect semantic selection.
- Compatibility fallbacks, source-text semantic inference, duplicate indexes, and order-dependent
  candidate selection are prohibited.
