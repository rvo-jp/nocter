# Nocter Development Handoff

## Current Task

Continue v0.14.0 Phase 6 from the completed compiler-owned semantic presentation and source
documentation boundary. The next editor work is chosen from the remaining specified tooling
surface; it must reuse immutable analysis generations and semantic identities rather than add a
protocol-owned source interpretation.
Discovery now resolves the standard package, prelude, built-in attachment modules, and every
standard semantic role into one immutable toolchain profile. The complete authored standard
library parses, discovers, lowers, prepares, and passes body checking as one real source graph
rather than a hand-assembled fixture. Phase 5 closes process entry state and I/O through the
generic syscall boundary without operation-specific backend roles.
The previous compiler is preserved by commit `f6c08da3` and removed from the active working tree.
No previous source, test, binary behavior, or implementation document may be used as an
implementation input.

## Immediate Work

1. Begin immutable editor snapshots on the completed structural formatter boundary.
   `nocter-filesystem` now freezes canonical open-document paths, accepted versions, and bytes into
   one immutable read-only overlay. Package resolution retains that overlay in its exact graph and
   discovery uses the same value for manifests, module candidates, imports, and source loading.
   Overlay-aware resolution is separate from package-state requests, so fetch and lock mutation
   cannot consume editor bytes. `nocter-analysis` now binds that source view to a monotonic
   generation identity and exactly one discovery-failed, syntax-failed, compiler-failed, or
   target-validated state. Every state retains its reached sources, syntax, and diagnostics; only a
   successful current generation exposes the checked program and `SourceIndex`, so stale semantic
   fallback is impossible. `WorkspaceDocuments` now owns the mutable acceptance gate: changes must
   increase the document version, stale changes do not advance generation, included save text is
   frozen before analysis, and close emits a new disk-fallback overlay. The next boundary is the
   JSON-RPC/LSP lifecycle above this state, followed by compiler-owned semantic presentation
   queries. `nocter-source` now owns exact normalized-byte/UTF-16 position and range conversion;
   non-ASCII scalar differences are indexed once, and invalid bytes, surrogate interiors,
   out-of-line positions, and reversed ranges cannot reach an analysis query. The bounded JSON
   parser and exact string renderer have moved from installation/diagnostic ownership into neutral
   `nocter-json`; the LSP protocol layer can now decode JSON-RPC without duplicating parsing or
   reversing a dependency on installation policy. `nocter-lsp` now owns bounded CRLF framing,
   exact request/notification envelopes, 32-bit/string request identities, and the explicit
   uninitialized/awaiting-initialized/running/shutdown/exited lifecycle. Its single response
   renderer owns success identity preservation and the standard JSON-RPC error vocabulary. The next
   `ProtocolSession` composition now yields exactly one immediate protocol error or one typed
   lifecycle event for each body. Open/change/save/close parameters now have exact typed decoders;
   the full-sync change boundary rejects incremental ranges and multiple replacements rather than
   guessing their meaning. Local absolute `file:` URI decoding is now a pure protocol operation;
   remote authorities, queries/fragments, invalid escapes, non-UTF-8 paths, and NUL never reach the
   filesystem. `nocter-language-server` now resolves existing files or virtual files under one
   existing canonical parent and freezes that URI-to-path identity across open/change/save/close.
   Each successful transition emits the existing immutable analysis generation, while stale changes
   remain generation-neutral. Initialize decoding now retains fallback/workspace roots and dynamic
   watcher support while accepting unknown future capabilities. Its response advertises only the
   implemented UTF-16 and full-document synchronization surface. Initialize is now a two-phase
   lifecycle transition: invalid params atomically restore the uninitialized state instead of
   leaving the session unable to retry. The executable sequential service and framed loop now
   handle initialize, initialized, full document synchronization, unknown requests, shutdown, and
   clean/premature exit without writing non-protocol stdout. Accepted document generations remain
   explicit outputs. The public `nocter lsp` entry now belongs to the shared command schema, accepts
   no positional or operational options, validates the installation once, and enters its stdio loop
   before ordinary CLI rendering. Its validated installation and captured cwd now form one immutable
   language-server environment. Initialize resolves workspace folders, the fallback root URI, or cwd
   in that order, canonicalizes directories once, collapses physical aliases, and assigns nested
   documents to the deepest root. Root validation participates in the transactional initialize
   commit. Accepted transitions now retain their triggering canonical document. The workspace
   selects its nearest bounded package declaration or exact single-file scope, resolves package
   graphs as locked/offline/read-only, discovers the root and every declared target, and runs the
   same overlay through `AnalysisSnapshot`. Latest results are immutable and scope-owned; topology
   changes remove the previous scope before exposing a new result, preventing stale semantic
   fallback. Package-preparation failures retain their accepted overlay separately from reached
   compiler snapshots. Source diagnostics now publish from their exact snapshot-owned primary and
   related spans after source-owned UTF-16 conversion. Canonical paths use one protocol-owned
   percent-encoding policy, open documents carry the accepted version, and scope-owned publication
   history clears diagnostics absent from the next complete result. Spanless package-preparation
   failures use `window/showMessage` rather than an invented zero-width diagnostic. JSON-RPC now
   distinguishes and validates client responses. Monotonic server request identities retain their
   pending methods and complete once. A capable client receives the correlated `**/*.nct` dynamic
   registration after `initialized`; registration success enables ordered create/change/delete
   batches. Each distinct URI advances an overlay-preserving external-change generation and enters
   the same analysis/publication path, while path-local failures do not suppress later changes. The
   Package resolution failures now retain their exact overlay, reached manifest `SourceMap`, and
   every reached syntax tree. Manifest trees enter the snapshot before declaration decoding, so a
   semantic declaration failure retains the node that owns its subject and publishes source-backed
   `E0800`; inherently spanless lock/store policy failures remain process messages. Compiler-owned
   hover now resolves exact interactive `SourceBinding` ranges only in the current successful
   snapshot and renders normalized declaration, type, owner, requirement, and explicit provenance
   facts from checked semantics. It does not slice source text, expose synthetic package/target or
   whole-file module projections, or surface compiler-inferred `from` clauses. `from static` and
   authored-versus-elided provenance survive declaration lowering as presentation-only facts that
   do not affect semantic provenance. Compiler-resolved full-document semantic tokens now use the
   same source-selection boundary and exact `SourceIndex` bindings. Protocol-independent compiler
   categories distinguish types, members, callables, binders, and readonly receiver parameters;
   checker-owned occurrence access distinguishes readonly and writable field paths. Declaration
   lowering now gives unnamed literals, coercions, operators, and opaque results exact semantic
   anchors, eliminating the former whole-declaration operator projection. The LSP layer owns only
   legend mapping, UTF-16 projection, and delta encoding. Syntax-sized visibility, brace, and
   whitespace ranges are never substituted for semantic ranges.
   Compiler-owned definition and references now share a presentation-independent
   `SemanticSelection`. Definition chooses declaration identity before an implementation-only
   fallback, module paths navigate as one namespace to their root source, and references enumerate
   only exact identity bindings in the reached immutable graph. Neither request performs textual
   search or ambient source discovery. Rename now plans every edit from that same identity, rejects
   dependency and standard-library occurrences through discovery-owned package ownership, and
   recompiles a frozen speculative overlay before returning one atomic workspace edit. Candidate
   bindings must preserve the original identity, so collisions, shadowing, and capture changes are
   rejected. Open sources carry accepted versions and closed sources remain unversioned.
   Compiler-owned binding families also carry local and parameter renames through explicit closure
   captures. Signature help now selects checked call nodes and dispatch, retains selected generic
   arguments, renders static, structural-callable, and closure signatures through the canonical
   presentation authority, and projects renderer-owned parameter ranges to UTF-16 only in the
   protocol adapter. Checked lexical scopes now retain their resolved bindings and exact block
   projections. Name completion follows those scope parents, respects declaration order and
   explicit closure capture boundaries, overlays the canonical module namespace, and reuses
   compiler-rendered details. A typed-body failure now retains the current generation's completed
   pre-body semantic program, so name completion survives an unknown member or another body rule
   without falling back to stale state. The retained value explicitly has no checked nodes, local
   types, dispatch, ownership, or provenance; ordinary command compilation uses a non-retaining
   path. Receiver-member completion now has a compiler-owned candidate-name index and applies the
   ordinary instance, conformance, requirement, visibility, capability, and one-step coercion
   selector to every result. Successful calls derive receiver facts from `CheckedReceiver`; an
   invalid member call retains an explicit typed interruption containing its exact body, source
   origin, receiver type, borrow capability, and consumability together with the monotonic type
   state reached by that failed generation. It is not a partial checked body and cannot publish
   dispatch for invalid source. The candidate authority also enumerates exact visible fields
   through the ordinary field selector. The LSP adapter only maps compiler field/method results and
   advertises `.` as a trigger. Incomplete member syntax now uses a separate editor-only lowering
   entry: it accepts no lexer errors and requires every parser diagnostic to be contained by an
   executable block. Declaration/header syntax therefore cannot cross it. Syntax diagnostics stay
   authoritative while missing/error body nodes stop normal checking and expose only the exact
   typed interruption reached before them. Production compile input still rejects every syntax
   error. A name rule now retains a distinct lexical recovery snapshot containing only declaration
   state plus scopes, bindings, and source projections completed before the failure. It invents no
   target for the failing spelling and cannot be consumed as body-checking input. Name completion
   uses that exact failed generation, so neither later bindings nor stale successful state can leak
   into its candidates. The editor completion recovery boundary is closed.
   Syntax documentation is also closed as a compiler-owned path. The syntax tree groups and
   normalizes line and block documentation once, and `ast` exposes those exact attachments.
   Declaration lowering projects public contract documentation to semantic identities and joined
   implementation documentation to exact identity-and-origin pairs in `SourceIndex`. Package docs
   come from `nocter.nct`; module docs come only from root or single-file sources. Hover appends the
   normalized Markdown selected from that index after canonical semantic code, so the LSP server
   does not rescan comments, copy authored declarations, or confuse co-located identities.
   Type hover now consumes the public-presentation view of one module-relative
   `ConstructionSurfaceTable`. The same table derives a distinct use-site view that retains private
   access inside the defining module; both preserve structural, variant/member, source-order, and
   default identities. One validated `SourceContext` owns physical-source-to-module selection for
   hover, completion, and signature help. Namespace traversal supplies the shortest visible
   semantic spelling, including import aliases, without source slicing. Presentation emits only
   valid nominal and bodyless `construct` declarations, restores semantic target results to `Self`,
   and reports table/context inconsistencies as internal query errors instead of empty hover.
   Type-owned construction completion now consumes the use-site view of that table. Complete
   variant/function references, invalid member selections, bare `Type.`, built-in owners, and
   `Type<Args>.` all converge on one semantic owner-family query. Only dot-expressible variants and
   construction functions are candidates; structural construction and literals retain their own
   syntax. Visibility and declaration order remain table-owned, while an owner with no named surface
   returns an ordinary empty result. Selection inconsistencies are typed internal completion errors
   rather than a fallback to unrelated lexical names.
   The public source-tooling boundary is active: `fmt`, `tokens`, and `ast` share one standalone
   source loader and one filesystem-independent `nocter-source-tooling` snapshot containing the
   normalized source, lexer output, and concrete syntax tree. All three commands bypass
   installation, package discovery, and target selection. `fmt` rejects syntax errors and comments,
   reparses its candidate output to prove concrete-syntax preservation, and publishes through a
   same-directory synchronized temporary followed by rename; `--check` never writes. Its emitter
   consumes parser-owned syntax tokens rather than lexer tokens, so subdivisions such as nested
   generic `>>` use the same layout model as ordinary delimiters. Single-line and multi-line comma
   lists, trailing commas, requirements, visibility/module paths, spacing, indentation, top-level
   separation, and the specified redundant expression/type grouping rewrites are structurally
   normalized and idempotent. Closure capture and parameter segments use the same CST layout plan:
   `;` replaces a capture trailing comma, while a multi-line parameter segment retains its trailing
   comma before `)`. The formatter portion of Phase 6 is closed.
   `tokens` and `ast` emit their specified flat version-1 JSON envelopes even when their inspected
   stage diagnoses source.
   The human-diagnostic command path is closed: `check` consumes the same package transaction,
   discovery, and target-validated session as build/run, then stops before executable
   specialization and code generation. Library-only packages and single files are valid. A named
   package check discovers only the selected executable module, rejects an unknown name instead of
   silently checking the root, and emits no artifact. `nocter-diagnostics` now owns one shared
   byte-coordinate projection, exact JSON escaping, and the `nocter.diagnostics` version-1
   renderer. Successful checks and source-diagnostic failures retain command/target/root/format
   presentation facts and produce exactly one JSON envelope without human text. Spanless argument,
   input-preparation, installation, package-state, and target-selection failures now use the same
   envelope. Pure argument parsing retains the first error while completing presentation selection,
   so option order cannot force an argv rescan. One progressive presentation snapshot preserves an
   authored root hint, canonical root identity, and selected target only when each becomes known.
   Source, user/environment, and internal failure classes select exit statuses independently from
   diagnostic codes; internal JSON failures use `E0900` and status 3.

   Discovery now returns `DiscoveryFailure`, which retains its immutable source snapshot and
   projects an unresolved authored module path to `E0263` at the exact `ModulePath` node. Command
   wrappers forward that snapshot like every later compiler phase. Graph/syntax inconsistencies
   remain spanless `E0900` failures instead of borrowing a user-facing import code.

   `--version` and `doctor` are now closed. Pure parsing gives them an exact argument surface.
   `nocter-installation` promotes a physically validated `NocterHome` to `CompilerInstallation`
   only when compiler host, manifest host, and the native-only default target agree. Both reports
   consume that profile, write successful text to stdout, retain typed status-2 failures, and never
   prepare user source or initialize package acquisition.

   Help is now closed without a handwritten second flag table. One immutable `CommandSchema` and
   `OptionSchema` vocabulary owns implemented command tokens, summaries, positional forms, option
   names/aliases, value requirements, applicability, and descriptions. Parsing and rendering both
   consume those identities. `--help`, `help`, `help <command>`, and `<command> --help` converge on
   one typed report before installation selection and source preparation.

   The public `test` command is closed through package-only planning, exact dependency state,
   test-target discovery, semantic target/case selection, MIR, machine lowering, ARM64, Mach-O, and
   independent child processes. Each case uses a unique private artifact and package-root working
   directory; launch, exit, captured streams, and cleanup become one ordered typed run result.
   Human and `nocter.tests` version-1 JSON reports consume that result, preserve raw output through
   UTF-8/base64 encoding, and return status 1 for test failures without converting them into
   orchestration errors. Package state resolves once and immutable manifest snapshots fork into
   target-local discovery/session inputs, so a source or semantic failure becomes that target's
   `compile_failed` run and later targets continue. The shared argument schema now owns `test`, `--test`, `--case`, and the
   specified `--target` surface for check/build/run/test.

   The public build/run/fetch and diagnostic boundaries are now closed.
   The new `nocter` binary reads argv, `NOCTER_HOME`, the real executable, and cwd once. It parses
   arguments before installation or source access, validates the exact installation, compares
   compiler and manifest host identities, derives `CommandToolchain` from the manifest default
   target and standard package, and delegates preparation/execution to `nocter-command`. Its native
   integration test builds a single file through a copied installed standard library. Spanless
   argument, filesystem, package-root, and home failures receive only their specification-owned
   codes. Compiler failures deliberately receive no invented CLI code.

   The public diagnostic boundary is now closed. The common origin retains either an exact semantic
   syntax subject or the normalized span owned by lexer/parser recovery. Discovery projects lexer
   and parser failures into the same `SourceDiagnostic` envelope in source order. A failed command
   compilation owns its phase-selected envelopes and immutable invocation `SourceMap`; wrapper
   errors only forward an existing semantic envelope, and the process adapter invokes the common
   renderer without semantic matching or filesystem access. The renderer validates
   source/range/UTF-8 identity and renders normalized paths, one-based character coordinates with
   four-column tab expansion, every intersected line, related notes, and help. `SourceIndex`
   remains an independent success-side semantic projection rather than a presentation lookup that
   could reconstruct a phase-selected span. Internal compiler failures remain visibly distinct and
   never receive a source rule code.

   The package-state transaction is now closed. Source-independent exact locks and staged-store
   overlays let the read-only resolver validate provisional state. `nocter-package-state` injects
   acquisition, validates the complete staged graph, publishes exact packages, reruns resolution
   through persistent stores, and commits a canonical root lock block only after comparing the
   retained source bytes. It rejects implicit lock generation below the selected root and cleans
   failed staging trees.

   Concrete acquisition and package-mode build/run/fetch routing are now closed. The process adapter
   selects `nocter-package-acquisition`; `nocter-command` passes only the abstract authority;
   `nocter-package-state` still owns workspaces, publication, graph revalidation, and source commit.
   Embedded public HTTPS/Git and bounded `.tar.gz` materialization invoke no helper executable and
   reject authentication, links, submodules, and Git LFS. The standalone `fetch` command accepts
   only package-root and resolution-policy options, then stops after the same graph-validated
   package transaction used by build/run; it cannot discover or compile source.

   The completed installation boundary remains:
   `nocter-installation` now selects exactly one canonical home from explicit process facts:
   nonempty configured `NOCTER_HOME` first, otherwise the real executable's parent. It validates
   contained physical `VERSION`, `MANIFEST.json`, `nocter`, `std/`, and `std/nocter.nct` entries,
   decodes `VERSION` once, and derives the release-owned standard-package identity. A bounded,
   duplicate-preserving JSON parser and exact `nocter.manifest` v1 decoder now reject unknown,
   missing, duplicate, mistyped, unsafe-path, version/archive, default-target, and required-file
   inconsistencies before producing the immutable installation profile. Host identity remains
   independent of compilation-target identity. The crate does not read process globals or search
   the working or user directory.

   The completed command connection remains: `nocter-package` owns the only structured
   interpretation of `nocter.nct`, including
   presentation metadata, disjoint git/archive/path dependency declarations, exact locks, and
   target name/kind/order/module facts with syntax origins. Discovery and declaration lowering no
   longer decode separate subsets of target directive text. `ResolvedPackageGraph` now loads every
   selected manifest once, owns its source/syntax/declaration snapshot, validates exact alias and
   package identities, requires remote locks, and proves path dependencies select the authored
   canonical root. Discovery consumes that graph without reopening package files. The exact
   resolver now derives the complete recursive graph from one root declaration, the package-local
   and Nocter-home exact stores, mutable path roots, and the toolchain-selected standard package.
   One shared graph builder ensures that resolution and graph closure load each manifest only once.
   Missing lock and fetch state crosses typed requirements; locked/offline policy forbids only the
   relevant mutation. Provisional exact locks and staged roots are immutable resolver inputs, and
   the separate package-state coordinator owns their graph-validated publication. The production
   command adapter now supplies an explicit target, Nocter home,
   and standard package, preserves the resolver-owned command-root identity, selects only the
   compile-root modules required by all/sole/named executable policy, and crosses the existing
   discovery, session, publication, and launch boundaries. Parsed `--locked` and `--offline`
   values reach exact resolution unchanged. Concrete transport remains selected only by the
   process adapter and outside the command and resolver authorities. Canonical
   `PackageId` construction is closed: Git and archive locks normalize to Windows-safe exact IDs,
   path packages hash their canonical absolute UTF-8 path, and one dependency-free SHA-256
   implementation is shared with Mach-O emission. Build/run
   arguments now parse from `OsString` without process or filesystem access, preserve exact path
   values, reject malformed option structure, retain `--locked`/`--offline` as package policy, and
   prepare only through the existing input and command plans. The public executable consumes this
   parser and adapter and defines no second flag table or compiler pipeline.
   `nocter-command` now compiles one exact executable only through `nocter-session`, commits
   persistent images failure-atomically, and stages/runs/removes temporary images while preserving
   child exit status. Discovery, compile input, and the immutable declaration graph now preserve
   exact command-root package identities separately from traversed dependencies, and one-target
   session selection consumes only those roots. The native session also compiles all root
   executables in declaration order from one shared `TargetProgram` and returns an all-or-error
   identity-bearing image set. Package build now freezes a collision-safe output plan before any
   filesystem mutation and publishes every planned entry through the common artifact protocol.
   Package/file input now resolves against an explicit invocation directory, rejects conflicting
   positional/`--file`/`--root` forms, canonicalizes one exact package root or `.nct` source, and
   never searches ancestors or guesses an entry. Build/run planning now closes all-target,
   sole/named selection, relative/implicit output, file-mode restrictions, and working-directory
   policy without inspecting declarations. Package graph resolution and CLI parsing must consume
   those plans; neither may scan semantic declarations or invoke compiler stages directly.

