# Specification-First Compiler Architecture

This document defines only implementation boundaries that cannot decide public language behavior.
The [language specification](../../spec/README.md) is the sole semantic authority. If a public
choice is missing there, implementation stops until the specification is amended.

## Program Pipeline

```text
SourceProgram
  -> SyntaxProgram
  -> DeclarationProgram
  -> AcceptedDeclarationProgram
  -> CheckedProgram
  -> TargetProgram
  -> ExecutableProgram
  -> MirProgram
  -> MachineProgram
  -> Arm64Program
  -> MachOImage
```

Each arrow is a one-way lowering boundary. A later program cannot recover a decision by revisiting
an earlier representation.

`nocter-conformance` is not another pipeline stage. It is the only test crate allowed to depend on
the complete chain, and it verifies that adjacent crate contracts compose into deterministic native
behavior. Production crates keep their one-way dependency edges; cross-pipeline test setup does not
leak back into a semantic or target authority.

## Separate Source Projection

Source files, byte ranges, comments, and editor offsets live in a `SourceIndex` beside the semantic
pipeline. Semantic nodes may retain a source-origin ID for diagnostics, but source locations never
identify types, declarations, callables, generic parameters, bodies, or executable instances.

```text
URI + offset -> SourceIndex -> semantic ID -> presentation
semantic ID -> SourceIndex -> diagnostic range
```

No inverse source lookup participates in type equality, dispatch, ownership, monomorphization,
reachability, ABI selection, or code generation.

Semantic validators receive `DiagnosticOrigins`, not `SourceIndex`. This narrow capability can
project an already selected semantic identity to its authored declaration or implementation
origin, but exposes no binding iteration, offset lookup, visible names, or reverse semantic
selection. The checking pipeline may own and extend the complete `SourceIndex` beside its semantic
result; checking rules cannot read it as semantic evidence.

When a lowering boundary creates new semantic identities, it consumes the existing `SourceIndex`
into one projection builder, adds exact projections, and freezes both deterministic lookup orders
again. Duplicate bindings, duplicate visible-name sets, conflicting visible-name mappings, or
duplicate documentation become `SourceProjectionIssue` values owned by the finished index. A
builder never selects one conflicting entity by insertion or identity order. Projection issues
cannot enter a lowering or checking error and therefore cannot change whether the semantic program
exists. The query seal rejects an inconsistent projection before editor facts are exposed.
Canonical semantic programs never depend on this projection value.

Declaration lowering also records each source's effective visible spellings in `SourceIndex`.
This editor-only projection is emitted beside, not read back from, the `FrontendBindings` consumed
by semantic checking. Hover, completion, signature help, and inlay hints select names from that
projection, so a source-local alias or direct see can affect presentation without entering a
module export table or becoming semantic input to a later compiler stage.

Editor descriptions follow the separate
[semantic presentation boundary](semantic-presentation-design.md). A successful immutable analysis
selects an exact interactive `SourceBinding` and renders its semantic identity from the checked
declaration graph and type store. Protocol code performs URI and coordinate conversion only. It
does not slice declaration text or recreate type, owner, visibility, requirement, or provenance
rules.

`SourceProgram` owns normalized UTF-8, source identities, lexical tokens, byte spans, newline
tokens, and each token's joint-to-next fact. String starts, text segments, interpolation boundaries,
and string ends are ordinary lexical tokens in that same stream. `SyntaxProgram` consumes those
facts once; it never re-reads source bytes to distinguish indexing from typed literals or rescans
string contents to discover interpolation.

`SyntaxProgram` stores nodes and child elements in flat immutable arenas addressed by tree-local
`NodeId` values. Deep valid prefix syntax therefore has no recursive ownership chain to overflow
the process stack during traversal or destruction. A parser event stream builds the arenas once;
missing syntax and subdivided token views are ordinary child elements. Every syntax token retains
its lexical-token identity, and the syntax pieces for one subdivided token must exactly partition
that token's normalized range.

Left-associative expression nodes use forward-parent links in that event stream. The builder
resolves each link once while opening arena frames; the parser does not recursively reparent an
already built subtree or allocate wrapper nodes for precedence levels that have no authored
operator. Deep unary and binary expressions therefore share the same bounded-memory arena path as
deep type prefixes.

Bounded syntactic ambiguity is parsed transactionally. A successful branch keeps the events it
already produced; a failed branch restores its cursor, token subdivision, nesting, events, and
ordinary diagnostics. The parser never performs a successful lookahead and then parses the same
branch again. Safety-limit diagnostics survive rollback. This rule keeps nested type-argument
recognition linear while leaving an unmatched `<` available to the enclosing expression grammar.
Token discriminators commit assignment, closure, and construction-owner branches independently
from the validity of their interiors. Once committed, malformed interiors retain the selected node
identity and focused diagnostics instead of being reinterpreted as another expression family.

One line-sequence parser owns newline-separated source and member containers. Leaf declarations
never consume their enclosing separator. A missing separator recovers to that container's next
newline or closing delimiter instead of letting one member reinterpret the following member's
tokens. Comma-delimited and line-delimited declarations therefore cannot silently accept each
other's separators.

One continuation-newline component owns leading-token and incomplete-expression consumption.
Statement-level syntax accepts exactly one continuation newline and never crosses a blank line;
delimiter-owned expressions consume the newlines admitted by their delimiter. At the active header
delimiter depth, the parser's control-header mode reserves the first `{` for the control body. A
struct literal, closure, recovery clause, or nested control expression at that level must therefore
be grouped; the parser never speculates toward a later brace or consults name resolution. Every
block classifies its final expression as a body result by source position before semantic checking.
Error recovery may retain missing or unexpected syntax nodes, but cannot revise any of these
choices after resolution or typing.

## Semantic Identities

Distinct domains represent packages, modules, authored declaration sites, semantic type
definitions, callables, fields, variants, associated types, generic parameters, parameters,
bodies, closures, expressions, statements, types, substitutions, and monomorphized items.

`nocter-model` is the syntax-independent owner of these IDs and structural type keys. It has no
dependency on `nocter-source` or `nocter-syntax`. Identifier spellings live in one immutable symbol
table built by sorting and deduplicating the complete input set, so symbol IDs do not depend on
source discovery order. Symbol IDs support lookup and presentation metadata only; nominal,
associated, generic, callable, and opaque semantic IDs form type identity.

`nocter-declarations` owns immutable typed arenas addressed by those ID domains. A consumed builder
is the only mutation path. Exact module identity is a package identity paired with a normalized
directory path; `.` and `..` never enter that path. Authored visibility is resolved once to private,
an ancestor-module descendant boundary, the declaring package, or all packages. Later lookup never
reinterprets `pub(../)` from a physical source path. The crate depends only on `nocter-model`, so a
semantic consumer cannot acquire source or syntax access through the declaration graph.

Mutually referential headers use one closed two-pass construction protocol. The builder reserves
each identity in canonical order, resolves generic parameters, member identities, requirements,
and structural types against those reservations, then defines each slot exactly once. Freezing
fails if any reservation remains incomplete or if a reference, reciprocal owner edge, member
position, callable shape, receiver capability, provenance origin, import target, or visibility
boundary is inconsistent. The immutable program therefore never carries a partially patched
declaration graph.

Declaration result provenance names either the exact receiver or exact `ParameterId`. Structural
callable-type provenance remains a separate normalized set of ordinary parameter positions. This
prevents `from self` from being encoded as a forged explicit-parameter position while keeping
parameter spellings out of both identities.

`nocter-source-index` is a sibling projection, not a field that defines declaration identity. It
stores semantic-to-source bindings twice in immutable deterministic orders: once for lookup by
semantic entity and once for lookup by source coordinate. Each origin can select either a complete
syntax node or an exact syntax-token view, so a declaration name never needs the keyword,
visibility, whitespace, or body as its editor range. Contract declarations, separate
implementations, and references remain explicit roles. Structural `TypeId` values are not source
entities because one interned type can occur at many sites; source type uses attach to their owning
declaration or checked expression instead.
The projection also exposes one complete stream of every referenced semantic identity, including
documentation owners and editor-visible names. It does not validate those identities itself or
depend on declaration storage; the analysis query kernel seals that stream against the current
semantic evidence.

`nocter-filesystem` owns one immutable map from canonical absolute paths to accepted open-document
bytes and versions. Reads select that map before disk, while writes, fetches, lock generation, and
publication have no API in the crate. Package resolution retains the exact map in its resolved
graph, and discovery consumes and retains that same value while loading module sources. Package-
directive decoding and module analysis therefore cannot observe different content for one editor
generation.
Overlay-aware resolution is a separate read-only entry point; package-state transactions accept
only the disk-backed request type and cannot mistake editor bytes for persistent package source.

`nocter-analysis` is the protocol-independent owner of one editor generation. Its immutable
`AnalysisSnapshot` retains the accepted generation identity, source overlay, reached source and
syntax snapshots, complete retained diagnostics, one diagnostic status, and one independent
semantic evidence result. The status is discovery failure, syntax failure, compiler failure, or
target-validated success. Session exposes both a successful target and retained recovery through
one borrowed `SemanticEvidenceView`; analysis does not inspect session storage variants or rebuild
their common graph, type, ownership, and projection contract. Recovered name and body analysis classifies every declared body as
accepted or rejected with its source-backed reason; an internal inconsistency cannot manufacture
partial semantic evidence. Protocol-independent queries are the only authority allowed to join
that evidence to `SourceIndex` occurrences, and set-valued queries report whether their coverage is
complete. The kernel validates the complete source projection against its declaration and
body-local domains, source map, syntax trees, and source ownership once per immutable generation;
the private `query/` module tree owns every operation that consumes the sealed context. Neither the
`AnalysisSnapshot` boundary nor a protocol adapter can name `SemanticQueryContext`,
`CompleteSemanticQuery`, checking recovery, or `SourceIndex`; they receive only complete
protocol-independent query results. A dangling binding, documentation owner, visible name, source,
syntax origin, or module owner is an integrity failure rather than an ordinary empty result. Query
accelerators are lazy caches keyed by compiler-owned identities;
they never infer an evidence-domain size or become a second semantic authority. A target failure
therefore does not erase checked source semantics, while
retaining checked semantics does not turn that target failure into success. A
failed current generation never exposes an older successful semantic program. Discovery failures
retain their reached syntax trees as well as sources, so invalidation and syntax-aware recovery do
not require reopening files. The same crate's `WorkspaceDocuments` is the only mutable accepted-
document boundary. It requires strictly increasing change versions, ignores stale changes without
advancing generation, applies included save text before analysis, and freezes one
`WorkspaceSourceRevision` for every accepted open, change, save, close, or external change batch.
Each revision contains the complete overlay, exact open-document set, and complete change set;
workspace analysis never reconstructs those facts from earlier generations. Previously emitted
revisions never observe later document mutations.

`nocter-source` is the sole coordinate-conversion authority. Compiler phases retain normalized
UTF-8 byte offsets and never store editor positions. Each immutable `SourceFile` converts those
offsets and ranges to zero-based UTF-16 positions, and validates the reverse conversion before an
editor request reaches analysis. The line index records only non-ASCII scalar differences, keeping
ASCII conversion constant-time while rejecting UTF-8 interior offsets, UTF-16 surrogate interiors,
out-of-line positions, and reversed ranges. CRLF normalization changes byte offsets but not the
line-and-character positions observed by an editor.

