# nocter-session

## Responsibility

Orchestrate one compiler semantic pipeline from a closed compile input through target validation or
explicit recovery evidence.

## Contract

The crate calls declaration lowering, preparation, body checking, and target construction in the
only production order. It publishes one immutable session outcome and one semantic-evidence view for
analysis. It does not implement stage rules, editor queries, native code generation, or protocol
projection.

## Internal Responsibilities

- production and recovery semantic pipeline composition
- complete diagnostic retention
- target and executable request composition
- semantic evidence handoff
- session profiling and test selection

## Invariants

- Production and recovery cannot choose different semantic stage functions.
- Successful and recovered evidence are exclusive variants.
- A later failure cannot expose an older successful program.
- Failure-specific repair evidence moves once into its typed recovery owner.