`ExecutableProgram` now freezes fully specialized declaration-order struct fields, enum payloads,
and opaque witnesses. `nocter-machine` owns the selected target facts and the complete recursive
stored-layout table for MIR-referenced scalar, pointer, view, error, nominal, array, closure,
outcome, and opaque types. Layout entries expose every downstream offset; later lowering must not
repeat ABI padding arithmetic. The responsibility map is recorded in
`development/docs/machine-program-design.md`.

`MachineAbiPlan` now freezes every direct function's zero/direct/indirect classification, argument
registers, non-reopening spill boundary, aligned stack slots, final call-boundary padding, result
transport, and dedicated literal-pack pointer lane. Call lowering must consume these exact plans;
it may not place direct items independently at each call site.

`MachineLinkageTable` now assigns code linkage from exact executable-item, process-target, and
test-declaration keys without source names. It retains test execution order separately. Static text
uses a content-sorted deduplicated `MachineDataTable`, so MIR traversal order cannot affect data
identity. Machine-operation lowering must reference these tables rather than allocate labels or
data on demand.

`MachineProgram` now translates each semantic linkage entry into a distinct dense machine function
and gives each body independent stack-object, drop-flag, address, SSA-value, operation,
literal-pack, and basic-block domains. Every MIR place now closes to layout-owned byte offsets,
dereference steps, and checked fixed/view indexes. Loads, address formation, stores, aggregate writes, stored-tag switches,
allocation contexts, scalar operations, block arguments, direct calls, returns, and root exits have
closed machine nodes. Stored, completion, and diverging SSA values are separate; user destruction,
region lifetime operations, and root error reporting consume machine identities. Standard
primitives retain closed roles and use the ordinary ABI planner through one common call-target
domain. Literal packs now consume exact value/address identities, and their residual destruction
uses only generated machine-function targets. Structural
comparison, checked index, and borrow weakening now consume exact scalar, tag, bound, stride, and
view-layout facts without retaining semantic dispatch operands. The independent `nocter-arm64`
crate now types physical register-31 roles and rejects truncating instruction encodings. ARM64
local labels bind exactly once and resolve after monotonic conditional-branch relaxation. ARM64
ABI register roles and deterministic fixed-frame placement are also closed. ARM64 selection and
spill-aware instruction materialization are the next closed boundaries. The deterministic
linear-scan allocator already assigns non-crossing ranges across `x11`-`x15` and `x19`-`x28`,
restricts call-crossing ranges to the callee-saved partition, and records dense spills plus the
exact preservation set. Frame prologue and
epilogue materialization handles both immediate and full-width scratch-register offsets. Dense
ARM64 function/data domains already validate every typed fixup, resolve
function branches after text layout, and defer page-relative function/data address pairs to one
checked relocation method after the writer supplies both section virtual addresses.
The independent Mach-O writer owns section/VM layout, native and dynamic-loader commands,
content-derived UUIDs, SHA-256 ad-hoc signing, and final bytes; its ARM64 macOS test executes the
generated image without external tools.

