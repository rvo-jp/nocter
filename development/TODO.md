# Current Development Work

## Immediate Work

1. Replace implicit transfer in the private value-access frontend with the adopted postfix source
   operation. Audit existing fixtures by their intended availability and commit timing; migrate
   syntax, checked supply, capture formation, initialization/access/resource consumers and private
   instances coherently. The [implementation boundary](architecture/pipeline/value-access-declarations.md)
   owns current support and limits. The [realization requirements](design/compiler-realization.md)
   own the remaining distinguishing cases and proof obligations.
2. Close typed sequence/string entries, coercion and user-operator/conditional const staging under
   the adopted design direction. Then close remaining expression/control, capability/library,
   package/platform and task/async-iteration subjects in the [source draft](design/source-grammar.md).
   Integrate ordinary reading/writing examples before extending accepted source scope.
3. Lower the migrated private instance graph into executable lifecycle operations, then qualify
   physical layout, ABI, MIR and native behavior. Keep required backing, frozen input channels,
   protected answers, actual cleanup and adapter disposal coherent.
4. Extend contextual/multiple/nested captures, general R/D/S schemas, interfaces/generics, outcomes,
   async and public modules only with their owning specification and end-to-end evidence.
5. Run the vertical [optimization experiments](design/optimization.md) after their ordinary source
   and executable paths are admitted. Qualify correctness and generated operations before timing.

Use the [development instructions](AGENTS.md) for delegated scope and syntax approvals, and the
[verification guide](verification/README.md) to select checks that consume the changed path.

## Open Findings

- Audit the derived-ordering selector/text discrepancy against the owning current contracts.
  Resolve it in the operator design before admitting comparison derivation; a method spelling
  alone must not select semantics.

## Release Boundary

Publication of the retained [candidate](packaging/candidate.md) requires separate authorization.
Preserve its qualified artifact and source identity during v2 work. Linux target work remains
deferred. No external blocker is established for current v2 work.
