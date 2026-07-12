# Nocter Continuation TODO

This file is the compiler handoff point for the next session.
Long-lived maintenance rules live in `AGENTS.md` and `docs/maintenance.md`.

## Current Repository State

Adopted user decisions:

- Continue the self-contained backend path; do not switch to LLVM for the current compiler line.
- Keep runtime safety checks always enabled; remove them only when the compiler can prove they cannot trap.
- Treat ordinary allocation failure as recoverable failure, not implicit abort.
- Use an owned `String` direction based on pointer, length, and capacity, implemented as an ordinary standard-library type.
- `.nocter/std/string.nct` now uses the ordinary `ptr`, `len`, and `capacity` representation with private fields; allocation-backed mutation remains unsupported until target allocation and aggregate lowering are in place.
- Do not add a runtime GC.
- Lower generics through monomorphization.
- Prefer static trait dispatch; require an explicit dynamic-dispatch design if it is added later.
- Keep the initial standard library small: trap/unreachable, process/stderr/syscall wrappers, allocator, owned `String`, and formatting support before larger collections or file APIs.

Recommended next implementation order:

1. Follow `compiler/docs/interpolation-lowering.md`: keep bare interpolation lowering disabled until an explicit allocator source is designed, and first make explicit `std/string` construction plus `std/fmt.append_*` calls buildable.
2. Use the resolved-type function ABI helper from IR/backend planning before adding aggregate storage code.
3. Add only the backend prerequisites needed by the explicit string path: aggregate `String`/`Allocator`/`RawBuffer` storage, aggregate stack-backed `var`, and owned aggregate return/move handling. Loaded imported scalar calls, scalar/view arguments, local scalar borrow arguments, scalar/view `var` plus simple assignment, and fallible propagation for the current scalar/view/void call subset are already buildable.
4. Lower interpolated strings only after the explicit standard-library construction path builds.
5. Defer broad control flow, aggregate values beyond the explicit `String` path, general mutable storage, ownership/drop lowering, and optimizer work until their ABI/storage rules are designed.

Recent committed work:

- Current checkpoint: lower from_addr pointer aggregate fields
  - lowers aggregate struct literal return fields whose ABI type is `Pointer` when the source value is the closed `std/ptr.from_addr(usize)` primitive
  - stores the lowered address word through the existing `StoreAggregateUsize` return-storage path, matching pointer-sized ABI fields
  - adds imported Nocter-home lowering coverage so `pub(nocter)` pointer construction is tested from a std module instead of user app code
  - keeps general pointer expressions, aggregate call-result slots from source, aggregate-backed `var`, aggregate load/copy, and owned aggregate moves disabled
- Current checkpoint: lower usize-field aggregate returns
  - lowers non-entry indirect aggregate `return Struct{ ... }` expressions when every stored field is a `usize` ABI field
  - emits `StoreAggregateUsize { destination: Return, offset, value }` in source field order using ABI-computed struct field offsets, followed by `Return`
  - reuses existing `usize` expression lowering for field values, so constants, locals, arithmetic, and lowerable `usize` calls can feed aggregate return fields
  - keeps pointer fields, `std/ptr.from_addr`, aggregate call-result slots from source, aggregate-backed `var`, aggregate load/copy, and owned aggregate moves disabled
- Current checkpoint: track indirect aggregate return types in IR signatures
  - adds `Type::Aggregate { layout }` as an IR return/signature type for ABI-indirect source values
  - makes function signature indexing reuse resolved ABI classification so 24-byte structs stay visible as indirect aggregate returns instead of being dropped from `FunctionSignatures`
  - allows non-entry function return-type lowering to identify indirect aggregates, then reports explicit aggregate-body diagnostics because source aggregate value return lowering is still disabled
  - keeps aggregate constructors, aggregate call-result slots from source, aggregate-backed `var`, aggregate load/copy, and owned aggregate returns/moves disabled
- Current checkpoint: store aggregate usize fields
  - adds `AggregateLocation::{Return, Slot}` and `StoreAggregateUsize` for 8-byte field writes into indirect return storage or reserved aggregate slots
  - adds ARM64 unsigned-offset `str xN, [xM, #imm]` encoding for aggregate return storage writes through `x8`
  - validates aggregate slot field offsets for 8-byte alignment and slot bounds before emitting stack stores
  - covers aggregate return stores, aggregate slot stores, and the `x8` return-storage write path in backend tests
  - keeps source lowering for struct literals, aggregate constructors, aggregate load/copy, aggregate-backed `var`, and owned aggregate returns/moves disabled
- Current checkpoint: add aggregate indirect-return call emission
  - adds a normal `CallAggregate { destination_slot, target, arguments }` IR instruction for calls that return indirect aggregate values
  - emits the reserved aggregate destination slot address into Nocter ABI register `x8` before the call
  - reuses existing scalar spill/reload and scalar argument staging around aggregate calls
  - makes reachability collect aggregate call targets so callees are lowered and patched like other calls
  - keeps source lowering for aggregate constructors, aggregate load/store, aggregate-backed `var`, and owned aggregate returns/moves disabled
- Current checkpoint: connect aggregate frame reservations to IR
  - adds a frame-only `ReserveAggregateSlot { slot_index, layout }` IR marker carrying ABI `ValueLayout`
  - makes `backend/frame.rs` collect aggregate slot requests from top-level instructions, nested `if` branches, and catch failure handlers
  - deduplicates repeated slot indexes with the same layout and reports conflicting slot layouts as backend diagnostics
  - treats reservation markers as codegen no-ops, non-terminating control-flow instructions, and reachability-neutral instructions
  - keeps aggregate load/store, aggregate-backed `var`, and owned aggregate returns/moves disabled until lowering can emit and use the reserved slots
- Current checkpoint: use ABI counts for IR parameter planning
  - changes IR function parameter lowering to count parameter ABI words through `abi_value_from_type_expr`
  - shares the ABI module's eight-register argument window constant instead of maintaining a separate IR-only limit
  - adds IR lowering coverage for rejecting a function whose direct `&str` and scalar parameters require nine ABI words
- Current checkpoint: expose ABI passing counts
  - adds ABI helper methods for parameter passing and return passing over the existing direct/indirect value classification
  - counts indirect parameters as one ABI pointer word and detects when parameter ABI words exceed the eight-register window
  - exposes indirect-return detection so later aggregate return lowering can reserve caller storage and pass the result pointer through the ABI-defined hidden return register