The selected ARM64 scalar execution slice is now closed. `Arm64SelectedFunction` owns
virtual/fixed register transfers, simple stack-address forms, extension-aware scalar loads,
stores, arithmetic, raw-value and readonly-borrow comparisons, direct scalar argument/result
transport, local CFG terminators, and direct function references; unsupported machine nodes are
typed selection failures. The spill-aware materializer consumes only the selected function,
`Arm64ValuePlan`, and `Arm64FunctionFrame`. Narrow signed loads have an explicit target instruction
and cannot silently become zero-extended values. `Arm64Program::lower_machine` then resolves dense
functions and static data through the existing program builder. The separate `nocter-conformance`
crate crosses source, checking, specialization, MIR, machine lowering, ARM64, Mach-O, and native
execution for deterministic constants, scalar calls/arithmetic, control, structural comparison,
and narrow signed values. CFG edges now own typed direct-lane parallel copies. The materializer
executes copies only on the selected edge, orders acyclic dependencies, breaks cycles through one
ABI-reserved temporary, and supports register/spill combinations without adding a hidden frame
protocol. Value-producing control flow therefore crosses native block parameters directly.

Static text constants now select the layout-owned pointer and length lanes, retain their dense
`MachineDataId` until whole-program materialization maps it to `Arm64DataId`, and use the existing
section-relative relocation authority. Direct values of one or two ABI words now load/store exact
lane widths and cross parameter registers, the non-reopening outgoing stack window, and result
registers. Native cases cover a returned `&str` and nine two-word view arguments, proving both
register and stack transport without a view-specific calling convention.