`nocter-json` is the dependency-free JSON syntax boundary shared by installation metadata,
machine-readable compiler output, and editor protocols. Its bounded parser preserves object member
order and duplicates so each consuming schema can enforce its own exact-member policy; it does not
silently collapse malformed inputs. The same crate owns JSON string escaping. Installation and LSP
code therefore depend on a neutral format layer instead of depending on each other or maintaining
divergent parsers.

`nocter-lsp` owns protocol framing, JSON-RPC request identity and envelope validation, and the LSP
lifecycle state machine. It accepts only bounded `Content-Length` frames with CRLF headers and
UTF-8 bodies. Initialization, the `initialized` notification, shutdown, and exit form explicit
states; invalid requests are rejected before document state or compiler analysis can observe them.
The crate contains no filesystem or language-semantic policy, so protocol transport cannot become
a second compiler front end. One response renderer preserves integer or string request identity and
owns the standard JSON-RPC error vocabulary; feature handlers supply typed results rather than
assembling envelopes or error objects themselves. `ProtocolSession` composes envelope validation
and lifecycle validation into either one immediate protocol response or one typed event. Invalid
input can never be delivered beside an event, and notifications outside their legal lifecycle
produce no response. Document synchronization parameters are decoded into distinct open, change,
save, and close values before they reach workspace state. Full synchronization accepts exactly one
unranged content replacement; incremental ranges, duplicate recognized fields, non-integer
versions, empty URIs, and missing text are explicit parameter failures.
`DocumentUri` separately decodes only local absolute `file:` URIs. Scheme parsing, local-authority
validation, query/fragment rejection, percent decoding, UTF-8 validation, and NUL rejection happen
without filesystem access. The server layer receives a platform path but remains solely responsible
for resolving that path against disk or a virtual open-document parent.
The inverse conversion accepts only absolute UTF-8 platform paths and percent-encodes path bytes
into one canonical local `file:` URI. Compiler source identities therefore reach diagnostics and
future semantic locations through the same protocol-owned URI policy rather than ad hoc string
prefixes. The common JSON-RPC renderer also owns server notifications; feature layers supply only a
method and typed JSON parameters.

Server-to-client requests use a separate monotonic identity allocator and pending-method table.
The JSON-RPC decoder distinguishes requests, notifications, success responses, and structured error
responses before lifecycle dispatch. A response completes exactly one retained request; unknown or
repeated identities and invalid result shapes remain typed protocol issues rather than being
mistaken for client requests.

`nocter-language-server` is that composition layer. An existing document resolves to its physical
canonical file; a new unsaved document resolves through one existing canonical parent. The
`DocumentWorkspace` freezes the resulting URI-to-path association only after open succeeds and
reuses it for every change, save, and close. Later filesystem or symlink changes therefore cannot
move an open document between compiler identities, and failed transitions cannot partially mutate
the URI map or immutable source overlay.

Initialization decoding retains the optional fallback root, ordered workspace folders, and the
client's dynamic watched-file registration support. Recognized nested capability fields reject
duplicates and wrong types, while unknown capabilities remain forward-compatible. The initialize
result is built from one protocol-owned schema and advertises only UTF-16 positions plus implemented
open/close, full-change, and included-save-text synchronization; semantic features are added only
when their production query path exists.
Receiving an initialize envelope enters a distinct `Initializing` state. The server commits that
transition only after parameter validation and returns to `Uninitialized` on failure, allowing a
corrected request without reconstructing process state. No document or semantic notification can
cross the pending-validation interval.

The validated executable supplies one immutable `LanguageServerEnvironment`: invocation directory,
native compilation target, Nocter home, and exact standard package. Successful initialization
resolves workspace folders before the legacy root URI and falls back to the captured invocation
directory only when neither exists. Every selected root must be a physical directory and is
canonicalized exactly once; aliases of the same directory collapse in authored order. A canonical
document belongs to the deepest containing root, so nested workspace folders have deterministic
ownership independent of client ordering. URI decoding, root filesystem validation, and lifecycle
commit are one transaction. A bad root therefore restores the uninitialized protocol state rather
than leaving compiler configuration partially visible.

The compiler-selected standard root is already canonicalized by the validated installation
boundary and is not a workspace-owned path package. Scope selection compares that exact root before
workspace containment and assigns all of its documents one `ToolchainStandard` scope, including
documents opened through navigation outside every workspace folder. Discovery owns a dedicated
toolchain-standard layout: it catalogs every directory module beneath that one closed package and
retains the installation's existing package identity. LSP composition neither recanonicalizes the
toolchain root, enumerates module directories, nor submits the standard root to ordinary
root-package resolution, so the package graph cannot acquire a second path-derived identity for the
same physical root. All open standard contracts and implementation sources participate in the same
immutable overlay snapshot.

The language-server service composes each validated protocol event with document state. Requests
without an implemented handler receive the standard method-not-found response; malformed
notifications produce no protocol response but remain typed server issues. Initialize and shutdown
responses preserve request identity, and clean versus premature exit produces an explicit process
status. The transport loop writes only framed JSON-RPC messages to its output.

Every accepted document transition now retains one complete immutable source revision.
`WorkspaceAnalyses` first selects the exact compiler-owned standard root. Other
documents select the deepest `index.nct` ancestor bounded by the owning initialized root; that
declaration selects package mode, while a `.nct` file without such an ancestor selects single-file
mode. Package mode resolves the complete exact graph under mandatory locked/offline policy and
discovers the package root, every declared executable and test module, and the canonical module
owning every source currently selected in that package scope. The complete set is one compiler
input; no physical source represents its peers, and source ordering cannot change the discovered
module roots. Toolchain-standard mode loads its closed package once and catalogs every module.
Single-file mode resolves only the
toolchain standard support package. All three routes pass the same source overlay through discovery
and `AnalysisSnapshot` target checking. Latest results are stored by package, toolchain-standard, or
standalone-file scope and shared with request/publication outputs through immutable ownership. Every
accepted overlay recomputes the scope membership of known documents. A membership change refreshes
each surviving old or new scope exactly once and invalidates an empty scope in one atomic analysis
batch. An empty scope emits an invalidation-only generation and never enters discovery or semantic
checking. A changed dependency source also refreshes each latest scope whose retained source map
reaches it. Independent scopes are not recompiled. Closed reached sources use a derived
source-to-scope index. An exact selected document scope wins; otherwise a unique physical owner may
answer. If multiple current package contexts reach the same dependency source and none owns it,
lookup returns `AmbiguousDocumentAnalysis` instead of selecting by path, insertion, or generation
order. A successful stale or arbitrarily ordered program therefore cannot answer queries after
package-topology or shared-source changes.

`DiagnosticPublisher` projects only compiler-owned `SourceDiagnostic` values. Primary and related
origins resolve through the snapshot's own `SourceMap`, and normalized byte spans convert through
the source-owned UTF-16 index. Each open-document publication includes the accepted version.
Published URI sets are retained by analysis scope, so a later complete generation emits empty sets
for every document whose diagnostics disappeared, including documents invalidated by a scope
change. Preparation failures without compiler-owned source spans are not attached to an invented
zero-width location; they use a separate error message notification until package preparation owns
a source-backed failure snapshot.

When the client supports dynamic watched-file registration, the server sends one correlated
`client/registerCapability` request after `initialized`. Its `**/*.nct` watcher covers create,
change, and delete events and becomes active only after a successful null response. Watched changes
resolve through the same canonical document-path boundary, advance the workspace generation without
replacing any accepted open-document bytes, and enter the same analysis and diagnostic publication
pipeline. A batched notification continues past path-local failures and suppresses duplicate URIs;
all resulting issues and immutable snapshots remain explicit outputs of that server step.

`nocter lsp` is part of the same declarative command schema as every other public command and has
an empty argument surface. Argument validation precedes installation access; a launch retains the
validated installation and current directory for later root/toolchain composition. The binary
enters the stdio loop before ordinary outcome rendering, guaranteeing that reports and diagnostics
cannot be printed to protocol stdout.

`nocter-package` is the sole data-interpretation authority for `index.nct`. It converts package
metadata, dependency sources, exact locks, and target declarations into one structured snapshot
with exact syntax origins. Git, archive, and path dependency shapes are disjoint; `std` is rejected
as an authored dependency or lock; lock kind is validated against its declared source. Package
target name, kind, declaration order, and normalized module path cross the later pipeline as facts.
Discovery and declaration lowering do not decode those fields again.

The same crate owns canonical exact `PackageId` construction. Git commits become `git-` plus the
lowercase locked commit, archive content becomes `sha256-` plus the lowercase locked digest, and a
mutable path package becomes `path-` plus SHA-256 of its canonical absolute UTF-8 path. These
Windows-safe strings are both resolved `PackageIdentity` values and store directory basenames;
display metadata and acquisition URLs cannot affect identity. `nocter-hash` owns the one
dependency-free SHA-256 implementation shared by package identity, Mach-O UUID generation, and
Mach-O code signing.

The same crate closes externally selected identities and roots into one `ResolvedPackageGraph`.
That graph owns the package-source `SourceMap`, syntax trees, decoded declarations, presentation names,
and exact alias edges. It rejects duplicate identities or canonical roots, unknown edge targets,
authored/resolved alias disagreement, missing remote locks, and path dependencies whose canonical
directory differs from the resolved target package. Syntax-invalid package sources remain owned by
the snapshot for ordinary diagnostic projection. Discovery consumes the graph by ownership,
reuses the already parsed package root `index.nct` as the root module source, and appends the other
inventoried module sources to the same source universe.

`resolve_package_graph` constructs that snapshot directly from one explicit root, Nocter home,
toolchain-selected standard package, and immutable locked/offline policy. It resolves mutable path
dependencies at their canonical authored directories and exact remote dependencies first at
`<root>/.nocter/packages/<PackageId>`, then at `<Nocter-home>/packages/<PackageId>`. A shared graph
builder loads each selected package root source once while recursively closing its dependencies;
resolution does not inspect a source and then ask the graph to reopen it. Missing locks and installed
packages cross typed `LockRequired` and `FetchRequired` boundaries. Locked/offline policy converts
only the forbidden requirement into a policy error. The resolver never writes a lock, contacts a
source, downloads content, or mutates either store. Source-independent `ExactDependencyLock`
values are distinct from syntax-bearing authored locks. A `PackageLockOverlay` can therefore close
a provisional graph before generated source is committed, while a `PackageStoreOverlay` selects
private staged roots before they are published. Both are immutable resolver inputs; neither grants
mutation authority.

The editor-facing retained resolver wraps every failure in `PackageResolutionFailure`. Its
`PackageSourceSnapshot` owns the exact source overlay, `SourceMap`, and all package-root syntax trees
reached before resolution stopped. A tree enters that snapshot before semantic package decoding,
so an invalid directive cannot discard the node that owns its error subject. Ordinary command APIs
still project the same underlying `PackageResolutionError`; editor analysis keeps the richer value
for invalidation and presentation. Package declaration failures therefore publish `E0800` at their
exact retained syntax node, while lock/store policy failures that have no authored source subject
remain non-positional messages.

`nocter-package-state` owns the mutation transaction above that read-only boundary. A transport
implementation receives typed lock-resolution and exact-fetch requests, but only the coordinator
chooses private staging roots, publishes verified package directories, or rewrites package source.
It resolves the complete graph through both overlays, publishes only after that graph succeeds,
reruns resolution using persistent stores alone, then atomically commits one canonical sorted root
`#lock` block inside the package-directive prefix. The source commit compares the exact retained
package-source bytes first, so a concurrent edit is rejected rather than overwritten; generated
source also preserves the source's LF or
CRLF convention. Missing locks below the selected root are rejected because an exact stored
package and a separately selected path package are not implicit mutation targets. Failed staging
removes its private transaction tree, and individually published exact packages remain valid cache
entries even if a later source commit fails.

