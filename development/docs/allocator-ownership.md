# Allocator and Ownership

この文書は v0.2.0 の owning runtime values を成立させる共通設計を定義する。
公開構文と型規則は [spec](../../spec/README.md) に従う。

## Separation of Responsibilities

| Layer | Responsibility |
|---|---|
| type checker | source place の initialized / moved / borrowed state |
| aggregate drop shape | 型ごとの再帰的 drop 構造。実行経路に依存しない |
| runtime drop obligation | その経路で実際に所有済みの subtree / prefix |
| IR | obligation の activate、publish、transfer、drop を明示する |
| backend | IR の状態遷移を ABI layout に従って実行する |
| `std/mem` | allocation layout、growth、deallocation、allocator provenance |
| `std/string`, `std/vec` | buffer と initialized length の型固有 invariant |

source place state と runtime cleanup state を同じ bitset や ad-hoc flag に畳まない。
前者は不正な source operation を拒否し、後者は失敗経路で取得済み資源だけを破棄する。

## Allocator Contract

`Layout` は `size` と `align` の組ではなく、検証済み allocation request として扱う。

- alignment は 0 ではなく power of two で、target 上限を満たす。
- `count * element_size` と growth 計算は allocation 前に checked arithmetic を通す。
- zero-sized value と zero-capacity collection は allocation を持たない canonical empty
  state を使う。free は allocation の有無を識別する。
- `RawBuffer` は pointer、allocated byte length、alignment、allocator provenance を保持する。
- free は確保時と同じ allocator と実 layout を使う。logical length を allocation size の
  代用にしない。
- grow は allocate → initialized bytes/elements の transfer → new state publish → old free
  の順で行う。publish 前の失敗は old state を変更しない。
- public callers は `RawBuffer` の representation を構築・改変できない。

OS syscall は allocator implementation の内側に置く。`String` は private `RawBuffer` へ
移行済みであり、`Vec<T>` が `alloc_pages` / `free_pages` を直接呼ぶ状態だけが残る。

## Recursive Drop Model

runtime obligation は最低限、次の状態を再帰的に表現する。

```text
Inactive
Complete(shape)
ArrayPrefix { completed, element_shape }
StructFields { initialized fields }
PayloadFields { active variant, initialized fields }
```

fixed array の要素を構築するときは、配列 prefix へ即座に追加しない。

1. 現在要素用の独立した recursive obligation を作る。
2. field / payload を一つずつ初期化し、その obligation に publish する。
3. 要素全体の完了後に current obligation を配列へ移し、completed prefix を増やす。
4. 途中失敗では current element を再帰的に破棄し、その後 completed prefix を逆順に
   破棄する。

これにより「完了した要素数」だけでは表せない nested partial initialization を扱う。

## Collection Ownership

`Vec<T>` の invariant は次の通り。

- `0 <= len <= capacity`
- allocation は capacity 分の storage を持つが、所有値は `[0, len)` のみ
- `[0, len)` は全要素が complete で、`[len, capacity)` は uninitialized
- drop と clear は `[0, len)` を逆順に一度だけ破棄する
- reserve は要素の ownership を新 storage へ transfer し、要素を複製しない
- push は destination element の complete obligation を publish した後だけ `len` を増やす
- pop は末尾要素を tracked return storage へ transfer し、vector の `len` を減らす

non-copy `T` に raw byte copy を使う場合でも、意味は copy ではなく storage relocation
である。旧 storage の obligation を無効化してから解放し、drop glue を二度走らせない。

`String` は UTF-8 byte prefix を所有する特殊化された buffer である。byte 自体に drop は
ないが、`0 <= len <= capacity`、failure atomic growth、allocator provenance は
`Vec<T>` と共有する。

## Error-path Invariants

- initializer が取得した値は、次の fallible operation より前に obligation へ登録する。
- complete として publish するのは全 subvalue の初期化後だけにする。
- replacement は新値を別 obligation で完成させてから旧値を破棄・置換する。
- cleanup は取得順の逆順で行い、元の error を cleanup の都合で置換しない。
- ownership transfer は source obligation の無効化と destination obligation の有効化を
  一つの IR-level operation として表現する。

## Rejected Shortcuts

- `Vec<T>.clear` で `len = 0` だけを行う
- non-copy element の storage bytes を複製して両側を live にする
- allocation size の代わりに logical length で free する
- failure 後に old pointer と new capacity を組み合わせた半更新状態を残す
- backend が AST を見て drop 対象を推測する
- 任意位置 remove を prefix model の例外処理で実装する

任意位置 remove が必要になった時点で sparse ownership state を独立設計する。