Aggregate selection now consumes the machine layout's byte-write recipe directly. It zeroes the
complete representation before applying tags and value writes, constructs memory-backed values in
their own frame object, and reuses one maximum-size staging object only while constructing direct
aggregates. Exact stack-copy instructions reject overlap and never widen past a representation
boundary. Native conformance covers 3-byte, two-lane, and 24-byte structs through construction,
local storage, field projection, and execution.

Address selection now normalizes every machine address into a checked stack object or a runtime
base-register calculation. Pointer, view, offset, dereference, and fixed/view index steps share one
materializer; place loads/stores/address formation and built-in index borrows no longer implement
parallel bounds or stride arithmetic. The selected memory schema accepts stack and register bases,
including exact non-overlapping copies between user-addressable storage and compiler-owned memory
values. Native conformance covers dynamic fixed-array load/store, borrowed fixed-array indexing,
and `&str` view indexing.

Indirect callable transport now uses the same immutable machine ABI plan as direct transport.
Callers pass large arguments by pointer and allocate large-result storage in their own frame;
callees copy each argument into callee-owned parameter storage and copy each return into the
caller-provided result object. A returning callee saves the dedicated result pointer before any
nested call. Register inputs are persisted before indirect copies begin, so late address
materialization cannot overwrite an unprocessed ABI lane. Native conformance covers nested large
results and the ninth large argument crossing the closed register window onto the outgoing stack.

Allocation-context transport now has one ARM64 selection authority. Program roots initialize a
two-word default header, context-consuming callees save incoming `x9` in their frame, and every
inherited call reloads or rematerializes the current pointer. Explicit `using` calls resolve their
checked machine address before ordinary argument staging and place that address in the same hidden
lane. The first primitive expansion reads allocator state/kind through its ordinary machine ABI;
it does not introduce a primitive call convention or search a standard-library name. Native
conformance crosses the root and two nested callable boundaries before reading both header words.

Pure pointer and view primitives now expand through that same ordinary ABI boundary. Identity
conversions preserve their existing lanes, raw view constructors preserve pointer/length pairs,
view observation selects the appropriate lane, and unchecked string subviews perform one explicit
pointer adjustment. Pointee size/alignment use the completed machine layout rather than target
arithmetic; primitive-only generic type arguments are included in the recursive layout closure.
The compiler-owned `bytes_from_str` contract now records its exact input origin. Native
conformance proves pointer identity, byte layout, string subviews, and slice/string observations.

Memory transfer primitives now reuse one machine-owned value classifier and the ordinary ABI
plan. Runtime-sized pointer copies use one zero-safe forward loop; byte stores and generic
`store/take<T>` compose dynamic base formation with existing exact lane loads/stores or indirect
memory copies. No primitive duplicates the target's direct/indirect size boundary. Native
conformance crosses string and pointer copies, indexed byte stores, a 24-byte indirect store, and
both direct and indirect generic takes. `drop_value_at_ptr<T>` now freezes its exact concrete
dependency before MIR, interns every nonempty machine plan by content, and becomes an ordinary
direct call to generated machine CFG. Empty plans remain an explicitly validated no-op. Recursive
struct, active enum/outcome, closure, opaque, and reverse fixed-array traversal never enters ARM64
as a plan or byte-operation special case.

Darwin system primitives now have one target-owned selection and materialization boundary. Generic
syscalls translate the ordinary Nocter argument registers to Darwin's number and argument lanes,
then normalize carry-based success or failure into the declared two-word result. Process exit
shares the root terminator's exact syscall emitter. Trap, unreachable, and allocation abort use
distinct compiler-owned break reasons and cannot fall through. Native conformance executes both
syscall result paths and primitive process exit; termination-only roles are materialized without a
runtime-library dependency.

Direct user destruction and conditional ownership flags now cross the ARM64 boundary. Machine
operations declare whether they may cross a callable boundary, so register allocation preserves
values across ordinary calls, user drop bodies, and future pack callbacks through one fact rather
than a `Call`-variant exception. Drop selection validates the frozen one-borrow/void ABI, forms the
checked place address in `x0`, inherits allocation context only when the target requires it, and
uses the ordinary direct call relocation. Every drop flag is initialized in the function entry and
read or written as one exact frame byte. Native conformance proves both an uninitialized skipped
cleanup and initialized reverse temporary destruction.

Every closed machine terminator now crosses the ARM64 boundary. Value switches compare one- or
two-word subjects without truncating their `u128` case domain; stored-tag switches load the exact
layout-owned tag byte through the common selected-address plan. Switch and conditional successors
share one edge object. Direct lanes use register/spill parallel copies, while memory-backed block
parameters use a separate cycle-safe parallel-copy scheduler and one maximum-size frame staging
object only when a cycle must be broken. Identity copies disappear, acyclic chains preserve source
bytes, and both selected and fallback 24-byte joins plus an optional-tag switch execute natively.

`MachineContextPlans` now computes separate allocation and process capability tables through one
least-fixed-point engine over ordinary calls, user drops, and literal-pack iterator or residual
destruction callbacks. Explicit `using` selections stop only allocation propagation; process state
remains ambient. ARM64 reserves `x9` for allocation and `x10` for process state. A root captures
`argc`, `argv`, and `envp` before another initializer can clobber the platform argument registers,
counts the environment once, and propagates the immutable context only through transitive
consumers. Indexed queries are bounds checked and return program-lifetime views. Native
conformance runs a generated image with a controlled argument and environment across a nested
ordinary call. Target selection consumes these tables directly and does not rescan the graph.