- Current checkpoint: classify function signatures for ABI planning
  - adds `AbiValue`, `AbiParameter`, `AbiReturn`, and `FunctionAbi` wrappers over ABI type, layout, and direct/indirect classification
  - adds `function_abi_from_signature` so later IR/backend planning can classify parameters and return values without duplicating type/layout logic
  - treats `void` and `never` returns as no-value ABI returns while leaving unsupported fallible/optional/generic shapes explicit at the ABI helper boundary
- Current checkpoint: move std String to pointer/length/capacity
  - changes `.nocter/std/string.nct` from the temporary `len`-only skeleton to private `ptr`, `len`, and `capacity` fields
  - constructs `empty()` through the restricted `std/ptr.from_addr` primitive instead of compiler-special-casing `String`
  - adds distributed std check coverage for the public `empty`/`view` path and confirms user modules cannot construct `String` by hidden fields
- Current checkpoint: reserve aggregate frame slots
  - extends `backend/frame.rs` with optional aggregate stack slot requests based on ABI `ValueLayout`
  - keeps existing scalar spill and argument staging placement unchanged
  - places aggregate slots above argument staging and below saved `x30`, preserving requested alignment up to the v0 16-byte stack alignment
  - leaves the new aggregate slot API disconnected from IR lowering until aggregate locals/returns are introduced
- Current checkpoint: connect ABI layout to resolved source types
  - adds `abi_type_from_type_expr` for resolved primitives, raw pointers, borrows, `&str`, slices, aliases, and non-generic structs
  - keeps fixed arrays, optionals, fallible layouts, enums, traits, and generics explicitly unsupported at this ABI helper boundary
  - verifies resolved `ptr`/`len`/`capacity` structs classify as 24-byte indirect values
  - verifies aliases to `str` under borrow lower to the two-word `&str` ABI view
- Current checkpoint: add ABI layout foundation
  - adds initial `abi` helpers for scalar, pointer, borrow, `&str`, slice, and struct value layouts
  - classifies values as direct when they are at most 16 bytes and indirect when larger, matching the Nocter ABI v0 rule
  - confirms a `String`-like `ptr`/`len`/`capacity` struct is 24 bytes and therefore indirect
  - represents raw pointer type expressions as explicit type-checker pointer types instead of lossy named strings
  - adds type-checker coverage for raw pointer value types and pointer argument mismatches
- Current checkpoint: implement borrow argument lowering
  - adds explicit `&expr` / `&+expr` AST nodes, parser support, formatting, JSON AST output, resolver traversal, LSP hover traversal, and type-checking
  - requires `&+` operands to be writable local `var` bindings in the current narrow checker
  - lowers local scalar borrow arguments for normal calls as one ABI word by passing the address of the caller spill slot
  - keeps tail calls with borrow arguments disabled to avoid passing addresses into a caller frame that has already been released
- Current checkpoint: lower scalar var assignment
  - type-checks assignment to immutable `let` bindings and parameters, plus assignment value type mismatches
  - lowers stack-backed scalar/view `var` bindings and simple whole-binding `=` assignment for `i32`, `u8`, `usize`, `bool`, `&str`, and slices in the current leading-statement subset
  - adds `nocter run` coverage for scalar assignment and reassigned `&str` output through `write_text_raw`
- Current checkpoint: execute Hello through distributed std
  - verifies `from std/io import print` plus `print("Hello")?` with the real distributed `.nocter/std` and `arm64-darwin` target overlay
  - covers the current Hello path end to end through `nocter run`, `std/io.print`, `std/io_impl.write_text_raw`, fallible `?`, and generated executable stdout
- Current checkpoint: cover non-i32 catch execution paths
  - adds native execution coverage for `catch` success and failure recovery across `u8`, `usize`, `bool`, `&str`, and `void`
  - keeps the tests in the narrow v0 lowering subset by avoiding broad non-terminal control flow and excessive local ABI pressure
- Current checkpoint: implement fallible catch lowering
  - adds dynamic `error.code` and `error.message` payload access for `catch` blocks
  - lowers `catch` for the current `i32`, `u8`, `usize`, `bool`, `&str`, slice, and `void` fallible call subset
  - avoids compiler magic around `make_error`; failure payloads come from the built-in `error` value shape and loaded `Error.new` constructor path
- Current checkpoint: support `usize` arithmetic and shifts
  - adds IR instructions and lowering for buildable `usize` `+`, `-`, `*`, `/`, `%`, `<<`, and `>>`
  - supports same-file and loaded imported `usize` normal calls inside lowerable `usize` arithmetic and shift expressions through the existing temporary staging path
  - emits runtime traps for unsigned addition carry, subtraction borrow, multiplication high-word overflow, zero divisors, and shift counts greater than or equal to 64
  - lowers `<<` through ARM64 `lslv` and `>>` through ARM64 `lsrv` for unsigned `usize`
  - adds ARM64 encoder, IR lowering, codegen, CLI build, and CLI run coverage, including overflow/division/shift trap paths
- Current checkpoint: document interpolation lowering direction
  - records that bare interpolation still cannot lower without an explicit source-level allocator
  - sets the next implementation path as explicit `std/string` construction plus `std/fmt.append_*` calls before lowering bare interpolation syntax
  - lists the remaining backend prerequisites: aggregate storage, aggregate stack-backed `var`, borrow arguments, and owned aggregate returns/moves
- Current checkpoint: add bool scalar call arguments
  - lowers `bool` parameters and calls whose arguments include `bool`, preserving ABI argument indexes for mixed scalar parameter lists
  - extends typed IR call arguments and ARM64 staging so `i32`/`bool` use W registers and `usize` uses X registers
  - adds IR and CLI run coverage for a mixed `i32`/`bool`/`usize` call returning `bool`
- Current checkpoint: add typed scalar call arguments
  - lowers `usize` parameters and calls whose arguments include `usize`, while preserving ABI argument indexes for mixed `i32`/`usize` parameter lists
  - changes IR call arguments from `I32Value` to typed scalar arguments and updates ARM64 argument staging to use W registers for `i32` and X registers for `usize`
  - adds IR and CLI run coverage for a mixed `i32`/`usize` call returning `usize`
- Current checkpoint: add target `std/io_impl` skeleton
  - adds `.nocter/targets/arm64-darwin/std/io_impl.nct` with `pub(nocter)` raw file-descriptor helpers
  - updates common `.nocter/std/io.nct` to keep the public `File` API while obtaining standard stream and future opened descriptors through the target overlay
  - adds distributed-home coverage that user code cannot import `std/io_impl`
