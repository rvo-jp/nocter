# Current Development Work

## Immediate Work

1. Close user comparison semantics and conditional generic const contracts under the adopted
   design direction. Qualify typed literal/conversion and const-operator source support against
   their owning contracts. Then close remaining expression/control, capability/library,
   package/platform and task/async-iteration subjects in the [source draft](design/source-grammar.md).
   Integrate ordinary reading/writing examples before extending accepted source scope.
2. Integrate the private activation/storage companion with value computation, actual input/result
   ABI transfer and disposal helpers into complete executable bodies; then qualify native behavior.
   Keep required backing, frozen input channels, protected answers, cleanup and adapter disposal
   coherent. Physical frame/effect planning alone does not qualify source execution.
3. Extend contextual/multiple/nested captures, general R/D/S schemas, interfaces/generics, outcomes,
   async and public modules only with their owning specification and end-to-end evidence.
4. Run the vertical [optimization experiments](design/optimization.md) after their ordinary source
   and executable paths are admitted. Qualify correctness and generated operations before timing.

Use the [agent instructions](../AGENTS.md) for delegated scope and syntax approvals, and the
[verification guide](verification/README.md) to select checks that consume the changed path.

## Open Findings

- The released native test `network_and_tls::public_tls_and_https_reject_plain_peers_and_https_advertises_http1`
  exits with status 1 instead of 0 in the full suite. Its cause is untriaged; the existing failure
  message does not identify the sync/async image or failure code. Investigate with the focused
  `nocter-native-session` test and its plain-peer fixture before changing product behavior or
  expectations. This path does not consume the private v2 Unit; the complete compiler gate is
  not qualified by the successful private-source checks.
- Audit the derived-ordering selector/text discrepancy against the owning current contracts.
  Resolve it in the operator design before admitting comparison derivation; a method spelling
  alone must not select semantics.

## Release Boundary

Publication of the retained [candidate](packaging/candidate.md) requires separate authorization.
Preserve its qualified artifact and source identity during v2 work. Linux target work remains
deferred. No external blocker is established for current v2 work.