The Phase 4 responsibility map is recorded in
`development/docs/target-program-design.md`. A closed `CompilationTarget` is now explicit in
compile-unit input and retained by `DeclarationGraph` through `CheckedProgram`. One shared
target-selection inventory excludes inactive items before block-import validation, symbol-table
construction, and declaration reservation. Unknown target gate names project `E0233`; recognized
reserved names remain distinct from implemented target availability.
Discovery-selected package target directives now pair their exact syntax node with one resolved
module identity. Declaration lowering derives target kind, name, and order from that directive,
allocates canonical `PackageTargetId` values, and projects each identity to its exact name literal;
it never parses an authored module path.
`nocter-target-program` now owns implementation availability. Recognition-only
`CompilationTarget` can no longer grant backend capability. An immutable `ToolchainSnapshot`
selects one inseparable backend, ABI, executable-writer, standard-package, and complete primitive
registry; currently only `arm64-darwin` can produce one. The registry has 49 closed semantic roles,
requires a unique callable for every role, and validates exact standard-package authority, module,
name, visibility, generic and parameter shape, result, provenance contract, target gate, and
bodylessness. The target-specific `SyscallResult` representation is validated down to copy shape,
field order, field types, and visibility. Extra primitives are rejected. `TargetProgram::build`
consumes `CheckedProgram`, proves target and standard-package identity plus package-target
integrity, and is the first public selected-target success boundary. An integration fixture crosses
the complete parser-to-target-program pipeline and proves that even same-shaped primitive roles
cannot be swapped.
Single-file lowering now creates one ordinary semantic executable target from its discovery-owned
package mode, root module, and display name. Its `PackageTargetId` projects to the file root, so
file and package execution have no parallel entry algorithm. Executable selection uses only the
selected module's authored namespace and freezes the exact `main` callable, body, module, target,
and one of the six accepted process-result contracts. Prelude fallbacks, re-exports, imported
modules, non-functions, generic or parameterized entries, bodyless entries, and other result types
cannot become executable roots. Test selection freezes only direct `TestId` declarations in the
selected module and retains their canonical declaration order; it never scans imports or
dependencies.
Callable specialization now uses one canonical key containing callable identity plus the complete
owner-and-callable generic domain. Missing, extra, and symbolic arguments are rejected, and owner
target types are derived rather than duplicated as receiver state. A single checked-body traversal
enumerates every executable static selection, closure, explicit pattern drop, referenced type, and
cleanup type while excluding unreachable retained source. Every explicit pattern drop retains its
declaration plus canonical generic substitution, so generic drop bodies do not lose the concrete
subject type before executable specialization. `ConcreteDispatchResolver` forks the
checked type store and resolves direct, interface, and structural dispatch into invocation,
comparison-lane, or index-lane plans containing direct, primitive, or indirect-callable steps.
Composite plans never encode operand ownership through array position. MIR will not receive an
unresolved requirement or repeat conformance selection.
Closure types now pair their lexical `ClosureId` with the complete enclosing generic domain. A
generic closure is no longer misclassified as globally concrete; specialization substitutes those
arguments into one distinct environment type, and the shared copyability authority carries its
capture condition across that specialization.
Opaque callable results now select one reachable witness pattern during body checking. A single
table proves the advertised interface and associated bindings, and checked conversions retain the
hidden representation through outcome injection. Callers see only advertised methods through an
`OpaqueMethod` edge; concrete dispatch opens the witness after specializing the opaque type's own
generic argument vector. Exact compiler-selected interface operations use the same opaque evidence
path as ordinary named methods, so collection iteration does not need an opaque-specific fallback.
Executable dispatch retains the opaque and witness receiver representations plus their exact owned,
readonly, or readwrite capability. MIR constructs the opaque aggregate and opens that lane through
one typed witness projection; validation specializes the declaration's witness pattern and rejects
any aggregate or projection whose concrete representation differs.
Concrete destruction now uses that same specialization authority. Exact generic drop selections
precede recursive reverse-order struct fields and active enum payloads; arrays, outcomes, closure
environments, and opaque witnesses retain explicit representation plans. Closure environment
metadata stores the captured binding and stored type as one field, preventing a non-owning
readwrite capture from being treated as ownership of its referent. The deterministic executable
closure can therefore enqueue every reachable user drop body without re-running source type
matching.
`ExecutableProgram` now owns the deterministic reachable closure. Callable, closure, drop, and test
keys enter one key-ordered work set; dense `ExecutableItemId` values are assigned only after the
set closes. Each concrete body freezes direct item IDs, typed standard/structural primitives,
statically specialized callable-value invocations, nested closure and exact drop edges,
source-to-concrete type mappings, and representation-specific cleanup glue. Bodyless callables are
accepted only through the closed toolchain primitive registry. Executable process and test roots
remain compiler metadata, while test cases retain declaration order. Enum residual cleanup is not
collapsed to its nominal type: it excludes
the already-run owner drop and every transferred payload.
Every executable item also freezes its complete concrete runtime signature independently of body
use. Unused parameters therefore remain ABI inputs; receivers precede ordinary parameters, closure
bodies receive one capability-correct environment input before their declared parameters, drops
retain their exact readwrite receiver, and tests retain an empty input domain. MIR never applies a
generic substitution to recover a function signature.