- Current checkpoint: add private `File` close-on-drop state
  - adds `close_on_drop` to `.nocter/std/io.nct`'s private `File` representation so borrowed standard streams can be distinguished from future owned handles
  - adds distributed-home coverage that user code cannot construct `File` through hidden fields
- `Add std io file method surface`
  - adds `.nocter/std/io.nct` skeleton methods for `File.open`, `File.read`, `File.write`, and `File.write_text`
  - keeps runtime I/O broadening deferred while checking the user-facing method surface through the distributed Nocter home
- Current checkpoint: keep `std/process` target-specific
  - keeps user-facing imports stable at `std/process`
  - places the physical `std/process.nct` implementation in the active target overlay because process context and termination depend on the process ABI
  - avoids a common wrapper that only delegates to a target-specific `process_impl` module
- Current checkpoint: add backend usize scalar foundation
  - adds IR and ARM64 lowering for annotated `usize` locals, non-entry `usize` returns, same-file and loaded imported normal calls returning `usize`, and `usize` comparisons in lowerable bool/terminal-if positions
  - widens framed scalar spill slots to 8 bytes so `i32`/`bool` and `usize` locals share the same conservative spill/reload path
  - adds CLI build/run coverage for same-file and imported `usize` call conditions
- Current checkpoint: cover imported scalar build variants
  - adds `nocter build` coverage for loaded imported alias calls, imported bool conditions, and imported nested arguments
  - verifies each generated executable returns the expected status code
- `Canonicalize imported call targets`
  - adds a declaration-span to function-name map inside IR lowering so imported aliases lower to the imported declaration name instead of the local alias
  - covers imported `bool` normal calls in terminal conditions at the IR and `nocter run` layers
  - covers imported nested arguments and imported alias calls through `nocter run`
- `Lower loaded imported scalar calls`
  - changes reachable lowering to lower loaded imported function definitions from the compile-unit function index
  - narrows imported-call diagnostics to unresolved imported placeholders
  - adds IR, `nocter build`, and `nocter run` coverage for a loaded imported `i32` call returning 42
- `Attach targets to IR functions`
  - adds `Function.target` so lowered function definitions can be keyed as same-file or imported definitions
  - changes backend function offset registration to derive `FunctionSymbol` from `Function.target`
  - keeps existing lowered functions same-file for now while preparing imported definitions to resolve imported call patches
- `Collect reachable IR call targets`
  - changes IR reachability collection from same-file function names to full `CallTarget` values
  - keeps imported targets in the queue representation while still deferring imported definition lowering at the current boundary
  - removes the now-unused `CallTarget::same_file_name()` helper
- `Resolve imported IR call targets`
  - threads resolver output into IR entry/function lowering through `LoweringContext`
  - changes normal-call, bool-call, and tail-call lowering to derive `CallTarget` from resolver symbols instead of always creating same-file targets
  - verifies that direct lowering can emit `CallTarget::Imported` before the public build/run pipeline enables loaded imported call lowering
- `Index IR functions by call target`
  - adds a lowering-time function index that keys root functions as `CallTarget::SameFile` and imported file functions as `CallTarget::Imported`
  - builds `FunctionSignatures` from the compile-unit function index, so imported scalar function signatures are present before imported call lowering is enabled
  - kept reachable lowering limited to same-file functions until loaded imported call lowering was enabled
- `Key IR signatures by call target`
  - changes IR lowering return-type lookup from raw function names to `CallTarget` keys
  - keeps test-only same-file signature construction available while production lowering builds same-file `CallTarget` signatures explicitly
  - keeps imported call lowering disabled, but prepares return-type validation to distinguish same-file and imported call targets
- `Centralize backend function symbols`
  - changes ARM64 entry and function offset registration to derive backend `FunctionSymbol` through one helper instead of open-coding same-file names
  - keeps generated machine code unchanged while leaving a single extension point for future imported function definitions
- `Add imported IR call target`
  - adds reserved `CallTarget::Imported { source, name }` with loaded imported declaration source identity
  - maps imported IR call targets to backend `FunctionSymbol::Imported` without enabling imported call lowering yet
  - keeps same-file lowering and current `E8006` imported-call boundary unchanged
- `Type backend function symbols`
  - changes ARM64 codegen function offset lookup and call patches from raw function-name strings to an internal `FunctionSymbol` key
  - keeps current same-file call codegen behavior unchanged while making future imported symbols distinguishable before branch patching
  - removes the now-unused `CallTarget::name()` helper in favor of explicit target-to-symbol conversion
- `Collect imported call targets`
  - changes `ir/lower/imported_calls.rs` from a diagnostic-only traversal into an imported call target collector plus diagnostic conversion
  - records loaded imported calls by declaration `SourceId` and unloaded placeholder imports by import path
  - keeps the existing `E8006` imported-call backend boundary behavior unchanged
- `Extract IR call reachability`
  - adds `ir/lower/reachability.rs` for same-file call target discovery
  - keeps reachable lowering behavior unchanged while giving imported call lowering a narrower boundary to extend next
  - adds direct coverage for nested `Instruction::If` call target collection order
- `Constrain IR call reachability to same-file targets`
  - changes IR lowering reachability collection to queue only `CallTarget::SameFile` names
  - kept imported targets from being accidentally treated as local functions before full call-target reachability was introduced
- `Type IR call targets`
  - changes IR call instructions from raw callee strings to backend-independent `CallTarget::SameFile`
  - keeps same-file call lowering and ARM64 codegen behavior unchanged
  - prepares the IR surface for a future `Imported` call target without teaching backend code to infer source identity from plain names
- `Cover imported call build diagnostic`
  - adds CLI build coverage for reachable imported calls
  - confirms the build command reports `E8006` and leaves no executable when imported call lowering is unsupported
- `Diagnose imported call lowering boundary`
  - adds `ir/lower/imported_calls.rs` to detect imported call targets using resolver output before same-file call lowering
  - reports a dedicated `E8006` for reachable imported calls such as `from std/math import answer; answer()`
  - keeps imported call runtime lowering disabled while making the boundary explicit for the next backend step
- `Diagnose interpolated string lowering boundary`
  - detects interpolated string expressions inside lowered `let` initializers
  - reports a dedicated `E8008` explaining that backend lowering is waiting on explicit `std/string` allocation and `std/fmt.append_*` lowering
  - keeps interpolation parsing and type checking enabled while runtime construction remains disabled
- `Cover std fmt import graph`
  - adds frontend coverage for a user module importing `std/fmt`
  - confirms the imported standard-library graph can load `std/fmt`, `std/error`, `std/string`, and `std/mem`
  - keeps the test at the import/type-check layer; interpolated string runtime lowering remains disabled