`nocter-package-acquisition` is the concrete transport authority. It embeds public HTTPS,
Git smart protocol, SHA-256 verification, gzip decoding, and tar interpretation; it never invokes
`git`, `curl`, a credential helper, or a checkout filter. The crate validates URL and redirect
policy before transport, resolves branch/tag selections to exact commits, and materializes Git
trees without repository metadata. Archive bytes and provisional bare repositories live only in
the coordinator-provided transaction workspace, so lock resolution can feed exact fetching without
an ambient cache or a duplicate download. Archive extraction and Git materialization share bounded
entry, path-depth, and expanded-data budgets and reject link-like entries before graph validation.
`nocter-command` receives only the abstract acquisition capability; the public process adapter is
the composition root that selects this concrete authority. A single command-owned package-state
adapter maps the coordinator's generic transport error while preserving exact resolution errors.
Build, run, and fetch all cross that adapter. Fetch stops at its returned graph-validated package
selection and therefore cannot create a second resolver, publisher, or lock-update path.

The production selection result retains command-root and standard `PackageIdentity` values beside
the graph, so command discovery never recovers either role from a path or display name. A graph-only
projection remains available only for consumers that genuinely do not need those roles.

`nocter-installation` owns deterministic selection and physical validation of the active Nocter
home. It accepts the configured-home value and executable candidate as explicit process facts; it
does not read environment variables, the current executable, the working directory, or user-home
state. A nonempty configured home has priority. Otherwise, the canonical executable's parent is
the only candidate. The selected root and its required entries are canonical, physically typed,
and contained within that root, so a required symlink cannot substitute state outside the
installation. `VERSION` is decoded once as one nonempty UTF-8 line. A bounded strict JSON parser
preserves object members rather than overwriting duplicate names, and one exact
`nocter.manifest`-v1 decoder rejects duplicate, unknown, missing, mistyped, and inconsistent fields.
It keeps host identity separate from compilation-target identity, requires the default target in
the ordered implemented-target set, validates portable release and relative-path vocabulary,
matches archive identity and `VERSION`, and closes the declared license files through the same
contained physical boundary. The resulting immutable profile owns the release, complete metadata,
canonical compiler, standard-library, license, and notice paths. Its release owns the standard
package identity. No command or compiler stage may inspect installation JSON independently.

`nocter-toolchain-contract` owns the dependency-free closed identities for compiler-selected
standard declaration roles and anonymous structural attachments. Discovery, compile-input, and
declaration construction all consume those identities directly; no earlier stage depends on the
declaration graph merely to name a later semantic responsibility. `nocter-compile-input` composes
those neutral identities with immutable syntax and model locators for the handoff from discovery
to semantic lowering. `nocter-discovery` consumes exact resolved package roots, inventories every physical
source owned by each selected directory module, distinguishes exact same-module `see` edges from directory-module `use` edges, rejects
ambiguous physical candidates and nested-package or cross-module escapes, and retains one edge for
every active authored `see` and `use`.
Lexically or syntactically invalid sources remain in the snapshot for diagnostic projection, but
the snapshot cannot be borrowed as a semantic input until those errors are absent. Discovery uses
the shared `nocter-target-selection` inventory, so an inactive gated import never probes the
filesystem and lowering cannot reinterpret target activity.

The discovery request is an explicit sum of declared-package and single-file layouts. File mode
requires one `.nct` path, derives its opaque package identity from the canonical source identity,
adds only the selected standard package as a dependency, and rejects package-local imports that
would silently turn the file into a directory graph. It then emits the same package, module,
source, source-visibility edge, import-edge, and toolchain snapshot consumed by declaration lowering. There
is no single-file semantic pipeline.

Declared discovery owns package-target module edges for the roots selected by its caller. It maps
the package snapshot's normalized module segments to the package's exact `ModuleIdentity` and
freezes that identity beside the target's declaration, name literal, kind, and authored order.
Declaration lowering verifies syntax-origin containment and package/module ownership, then consumes
those facts without reading directive text. Target directives for modules outside the requested
compile roots do not expand the unit.

`nocter-declaration-lowering` accepts one explicit compile-unit input after package discovery. A
package has an opaque resolved identity distinct from its display name. A module has that package
identity plus normalized directory segments. `PackageInput` deliberately carries no declaration
syntax: the declared package's root module `Root` source is the sole semantic representation of the
package `index.nct`. Physical root-module sources, child-module roots, ordinary module sources, and
single-file inputs carry canonical path keys, but those keys are used only to reject duplicate
ownership and to produce deterministic source projections. Lowering does not probe directories or
reinterpret paths. It sorts packages, modules, and sources by their exact input identities before
allocating semantic IDs, so discovery order cannot change the declaration program. Declared
packages require one root module with one `Root` source; single-file mode requires exactly one root
module with one `SingleFile` source. The two layouts cannot be silently substituted for each other.

The resolved `PackageIdentity` is a syntax-independent model value, and each semantic `Package`
retains it beside its presentation-only display-name symbol. Declaration reservation rejects a
repeated resolved identity even when the display names differ. Dense `PackageId` values remain the
internal relation keys, while commands, caches, and diagnostics can recover the exact resolver
identity without correlating arena order or reparsing source metadata.

Package discovery supplies disjoint resolved inputs for source composition and module imports. An
`SourceVisibilityResolutionInput` identifies one exact physical source for a top-level `see`; a
`UseResolutionInput` can identify only a directory module for a top-level or block `use`. Neither
type can encode the other's target, so lowering never derives the distinction from path text or
filesystem layout. Every inventoried source is checked independently of visibility reachability.
See edges may cycle and remain idempotent.
Module-use edges must target a module in the compile unit and form an acyclic graph. The canonical
surface retains source-visibility edges and imports in separate collections, so later namespace preparation has
no path-probing fallback or source-or-module union.

Module edges also retain the exact authored `use` node selected by discovery. Acyclic validation
first removes the deterministic acyclic prefix, then derives one canonical complete cycle from the
residual graph. The cycle is rotated by canonical module identity, so compile-unit input ordering
cannot change its primary edge or ordered notes. Missing, duplicate, and stale
discovery inputs remain internal boundary failures; only authored source-visibility boundaries and
module-cycle rules receive source diagnostic codes.

The authored standard library now crosses this production boundary as one declaration unit. This
qualification exposed and removed stale `destruct` syntax, same-line statement sequences, the
`std/string` to `std/str` back-edge, and the `std/iter` to `std/vec` collection-terminal back-edge.
Allocation-backed iterator collection lives in `std/iter/collect`, leaving the core Iterator/Vec
dependency graph acyclic. Separated construction implementations also reuse declaration-pattern
binder identities through a spelling-keyed projection; repeated pattern tokens project as
references instead of being rejected as ordinary duplicate generic parameters.

The selected toolchain is part of the discovery request and the immutable compile input. It names
one exact standard package, prelude module, the structural slice attachment module, every named
built-in type declaration, a set of standard semantic declaration roles, and closed primitive
roles. Discovery loads every selected module and resolves each declaration locator to one exact
declaration-name token before semantic lowering. Standard
semantic roles deliberately select visible contract declarations and ignore private implementation
bodies. Primitive roles deliberately select primitive declarations from the complete authored
module source set, including private declarations in directly seen ordinary sources. Lowering then
normalizes each selected declaration's ordinary source visibility. Target validation requires that
visibility to equal the closed registry's source-private, package-visible, or public exposure.
Lowering binds each named built-in token to its existing canonical semantic type and derives its
inherent-surface module authority from that declaration. Only the anonymous structural slice
surface retains a separate attachment authority. Checking consumes the projected frontend
bindings; target setup consumes the separately projected primitive registry. Neither uses the
presentation index as semantic input. No later stage may recover toolchain
authority from a package name, module path, declaration spelling, visibility guess, or
opportunistic presence in the source graph. The complete authored standard source graph now passes
declaration lowering and body checking through this boundary as one qualification case.

Architecture tests read Cargo's resolved metadata rather than interpreting dependency-section
text. They freeze reviewed core-layer edges and reject a declaration-storage dependency anywhere
in the transitive discovery or compile-input graph, so manifest formatting, target sections, and
dependency ordering cannot bypass the boundary.

`nocter-session` owns the only production transition from a syntax-clean discovery snapshot to a
target-validated semantic program. It invokes the declaration facade, checking preparation and
body boundary, exact primitive registry resolution, target capability selection, and
`TargetProgram` validation in one fixed ownership chain. Lower stages remain independently
testable, but a command must not publish success from one of those partial boundaries. A completed
session retains the `TargetProgram` and `SourceIndex` as separate immutable values; source
projection never enters target semantic identity. Production compilation, retained editor
recovery, and incomplete-body admission all use the same recovering lowering, preparation, and
body-checking traversal. Syntax admission changes only the accepted input. Evidence retention is a
decision made after that traversal, so it cannot select a parallel semantic implementation.

Executable-producing session requests additionally own presentation-name selection. The request
selects the sole executable or one declared name among the exact command-root packages, resolves it
to exactly one `PackageTargetId`, and consumes the target program into `ExecutableProgram`.
Discovery freezes command-root `PackageIdentity` values before dependency traversal; declaration
lowering translates them once to `PackageId` values retained by `DeclarationGraph`, while each
semantic package retains the corresponding resolver identity. Session code therefore never infers
root authority from package names, target presence, arena order, or dependency shape.
Absence, multiplicity, unknown names, and cross-root ambiguity are closed selection errors. Build
and run cannot repeat this lookup or call executable specialization directly.

The native session consumes that executable result through MIR, machine lowering, ARM64 selection,
and Mach-O construction and returns one immutable image beside the unchanged `SourceIndex`.
`NativeSessionError` preserves the exact failed boundary instead of flattening backend integrity
failures into a command message. Commands may choose paths, write image bytes, and launch an
artifact, but they cannot invoke or reorder individual compiler stages.

Package-wide native compilation performs frontend checking and `TargetProgram` construction once,
then gives each root executable immutable shared ownership of that target snapshot. It closes and
lowers every executable independently in canonical package-target declaration order and returns a
complete image set only after every entry succeeds. Each entry retains its resolved
`PackageIdentity`, authored target name, and dense `PackageTargetId`; output planning does not
recover identity from filenames or display names.

`nocter-command` owns executable filesystem and child-process effects after that session. Build
writes a uniquely created sibling file, applies executable permissions, synchronizes it, and
atomically renames it over the requested output; failures remove the private file and never expose
partial bytes. Run stages the same image in a private temporary directory, inherits standard
streams, waits for the child, preserves its `ExitStatus`, and explicitly removes the artifact.
Launch and cleanup failures remain distinct, including the case where both occur. This crate
depends on `nocter-session` rather than semantic or backend crates, so command code cannot assemble
a second compiler pipeline.

Package build first creates a pure `BuildOutputPlan` for the complete native image set. The plan
maps each identity-bearing entry to its authored executable name directly below the caller-selected
package root, rejects names that are not one filename, and rejects cross-root output collisions
before the first filesystem mutation. Publication then consumes that frozen order. Each path is
committed through the same sibling-file protocol, and artifact errors retain the exact executable
identity that failed.