`nocter-mir` now owns the canonical backend-independent representation. Function-local locals,
drop flags, places, SSA values, operations, and blocks use separate dense identity domains and can
be created only through a consuming builder. Block parameters carry typed merge values; exact
terminators own every successor and edge argument. Storage switches inspect enum, optional, and
fallible places without moving them, while conditional cleanup uses explicit drop-flag branches.
The validator checks concrete type references, specialized nominal member projections, aggregate
layouts, operation typing, block closure and reachability, edge arity and types, SSA dominance,
switch subject shape, direct semantic item references, and terminal result behavior. A narrow
`MirValidationEnvironment` supplies only immutable type, declaration, and executable-item
authority, leaving package and source setup outside MIR. `MirProgramBuilder` requires exactly one
function for every executable item plus the exact compiler-owned root set, and validates
direct-call and drop-body signatures across functions and roots. Functions and roots share one
`MirBody` CFG schema without a synthetic executable item. Process roots lower all six entry
contracts into root-only exit and allocation-free error-reporting operations; test targets retain
one isolated root per declaration-order case and preserve empty targets. The checked-body lowering
path now consumes frozen concrete item and
primitive signatures, materializes receiver borrow capability, performs selected receiver and
operand coercions, and lowers primitive or selected comparisons without reopening selection.
Selected and coerced index projections now lower by borrowing the current place prefix, executing
their frozen receiver lane, and continuing from the returned borrow as a new MIR place root.
Outcome injection, absence, failure, propagation, force, and recovery share one typed temporary,
discriminant-switch, and payload-projection path. Propagation preserves every outer outcome layer,
catch bindings receive their failure payload before the fallback block, and the propagation edge
runs its exact checked cleanup schedule. Unconditional cleanup now lowers owned paths and values,
assignment replacement, user drop calls, reverse structural destruction, active outcome/enum
payload switches, opaque witnesses, and lexical region release from frozen executable plans.
Borrowed receiver roots remain initialized for flow checking but are excluded from callee-owned
destruction. One canonical value-storage authority now prevents borrow preparation, outcome
inspection, and cleanup from duplicating ownership. Conditional path and value cleanup reserves
entry-visible drop flags, updates them on initialization, move, replacement, and destruction, and
branches without reconstructing source control history. MIR places are interned by exact typed
shape, so flags and ordinary operations share storage identity. Concrete closure construction and
invocation now consume one executable-owned layout containing the specialized environment type,
invocation capability, and binding-preserving stored capture types. MIR aggregate validation
checks the exact closure body, capture order, binding identities, and concrete value types. Closure
body places reify the hidden environment borrow, capture field, and stored capture borrow as typed
projections; owned capture moves and recursive destruction use those same projections and cleanup
flags. Each executable body also freezes its deterministic reachable node domain, preventing an
outer function from preparing cleanup state for nested closure nodes that share its `CheckedBody`.
Callable bounds remain compile-time structural evidence rather than an erased runtime ABI.
Executable construction resolves each concrete bound subject to its exact closure item and freezes
the contract plus any caller-owned post-call destruction. MIR evaluates and prepares the callable
place first. An owned environment enters canonical temporary storage until every later argument
has succeeded, so propagation cleans it before transfer. MIR then calls the generated body directly
and performs explicit environment destruction when an owned contract invokes a readonly or
readwrite body. No indirect callable object reaches MIR.
Static string constants now retain their actual readonly `&str` type in MIR. Typed string literals
invoke the target-frozen literal body directly. An explicit `using` place is carried as a validated
call-scoped allocation override, accepted only for a literal executable item and only when its
concrete place type is the compiler-selected aborting allocator or allocation-context nominal.
Ordinary calls inherit the lexical context, and no allocator value is copied or moved to encode the
override.
Block fallthrough now consumes the same checked
`BeforeTransfer` event as explicit return and loop transfer. Explicit `drop`, compound integer
assignment, `break`, `continue`, while loops, breakable/nonbreaking infinite loops, and integer
ranges lower to closed CFG directly. A checked `never` loop has no invented exit block, and range
continuation uses a dedicated increment latch. Collection iteration now consumes its frozen source
expansion and `next` dispatch, retains the iterator in canonical value storage, borrows it at each
header, switches on the returned optional place, moves only the present payload into the loop
binding, and shares one iterator drop flag across exhaustion, break, and return cleanup. The common
compiler fixture can opt into exact iterator semantic roles without changing unrelated fixtures.
Enum pattern lowering now consumes checked binding modes and owned-remainder plans rather than
repeating copyability or cleanup selection. It switches on canonical subject storage, projects
specialized payload places, invokes a frozen type-owned drop before the first move, and joins any
number of value-producing arms through typed block parameters. Complete and variant-residual
cleanup obligations have distinct flags even when they share one subject slot, so explicit arms,
fallbacks, and implicit `if is` nonmatches cannot destroy one another's storage.
Lexical regions now lower through paired `CreateRegion` and `ReleaseRegion` operations. Creation
consumes the already checked parent borrow and initializes the compiler-owned context local;
release remains an ordinary ordered cleanup event, so nested fallthrough and early transfer drop
body-owned values before releasing each child from inner to outer. MIR validation checks the exact
compiler-selected allocator/context nominal identities instead of trusting a generic region local.

Phase 2 is complete. `lower_compile_unit_declarations` is the sole production declaration facade
and returns one immutable `DeclarationProgram` plus an independent `SourceIndex`. Every facade
failure is exhaustively classified as an authored rule or an internal compiler/discovery integrity
error. Declaration-owned G006-G010, G012-G013, and G015-G018 fixtures compare complete projected
diagnostics under reversed package and module input order. Type equalities are validated after
alias expansion, and projection-free general equalities project `E0320` without retaining syntax
inside canonical requirement identity.
The Phase 3 responsibility map is recorded in `development/docs/checked-program-design.md`.
`DeclarationProgram` now retains authored and prelude-fallback module namespace layers as the sole
body-lookup authority. `nocter-checking` catalogs every `BodyId` from exact source projection and
validates its physical source against the semantic owner module. Missing or inconsistent
projections remain internal boundary errors.
Body-owned resolution now creates dense scope, local, and explicit-capture identities for every
lexical construct. It resolves value uses to parameter, local, capture, exported, or built-in
identity; rejects implicit captures; selects block imports through exact discovery-to-module
projection; extends `SourceIndex`; and compares complete diagnostics under reversed input order.
The synthetic prelude is consistently a shadowable fallback rather than an authored collision
layer.
The program-wide `ConformanceTable` now owns refinement normalization, overlap unification, exact
required/default method selection, signature substitution, conditional requirements, associated
bindings, and associated interface/callable bound proof. Generic matching and bound proof query
that table; they do not reconstruct declaration patterns or rank a more-specific conformance. A
parallel `InstanceOperationTable` is the sole normalized index for instance-owned operations. It
consumes binder refinements and retained predicates once, rejects overlapping instance target
patterns as `E0355`, and supplies identity-keyed generic substitutions to body selection.
One iterative normalized-type validator now covers every declaration-owned data position,
callable result, non-value type operand, borrow/raw-pointer pointee, generic argument, structural
callable, and outcome layer. It is source-independent so concrete substitution can invoke the same
rules before specialization enters checked bodies or later representations.
`PreparedChecking` now owns the single graph/type/conformance/construction-surface/name input after
program-wide rules,
while `CheckedProgram` and `CheckedBody` define the syntax-independent output schema. Places and
static dispatch retain exact decisions, and generic arguments are identity-keyed and canonical.
`check_prepared_program` now consumes the preparation state and produces a closed `CheckedProgram`
for the current vertical body slice: scalar literals, inferred and annotated locals,
parameter/local/named-field places, readonly borrows, binding/discard, return/body-result checking, recursive outcome
injection and elimination, `catch`/`otherwise` recovery, ordinary conditionals,
while/infinite/integer-range loops, calls and receiver methods, named construction functions,
named-field struct/enum construction, fixed arrays, and enum pattern control. Every typed node
receives an exact `BodyNodeId` source projection, and no partial program escapes an unsupported construct or
failed rule. `CopyabilityTable` collects normalized `copy`
proof identities once, memoizes structural outcome/array/borrow/enum and substituted `copy struct`
facts by canonical `TypeId`, closes over the final type store, and remains owned by
`CheckedProgram`. Ordinary structs, unconstrained generics, readwrite borrows, and callable
contracts are never guessed copyable. Copy-struct families retain `Always`, generic `Requires`, or
`Impossible` conditions; an unconditionally move-only field now projects `E0366` at its declaration
instead of creating a never-copy family. `ConstructionSurfaceTable` is the sole target-family
index for `construct` declarations and remains in the final checked program for body and editor
queries. Construction calls resolve unqualified or qualified semantic owners, enforce member
visibility, project the exact member identity, infer omitted owner arguments, accept only complete
explicit owner arguments, combine owner and callable generics by identity, and validate both the
callable and specialized nominal requirements through the common proof authority. One enum-only
pattern plan serves both `if is` and `match`. It freezes the target's retained-place,
consumed-place, owned-temporary, or borrowed preparation; exact nominal and variant identity;
positional parameter-to-local binding map; fallback reachability; and unmatched `if is` path.
Coverage rejects duplicate variants, missing variants, and non-final fallbacks. Payload binding
types are specialized from the subject's nominal arguments. Retained places may name only copyable
payloads, while borrowed subjects bind every named payload with the subject borrow capability.
When a type-owned drop body must run before a move-only payload leaves, the pattern freezes its
exact `DropId` and canonical declaration-generic substitution; copy-only bindings retain the
complete enum for ordinary value cleanup instead.
Whole-binding state now tracks parameter and local move
paths, emits exact `Move` nodes, rejects moves of copy values and borrow bindings, and reports
later uses through `E0376`-`E0378`. Statically named fields now resolve through one visibility-aware
selector that substitutes the nominal owner's generic arguments and projects the exact field
identity back to source. Move paths retain field identity, preserve disjoint siblings, invalidate
their parent, and join inherited field state without enumerating a struct eagerly. `DropTable` is
the sole nominal-family-to-drop authority; partial moves inspect nearest enclosing families and
project `E0381` with the owning drop declaration. The entry-relative branch join cannot leak
branch-local paths.