- `Specify std string formatting boundary`
  - adds `.nocter/std/fmt.nct` with explicit append APIs for `&str`, `String`, `i32`, and `bool`
  - expands `.nocter/std/string.nct` from a placeholder owning type to the initial pointer/length/capacity ABI direction plus `empty`, `with_capacity`, `from_str`, `view`, and `push_str`
  - adds common `error` helper functions in `.nocter/std/mem.nct` for `"std.mem.out_of_memory"` and `"std.mem.invalid_argument"`
  - documents that `std/mem`, `std/string`, and `std/fmt` fail through the built-in `error` payload rather than domain-specific fallible error types
  - keeps interpolated string runtime lowering disabled until an explicit allocator source and backend storage/call prerequisites are implemented
- `Lower i32 shifts`
  - adds IR instructions and lowering for buildable `i32` `<<` and `>>`
  - supports same-file `i32` normal calls inside shift operands through the existing left-to-right temporary staging path
  - emits runtime shift-count traps for negative counts and counts greater than or equal to 32
  - lowers `<<` through ARM64 `lslv` and `>>` through ARM64 `asrv` for signed `i32`
  - adds ARM64 encoder, IR lowering, codegen, CLI build, and CLI run coverage, including negative and too-large count trap paths
  - keeps imported calls, aggregate arguments/returns, ownership/drop lowering, `var`/reassignment, and broader control-flow disabled
- `Add i32 arithmetic overflow traps`
  - emits signed-overflow traps for lowered `i32` addition, subtraction, and multiplication
  - lowers `+` and `-` through ARM64 `adds`/`subs` followed by a `b.vc` guarded `brk #0`
  - lowers `*` through signed 64-bit `smull`, sign-extension comparison, and a `brk #0` when the product does not exactly fit in `i32`
  - adds ARM64 encoder helpers and unit coverage for `adds`, `subs`, `smull`, `sxtw`, 64-bit `cmp`, and `b.vc`
  - adds codegen coverage and CLI run coverage for addition, subtraction, and multiplication overflow trap paths
  - kept shift lowering, imported calls, aggregate arguments/returns, ownership/drop lowering, `var`/reassignment, and broader control-flow disabled at that checkpoint
- `Lower i32 division and remainder`
  - adds ARM64 encoder helpers for `sdiv`, `msub`, and `brk`
  - adds IR lowering and ARM64 codegen for lowerable `i32` division and remainder
  - supports same-file `i32` normal calls inside `/` and `%` arithmetic expressions
  - keeps arithmetic expression evaluation left to right through the existing temporary staging path
  - emits zero-divisor and signed-overflow trap checks before ARM64 `sdiv`
  - adds IR lowering, codegen, CLI build, and CLI run coverage for user-visible `i32` division and remainder, including zero-divisor and signed-overflow trap paths
  - kept imported calls, aggregate arguments/returns, ownership/drop lowering, `var`/reassignment, broader control-flow, and overflow checks for `+`, `-`, and `*` disabled at that checkpoint
- `Add string interpolation front-end`
  - accepts `${...}` inside single-line and multi-line string source forms while keeping escaped `\${` as literal text
  - adds `InterpolatedString` AST nodes with source-preserving text and expression parts
  - parses interpolation expressions with the normal expression parser over their original byte spans
  - type-checks interpolated string expressions as `String!`
  - accepts interpolation parts of type `&str`, `String`, integer, and `bool`
  - reports `E0379` for unsupported interpolation part types such as arrays
  - traverses interpolation expressions during resolution, return/propagation checks, documentation collection, LSP hover collection, and IR call-containment analysis
  - keeps runtime lowering for interpolated string construction disabled until the standard-library formatting/allocation API is finalized
- `Add multi-line string literals`
  - adds shared string literal decoding for single-line and multi-line string literals
  - lexes multi-line `"""..."""` string literals as one `StringLiteral` token without emitting statement newlines for literal content
  - validates multi-line opening newline, closing indentation removal, final UTF-8 after escapes, and `\$`
  - diagnosed unescaped `${` as unimplemented string interpolation instead of accepting it as literal text at that checkpoint
  - updates comment scanning so `//` and `/* */` inside multi-line string literals do not count as comments
  - lowers static fallible failure reports from single-line or multi-line string literals through a loaded static `error` constructor call
  - kept general `&str` values, owned `String`, interpolation parsing/typechecking/lowering, imported calls, aggregate values, ownership/drop lowering, `var`/reassignment, and broader control-flow disabled
- `Lower i32 call arithmetic`
  - adds IR lowering and ARM64 codegen for lowerable `i32` subtraction and multiplication alongside existing addition
  - supports same-file `i32` normal calls inside `+`, `-`, and `*` arithmetic expressions, such as `return answer() * 2 - offset()`
  - keeps arithmetic evaluation left to right through the existing temporary staging path
  - adds IR lowering, ARM64 encoder, CLI build, and CLI run coverage for user-visible `i32` call arithmetic
  - keeps imported calls, aggregate arguments/returns, ownership/drop lowering, `var`/reassignment, and broader control-flow disabled
- `6a9553d Cover i32 comparison short-circuit calls`
  - adds IR lowering and CLI run coverage for short-circuit bool expressions that combine `i32` call comparisons with bool calls
  - covers terminal conditions such as `if answer() == 42 && ready()`
  - covers bool value materialization such as `let matched = answer() == 42 && ready()`
  - confirms the existing short-circuit branch lowering can consume staged `BoolValue::I32Comparison` conditions without additional backend work
  - keeps imported calls, aggregate arguments/returns, ownership/drop lowering, `var`/reassignment, and broader control-flow disabled
- `f3f9df9 Lower i32 call comparisons`
  - lowers same-file `i32` normal calls as `i32` comparison operands
  - supports `if answer() == 42`, `let matched = left() <= right()`, and `return left() < right()`
  - evaluates comparison operands left to right through the existing `i32` expression staging path
  - keeps imported calls, aggregate arguments/returns, ownership/drop lowering, `var`/reassignment, and broader control-flow disabled
  - kept unsupported `i32` call expressions such as `return answer() * 2` reporting an IR lowering diagnostic at that checkpoint
  - adds IR lowering and CLI run coverage for `i32` call comparisons