The command input boundary resolves package and single-file modes before package graph discovery.
It receives the invocation directory explicitly rather than reading process-global state. No file
input selects exactly that directory as a package root and requires its `index.nct`; `--root`
selects exactly the requested directory. Positional and `--file` sources converge only after their
mutual-exclusion check, require the `.nct` extension and one regular file, and cannot coexist with
`--root`. Resolution canonicalizes the selected identity once and never searches ancestors,
guesses `main.nct`, or invents a package.

Build, run, and check planning consume only that normalized input plus parsed
executable/output options.
Package build without `--executable` or `-o` selects the complete executable set. A named target
selects one executable and defaults its path to the authored name under the package root; `-o`
selects one sole/named target and resolves relative to the invocation directory. File build always
selects its discovery-owned executable and defaults to the source stem under the invocation
directory. Run maps package input to sole/named selection and package-root working directory, while
file run uses sole selection and the invocation directory. `--executable` is rejected for file
input before compilation; target existence and sole-target cardinality remain session facts. A
package check without a name selects the root and every executable module; a named check selects
the root and that exact executable module. A single-file check selects only its synthetic root.
Unknown named targets are rejected at this graph-selection boundary rather than being mistaken for
a successful root-only check.

The public command argument boundary is a pure `OsString` parser. It receives arguments without
reading process-global state, keeps positional and `--file` forms distinct, supports `--` without
forcing paths through Unicode, and rejects unknown, duplicate, missing-value, or command-inapplicable
options before filesystem access. A declarative command shape controls which options and positional
inputs each command accepts instead of scattering command-name conditionals through parsing. That
shape is now the single `CommandSchema`: it owns each implemented command token, summary,
positional form, and accepted `OptionSchema` identities. Option schemas own canonical/short names,
value requirements, and descriptions. Parsing resolves tokens to those identities, while overview
and command-specific help project the same tables. `--help`, `help`, `help <command>`, and
`<command> --help` therefore cannot drift into a second flag vocabulary. Help exits from the pure
parse result before installation selection, source preparation, or package acquisition.
Preparation then invokes the existing input resolver and command planner in that fixed order.
Fetch uses the package-only projection of the same input authority, so a source-file mode is not
representable after parsing. `--locked` and `--offline` survive as a separate immutable package
resolution policy; they never alter executable or output selection.

`init` is the only installation-independent package mutation. Its command-owned transaction
preflights root `index.nct` and `tests/unit/index.nct`, records each newly created
file and directory, and rolls those paths back after any later failure. It never removes unrelated
content from an existing destination. `graph` has the opposite capability: it receives a validated
`CommandPackageContext` but no acquisition authority, invokes the read-only exact resolver, and
projects the resulting typed identities and edges through one human/JSON presentation contract.
Missing locks or packages remain explicit resolution requirements; inspection cannot satisfy them
by changing source or store state.

`fmt`, `tokens`, and `ast` are source-only commands with exact-one-file parse results. One
command-owned standalone loader resolves the canonical path, retains the original bytes, selects
the single source-file parse goal, and constructs `nocter-source-tooling`: one
filesystem-independent snapshot owning normalized source, lexer output, and its concrete syntax
tree. The `nocter.tokens` and `nocter.ast` version-1 renderers project that snapshot directly into
flat, ID-based JSON; they do not create a second AST. Lexer/parser diagnostics use the same
projection functions as discovery.

Direct CST navigation is also a syntax-layer contract. `nocter-syntax` owns the source-ordered
queries for child nodes, child nodes of one kind, child tokens, and the first token of one kind.
Lowering, checking, constant evaluation, and analysis consume those queries instead of maintaining
lookalike tree walks with subtly different missing-node behavior. Domain traversals may still stop
at declaration or block boundaries, but they build on this one direct-child definition.

Formatting consumes that same snapshot as a concrete-syntax layout operation. It cannot resolve a
name or inspect a semantic program. One shared ordered-token projection returns every CST token
once and retains parser-owned subdivisions of lexical tokens such as nested generic `>>`; AST JSON
and the formatter therefore cannot disagree about their concrete token vocabulary. A CST-derived
layout plan selects delimiter indentation, line breaks, joins, and optional trailing-comma edits
before emission. The first candidate is parsed again under the identical root goal and must retain
the same non-newline token text and syntax topology after normalizing only optional trailing
commas. Specification-owned redundant grouping is then selected from that validated CST and a
second parse proves that only the selected parenthesis tokens disappeared. The command boundary
compares original bytes for `--check` and otherwise writes a complete
same-directory temporary, preserves source permissions, synchronizes it, and performs one final
rename. Syntax, comment-preservation, formatting-integrity, or publication failure therefore
cannot expose a partially formatted source. The process adapter completes all three commands
before installation selection because package, standard-library, host, and target state cannot
affect lexical or syntactic inspection.

Prepared check, build, and run commands cross one production compiler-command adapter. Its explicit
`CommandToolchain` contains the already selected target and a `CommandPackageContext` containing
the Nocter home and standard package. Fetch receives only that package context, not compilation
target authority. The adapter never reads process globals or reconstructs installation state. The
sole `nocter` process
entry reads arguments, `NOCTER_HOME`, the real executable, and the current directory once. It
validates argument structure before installation or source access. Source-only inspection and
package initialization then exit through their independent boundaries. Remaining non-help
commands delegate installation
selection to `nocter-installation`, then create `CompilerInstallation` only after the manifest
host and native default target both match the running compiler host. This wrapper makes the
compatibility relationship a prerequisite for every installation-dependent command instead of a
repeated CLI condition.
The process adapter creates the command toolchain from its default target and standard package.
`--version` and `doctor` render only this validated profile; they do not prepare source or
initialize package acquisition. Package mode invokes exact
package selection, retains the resolver-owned command-root identity, selects the root module and
only the executable modules required by the command, and then creates one ordinary declared
discovery request. A named build or check does not open an unselected executable module. Package-set and
sole-selection modes retain every executable root needed for their cardinality rules. Single-file
mode loads only the self-contained standard package and creates the normal single-file discovery
request. Both layouts then use the same session, artifact, and launch boundaries. There is no
separate CLI compiler pipeline. Check returns the session-owned `CompiledTarget` immediately after
target validation and never gains executable-specialization, artifact, or launch authority.

The process adapter attaches a spanless diagnostic code only when a command boundary owns the
complete classification: command syntax, filesystem input selection, package-root selection,
Nocter-home validation, package state, or target selection. It leaves authored compiler-stage
failures unclassified rather than replacing a source-backed diagnostic with a generic CLI code.
Failed compilation retains the invocation `SourceMap` beside
the phase-selected `SourceDiagnostic` values. The common renderer consumes that snapshot directly;
the process adapter neither reopens files nor classifies semantic errors.

Discovery uses the same rule before a complete `DiscoveredUnit` exists. `DiscoveryFailure` owns the
partially loaded `SourceMap` and projects an authored resolution failure to `E0263` at the exact
`ModulePath` syntax node. The command boundary forwards that snapshot rather than flattening the
failure to package or filesystem text. Failures that prove an impossible discovery graph or syntax
identity carry no authored origin and remain internal `E0900` failures.

Machine-readable source diagnostics use the same validated origin projection. The projection
checks source identity, range bounds, and UTF-8 boundaries once, then exposes exclusive byte
offsets and one-based byte line/column coordinates to the versioned JSON renderer. JSON escaping,
notes, help, nullable absolute paths, and the `nocter.diagnostics` envelope belong to
`nocter-diagnostics`, not the CLI. A check invocation owns one progressive presentation snapshot
independently from its failure. Argument parsing records the first structural error but continues
its pure token pass to retain a later `--format=json`; the process adapter never reparses argv. The
snapshot starts with an authored root hint, adds target identity after installation validation,
and replaces the hint with canonical root identity after input preparation. Successful checks emit
one empty envelope; source-backed and spanless failures use the same versioned renderer without
mixing human text into stdout. Source, user/environment, and internal failure classes select
statuses 1, 2, and 3 independently from diagnostic codes, so an `E0900` JSON object cannot turn an
internal failure into status 2.

Before semantic identities are reserved, lowering produces one temporary declaration-surface
inventory. It visits module roots before implementation sources, sorts implementation sources by
canonical physical identity, and records each declaration or member with its exact syntactic
owner. Blocks are opaque to this pass; body syntax cannot create or alter a declaration header.
The pass also enforces the module rule that implementation sources cannot add visibility,
re-exports, or bodyless nominal contracts. Private representations, helpers, construction entries,
and inherent methods may remain implementation-only. A following contract pass joins every
bodyless public callable and opaque nominal to one reciprocal direct-see definition. Because an
interface implementation changes program-wide dispatch and has no private visibility form, each
`impl Interface` fact is authored in an `index.nct` instance fragment. A reciprocally seen private
source may provide ordinary method bodies in an exact-header instance fragment, but cannot repeat
or introduce the `impl` fact. The contract pass unifies those open fragments before semantic IDs
are reserved; required signatures remain solely on the interface. This inventory is consumed by
declaration reservation and never becomes a second long-lived program model.

The compile-unit input carries one closed `CompilationTarget`, and the frozen declaration graph
retains that identity through checking. Before import-edge validation or symbol collection, one
temporary target-selection inventory decodes every `#target` gate. Import validation, symbol
collection, and surface collection consume that same inventory. An inactive item therefore
contributes no block import, spelling, declaration, body, or semantic ID. Later semantic stages do
not filter declarations or reinterpret target strings.

Before allocating declaration IDs, contract joining compares canonical header token sequences. It
excludes visibility, bodies, and newlines while retaining names, owner patterns, generic
requirements, parameter names and types, results, and authored provenance.
One eligible public bodyless root callable must match exactly one private implementation body.
Source-defined operators use the same callable rule. Exact-header instance fragments in one source
or in reciprocally visible sources share one instance identity, while every child method keeps its
own source-backed identity. An implementation source carrying an `impl Interface` fact is rejected
before representative selection, so this source-role rule cannot depend on traversal or join
order. A separate nominal-
representation pass matches each public opaque struct or enum contract to one private complete
representation. The definition occurrence and its implementation container map to the public
representative; private members in that container retain their own identities under the shared
owner. Header definition enumerates only canonical child representatives, so a modifier repeated
by a matching implementation cannot be counted as a second semantic member. Missing, mismatched,
and duplicate definitions fail before reservation. Later stages never merge already-distinct
semantic IDs or search for a representation by name.

One production declaration-lowering facade owns the pass sequence: surface collection, declaration-
contract joining, identity reservation, header preparation, generic preparation, authored imports,
compiler-selected prelude composition, type binding, header-constant evaluation, type normalization,
and header definition.
The facade performs no semantic work of its own. It prevents tools and later compiler stages from
assembling a partial or differently ordered declaration graph while keeping each pass independently
testable.

The phase-neutral constant-expression component owns typed expression plans, scalar operation
rules, conversions, short-circuit evaluation, dependency ordering, cycle detection, and arithmetic
failure. It owns no namespace, lexical scope, declaration storage, or body representation. A
caller must resolve every referenced constant and conversion type through a narrow resolver
contract before evaluation can produce a value.

Declaration lowering adapts header-bound names and types to that contract. It builds the complete
named-constant dependency graph, freezes immutable scalar declaration values, and evaluates only
fixed-array lengths present in bound declaration headers before structural type normalization.
Declaration lowering treats blocks as opaque and publishes no body expression result through
`FrontendBindings`.

