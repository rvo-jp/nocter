# Interfaces and Static Operation Binding

## Scope

An interface is a nominal contract for operations, associated types, prerequisites and documented
semantic obligations. Presence of similarly named members is not conformance. Explicit
conformance freezes a static operation-to-implementation relation; it does not introduce a vtable,
runtime dictionary, allocation, class hierarchy or UFCS.

This chapter closes declaration roles, associated binding placement, default selection and named
operation mapping. The [grammar](syntactic-grammar.md#files-and-declarations) owns their source
forms. Complete generic solving, associated-projection normalization, opaque-result inference,
effect inference, other operator families and executable realization remain separate gates.
These rules are not compiler acceptance evidence for those features.

## Interface Members

```text
interface Parser {
    type Output
    func create(): Self
    method edit self.parse(text: str): Self.Output!
    default method self.available(): bool
}
```

Associated types are required selections, without stored fields, generic parameters or defaults.
An operation without `default` is required and bodyless. A `default` operation has a checked
implementation, declared bodylessly in a public root and completed in a reciprocally seen
ordinary interface fragment. A private interface may define its defaults inline. A completion
fragment may repeat only the contracted default headers and bodies; it cannot introduce
requirements, associated types or a second interface identity.

Interfaces may require receiver methods, type-owned construction functions and the supported
comparison/index operation shapes. Construction functions must construct implementing `Self`
under [type behavior](type-behavior.md#functions-and-methods); this does not admit receiverless
utility functions. Interfaces do not define fields, literal storage, destructors or arbitrary
operator spellings. Every operation is identified by its owning interface plus member identity,
not by its helper name, rendered signature or declaration order. At use, retain the selected
interface application's ordered generic arguments as well; different applications cannot be
merged by an equal member name or normalized result.

`Self` denotes the implementing type. `Self.Output` denotes the associated declaration selected
on this interface/prerequisite set. Associated names occupy the type namespace; named functions
and methods occupy the operation namespace. Distinct interfaces may use the same operation or
associated-type name without becoming the same contract. An unqualified associated projection
requires one unique owning declaration; equal resulting types cannot resolve an ambiguous name.
General associated-projection disambiguation/normalization is not supplied by name coincidence.
The [generic contract](generics-and-constants.md#requirements-and-associated-projections) owns
required type-position interface selection and its normalization obligations.

## Conformance and Implementation Bindings

An inner `impl Interface` in `impl Type` commits that owner to one interface application. The
outer target and inner interface roles follow syntax position; no name lookup reinterprets one
role as the other. The owner must have one valid implementation for each prerequisite and every
required member. Duplicate or overlapping facts for the same interface application are errors
rather than ranked specializations. Distinct closed interface applications remain distinct
capabilities, not candidates from which inference may guess an unknown interface argument.

Associated bindings and named operation mappings share one comma-delimited block. A leading
`.` identifies a type binding syntactically:

```text
impl DocumentParser {
    impl Parser {
        .Output = Document,
        parse = parse_document,
    }
}
```

Bind every associated declaration owned directly by that interface exactly once, and no unknown
declaration. Check its declared requirements. Inherited associated bindings belong to their
owning prerequisite implementation, not a duplicate binding in the derived implementation.
Compare projections after the uniquely established substitutions; a normalization cycle is not
an invitation to guess a type.

For a public owner, the root publishes the conformance and its associated bindings. Its ordinary
reciprocally seen implementation fragment repeats the same owner/interface application and
associated bindings when it supplies named mappings. That fragment cannot add another conformance
or change a binding. If no named mappings are needed, do not author a redundant completion;
the root fact resolves directly to the unique applicable member/default. A private owner may
declare its whole conformance in its ordinary behavior source.

For required named operations, select the unique same-name owner member unless a necessary
explicit mapping selects another member. A mapping is necessary to bind a different helper name,
resolve an implementation collision, or explicitly replace a default. A redundant same-name
mapping for an already unique required member is invalid. Helpers may remain private; the
conformance grants the interface operation, not a public alias for the helper.

Selection uses the member identity first, then validates compatibility. Do not search by signature
for a different helper after failure. Compare callable kind, receiver/input operations, generic
requirements, data/result constructors, packs, origins and edited-state contracts after interface
substitution. Parameter spellings may differ: translate dependency clauses through their resolved
input positions. This semantic compatibility differs from the canonical-source repetition
required between one public callable contract and its own body. No coercion, hidden clone or
stronger implementation precondition repairs an incompatible requirement. An implementation
may promise a narrower result/post-state dependency bound only when it satisfies every behavior
permitted by the interface; clients continue using the interface's advertised bound.

The frozen relation also covers supported required operators. They match by fixed operation
family/shape and specialized signature, not a made-up helper name. The named `member = helper`
mapping does not rename operators or introduce arbitrary operator adapters/defaults. Missing
operator evidence is a conformance error. A type's members can satisfy several compatible
requirements, but doing so never merges their owning interface identities.

## Defaults and Prerequisites

Absent an explicit override mapping, a default operation selects its declared default body.
Adding a same-name inherent member cannot replace it implicitly. Replacing it requires a mapping,
including `name = name` when that same-name member is the intended override. This is a necessary
selection, unlike the redundant mapping forbidden for a required operation.

A default body is checked under only its interface requirements, normalized prerequisites and
ordinary accessible APIs. It cannot inspect a concrete representation or acquire stronger
access merely because all currently known implementing types permit it. The default's complete
implementation header must agree with its root contract. Overrides preserve the same operation
identity and are checked by the same compatibility rule as required operations.

An opaque default result additionally uses the
[selected-witness rule](opaque-results.md#default-overrides): the explicit mapping establishes
one witness under that operation's instantiated opaque identity. It does not require the helper
to spell or acquire equality with a private opaque type it cannot name.

A prerequisite is required capability evidence, not stored inheritance or an implementation
generator. Each concrete type needs a unique explicit prerequisite conformance with consistent
associated bindings for every admitted specialization. Reaching the same owning declaration by
two prerequisite paths does not duplicate it; reaching different declarations with the same name
does not merge them. Generic applicability and prerequisite normalization must be closed before
accepting those forms, without runtime search or source-order ranking.

## Calls and Name Collisions

Unique lookup uses ordinary calls. Ambiguous interface-operation names require the existing
selection suffix; qualification is not an optional style when lookup is unique:

```text
func inspect<T>(value: T): void
where T impl Reader, T impl OtherReader {
    value.(Reader.read)()
    value.(OtherReader.read)()
}
```

A bounded generic receiver exposes only its declared capability set. Two distinct interface
operations named `read` remain ambiguous even if signatures or eventual concrete bodies match.
The selector chooses an operation identity, not a helper or stronger permission. Apply the same
selection rule to construction functions on a type owner, such as `T.(Parser.create)()` when a
genuine construction-name collision requires it. Calling a method through `T` is still invalid.

Concrete lookup exposes accessible inherent members and contracted interface operations. Do not
count an inherent member twice solely because a same-name automatic requirement binding points
back to that member. An explicitly mapped interface surface with a different member, or a default
with its own body, remains a distinct candidate. A collision is resolved by the selector, not
by argument types, result expectations, access failure, declaration order or import order.
An inaccessible selected helper is usable through its valid public interface binding; it does
not become a public inherent candidate.

The selector names interface operations, not inherent methods. If an inherent member collides
with a different default/mapped operation, rename the inherent member or deliberately bind the
interface operation to it when that expresses the intended conformance. Do not add a nominal
owner selector, UFCS spelling or priority rule to keep both same-name meanings callable. An
ambiguity diagnostic must explain this distinction rather than suggest an unavailable selector.

A constructor interface mapping is selected statically at specialization just as a method mapping
is. Defaults and mappings remain independent of runtime capture/activation state. Later compiler
stages consume the frozen mapping without re-resolving names or matching signatures.

## Semantic Laws and Optimization

Document laws such as equality/hash consistency separately from compiler-checked data, access,
origin, effect and initialization facts. A declaration of conformance alone is not a proof of
purity, total ordering, safe bounds elimination, callback absence or destruction freedom. An
unproved law cannot authorize memory-unsafe code. A transformation using a law needs its owning
behavioral contract and verification argument; static dispatch alone promises no speedup.