- `3ebddf6 Lower nested tail-call arguments`
  - lowers nested same-file `i32` tail-call arguments such as `return outer(inner())`
  - evaluates nested tail-call arguments left to right through the existing `i32` expression staging path
  - emits child calls before the final `TailCall`, then uses tail-call argument staging for the final branch
  - updates frame planning so `TailCall` argument locals are counted
  - keeps imported calls, bool/aggregate tail-call arguments, ownership/drop lowering, and broader control-flow disabled
  - adds IR lowering, frame planning, and CLI run coverage for nested tail-call arguments
- `1c8a66a Lower bool call comparisons`
  - lowers same-file bool-returning normal calls as atomic bool equality/inequality operands
  - supports `let value = ready() == true`, `return left() != right()`, and `if left() == right()`
  - stages bool call operands left to right before building `BoolValue::BoolComparison`
  - keeps compound bool comparison operands with calls, such as `(ready() && other()) == true`, disabled
  - adds IR lowering and CLI run coverage for bool call comparisons
- `8af4c6b Lower short-circuit bool value calls`
  - lowers same-file bool-returning normal calls in short-circuit bool value expressions
  - supports `let value = ready() && other()` and `return ready() || other()`
  - expands `&&` and `||` to nested `Instruction::If` nodes and materializes `true` or `false` into the destination bool location
  - keeps imported calls, broader control-flow, `var`/reassignment, ownership/drop lowering, and aggregates disabled
  - adds IR lowering and CLI run coverage for short-circuit bool value calls
- `803e63b Lower short-circuit bool condition calls`
  - lowers same-file bool-returning normal calls in terminal `if` `&&` and `||` conditions
  - expands short-circuit conditions to nested `Instruction::If` nodes so the right-hand call is only emitted in the branch where it should execute
  - updates reachable call-target collection to scan nested `Instruction::If` bodies
  - kept short-circuit value expressions with calls, such as `let value = ready() && other()` and `return ready() && other()`, disabled
  - adds IR lowering and CLI run coverage for `&&` and `||` condition calls
- `c8bffa3 Lower bool condition calls`
  - lowers direct same-file bool-returning normal calls in terminal `if` conditions
  - supports `if ready() { ... } else { ... }` and `if !ready() { ... } else { ... }`
  - stages the bool call result in a temporary scalar local before `Instruction::If`
  - kept short-circuit bool expressions with calls, such as `ready() && other()`, disabled until staging can preserve short-circuit evaluation
  - adds IR lowering and CLI run coverage for direct bool normal-call conditions
- `b017d59 Lower unary bool normal-call expressions`
  - lowers bool-returning normal calls under unary `!`
  - supports `let disabled = !ready()` and `return !ready()`
  - stages the bool call result in a temporary scalar local before materializing `BoolValue::Not`
  - keeps short-circuit bool expressions with calls, such as `ready() && other()`, disabled until staging can preserve short-circuit evaluation
  - adds IR lowering and CLI run coverage for unary bool normal-call expressions
- `b26f8b7 Lower bool normal calls`
  - adds `Instruction::CallBool` for bool-returning same-file normal calls
  - lowers `let value = ready()` when `ready` returns `bool`
  - emits bool normal calls with the existing framed normal-call sequence, scalar spill/reload, and `i32` argument staging
  - keeps calls directly inside conditions such as `if ready()` reporting `E8006`
  - keeps bool non-tail calls inside compound bool expressions such as `ready() && true` reporting `E8006`
  - adds IR lowering, frame planning, codegen, and CLI run coverage for bool-returning normal-call `let` initializers
- `19c4b92 Stage tail-call arguments`
  - lowers reordered `i32` tail-call arguments such as `return second(b, a)`
  - uses the existing frame argument staging slots for tail calls with arguments, then restores the frame before branching
  - keeps no-argument tail calls frameless
  - keeps nested tail-call arguments such as `return outer(inner())` reporting `E8006`
  - adds IR lowering, frame planning, codegen, and CLI run coverage for reordered tail-call arguments
- `ca2eef1 Lower nested i32 normal-call arguments`
  - lowers normal-call arguments through the same expression-to-value staging path used by additions
  - supports `let value = outer(inner())`, `let value = add(left(), right())`, and `return outer(inner()) + 1`
  - evaluates nested normal-call arguments left to right before the parent `CallI32`
  - keeps nested tail-call arguments such as `return outer(inner())` reporting `E8006`
  - adds IR lowering coverage plus CLI run coverage for nested normal-call arguments
- `9931bf9 Support multiple i32 normal-call result staging`
  - changes expression-to-value lowering to use a shared temporary allocator for each lowered expression
  - evaluates addition operands left to right and stages each normal-call result in a distinct temporary local
  - supports `return left() + right()`, `let value = left() + right()`, and nested additions such as `return (left() + right()) + base`
  - adds IR lowering coverage for temporary/local collision avoidance and CLI run coverage for multi-call additions
- `210d489 Generalize one-call i32 result staging`
  - generalizes one-call `i32` result staging for lowerable additions and grouped forms
- `00c3282 Add normal-call argument staging`
  - extends v0 frame layouts with stack-backed argument staging slots sized to the maximum `CallI32` argument count in a function
  - emits normal-call arguments by evaluating each `i32` argument into a staging slot, then loading `w0` through `w7` from those slots before `bl`
  - allows reordered parameter arguments for source-level normal calls such as `let value = second(b, a)`
  - kept tail-call reordered parameter arguments rejected at that checkpoint
- `6aca787 Lower source i32 normal-call subset`
  - lowers direct same-file `i32` normal calls to `CallI32` in `let` initializers
  - lowers simple `i32` return additions with one direct normal call by staging the call result in a temporary scalar local
  - keeps imported calls, aggregate args/returns, bool-returning normal calls, ownership/drop lowering, nested call arguments, and general condition calls disabled
- `07c80e9 Add backend normal-call foundation`
  - adds the backend v0 normal-call design, ARM64 frame/spill encoder helpers, fixed frame planning, framed prologue/epilogue emission, and hand-built IR `CallI32` codegen coverage
  - keeps source-level normal-call lowering disabled at that checkpoint
- `4fdbe41 Add build lowering for bool equality`
  - represents lowerable bool equality/inequality as `BoolValue::BoolComparison`
  - lowers bool equality/inequality when both operands are bool literals, bool locals, or grouped forms of those atoms
  - reports a dedicated `E8008` diagnostic when bool equality/inequality uses lowerable but non-atomic bool operands such as `!ready` or `ready && !blocked`
  - adds ARM64 Darwin codegen for `BoolComparison` using the existing bool register representation and `cmp`/conditional branches
  - adds CLI build/run and IR lowering tests for bool equality/inequality through the native backend path, plus unsupported compound bool equality diagnostics
  - updates implementation status and architecture docs to list bool equality/inequality over literal/local operands in the buildable bool subset