Body name resolution traverses body type annotations along with value expressions. It freezes each
source-backed lexical or module-namespace identity used by a value, body type path, or constant
subexpression. Named built-in types use the same source-backed identity path as other declarations;
generic parameters, `Self`, and associated projections remain type-directed. Body type checking
resolves those type-directed forms through its ordinary type authority, consumes every
source-backed identity selected by name resolution, then adapts constant
targets to the shared planner and evaluates each body fixed-array length once. It cannot repeat a
module namespace or visibility lookup after name resolution selected an identity. MIR, target
specialization, machine lowering, and editor presentation consume normalized types and frozen
constant values only; they cannot inspect or evaluate constant-expression syntax.

Reservation consumes that grouping in canonical surface order. Nominals, aliases, interfaces,
associated types, callables, construction surfaces, instances, interface implementations, variants, drops,
tests, and opaque result types receive their final typed IDs before any header type is resolved.
Fields, generic parameters, ordinary parameters, requirements, and bodies are added later because
their identities cannot participate in recursive header lookup before their owner exists.

Header preparation reads the exact name-token identity already selected by the surface inventory,
resolves `pub`, `pub(./)`, ancestor scopes, and `pub(/)` once to semantic package/module
boundaries, and allocates declaration sites. It rejects module, member, and test-name collisions in
canonical order. Only after a name is known does source projection bind a named entity to its exact
token; unnamed containers use their syntax node. A joined body is an implementation binding of the
contract entity rather than a second declaration. This prevents editor ranges from expanding to a
keyword, visibility prefix, brace, or surrounding whitespace.

Generic preparation runs only after recursive owners and declaration names exist. Explicit generic
lists allocate unique, non-shadowing parameters in authored order. A declaration type pattern
allocates a parameter at the first spelling and resolves later occurrences of that spelling to the
same identity; declaration and reference source roles preserve that distinction. Members inherit
an immutable, symbol-sorted owner scope and add only their own explicit binders. Joined callable
implementations reuse the representative contract's parameter IDs and must repeat the exact binder
sequence. Structural type lowering therefore receives one complete lexical generic environment and
never infers binding identity from a type occurrence. Each lexical entry also retains the exact
declaration token selected when the binder was allocated. Reserved names, explicit same-list
duplicates, and nested shadowing project distinct `E0280`-`E0282` diagnostics from those stored
subjects; the diagnostic adapter never searches syntax or reconstructs scope ancestry.

Authored module imports are resolved after generic scopes but before type construction. Every
source owns one symbol-sorted authored namespace containing its declarations, declarations copied
from exact direct see targets, and only the imports written in that source. Source-visibility
composition uses a snapshot of each target's authored declarations, so it cannot become transitive
through the target's own see edges or imports. Separately, every module owns the export table authored by its
root `index.nct` or single-file root. Implementation declarations never enter that table, including
when another source sees them. Dependency modules are completed before importers. Selection
checks the target export from the importing module, and a re-export's normalized visibility must
denote a subset of the target boundary. Declaration lowering freezes each source namespace into a
dedicated frontend binding contract consumed by header binding, lexical name resolution, and typed
body type paths. Those consumers cannot reconstruct private visibility from module membership. The
same resolved namespace is independently projected into `SourceIndex` for editor spelling only;
semantic consumers cannot read that projection. The compiler-managed prelude remains a distinct
fallback layer, preserving authored shadowing without turning collisions into priority rules.

Declaration lowering separately freezes a `SourceAccessTable`. It maps every source to itself and
its exact direct see targets, every declaration site to its authored source, and every nominal
representation to the source that owns its fields or variants. It also records whether a bodyless
public nominal contract seals that representation. Name lookup consumes `SourceNamespaceTable`;
private field, method, operator, coercion, variant, and raw structural-construction selection consume
`SourceAccessTable`. Neither authority is reconstructed from module membership, and neither reads
`SourceIndex`. The explicit representation entry is required for empty opaque structs, where no
field declaration exists from which a consumer could infer the private source boundary.

Checking exposes this authority to semantic analysis only through an immutable
`SourceAccessContext` for one exact source. Its declaration-site query combines ordinary module
visibility with direct-source private access; its representation query combines direct-source
access with the explicit opaque-contract bit. Body checking, construction selection, completion,
and nominal hover consume those queries. Analysis cannot inspect the table or independently infer
private access from a module ID.

Import preparation retains the exact module-path node for every semantic module import. Prelude
composition consumes that retained origin instead of reading the syntax tree again. An authored
import of the compiler-selected standard prelude is therefore the shared import rule `E0262`, while
a missing selected prelude, an absent retained origin, or rejected program authority is an internal
composition failure.

Declaration freezing stores the authored namespace and compiler-selected fallback as separate
tables in `DeclarationProgram`. Authored entries retain effective visibility; fallback entries are
local-only and cannot become re-exports. Body checking consumes these tables directly instead of
rebuilding lookup from declaration/import iteration. Block imports remain lexical checked-body
data and are not represented as declaration imports.

Header type binding owns a closed distinction between authored rules and broken compiler input.
Authored name, entity-kind, arity, `Self`, fixed-array, callable provenance, opaque-binding, and
requirement failures retain `SyntaxOrigin` values at rule selection and project `E0290`-`E0302`.
Path segments retain their own name token and optional argument node, while duplicate constructs
retain the first and later tokens. Missing syntax nodes, missing discovery-owned sources, stale
symbols, and duplicate source-index insertion remain internal `TypeBindingError` variants. The
production facade is the only layer that converts an authored violation to `SourceDiagnostic`.

The mutable binding boundary is one `BindingArena`: a bound-kind arena, syntax-root index,
declaration-context index, and temporary `NormalizationOrigins` side index. The side index records
only subjects that a later normalization rule can select. It retains alias declaration tokens,
exact associated-selection tokens, and callable type nodes without contaminating `BoundTypeKind`
or canonical `TypeKind`. Normalization projects `E0310`-`E0314`; alias cycles are rotated by
canonical declaration identity and retain every declaration in the cycle. Missing bound state,
interface-application context, alias definitions, normalized `Self`, or associated-index invariants
remain internal failures. A declaration-pattern equality is lowered directly as a directed binder
refinement; equality outside that owner is rejected during type binding. An interface requirement
owns its normalized interface application and all associated bindings as one semantic fact, so
checking and presentation never recover predicate structure from neighboring requirements.

Callable type keys erase parameter spellings after resolving authored provenance names to sorted,
unique parameter positions. Result provenance is therefore part of the structural callable
contract without making a rendered name part of type equality. Static and fresh storage retain no
caller-managed place and normalize to an empty external-origin set.

The compile-unit type store interns structural types. Its keys contain typed semantic IDs and
normalized constants, never rendered names, source text, or byte positions. Declaration lowering
freezes a structurally valid `DeclarationProgram` containing the immutable declaration graph and
the header-type prefix. Complete authored-rule validation wraps it in
`AcceptedDeclarationProgram` together with a non-optional analysis-admission authority. Rejected
programs can enter only explicit editor recovery capabilities. Checking consumes the accepted value
exactly once into `DeclarationGraph` plus the owned
`TypeStore`, then interns body, closure, inference, and specialization types after the existing
IDs. The checked program freezes the extended store. It never copies a header store, translates a
`TypeId`, or creates an overlay lookup authority. `TypeExpr` belongs to syntax lowering and
presentation; it does not cross into checked semantics.

Header definition consumes the normalized roots and the temporary surface inventory exactly once.
It allocates fields, parameters, receivers, requirements, and bodies in canonical order, then
defines every reserved nominal, alias, interface, associated type, callable, construction,
instance, interface implementation, drop, test, variant, and opaque-result slot. A joined public contract and
private implementation share callable and parameter identities; their declaration and
implementation projections remain distinct in `SourceIndex`. Authored result-provenance clauses
are stored as declarations, while elided body-owned provenance remains explicitly inferred rather
than being guessed from a header.

Rule selection at this final syntax-consuming boundary retains exact subjects for declaration
facts that do not exist in canonical type identity: declaration provenance origins, the result type
of an ambiguous bodyless callable, and interface-implementation associated-type binding names. These rules
project as `E0315`-`E0319`. Duplicate rules store both authored tokens when the second occurrence
is observed. `HeaderDefinitionError` separately carries
malformed normalized state and program-builder failures; the production facade cannot expose those
failures through the source-diagnostic variant.

Compilation setup resolves the selected standard package and each compiler-owned built-in surface
to exact package and module IDs. The immutable declaration program stores those authorities.
Freeze-time attachment validation compares IDs, never a package display name or textual `std`
path. The same validator owns empty-enum, construction-result, unique construction-family, drop,
opaque-result, associated-binding, and owner-site invariants; lowering does not maintain a second
semantic prevalidation table.

Standard-library semantics that cannot be expressed by an ordinary source declaration use the
same declaration-graph authority as built-in attachments. Discovery supplies the exact declaration
name token for each compiler-owned role. Declaration lowering resolves each token against its
reserved semantic identity once and records the typed role-to-declaration mapping in
`StandardLibrary`. Graph integrity verifies that every recorded ID exists and belongs to the
selected standard package. Checked-program preparation consumes that mapping directly and validates
the complete semantic shape and required public surface; it does not receive role tokens,
`FrontendBindings`, or `SourceIndex` for role selection. Consumers cannot search for `Allocator`,
`String`, `Format`, `format_into`, `abort`, or a textual module path. A missing or malformed role is
a toolchain integrity failure; an authored `using` place whose resolved type is not one of the
validated allocator/context families is a source rule.

Standard contract validation completes before other program-wide preparation authorities. If a
later authority rejects source, declaration recovery may retain that validated capability beside
the graph. Editor mutations that require a standard operation, such as required-method scaffolding,
must query the retained capability and compare semantic callable IDs. They cannot trust a raw graph
role merely because it was selected, depend on preparation ordinal numbers, or match a completion
label or import path.

Primitive runtime roles remain a separate target contract. Declaration lowering projects their
selected tokens through `FrontendBindings` to exact callable IDs, and the session freezes one
`PrimitiveRegistry` before target validation. It never searches editor bindings in `SourceIndex`.

Program validation separates authored declaration-rule violations from malformed compiler graph
integrity. Integrity validation is fail-fast because a malformed graph cannot support trustworthy
semantic decisions. Rule validation instead produces one ordered, duplicate-free report and one
set of admission facts for the entire structurally valid graph. A declaration rule owns its stable
error code, source-level message, correction direction, primary declaration-site ID, and optional
related declaration-site ID. Only after the report is complete does declaration lowering project
all IDs through the completed `SourceIndex` to exact syntax origins. The same pass decides which
construction, inherent-instance, interface-implementation, and destruction declarations editor analysis may use; recovery
cannot rescan declarations or infer admission from a diagnostic code. Changing a diagnostic span
therefore cannot change rule selection, and adding a diagnostic cannot create a second attachment
or declaration-shape evaluator.

Checking converts accepted declarations or the frozen editor admission facts into one
`AdmittedOperations` identity set before building any program-wide table. Construction,
instance-operation, interface-implementation, and drop builders receive only their admitted IDs; they never
interpret an optional recovery mode or traverse rejected containers. One declaration-pattern table
normalizes instance and interface-implementation substitutions, targets, associated bindings, requirements, and
refinements exactly once. Body-local assumptions and admitted global table builders consume that
same immutable contract. A quarantined container therefore retains local editor semantics without
becoming a dispatch, overlap, destruction, or provenance candidate or triggering a second pattern
normalization pass.

