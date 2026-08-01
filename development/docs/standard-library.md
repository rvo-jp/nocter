# Standard Library Runtime

この文書は repository に追跡され、配布時に `.nocter/std` へ入る実装の状態を記録する。
公開 API の規範は [Standard Library, Primitives, and OS](../../spec/11-stdlib-primitives-os.md)
であり、この文書は仕様を追加しない。

## Current Modules

| Module | Current role | v0.2.0 work |
|---|---|---|
| `error` | structured recoverable error | allocator / collection error IDs を安定化 |
| `fmt` | scalar and text formatting helpers | owning text の動作確認 |
| `io` | file open/read/write and stdout/stderr | `Vec<File>` の deterministic drop を検証 |
| `mem` | `Layout`, `RawBuffer`, `Allocator`, page boundary | layout/grow/free 契約を完成 |
| `os` | target-gated syscall boundary | allocator の内側へ限定 |
| `prelude` | implicit common declarations | v0.2.0 で拡張しない |
| `process` | exit/abort/cwd/args; env is check-only | allocator 完成に必要な範囲だけ維持 |
| `ptr` | restricted pointer primitives | `pub(nocter)` trust boundary を維持 |
| `string` | owning UTF-8 bytes | common allocator、failure-atomic growth |
| `vec` | owning generic sequence | non-copy initialized-prefix drop と pop |

## Runtime Baseline

`std/mem` は checked `Layout`、canonical empty buffer、private allocator provenance、
failure-atomic grow、deterministic `RawBuffer` drop を持つ。alignment、zero-size、OOM、grow失敗
後の内容保持は distributed-home runtime tests で固定されている。

現在の `String` は empty、with_capacity、from/copy、view、len/capacity、reserve、clear、
push_str、bytes、storage release を持つ。現在の `Vec<T>` は empty、with_capacity、
from_slice、len/capacity、reserve、push、clear、views、storage release を持つが、要素 drop
を伴う non-copy collection の契約は未完成である。

`String` と `Vec<T>` はまだ page allocation primitive を直接使用している。これは完成した
`std/mem` 契約へ移す対象であり、安定 API とみなさない。

## v0.2.0 Required Behavior

### `std/mem`

- checked `Layout` construction
- canonical empty allocation state
- allocation、growth、free の allocator identity 保持
- overflow、invalid alignment、out of memory の recoverable error
- old allocation を保つ failure-atomic growth
- representation fields を `pub(nocter)` より外へ公開しない

### `std/string`

- empty / with_capacity / from_str / copy
- len / capacity / is_empty / view / bytes
- reserve / push_str / clear / drop
- repeated growth 後も UTF-8 view と所有 storage が一致する
- allocation failure 後も元の内容、len、capacity が変わらない

Unicode scalar/character indexing と normalization は v0.2.0 の条件に含めない。境界を
曖昧にした byte indexing API は追加しない。

### `std/vec`

- copy と non-copy の両方で empty / with_capacity / reserve / push / clear / drop
- copy element に対する from_slice と immutable/mutable view
- 末尾 ownership extraction としての pop
- nested owning element の再帰 drop
- capacity overflow と allocation failure の原子性

non-copy element を借用 slice から複製する意味はまだ定義しないため、`from_slice` は
copyable `T` に限る。制約を型システムで表せない間は、公開範囲を不正に広げず
source-backed diagnostic で拒否する。

## Acceptance Matrix

| Scenario | Required observation |
|---|---|
| `String` repeated growth | bytes preserved; one final storage free |
| failed `String.reserve` | pointer/content/len/capacity unchanged |
| `Vec<String>` growth | each string remains usable; each drops once |
| `Vec<String>.pop()` | returned string remains owned after vector drop |
| `Vec<File>.clear()` | initialized handles close once; later vector drop is empty |
| `Vec<Vec<String>>` early `?` | completed prefixes unwind in reverse order |
| zero-capacity values | no allocation and no invalid free |
| packaged-home execution | same behavior as repository-local source |

Tests should observe semantic effects such as handle closure, output, error identity, and post-operation
state. Backend instruction snapshots alone do not prove the standard-library contract.

## Deferred Surface

Environment value retrieval, rich path APIs, insert/remove, iterator protocols, multiple allocator
families, implicit allocator selection, interpolation allocation, and collection literal/spread are not
v0.2.0 release gates. Add them only after their ownership and failure behavior is specified.