- `d5b1a89 Extract LSP document symbols module`
  - added `driver/lsp/symbols.rs`
  - moved document symbol construction out of `driver/lsp/mod.rs`
  - updated LSP architecture and roadmap notes for the symbols extraction
- `b666f99 Extract LSP completion module`
  - added `driver/lsp/completion.rs`
  - moved keyword and resolved symbol completion item construction out of `driver/lsp/mod.rs`
- `b505f4f Extract LSP hover module`
  - added `driver/lsp/hover.rs`
  - moved hover contents, hover symbol collection, documentation attachment, and resolved-reference hover labels out of `driver/lsp/mod.rs`
  - updated LSP architecture and roadmap notes for the hover extraction
- `6dde4a1 Extract LSP semantic tokens module`
  - added `driver/lsp/semantic.rs`
  - moved semantic token classification and encoding out of `driver/lsp/mod.rs`
  - updated LSP architecture and roadmap notes for the semantic extraction
- `b2643a7 Extract LSP diagnostics module`
  - added `driver/lsp/diagnostics.rs`
  - moved publishDiagnostics payload construction and diagnostic span conversion out of `driver/lsp/mod.rs`
- `deda50e Split LSP foundations and document maintenance policy`
  - moved the LSP server to `driver/lsp/mod.rs`
  - added `driver/lsp/protocol.rs` and `driver/lsp/documents.rs`
  - added `compiler/AGENTS.md` and `docs/maintenance.md`
- `2c73726 Track local symbols in resolver`
  - records local symbols and local identifier targets in resolver output
  - uses local symbols for LSP hover and go-to-definition
- `b318f0d Add basic LSP completions`
  - adds keyword and resolved symbol completions
- `16a13bb Add LSP document symbols`
  - adds document symbol support
- `2dc5785 Add LSP go to definition`
  - adds go-to-definition for resolved symbols

Known unrelated local user changes:

- None currently visible in `git status --short`.

Do not stage, revert, or modify unrelated files unless the user explicitly asks.

Current uncommitted compiler work:

- None expected after committing `Support usize arithmetic and shifts`.

## Verification Already Run

For backend `usize` arithmetic and shifts, from `compiler/`:

```sh
cargo check --quiet
cargo test --quiet usize
cargo test --quiet --lib
cargo test --quiet
./scripts/verify.sh
```

For imported scalar alias/bool/nested coverage, from `compiler/`:

```sh
cargo test --quiet imported_alias_call_uses_imported_declaration_name_as_target
cargo test --quiet lowers_imported_bool_normal_call_in_terminal_if_condition
cargo test --quiet run_command_returns_imported_alias_function_call_exit_code
cargo test --quiet run_command_returns_imported_bool_condition_exit_code
cargo test --quiet run_command_returns_imported_nested_argument_exit_code
cargo test --quiet --test cli_build build_command_lowers_imported_alias_i32_call
cargo test --quiet --test cli_build build_command_lowers_imported_bool_condition
cargo test --quiet --test cli_build build_command_lowers_imported_nested_argument
cargo fmt
./scripts/verify.sh
```

For the imported-call build diagnostic coverage, from `compiler/`:

```sh
cargo test --quiet --test cli_build build_command_lowers_imported_i32_call
cargo fmt
cargo test --quiet
```

For type IR call targets, from `compiler/`:

```sh
cargo test --quiet backend::frame
cargo test --quiet lowers_entry_i32_let_initializer_normal_call
cargo test --quiet backend::codegen::tests::generates_framed_i32_normal_call_from_hand_built_ir
cargo fmt
cargo test --quiet
./scripts/verify.sh
```

For same-file call reachability tightening, from `compiler/`:

```sh
cargo test --quiet lowers_entry_returning_same_file_function_call
cargo test --quiet lowers_imported_i32_normal_call
cargo fmt
./scripts/verify.sh
```

For the IR call reachability extraction, from `compiler/`:

```sh
cargo test --quiet ir::lower::reachability
cargo test --quiet lowers_imported_i32_normal_call
cargo test --quiet lowers_entry_returning_same_file_function_call
cargo fmt
./scripts/verify.sh
```

For imported call target collection, from `compiler/`:

```sh
cargo test --quiet imported_placeholder_symbol_becomes_unloaded_imported_call_target
cargo test --quiet collects_loaded_imported_call_targets
cargo test --quiet lowers_imported_i32_normal_call
cargo fmt
./scripts/verify.sh
```

For backend function symbol typing, from `compiler/`:

```sh
cargo test --quiet backend::codegen::tests::generates_same_file_function_call
cargo test --quiet backend::codegen::tests::generates_framed_i32_normal_call_from_hand_built_ir
cargo test --quiet run_command_returns_same_file_function_call_exit_code
cargo fmt
./scripts/verify.sh
```

For adding the imported IR call target, from `compiler/`:

```sh
cargo check --quiet
cargo test --quiet maps_imported_call_target_to_imported_function_symbol
cargo test --quiet ir::lower::reachability
cargo fmt
./scripts/verify.sh
```

For the imported-call lowering-boundary diagnostic, from `compiler/`:

```sh
cargo test --quiet lowers_imported_i32_normal_call
cargo fmt
cargo test --quiet
```

For the interpolated string lowering-boundary diagnostic, from `compiler/`:

```sh
cargo test --quiet reports_unsupported_interpolated_string_binding_lowering
cargo fmt
cargo test --quiet
```

For the `std/fmt` import graph frontend coverage, from `compiler/`:

```sh
cargo test --quiet check_loads_std_fmt_import_graph_from_nocter_home
cargo fmt
cargo test --quiet
```

For the standard-library string/formatting boundary work, from `compiler/`:

```sh
NOCTER_HOME=/Users/manaberyou/Desktop/nocter/.nocter cargo run --quiet -- check ../.nocter/std/fmt.nct --format json
cargo test --quiet
```

The direct `check` command exits with the expected executable-root diagnostic `E0300` because `std/fmt.nct` is not an executable root file; it produced no import, parse, or type diagnostics after that.

From repository root:

```sh
git diff --check
```