All source-backed compiler diagnostics share one phase-neutral envelope containing a stable code,
primary source origin, zero or more related source notes, and optional correction guidance. The
envelope does not select a rule and has no dependency on declaration or checked-program models.
Each phase owns its rule selection and exact syntax-to-source projection. Contract joining projects
its exact syntax subjects into the envelope before reservation; module-surface validation does the
same only for authored root-versus-implementation rules. An unmatched private construction,
literal, or coercion entry has its own `E0254` rule rather than being rendered as a mismatch against
a contract that does not exist.
One phase-neutral human renderer consumes only this envelope and the invocation `SourceMap`. It
projects normalized source names, one-based character coordinates with four-column tab expansion,
every line intersected by the exact primary or related range, related messages, and help. It
validates source identity, range bounds, and UTF-8 boundaries; it does not inspect a semantic error
enum, search syntax, or reopen a file. Session failures must retain these two inputs rather than
reconstruct presentation after the source snapshot has been discarded.
The envelope origin is either an exact semantic `SourceOrigin` or a lexer/parser-owned normalized
`Span`; both project the same source identity and range to presentation. Discovery orders lexical
and parse envelopes without interpreting them. A command compilation failure then owns the
immutable invocation sources and the phase-selected envelopes. Error wrappers expose only the
existing envelope, and the public process adapter invokes the common renderer instead of matching
compiler error variants. `SourceIndex` remains an independent semantic-to-source projection for
successful compiler clients; human error rendering must not use it to reconstruct a span already
selected by the rejecting phase. Public error wrappers expose `source_diagnostics()` as a slice;
there is no singular outer diagnostic API whose cardinality silently truncates a phase result.
Freeze-time validation projects semantic declaration-site subjects through
the completed source index. Stage-specific wrappers preserve the selected rule identity. Errors
that indicate malformed syntax snapshots, incomplete discovery inputs, or inconsistent compiler
state stay outside this envelope and have no public language code.

Name collision and authored visibility rules form one namespace-rule domain rather than separate
header and import diagnostics. Header preparation stores exact name-token or visibility-node
subjects in a `NamespaceViolation` at rule-selection time, before its temporary surface IDs are
consumed. Diagnostic projection therefore never searches by rendered name or reevaluates a
namespace to recover the first declaration.

Authored import resolution consumes that same namespace-rule domain for reserved local names,
collisions, and visibility boundaries. Every namespace binding records the exact token that
introduced its local name rather than the enclosing declaration node. Import-specific violations
record the selected-name token and, for access or widening failures, the target binding's exact
origin. Missing names, access boundaries, and re-export boundaries are therefore selected once and
projected without repeating import lookup in a diagnostic adapter.

Type well-formedness is validated on interned semantic types after alias expansion and concrete
generic substitution. Invalid constructed types, including an optional layer whose eventual
payload is `void` or any outcome whose eventual payload is `never`, cannot enter checked bodies,
monomorphized keys, MIR, or ABI layout.
The `never` semantic type is admitted only in callable result slots and as the type of terminating
checked nodes. It cannot appear in a data layout or concrete substitution. Constraint collection
treats a `never` expression as non-producing and never binds a generic parameter to `never`.
The `void` semantic type is admitted only for normal-completion nodes, callable result slots, the
payloadless success branch of `void!`, and opaque `*void`. It never receives a zero-sized value
layout or enters a concrete data substitution.
Checked contextual injection represents `void` consumption as a sequencing edge, not a value
operand. Construction of `void!` writes its success tag after that edge; a terminal edge cannot
produce or initialize the outcome.
Declaration validation rejects zero-variant enums before semantic type interning creates usable
enum identities. Later exhaustiveness and layout stages may therefore assume every enum has at
least one valid tag.
Each nominal type definition owns at most one resolved top-level drop body. Declaration validation
admits it only for an ordinary struct or payload-bearing enum declared in the same module and
rejects it independently of `instance` and interface-implementation lookup. Copyability is derived from the
type declaration before that association and is never changed by cleanup availability. Checked
ownership and MIR consume the resolved drop-body identity; neither performs method lookup to find
cleanup.
The checked place model rejects a field move when any proper-prefix aggregate would become partial
while owning a drop body. Cleanup plans may therefore invoke a type-owned body only with complete
`Self`, then traverse remaining structural children. They never carry a partial-self drop ABI or a
runtime flag that conditionally suppresses user cleanup.
Every checked move place also records its owned root and projections. A borrow projection can
produce a writable place but never an owned move place. Returns, arguments, assignments, captures,
patterns, iteration, and spread consume this same classification rather than maintaining
context-specific move-source rules.
Initialization dataflow is keyed by those semantic places. Joins merge each named-field state to
initialized, uninitialized, or maybe initialized. Assignment consumes that one state to select no
drop, unconditional drop, or conditional drop before storing and marking the place initialized;
scope cleanup and whole-parent replacement consume the same state instead of recomputing liveness.
The program-wide copyability table derives optional copyability structurally from its present
payload. The built-in `error` and every fallible type are unconditionally move-only because the
failure branch owns a runtime node; a mixed outcome containing a fallible layer is therefore
move-only as well. The table memoizes the result by canonical `TypeId` and closes over the complete
semantic type store before checked-program construction finishes. Flow analysis never reclassifies
an outcome from its active tag, and MIR consumes the same type fact instead of deciding whether a
particular tagged value may be copied.
Generic `copy struct` validation builds one normalized conjunction of field-copy requirements. A
concrete specialization substitutes its arguments and evaluates its field types once through the
same copyability table. An unconditional move-only field rejects the declaration, while a generic-
dependent atom remains a specialization condition; body checking and MIR do not traverse fields
again to rediscover copyability.
Checked closure construction records one environment field for each resolved capture and derives
copyability with the same structural operation used by aggregates. Invocation capability is a
separate callable fact and cannot overwrite the environment's ownership class. Later
monomorphization and MIR consume both decisions without reconstructing either from closure syntax.
Each closure-body capture binding resolves directly to one typed environment-field projection with
readonly, readwrite, or owned access. Body checking never discovers captures by free-name scanning,
and later stages never reinterpret a stored borrow as an implicit source dereference. Copies and
moves of a closure retain the same capture and loan identities through ordinary value flow.
Closure identities are reserved in lexical source order before contextual inference begins. One
program-owned definition then records a structural closure type, normalized signature, parameter
bindings, captures, structural callable evidence, and independent body root. The type pairs its
`ClosureId` with the enclosing body's complete generic domain; substitution therefore creates a
distinct concrete environment type for each callable specialization instead of treating a generic
capture layout as globally concrete. Callable-contract and ordinary argument constraints are
solved as a dependency graph rather than in argument order.
Executable construction freezes that specialization as one environment layout containing the
concrete closure type, invocation capability, and ordered capture binding/type pairs. MIR closure
aggregates name the exact executable body and retain each `CaptureId` beside its value. Capture
places explicitly project through the capability-correct environment input, the stored field, and
any stored borrow; construction, body access, and recursive destruction therefore cannot disagree
about capture order or reinterpret a borrow as owned storage. Executable bodies freeze the node
domain reachable from their own root, so preparation never crosses into a nested closure root that
happens to share the checked-body arena.
Callable-bound generic calls use the concrete closure type to select that same generated body.
Readonly and readwrite contracts borrow the source place with the body's intrinsic capability. An
owned contract moves the environment; when the intrinsic body only borrows it, MIR keeps the moved
environment in temporary storage, invokes the body directly, and applies target-frozen destruction
after a returning call. Owned environments enter that canonical storage before argument evaluation
and leave it only after every argument succeeds. An argument propagation edge therefore addresses
the same storage through the checked cleanup schedule. Destruction is not hidden inside a call
convention.
Static text constants lower as readonly borrows of built-in `str`, matching checked HIR instead of
inventing an unsized by-value result. A typed string lowers to one direct call of its selected
literal item. The call carries either inherited allocation or one explicit allocator/context place;
the latter changes ambient allocation only for that call and requires both literal-item authority
and a compiler-selected standard nominal. It does not read, copy, or move allocator identity into
an ordinary value argument.
Unannotated result inference joins tail values, explicit returns, outcome propagation, and
divergence at that root. Ownership, provenance, liveness, and loan passes enter each closure root
with its parameters and capture fields initialized, then summarize result dependence on invocation
arguments, captured values, environment storage, and ambient allocation separately.

## Checked Program

Body name resolution is a one-way checking input, not a second program graph. One iterative action
machine creates dense body-local scope, local, and capture identities while walking each exact
`BodyId` source projection. Initializers are resolved before their binding is inserted; loop,
region, pattern, catch, and closure names are inserted only into the scope defined by the language.
Closure roots cut the ordinary lexical lookup edge, resolve explicit captures to immediately
enclosing callable bindings, and expose new capture identities inside the closure body. A free use
cannot become an implicit capture.

Block imports enter the same lexical insertion path as body bindings but create no storage
identity. Their discovery-owned module identities are converted to `ModuleId` only through exact
physical-source projections, then selected names use the frozen authored export namespace and
normalized visibility rules. Namespace-qualified value and body-type segments use that same
selection authority; a re-export retains its selected declaration identity rather than being
reinterpreted as a declaration owned by the qualifier module. Synthetic prelude names remain a
separate shadowable fallback.
Successful resolution extends `SourceIndex` with local/capture declarations and exact references
only after every body succeeds; temporary syntax-keyed uses are consumed by typed-node
construction.

Every checked body owns one typed node arena. Authored and compiler-generated operations use the
same node model and carry explicit body ownership. Comparison, indexing, conversion, iteration,
interpolation, construction, calls, ownership transitions, and failure handling are not retained as
unrelated side maps selected by source containment.

Typed-HIR construction freezes a body exactly once before flow-dependent ownership analysis. The
ownership walker may revisit immutable control nodes to compute loop fixed points, but it cannot
allocate a node, place, local, or loop identity. This separation prevents analysis order and the
number of fixed-point iterations from changing semantic identity or source projection.

One checked-place construction path handles field and index projections for reads, borrows, and
assignment. It records implicit borrow dereferences rather than collapsing them into the final
readonly/readwrite authority. This preserves the exact owned field prefix required by
initialization analysis and gives MIR an ordered projection plan. Each projection stores the
checked type produced immediately after that step. Executable specialization freezes those
intermediate types beside the final place type, so MIR never repeats field lookup, dereference
typing, or index-result selection while rebuilding a concrete place. Dynamic index expressions
are stored as checked node identities once; consumers execute those nodes in projection order
instead of reconstructing an index expression from syntax.

Name resolution assigns each syntax block one exact body-scope identity and checked construction
stores that identity on the block node. Ownership analysis therefore computes scope-exit edges
without source containment queries. Its dense cleanup table is keyed by the operation that owns
each event. One node may own separate statement-end, control-header-end, pre-store, propagation,
and control-transfer events; later lowering never derives timing from an operation variant. Path
actions retain only an owned
root, exact field identities, type, and unconditional-or-conditional state; borrowed replacement
actions retain an already evaluated place; discarded owned temporaries name their checked value
node. An enum-pattern residual action identifies the evaluated subject, selected `VariantId`,
specialized enum type, and exact still-initialized payload parameters. MIR expands those semantic
targets into control-flow cleanup blocks and structural drop glue
without inferring execution order from a control-operation variant.