Typed HIR construction is now independent of flow-dependent ownership. It freezes each body and
its stable node/place/loop identities exactly once; a repeatable ownership analysis then evaluates
that immutable graph. Ordinary `if`, `if is`, `match`, and `else if` join only reachable branch
exits. While, infinite, and integer-range loops use exact `LoopId` targets and a conservative
header fixed point;
zero-iteration exits, `break`, `continue`, and body backedges cannot leak loop-local paths. Range
endpoints are evaluated once before iteration and the typed loop binding is initialized per
iteration. A repeated move is therefore rejected without rebuilding HIR or allocating different
semantic identities on an analysis pass. Unreachable source after a terminal remains under an
explicit `Unreachable` edge. It is still name-, type-, visibility-, requirement-, and structurally
checked but creates no flow-dependent initialization continuation. A fallback after exhaustive
explicit pattern arms is still ownership-checked but cannot create a runtime continuation or loop
edge. Collection iteration now shares the exact iterator-acquisition authority used by sequence
spread without requiring exact-size evidence. Explicit readonly and readwrite modes select their
matching expansion; moved sources prioritize direct Iterator evidence over owned expansion; bare
sources admit direct Iterator evidence only. The checked loop owns one retained iterator temporary,
initializes the Item binding per iteration, preserves the iterator across `continue`, and cleans
the current item before the iterator on exhaustion or outward transfer. Provenance and loans map
the binding through the selected `next` contract, while liveness keeps borrowed sources active
through the body. Authored acquisition and Iterator failures project `E0404`-`E0405`. Authored
local and closure annotations now resolve through one body
type-use authority, validate normalized data or callable-result position, and pass their resolved
type into the ordinary expected-type conversion boundary. The checked local therefore retains the
declared destination type rather than an initializer-side approximation. Invalid body type uses
and invalid discard forms project `E0406`-`E0407`; normalized shape violations continue to use
`E0360`-`E0365`.

The construction surface now indexes named functions and both literal shapes once. Literal
selection uses exact construction and callable identities, and a checked literal retains one
`StaticSelection` with every construction-binder argument rather than losing generic
specialization behind a bare callable ID. Fixed sequence elements, empty contextual sequences,
decoded typed strings, and ordinary static `&str` expressions pass expected-type inference,
ownership, provenance, and loan analysis. The sequence delimiter or string opener is the exact
callable source projection. Declaration validation rejects a string literal parameter other than
readonly `&str` and rejects outcome-wrapped literal results before body checking.

Exact-size typed-sequence spread is now closed over the same semantic authorities. Standard
`Iterator`, `Iterator.Item`, `Iterator.next`, `ExactSizeIterator`, and its `remaining_len` method are
exact validated roles rather than source spellings. Readonly and owned expansion use
`InstanceOperationSelector`; consuming direct iterators have fixed priority and cannot fall back
when exact-size evidence is absent. One `IteratorAcquisition` node gives iterator storage an
identity distinct from its source, while `TypedIteration` freezes `next`, `Item`, and exact-size
dispatch. Fixed and spread elements share one source-order construction inference session.
Ownership transfers acquired iterators into the element pack and cleans partial acquisition on
propagation. Provenance and loans map yielded values through `next` and the shared spread
contribution-type projection, preserving retained borrows without extending loans for copied
storage-independent values. Authored acquisition, iterator, and element failures project
`E0401`-`E0403`.

Compilation input can now attach compiler-owned standard semantic roles to exact declaration-name
tokens. One program-wide `StandardSemanticTable` resolves those tokens through `SourceIndex`,
rejects project-owned declarations and duplicate roles independently of input order, and validates
the non-generic allocator/context/String families plus the exact `Format.format_into` semantic
shape. Body checking never searches for a standard spelling or path. Typed literal `using` now
accepts only a place of an established aborting allocator or allocation-context family, records the
place as an explicit HIR operand, and projects `E0399` for an authored wrong type. Ownership,
provenance, loan, and closure-capability consumers all evaluate that operand before literal
elements; current-region literals retain the existing implicit selection.

Executable `region` statements now consume that same allocator-place authority and construct one
typed `AllocationContext` binding plus an explicit checked parent operand and body edge. Region
handles cannot enter ordinary copy, move, owned-receiver, moved-capture, or explicit-drop paths.
Ownership treats a region as a lexical resource rather than ordinary storage: every reachable
fallthrough, `return`, `break`, `continue`, and postfix-propagation edge cleans body-owned values
before one explicit region-release action. Nested cleanup follows scope order, while a `never`
edge schedules no release. The parent allocator/context remains loan-live through the child body
and its loan ends at the release action, before any enclosing parent cleanup. Provenance uses the
same region binding and current-allocation identity to reject direct and indirect storage escape.

Ordinary interpolation now decodes text and expression parts once in source order, normalizes
multiline indentation across interpolation boundaries, and constructs the exact role-selected
owned `String`. Every non-diverging expression is a shared readonly operand plan and selects only
the exact role-selected `Format.format_into` method through a concrete conformance or lexical
generic requirement; a same-spelled project interface has no authority. The checked operation
retains the formatter dispatch, allocation selection, and partial-output type independently of its
possibly diverging result type. Ownership activates the partial `String` before operands, keeps
formatted temporaries alive through their call, and places partial-output destruction on a later
postfix-propagation edge. Provenance and loans consume the same source order without reconstructing
format lookup. Missing or ambiguous formatting evidence projects `E0400`.
The standard semantic table now separately validates and freezes the owned-String constructor and
readwrite text appender. Executable dependency closure resolves those callables together with each
formatter. MIR invokes all three as ordinary selected calls, retains the partial output in the
interpolation node's canonical value-storage slot, moves it once on success, and lets propagation
or explicit return destroy that same slot through the checked cleanup schedule. A forced-outcome
trap retains the specified no-cleanup behavior. No MIR operation or backend rule knows the
`String` layout or recovers an operation from a name.

Every checked block now retains its exact `BodyScopeId`; name resolution passes that identity
directly into HIR instead of requiring a later syntax or source-index reverse lookup. Ownership
analysis materializes one dense `CleanupTable` keyed by the checked node that owns each scheduled
event. A node may own independent pre-store, statement-end, control-header, propagation, and
control-transfer events; no node kind is asked to imply timing. Pattern residual storage has an
identity distinct from its subject value and from every other arm. Named owned payloads transfer
their obligations to branch locals; only unnamed move-only payload fields remain in the residual
action. A fallback retains the complete active enum. Branch joins make mutually exclusive
residuals conditional, and normal statement, `return`, and postfix-propagation edges consume the
same temporary authority without double drop. Normal block exits, `return`,
`break`, and `continue` all derive cleanup from the same
field-sensitive initialization state. Actions preserve reverse declaration order, distinguish
unconditional from maybe-initialized destruction, omit moved roots and non-owning borrows, expand a
partially moved struct to only its remaining fields, and represent a discarded move-only result as
a value cleanup rather than an invented local. Loop-edge cleanup removes loop-local roots before
the fixed-point join. Simple assignment accepts whole mutable bindings, their statically named
fields, and fields reached through readwrite borrows. It checks the RHS before replacement, applies
the destination expected type, restores moved and maybe-initialized paths, rejects immutable or
unavailable-parent targets, and obtains old-value cleanup from the same partial-path planner used
by scope exit. Each cleanup schedule declares its exact event timing, so later MIR cannot infer
ordering from the node kind. Evaluated owned
temporaries use the same flow state as named paths: call/aggregate staging consumes them on
success, branch joins make one-sided creation conditional, and statement/control-header edges
destroy remaining values in reverse creation order. Postfix propagation owns a distinct failure
edge that destroys active temporaries before scope storage, while forced unwrap and a `never` call
retain the specified no-unwinding behavior. Checked integer
arithmetic selects `Add`, `Subtract`, `Multiply`, `Divide`, or `Remainder` once and evaluates
operands left-to-right. Compound assignment reuses that selection, retains one target and one RHS,
requires a definitely initialized numeric place, and never constructs a fictional binary
expression. Body errors retain their `BodyRule` identity separately from the projected diagnostic,
so the compound boundary can classify its required dedicated diagnostic without comparing rendered
codes. Built-in fixed-array, slice, and `str` indexing now uses the same checked-place constructor
as field reads and borrows. Every implicit borrow dereference is an explicit place projection, so
the owned initialization prefix and final storage authority remain distinct. Index expressions
occur once in projection order. Simple and compound indexed assignment visit the RHS first, then
those index nodes, and retain the evaluated place for pre-store cleanup. Source-defined readonly
and readwrite index operations and the permitted one-step receiver coercion now enter that same
place model. Selection prefers a unique direct operation over coercion paths, rejects equally
ranked paths as `E0388`, and carries one complete `StaticSelection` containing dispatch identity
and generic arguments. Lexical structural index requirements dispatch through their exact
`RequirementId`; concrete instance candidates must satisfy normalized declaration and callable
requirements, while unresolved generic receivers require lexical evidence. Executable MIR
lowering now has an end-to-end scalar/control/direct-call slice. Non-empty cleanup schedules and
the remaining checked operation families fail explicitly rather than being omitted.