For the i32 shift backend work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet target::arm64::encoder
cargo test --quiet ir::lower::tests::lowers_entry_i32_shifts_with_normal_calls
cargo test --quiet generates_i32_shift_left_with_count_traps
cargo test --quiet generates_i32_shift_right_with_count_traps
cargo test --quiet --test cli_build build_command_lowers_i32_call_shifts
cargo test --quiet --test cli_run run_command_returns_i32_call_shift_exit_code
cargo test --quiet --test cli_run run_command_traps_i32_negative_shift_count
cargo test --quiet --test cli_run run_command_traps_i32_too_large_shift_count
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully. Local assembler output was used once to confirm `lslv` and `asrv` instruction bytes for encoder tests; the compiler implementation still emits those bytes directly and does not depend on an external assembler.

For the i32 arithmetic overflow backend work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet target::arm64::encoder
cargo test --quiet backend::codegen
cargo test --quiet generates_i32_addition_with_overflow_trap
cargo test --quiet generates_i32_subtraction_with_overflow_trap
cargo test --quiet generates_i32_multiplication_with_overflow_trap
cargo test --quiet --test cli_run run_command_traps_i32_addition_overflow
cargo test --quiet --test cli_run run_command_traps_i32_subtraction_overflow
cargo test --quiet --test cli_run run_command_traps_i32_multiplication_overflow
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully. Local assembler output was used once to confirm `smull`, `sxtw`, and 64-bit `cmp` instruction bytes for encoder tests; the compiler implementation still emits those bytes directly and does not depend on an external assembler.

For the i32 division/remainder backend work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet target::arm64::encoder
cargo test --quiet generates_i32_division_with_safety_traps
cargo test --quiet generates_i32_remainder_with_safety_traps
cargo test --quiet ir::lower::tests::lowers_entry_i32_divide_and_remainder_with_normal_calls
cargo test --quiet --test cli_build build_command_lowers_i32_call_division_and_remainder
cargo test --quiet --test cli_run run_command_returns_i32_call_division_and_remainder_exit_code
cargo test --quiet --test cli_run run_command_traps_i32_division_by_zero
cargo test --quiet --test cli_run run_command_traps_i32_signed_division_overflow
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully. One attempted targeted `cargo test` command passed two test names to Cargo and failed argument parsing before being rerun with separate filters.

For the string interpolation front-end work, from `compiler/`:

```sh
cargo fmt
cargo test -q parser::tests::expressions::parses_interpolated_string_expression
cargo test -q typecheck::tests::strings
cargo test -q literals::tests::
cargo test -q lexer::tests::
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the multi-line string literal work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet literals
cargo test --quiet lexer
cargo test --quiet comments
cargo test --quiet parser::tests::expressions::parses_multi_line_string_literal_expression
cargo test --quiet format::tests::formats_multi_line_string_with_comment_markers_stably
cargo test --quiet ir::lower::tests::lowers_fallible_entry_return_static_error_constructor_with_multi_line_message
cargo test --quiet --test cli_run run_command_reports_fallible_entry_failure_multi_line_message
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

After the bool equality/inequality lowering work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

All passed.
The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

From repository root:

```sh
git diff --check
```

Passed after the bool equality/inequality lowering work.

For the non-tail call diagnostic work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet ir::lower::tests::reports_unsupported_i32_non_tail_call
cargo test --quiet ir::lower::tests::reports_unsupported_bool_non_tail_call
cargo test --quiet --test cli_build build_command_reports_unsupported_non_tail_call
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the ARM64 encoder frame/spill helper work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet target::arm64::encoder
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the backend frame planner work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet backend::frame
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the framed-function exit emission work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet backend::codegen::tests::emits_framed
cargo test --quiet backend::frame
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the IR `CallI32` and hand-built normal-call codegen work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet backend::codegen::tests::generates_framed_i32_normal_call_from_hand_built_ir
cargo test --quiet backend::codegen::tests::normal_i32_call_spills_and_reloads_scalar_locals
cargo test --quiet backend::frame
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the source-level normal-call subset work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet ir::lower::tests::lowers_entry_i32_let_initializer_normal_call
cargo test --quiet ir::lower::tests::lowers_entry_i32_let_initializer_normal_call_with_arguments
cargo test --quiet ir::lower::tests::lowers_i32_let_initializer_normal_call_with_non_reordered_parameter_arguments
cargo test --quiet ir::lower::tests::lowers_entry_i32_return_expression_normal_call
cargo test --quiet ir::lower::tests::lowers_entry_i32_let_initializer_normal_call_addition
cargo test --quiet ir::lower::tests::lowers_entry_i32_return_expression_local_plus_normal_call
cargo test --quiet ir::lower::tests::lowers_entry_i32_nested_return_addition_with_one_normal_call
cargo test --quiet ir::lower::tests::lowers_entry_i32_return_expression_with_multiple_normal_calls
cargo test --quiet ir::lower::tests::lowers_entry_i32_let_initializer_with_multiple_normal_calls
cargo test --quiet ir::lower::tests::lowers_entry_i32_multiple_normal_calls_without_colliding_with_local
cargo test --quiet ir::lower::tests::reports_unsupported_nested_i32_tail_call_argument
cargo test --quiet ir::lower::tests::lowers_entry_i32_let_initializer_nested_normal_call_argument
cargo test --quiet ir::lower::tests::lowers_entry_i32_let_initializer_multiple_nested_normal_call_arguments
cargo test --quiet ir::lower::tests::lowers_entry_i32_return_addition_with_nested_normal_call_argument
cargo test --quiet ir::lower::tests::lowers_reordered_normal_call_arguments
cargo test --quiet ir::lower::tests::reports_unsupported_reordered_tail_call_arguments
cargo test --quiet ir::lower::tests::reports_unsupported_bool_returning_normal_call
cargo test --quiet ir::lower::tests::reports_unsupported_call_in_condition
cargo test --quiet --test cli_build build_command_lowers_i32_normal_call_let_initializer
cargo test --quiet --test cli_build build_command_reports_unsupported_non_tail_call
cargo test --quiet --test cli_run run_command_returns_i32_normal_call_exit_code
cargo test --quiet --test cli_run run_command_returns_reordered_i32_normal_call_exit_code
cargo test --quiet --test cli_run run_command_preserves_local_across_i32_normal_call_addition
cargo test --quiet --test cli_run run_command_returns_multiple_i32_normal_call_addition_exit_code
cargo test --quiet --test cli_run run_command_returns_nested_i32_normal_call_argument_exit_code
cargo test --quiet backend::frame
cargo test --quiet backend::codegen::tests::normal_i32_call_spills_and_reloads_scalar_locals
cargo test --quiet backend::codegen::tests::generated_i32_normal_call_stages_reordered_arguments
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the tail-call argument staging work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet backend::frame
cargo test --quiet ir::lower::tests::lowers_reordered_tail_call_arguments
cargo test --quiet backend::codegen::tests::generates_i32_tail_call_with_arguments_and_add
cargo test --quiet --test cli_run run_command_returns_reordered_i32_tail_call_exit_code
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the bool-returning normal-call work, from `compiler/`:

```sh
cargo fmt
cargo check --quiet
cargo test --quiet --no-run
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.
Running test binaries in this sandbox currently hangs before `--list` or `running ...` output, so `cargo test --quiet` and targeted runtime tests could not complete in this environment after the change. An escalation attempt for the targeted lowering test was rejected by the automatic approval reviewer.