Region construction reuses the exact standard-semantic allocator/context roles used by literal
allocation overrides. Its checked node owns one lexical context binding, parent operand, and body.
Ownership excludes that context from ordinary local destruction and emits a dedicated release
action after body-owned values on every reachable scope exit. The action also ends the parent
allocator loan, allowing enclosing cleanup only after the child has released. Provenance and loan
analysis consume those identities directly; neither infers a region from source spelling or block
containment.

MIR creates the context local as a non-movable resource rather than a temporary SSA value followed
by initialization. Lowering records the innermost region identity on ordinary calls, literals, and
authored destruction. A CFG stack validator rejects selection before creation, outer-first release,
inconsistent merges, and live regions at normal terminals. Machine and target lowering preserve
that identity until ARM64 frame placement assigns the closed runtime header and allocation-list
storage.

Explicit source `drop` is a checked control operation over the same owned root path. It attaches an
unconditional action to that statement's outgoing edge and consumes the path state; it does not
call a method, allocate hidden storage, or maintain a second explicit-destruction liveness table.

A checked body records reachability on its control-flow nodes. Source after a proven terminal node
still receives declaration and type identities, but no synthetic initialization, move, loan, or
provenance state is created for the impossible continuation. Such nodes remain available to
diagnostics and editor projections and are excluded from executable reachability and MIR.

Contextual expression checking lowers optional and fallible construction into explicit checked
nodes at every authoritative expected-type boundary. Each node names its expected outcome type,
selected tag, and recursively checked payload. An expression already having the complete expected
outcome type requires no injection node. MIR must consume these decisions; it cannot reopen a
rendered type spelling or reconstruct outcome order.
Checked propagation nodes identify the immediate operand layer and the ordered enclosing success
or presence injections required by the callable result; they do not encode only an unqualified
"failure" or "absence" action. Their failure edge cleans every already-created statement
temporary before live scope storage. Forced unwrap names the same immediate layer but has no trap
cleanup edge. Recovery nodes retain the operand, matching layer, optional catch binding, and
fallback block; ownership joins only the success and normally completing fallback states.

Evaluated owned temporaries participate in the same branch state as named storage. Callables,
owned receivers, arguments, and aggregate children remain staged until their enclosing operation
commits. A later propagating child therefore sees and cleans earlier staged values; successful
commit consumes them into the callee or aggregate. Borrowed receiver and comparison temporaries
remain live to the statement boundary. A value created on only one reachable branch joins as
conditionally initialized, so the statement-end event emits one conditional action rather than
duplicating branch-specific lifetime logic.

`if is` and `match` share one enum-pattern control operation. The subject records whether it is a
retained place, an explicitly consumed place, a newly produced owned temporary, or a readonly or
readwrite borrow. Each explicit arm records one exact `VariantId` and one positional slot for every
payload `ParameterId`; a slot either names its branch-local `LocalBindingId` or retains `_` as no
binding. The checker specializes payload types from the subject's canonical nominal arguments.
Retained places may bind only copyable payloads. Borrowed subjects bind every name as a borrow with
the subject capability. The control node separately records an explicit fallback and the implicit
non-match edge of `if is` without `else`, so MIR never reconstructs coverage from arm count.

Owned pattern subjects enter arm-specific residual storage. Named payloads transfer their drop
obligations to branch locals, while unnamed move-only payloads stay in an `EnumResidual` cleanup
target. Fallback and implicit non-match edges retain the complete active enum. Residual identity is
separate from value-temporary identity, allowing mutually exclusive arms to join as independent
conditional cleanups. Early `return` and postfix propagation see the same residual state as normal
statement completion. A source fallback after exhaustive explicit arms is fully checked but is
excluded from runtime state and enclosing-loop joins.

When a selected enum family owns a drop body, a pattern that transfers a move-only payload stores
that exact `DropId` as a before-transfer operation. The drop body therefore observes complete
`Self` once, before the payload leaves, and residual cleanup cannot call it again on partial
storage. If every named payload is copyable, the pattern copies those bindings and retains the
complete enum for ordinary value cleanup instead.

Generic inference may project a statically known expected outcome shape to collect payload
constraints. It completes the unique substitution before checked injection nodes are built.
Contextual tag literals contribute no payload constraint, and injection never feeds a guessed type
back into callable selection.

The program-wide instance-operation table owns normalized target patterns, binder refinements,
retained requirements, member identities, and overlap rejection. Body checking queries it for
source-defined indexing and the permitted one-step receiver coercion. A unique direct operation
outranks coercion-derived candidates; peers are ambiguous. The resulting checked place freezes the
selected dispatch and canonical declaration-generic arguments together, so instantiation and MIR
cannot rescan instance declarations or derive priority from source order. Conditional index and
coercion requirements recursively call that same selector under an active-predicate guard; there
is no weaker requirement-only operation registry.

The program-wide construction-surface table separately maps each inherent target family to its
validated `construct` declaration. Named construction calls resolve module and type owners to
semantic identity, apply member visibility at the use module, and then enter the common declared
call planner. The table remains in `CheckedProgram`, so editor queries, instantiation, and body
checking cannot invent independent construction-member indexes.

Type checking selects either a direct callable or an exact abstract requirement. When generic
substitution makes an abstract receiver concrete, the checking-owned `ConcreteDispatchResolver`
resolves that requirement once through the retained interface-implementation and instance-operation tables.
Its ordered plan distinguishes direct callable bodies, compiler primitives, and indirect
callable-value invocation; coercion-plus-operation evidence is not flattened. MIR and later stages
have no requirement or interface-implementation dispatch API.

## Target Program

`TargetProgram` owns the selected target and exact toolchain capability validation for the complete
checked compile unit. It is the common public acceptance boundary for `check`, `build`, and `run`
under one target and toolchain snapshot. A library-only `check` may stop there without inventing an
executable entry. Frontend-only experiments remain internal tests and never create a second public
language subset.

## Executable Program

Entry-driven instantiation produces the only reachable callable graph. A monomorphized callable
key contains semantic callable identity and one canonical substitution covering both its owner and
callable generic domains. Receiver type is derived from the owner declaration and is not duplicated
in the key. Missing, extra, duplicate, or symbolic arguments are integrity failures.

The checked-program layer owns concrete dispatch resolution because it already owns interface-implementation,
instance-operation, and recursive requirement-proof authorities. Specialization supplies one
concrete enclosing substitution and receives an ordered plan containing direct callable steps,
compiler primitives, or a concrete callable-value subject and contract. Composite structural
evidence such as coercion followed by built-in indexing remains composite. The executable-program
layer resolves a callable-value subject to its generated closure body and enqueues that body; MIR
cannot repeat requirement proof or interface-implementation selection. Callable contracts are not erased
runtime types, so no indirect callable ABI or vtable enters MIR.

MIR construction and linkage consume this graph. They cannot build parallel callable indexes.
Runtime symbol spelling is generated after item selection and cannot be used to find a semantic
item. `build` and `run` cannot reject a source-language construct that the corresponding
`TargetProgram` accepted; a later failure is an internal compiler or output-system failure, not a
second language diagnostic.

## MIR and Machine Program

MIR represents control flow, places, initialization, moves, loans, regions, cleanup, calls, and
outcomes for concrete executable items. Calls target monomorphized item IDs. MIR construction does
not receive AST, a resolver, rendered types, or runtime names.

Function-local locals, drop flags, places, SSA values, operations, and blocks occupy distinct dense
identity domains. Typed block parameters carry merge values, and every block has one exact
terminator. Conditional destruction branches on a drop flag; representation switches inspect
enum, optional, or fallible storage without moving it. A consuming builder is the only mutation
path inside the MIR crate. `MirProgramBuilder` owns the exact executable environment and is the sole
whole-function semantic validation boundary; function construction does not validate and then ask
the program builder to repeat the same walk. Validation checks concrete projections and
aggregates, operation types, complete reachable control flow, typed edge arguments, SSA dominance,
switch subjects, semantic call targets, and terminal results before a `MirProgram` can escape.

Machine lowering projects validated MIR into ABI storage and target-independent operations.
Target code generation consumes only the machine program, target description, and one-way linkage
table. Optimizations may replace ordinary operations with constants but cannot define source
semantics. In particular, built-in `error` values use ordinary value and call paths.

The machine program closes compiler-propagated ambient capabilities before target selection. One
fixed-point algorithm produces separate allocation and process tables across primitives, ordinary
calls, user drop, and hidden argument-pack callbacks or destructors. Explicit `using` selections
satisfy an allocation-dependent callee without propagating that allocation requirement; process
state remains ambient. ARM64 lowering consumes both tables and cannot rediscover context
requirements from operation shapes.

On ARM64-Darwin, `x9` is the fixed allocation-context lane and `x10` is the fixed immutable
process-context lane. Both are excluded from general allocation together with argument, result,
compiler-scratch, and platform-reserved registers. Virtual values use the disjoint `x11`-`x15`
and `x19`-`x28` pools. The allocator derives
call-crossing ranges from recorded call positions, confines them to callee-saved registers or
spills, and reports the exact preservation set to fixed-frame planning.

One interned machine-layout store is the authority for field offsets, outcome payload offsets,
aggregate size and alignment, argument and return classification, active-payload validation, and
drop-glue addressing. Optional and fallible layers receive distinct semantic type identities but
use the same recursive binary tagged-union layout operation. Code generation cannot calculate a
second layout from rendered types or special-case a different tag width for calls.
Stored scalar size and alignment are target facts in that layout store. ABI argument extension is
a transport projection of an existing layout and cannot mutate or replace the stored type layout.
The target description supplies endianness and integer width, while signed semantic types require
two's-complement interpretation. Machine lowering cannot select an alternative signed
representation or defer that choice to individual primitives.
Syntax retains unary negation and integer literal nodes separately. Contextual checking may
normalize that exact grouped shape into one signed integer constant, with a source origin covering
both syntax nodes. MIR never receives an out-of-range positive constant for the signed-minimum case.
Checked integer shift nodes distinguish left, unsigned-right, and signed-right operations and
retain the validated source width. Machine lowering emits an explicit count-range trap before a
target shift and cannot inherit the target instruction's count masking behavior.
Checked comparisons retain source operand order separately from strict-order reversal and result
negation. Each operand records whether lowering borrows a place or temporary, uses a readonly
borrow, weakens a readwrite borrow, and invokes one selected coercion. The plan also retains either
primitive implementation or exact static dispatch. Machine lowering always evaluates source nodes
left-to-right before preparing and arranging the semantic operands; it performs no operator or
coercion lookup. Short-circuit logic is a control operation whose ownership state joins the
executed RHS edge with the bypass edge; it never enters an eager binary-operation lowering.
Checked division and remainder nodes retain signedness and width. Machine lowering emits zero and
signed-minimum/`-1` guards before either operation and cannot inherit a target's overflow result or
remainder convention.
Checked assignment and compound assignment each own one target-place plan and one right-hand-side
expression; compound assignment additionally owns the selected numeric operation. Simple named-
place assignment already records old-value destruction as a `BeforeStore` cleanup schedule and
uses the same ownership-state transition for reassignment, reinitialization, and partial-field
repair. Integer compound assignment uses the same closed arithmetic selector as ordinary integer
expressions, but remains one control operation rather than a desugared expression tree. Ownership
analysis visits its RHS first and then requires the target to be initialized. MIR emits RHS
evaluation before the place plan and emits that plan only once. No stage expands compound
assignment into source-shaped duplicate target expressions.