Closed prefix, shift, logical, and comparison selection is complete. A directly negated
integer literal becomes one signed `i128`-domain constant, including each exact signed minimum;
runtime negation remains an explicit unary operation. Signed and unsigned right shift are distinct
checked operations. One comparison plan covers primitive, lexical structural, and source-defined
implementations. It freezes readonly place/temporary preparation, readwrite weakening, per-source
one-step coercions, static dispatch, source operands, and independent `reverse`/`negate` derivation
facts. Exact receiver declarations outrank coercion routes; ambiguity is `E0389`. Conditional
equality and ordering requirements recursively re-enter the same selector and fail closed on
cycles. `&&` and `||` remain control nodes whose ownership joins the RHS path with the bypass.

Direct module function and primitive calls are now checked from resolved callable identity. One
ranked `CallableInference` result supplies canonical generic arguments; normalized callable
requirements re-enter the shared instance-operation proof authority. Concrete parameters
contextualize literals before inference, `none` remains deferred until another constraint fixes its
payload, and the result context prefers complete identity before outcome injection. `CheckedCall`
retains exact static dispatch and source-order arguments. Ownership visits the callee value,
receiver, and arguments in language order, so explicit moves and use-after-move
share ordinary place state. The common expected-type boundary now owns exact compatibility,
recursive outcome injection, built-in readwrite-to-readonly weakening, and one-step source-defined
borrow coercion. It records the exact target, source preparation, and static selection in
`CheckedBorrowConversion` and never chains conversions. Readwrite place arguments remain place
drafts through generic inference so a reborrow cannot be misclassified as an implicit copy.
Generic parameter and result evidence admit the same built-in capability weakening. The operation
selector prefers minimum receiver authority, falls back to a readwrite receiver only when required,
and uses lexical coercion requirements in generic bodies. Duplicate coercion identities are
rejected by the program-wide table as `E0356` before body selection. Calls through generic values
now select one exact lexical callable requirement and retain its `RequirementId`, capability, and
callee place. Readonly and readwrite calls borrow the place without copying its environment;
readwrite calls require writable storage. Owned calls consume the callee before their arguments,
independent of closure copyability. Construction functions use that same planner after the
construction-surface table has selected one accessible semantic member. Omitted owner arguments
participate in inference; explicit owner arguments become fixed substitutions before callable
generic inference begins. Receiver methods now use one semantic selector over normalized instance
and conformance tables. Exact lookup combines inherent, concrete conformance/default, and lexical
generic-interface candidates without overload ranking. Interface `Self`, interface arguments,
associated types, instance arguments, and callable generics enter the shared declared-call planner
as one substitution. Only an empty exact set permits one receiver coercion; minimum-authority
coercion tiers, ambiguity, and direct-method priority match other instance operations.
`CheckedReceiver` freezes owned copy/move, place or temporary borrowing, existing-borrow
preservation/weakening, selected coercion dispatch, and post-coercion weakening. Concrete calls
freeze their implementation/default callable; generic calls retain the exact interface
requirement. A program-wide provenance fixed point now derives exact caller-visible origins and
compiler-owned current-allocation dependence after ownership has attached cleanup. It retains
field, enum-payload, outcome, and element projections independently, maps results through static
and structural calls, and records a dense node/body/callable authority in `CheckedProgram`.
Return validation rejects local, owned-parameter, temporary, region, unknown, and undeclared input
origins as `E0395`; conformance implementations are additionally bounded by the corresponding
interface method contract. A separate dense `LoanTable` derives source-level non-lexical
liveness over checked places and node temporaries. It retains explicit and implicit loan identity,
capability, canonical field-sensitive places, reborrow ancestry, and per-node live sets. Readonly
and exclusive conflicts, move/drop/assignment conflicts, dynamic-index conservatism, branch and
loop joins, receiver-derived results, lexical storage escape, temporary receiver escape, and
type-owned drop observation order project `E0396`-`E0398`. Closure expressions now have lexically
reserved `ClosureId` identities, concrete closure types, and one program-owned
signature/environment definition. Parameter and result inference may use a structural callable
contract without depending on source argument order. Unannotated results join tail values,
explicit returns, absence, failure propagation, and divergence at the closure boundary. Each
capture is an explicit initialized environment field whose stored type determines copyability;
reads, mutations, moves, and nested callable invocations independently determine invocation
capability. Ownership, provenance, liveness, and loans analyze every closure body as a separate
execution root while mapping parameter, capture-value, and environment-storage origins through
direct and generic calls.

`ConstructionSurfaceTable` now indexes the complete construction surface of every nominal family:
structural field identity and declaration order, enum variants by semantic name, and any authored
`construct` declaration. Structural visibility restrictions from explicit defaults and empty
construct declarations are answered there, while the shared field selector remains the sole
field-visibility authority. Named struct literals, payload and payloadless enum variants, and
fixed-array literals now produce closed aggregate operations. Struct fields and variant payloads
reuse the same source-order contextual-inference planner as callable arguments, so omitted owner
arguments, expected result evidence, deferred absence, explicit moves, and nominal requirements do
not form a parallel inference system. Aggregate ownership traverses retained children in source
order. Earlier initialized children become staged value temporaries until the aggregate commits;
a later propagating child cleans them on its failure edge and successful construction consumes
them into the aggregate.

Explicit `drop name` now constructs the same root `CheckedPlace` used by move analysis. Structural
checking rejects copy and borrow bindings as `E0383` even in unreachable source. Reachable drop
requires an exactly initialized path, emits one unconditional path cleanup on the drop node, and
then marks the binding uninitialized; later use and a second drop therefore use the ordinary
`E0378` state rule. Automatic scope cleanup sees the updated state and cannot destroy the binding
again. Explicit destruction and scheduled type-owned destruction enter the same loan analysis;
redundant initialized child move paths are normalized instead of turning whole values with a drop
body into fictional partial states.

## Guardrails

- Do not restore or inspect the archived compiler.
- Do not migrate archived tests or diagnostics.
- Do not run a released compiler to discover unspecified behavior.
- Do not treat the existing standard-library implementation as language semantics.
- Do not mark specification closure complete while an observable choice remains implicit.
- Do not let Phase 3 reparse declaration headers, infer syntax from resolved names, or place source
  ranges and rendered names in checked semantic identity.

## Verification

```sh
cargo fmt --manifest-path development/compiler/Cargo.toml --all --check
cargo clippy --manifest-path development/compiler/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path development/compiler/Cargo.toml --workspace
node docs/build-docs.js
git diff --check
```