For the unary bool normal-call expression work, from `compiler/`:

```sh
cargo fmt
cargo check --quiet
cargo test --quiet --no-run
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.
Runtime test execution remains blocked by the same sandbox test-binary hang described above.

For the bool condition call work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet ir::lower::tests::lowers_entry_i32_if_condition_normal_call
cargo test --quiet ir::lower::tests::lowers_entry_i32_if_condition_not_normal_call
cargo test --quiet ir::lower::tests::lowers_bool_if_condition_normal_call
cargo test --quiet --test cli_run run_command_returns_bool_condition_call_exit_code
cargo test --quiet --test cli_run run_command_returns_not_bool_condition_call_exit_code
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the short-circuit bool condition call work, from `compiler/`:

```sh
cargo fmt
cargo check --quiet
cargo test --quiet ir::lower::tests::lowers_entry_i32_if_condition_and_normal_calls
cargo test --quiet ir::lower::tests::lowers_entry_i32_if_condition_or_normal_calls
cargo test --quiet ir::lower::tests::lowers_entry_i32_if_condition_left_nested_and_normal_calls
cargo test --quiet --test cli_run run_command_returns_and_bool_condition_call_exit_code
cargo test --quiet --test cli_run run_command_returns_or_bool_condition_call_exit_code
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the short-circuit bool value call work, from `compiler/`:

```sh
cargo fmt
cargo check --quiet
cargo test --quiet ir::lower::tests::lowers_bool_let_initializer_and_normal_calls
cargo test --quiet ir::lower::tests::lowers_bool_return_or_normal_calls
cargo test --quiet --test cli_run run_command_returns_and_bool_value_call_exit_code
cargo test --quiet --test cli_run run_command_returns_or_bool_return_call_exit_code
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the bool call comparison work, from `compiler/`:

```sh
cargo fmt
cargo check --quiet
cargo test --quiet ir::lower::tests::lowers_bool_let_initializer_normal_call_comparison
cargo test --quiet ir::lower::tests::lowers_bool_return_normal_call_comparison
cargo test --quiet ir::lower::tests::lowers_entry_i32_if_condition_normal_call_comparison
cargo test --quiet --test cli_run run_command_returns_bool_call_comparison_let_exit_code
cargo test --quiet --test cli_run run_command_returns_bool_call_comparison_return_exit_code
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the nested tail-call argument work, from `compiler/`:

```sh
cargo fmt
cargo check --quiet
cargo test --quiet ir::lower::tests::lowers_entry_i32_nested_tail_call_argument
cargo test --quiet ir::lower::tests::lowers_entry_i32_multiple_nested_tail_call_arguments
cargo test --quiet backend::frame::tests::tail_call_with_local_argument_counts_argument_local
cargo test --quiet --test cli_run run_command_returns_nested_i32_tail_call_argument_exit_code
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the i32 call comparison work, from `compiler/`:

```sh
cargo fmt
cargo check --quiet
cargo test --quiet ir::lower::tests::lowers_entry_i32_if_condition_i32_normal_call_comparison
cargo test --quiet ir::lower::tests::lowers_bool_let_initializer_i32_normal_call_comparison
cargo test --quiet ir::lower::tests::lowers_bool_return_i32_normal_call_comparison
cargo test --quiet --test cli_build build_command_reports_unsupported_i32_call_expression
cargo test --quiet --test cli_run run_command_returns_i32_call_comparison_condition_exit_code
cargo test --quiet --test cli_run run_command_returns_i32_call_comparison_return_exit_code
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

For the i32 comparison short-circuit coverage work, from `compiler/`:

```sh
cargo fmt
cargo test --quiet ir::lower::tests::lowers_entry_i32_if_condition_and_i32_call_comparison
cargo test --quiet ir::lower::tests::lowers_bool_let_initializer_and_i32_call_comparison
cargo test --quiet --test cli_run run_command_returns_and_i32_call_comparison_condition_exit_code
cargo test --quiet --test cli_run run_command_returns_and_i32_call_comparison_value_exit_code
cargo test --quiet
cargo clippy --all-targets --quiet -- -D warnings
```

From repository root:

```sh
git diff --check
```

All passed. The shell printed `/bin/ps: Operation not permitted` from Homebrew shellenv, but the commands exited successfully.

## First Action In Next Session

1. Run `git status --short`.
2. Review any uncommitted changes before editing.
3. If the user asks for a commit, stage only compiler files unless there are unrelated local changes.

## Next Implementation Direction

The scalar `i32` and `usize` backend subsets now have runtime safety checks for `+`, `-`, `*`, `/`, `%`, `<<`, and `>>`.

Recommended next small task for the next session:

1. Follow `compiler/docs/interpolation-lowering.md`: keep bare interpolation lowering disabled until an explicit allocator source is designed, and first make explicit `std/string` construction plus `std/fmt.append_*` calls buildable.
2. Add only the backend prerequisites needed by that explicit path: aggregate `String`/`Allocator`/`RawBuffer` storage, aggregate stack-backed `var`, borrow argument lowering, and owned aggregate returns/moves.
3. Consider broader terminal control-flow only after its lowering rules are designed.
4. Keep unrelated imported calls, aggregates, ownership/drop lowering, general mutable storage, and broader control-flow disabled until their ABI, storage, and join rules are designed.
5. Add CLI build/run coverage for any newly buildable source subset.

## Design Constraints To Preserve

- No LLVM, `clang`, `as`, `ld`, Xcode Command Line Tools, or external linker backend.
- Keep the compiler self-contained.
- Prefer maintainable module boundaries over adding more logic to already busy files.
- Keep behavior changes and pure refactors in separate commits when practical.
- Update `TODO.md`, `docs/implementation-status.md`, `docs/roadmap.md`, or `docs/architecture.md` when their durable facts change.