Checked static calls retain one exact dispatch identity, canonical generic substitution, optional
receiver, and source-ordered arguments. Body checking resolves and proves that plan once. Ownership
and MIR visit a callable value first when present, then the receiver, then arguments from left to
right; they do not re-run generic inference or requirement proof. After ownership attaches cleanup,
one program-wide provenance fixed point interprets the same immutable HIR. It retains
projection-sensitive value origins, maps effective callable summaries through receiver and
argument positions, and keeps caller-visible input origins separate from compiler-owned
current-allocation dependence. The final `ProvenanceTable` is dense by callable, body, and checked
node. Return validation rejects storage outside authored or inferred contracts, including
temporary receiver results, and intersects implementation origins with the selected interface
method contract. This post-body authority does not change call selection identity or evaluation
order.

Callable boundaries distinguish two input channels. The carried channel describes loans and
storage already represented by the receiver or argument value. The invocation-place channel
describes only an implicit borrow created to call through an owned place. A result may retain the
second channel only when its declared representation contains an explicit borrow slot; raw
pointers and scalar-only owned storage can preserve storage authority without borrowing the local
allocator or container place. Nominal traversal substitutes declaration binders and inspects
fields, variant payloads, and concrete generic arguments. Unresolved generic and associated result
types remain carried values rather than acquiring the invocation lifetime. Callable summaries are
flattened only at the boundary, keeping their origin domain finite under recursive calls and loops;
aggregate values inside a body remain projection-sensitive.

A following program-wide loan analysis consumes that provenance authority and the immutable
cleanup schedules. Reverse structured liveness computes source-level last uses for checked places
and node temporaries; forward flow assigns stable loan identities, maps them through aggregate and
call results, and retains reborrow ancestry. Canonical loan roots distinguish owned places from
external storage reached through input borrow carriers. Named fields alone prove disjointness;
indexes and other computed projections remain conservatively overlapping. Type-owned drop bodies
keep the loans stored in their value live until their scheduled destruction action, while plain
non-owning fields do not invent a destructor use. The resulting `LoanTable` is dense by body and
node, and MIR consumes it without deriving borrow ranges from machine addresses or source syntax.

Interpolation lowering owns its partial `String` as an ordinary MIR temporary. Recoverable exits
use normal cleanup edges, while the shared safety-trap operation has no cleanup edge. Interpolation
cannot install a special failure rule for bare calls or forced unwrap.

Authored string-literal expressions retain distinct semantic and source identities through checked
IR. Machine constant layout may pool or overlap their decoded static byte ranges and rewrites each
slice constant through the construction-only static-data plan. Pooling never merges semantic nodes or becomes a basis
for type, equality, or editor identity.

Zero-sized types retain logical initialization, ownership, element counts, and drop operations in
MIR. Machine lowering erases only their storage and transport; it cannot erase evaluation or
destruction merely because a layout has size zero.
MIR place identities and projections remain authoritative for borrow conflicts. Machine lowering
may coalesce zero-sized storage or synthesize aligned non-null borrow addresses, but neither it nor
an earlier semantic stage may merge places by comparing those addresses.

One target-independent trap operation represents always-on safety failure, including failed
postfix `!`. Machine lowering does not route that operation through standard-library formatting,
stderr, entry-wrapper failure, or cleanup edges.

## Dependency Enforcement

The workspace encodes the dependency direction in crate boundaries. Executable architecture tests
freeze the reviewed production dependency sets for the language, model, declaration, target, MIR,
machine, ARM64, and Mach-O layers. They also reject source-index lookup in standard-role and
primitive-role selection, repeated package-target validation, standard-API spelling checks in
editor mutations, duplicate built-in type definitions, and any direct language-server dependency
on checking, declaration, source-index, or target-program storage. Type-resolved Clippy restrictions,
rather than source-text scans, prevent downstream code from naming semantic construction
authorities, transactions, closure construction sequences, or persistent collections through an
alias or a fully qualified path. The broader architecture review rejects:

- source or syntax types in semantic identity
- rendered-name type equality
- source-range semantic lookup after syntax binding
- resolver iteration used for candidate selection
- dispatch outside checking or instantiation
- AST or resolver inputs to MIR and code generation
- runtime-name lookup of semantic targets
- multiple executable-program registries
- compatibility imports from archived compiler code

New responsibilities receive crates only after their required specification gate closes. Empty
crate scaffolding is not evidence that a responsibility has been designed.

## Error-Tolerant Tooling

Editor analysis may retain explicit invalid syntax and immutable semantic evidence from the current
generation. It never converts incomplete source into a second successful semantic model. Name and
body recovery classify every declared body as accepted or rejected, and each rejection owns the
source reason that makes its absent facts legitimate. The session owns the sole stored authority
and projects it once into `SemanticEvidenceView`; a successful target uses that same contract.
Analysis exposes semantic capabilities through one sealed query boundary over the view. Hover, completion, definition,
implementation, references, rename, tokens, signature help, inlay hints, diagnostics, and code
actions consume those query outcomes rather than maintaining feature-local recovery adapters.
Every generation first freezes its complete open-document overlay; package resolution, discovery,
syntax, and semantic analysis all consume that one value.

An authored declaration-authority violation may enter an editor-only admission path when the
declaration graph remains structurally valid. One whole-graph admission scan classifies every
construction, instance, interface implementation, and drop independently of validation traversal order.
Unauthorized containers and locally invalid construction or drop surfaces stay available as lexical
body owners but are excluded from global candidate indexes, overlap validation, destruction, and
provenance selection. The analysis-only declaration and prepared-program types cannot enter the
production checking transition. Their sole body endpoint stops before ownership and provenance.
Declaration validation constructs either a declaration-only value or the distinct body-analysis
input type; no public checking entry accepts the former and no caller-controlled capability test
opens that boundary.
Body checking is transactional per body: every body receives either typed evidence or a rejected
evidence value, while independently successful bodies and their source projections remain usable
when another body fails. Executable dispatch and target closure remain unavailable. Type-store,
copyability, and closure-table rollback boundaries
are captured together. Each journaled copyability or closure authority issues a fresh opaque
transaction identity and validates it before commit or rollback; a token from another store or an
earlier transaction cannot control the active journal. Transaction identities are construction
state and never survive in a checked program or recovery snapshot.

Full semantic-token results retain that snapshot identity at the protocol boundary. The language
server passes the exact accepted analysis generation to the neutral LSP encoder as an opaque
`resultId`; a later accepted document transition therefore cannot be mistaken for the earlier token
set. The encoder still owns only legend mapping and delta encoding and never imports workspace
state.

Syntax-owned completion is a separate analysis input from semantic namespace completion. It reads
the immutable CST and source at the requested byte position and emits only grammar-fixed contextual
keywords. Those are currently top-level `test` and intrinsic requirement `copy`; declaration
identities and automatic imports remain in the semantic completion path. This allows header-syntax
failures such as a partial `where co` to return a valid keyword without constructing a fake semantic
program. The LSP adapter maps the compiler's closed `Keyword` category and never supplies snippets
or its own keyword table.

Source mutation features share one protocol-independent edit value containing an exact `SourceId`,
byte range, and replacement. Analysis binds a complete edit set to its immutable source snapshot and
derives the only candidate overlay that workspace compilation may consume. The resulting candidate
must retain that exact overlay and pass the whole semantic query seal. A consumed
`ValidatedSemanticMutation` owns the original snapshot relation, ordered edits, and compiled
candidate; LSP projection receives only that value and therefore cannot substitute an unrelated
snapshot, edit set, or successful candidate. Overlapping edits, missing sources, invalid UTF-8
boundaries, source ownership failures, invalid syntax origins, missing semantic domains, and source
projection issues cannot reach the client. Checked body completion alone is not publication
authority. Target
construction is required only for target-specific mutations. Automatic-import code actions select
the current compiler diagnostic by stable rule code and exact source subject, reuse the semantic
import candidate attached to completion, and publish it only when the ordinary compiler accepts
the candidate overlay. The adapter ignores client diagnostic contents and never derives edits from
diagnostic messages or help strings.

Each phase-specific failure retains its complete evidence contract and every diagnostic that
explains a rejected domain. The session composes those contracts into the one semantic-result sum.
An interface-implementation-table failure owns declaration
analysis containing the graph, monotonic type store, and source index, but no interface-implementation table,
construction surface, name result, or checked body. While selecting
required interface methods, the checker captures every missing method after applying the same
interface, target, associated-type, and callable-generic substitutions used by signature
compatibility. The code-action layer renders those typed contracts through canonical presentation,
selects an existing private instance fragment when the implementation is separated, inserts all
missing methods there with diverging `std/process.abort()` bodies, and obtains the import edit in
that same source from semantic automatic-import completion. An inline instance remains the edit
destination when no private fragment exists. The shared speculative transaction publishes the action only
when the whole edited program reaches checked semantics; an unrelated target-boundary failure does
not invalidate a source-semantic repair.

A failed postfix-propagation check may retain a separate `OutcomeContract` interruption only after
the operand's immediate optional or fallible layer is typed. The interruption owns that layer and a
proposed canonical result identity. Before that type crosses the checking boundary, recovery
projects it and its transitive structural dependencies into a self-contained `TypeProjection`.
Analysis renders through that projection rather than observing the monotonic checker store reached
by the failed body. It locates the editable result annotation by the declaration kind's direct CST
path; it does not search the declaration range for a nested `CallableTail`. Fixed-result and
grammar-restricted operators are explicitly non-editable. The shared speculative transaction
remains the final authority, so a contract change that creates a public-call, interface implementation,
provenance, or body error is not exposed.
Callable inference distinguishes a complete-result context from a postfix-propagation payload
context. The latter projects exactly one statically declared optional or fallible layer from a call
result and constrains its payload without materializing the enclosing callable's result type on the
operand. Typed places and non-generic calls are checked without that premature wrapper. This lets a
missing inner layer reach `OutcomeContract` even when another outcome layer already exists, while
generic calls retain payload inference and unconstrained result shapes remain errors.
If the enclosing result contains both outcome layers, the call declaration's immediate layer
selects the matching expected propagation payload through the ordinary expected-type planner.
This keeps optional and fallible generic calls deterministic without ranking one wrapper globally.

That declaration-kind path now belongs to one `CallableSourceProjection` authority shared by
source mutation and inlay presentation. It selects the semantic callable's exact declaration from
its `SourceIndex` origin, exposes an editable result node only when that callable grammar permits
replacement, and separately exposes the end of every source-visible result. Ordinary callable
tails are traversed directly through their declaration or method signature; nested closure tails
are unreachable. Coercion, index, and expansion results therefore receive the same provenance-hint
placement without making fixed index results editable.
Requested inlay ranges remain half-open after UTF-16 validation and byte projection. Analysis owns
that containment rule, so a hint exactly at the range end is excluded without protocol-specific
filtering.
Half-open containment and overlap for source bytes belong to `TextRange`. Code-action matching adds
one explicit point-query rule for an empty editor selection; it does not turn adjacent non-empty
ranges into overlaps or duplicate interval arithmetic in the protocol adapter.
`CodeActionParams` also retains the client's optional hierarchical kind filter as the typed
question “are quick fixes requested?”. A negative answer returns an empty result before semantic
lookup or speculative compilation; action-family policy does not leak into JSON handling.
