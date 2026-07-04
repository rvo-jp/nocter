# Nocter

Nocter は、人間が読みやすい静的型付け高級言語を設計し、ARM64 macOS 向けのネイティブ実行ファイルへ直接コンパイルすることを目指すコンパイラプロジェクトです。

言語としては、静的型付け・値中心・モジュール指向・低依存システム言語を目指します。class 継承を中心にしたオブジェクト指向言語ではなく、`struct`、関数、モジュール、所有権、借用、標準ライブラリを軸にします。

言語名は Nocter、ソースファイルの拡張子は `.nct` です。

文法と意味論の詳細は [SPEC.md](/Users/manaberyou/Desktop/nocter/SPEC.md) に記録します。仕様上の採用事項は README の概要より SPEC を優先します。

最重要方針は、外部ツールやランタイムへの依存をなくすことです。最終的には、`.nocter/` ディレクトリだけを配布すれば利用できる状態を目指します。ユーザーはこのディレクトリを `~/.nocter` などに配置し、PATH に通します。

```text
~/.nocter/
    nocter
    std/
```

利用者は `clang`、`as`、`ld`、Xcode Command Line Tools、外部ランタイムライブラリを必要としません。コンパイラ自身が、字句解析から Mach-O 実行ファイルの生成までを一貫して担います。

## ディレクトリ構成

このリポジトリでは、コンパイラの実装と利用者へ配布する完成品を分けます。

```text
README.md
    ユーザー向けの全容

SPEC.md
    ユーザー向けの言語仕様書

src/
    コンパイラ本体の実装
    README.md
        コンパイラ開発者向けの実装設計書

.nocter/
    nocter
    std/
```

`README.md` は Nocter の目的、対象環境、配布形態、設計思想を説明する入口です。`SPEC.md` は Nocter を書く人向けの言語仕様書です。`src/README.md` はコンパイラ開発者向けの内部設計書であり、ユーザー向け文書には実装内部の詳細を入れすぎない方針とします。

`src/` は開発用のソースツリーです。`.nocter/` は利用者に配布する完成品の配置先であり、コンパイラ本体と標準ライブラリを含みます。

ユーザーは配布された `.nocter/` を `~/.nocter` などへ配置し、次のように PATH を通します。

```sh
export PATH="$HOME/.nocter:$PATH"
```

標準ライブラリは `NOCTER_HOME` が指定されていればそこから探し、指定がなければ実行中の `nocter` コマンドが置かれたディレクトリから探します。

## 対象環境

現時点の対象は Apple Silicon Mac に限定します。

- CPU: Apple Silicon / ARM64
- OS: macOS
- 出力形式: Mach-O executable

クロスコンパイル、Intel Mac、Linux、Windows、他 CPU アーキテクチャへの対応は現在の目標に含めません。対象を限定することで設計を単純にし、ARM64 macOS 向けコンパイラとしての完成度を優先します。

## 設計方針

### パス由来モジュール

Nocter には `module` 宣言を置きません。モジュール名は import root からの相対ファイルパスで決まります。

```text
examples/word_count.nct => examples.word_count
.nocter/std/io.nct      => std.io
```

ファイルパスを唯一の情報源にすることで、ファイル位置とモジュール宣言の不一致を防ぎます。

`import` は明示的な名前指定を基本にします。ワイルドカード import と相対 import は初期仕様では採用しません。

```nct
import std.mem.Allocator
import std.io.{File, stdout, stderr}
import std.io as io
import std.io.File as StdFile
```

モジュール内の定義はデフォルトで private です。他モジュールから import できる API には `pub` を付けます。`struct` のフィールドと `impl` 内の関数もデフォルト private です。

```nct
pub struct File {
    fd: i32
}

impl File {
    pub func open(path: StringView): File!Error {
        ...
    }
}
```

### 静的型付け・値中心・モジュール指向

Nocter は、class を言語の中心に置きません。データは `struct`、振る舞いは関数、名前空間と再利用単位はモジュールで表現します。

```nct
struct File {
    fd: i32
}

func write(file: &+File, data: StringView): void!Error {
    ...
}
```

`impl` 内の `func` は型に関連付く associated function です。`impl` 内の `method` は receiver を持つメソッドです。`self` / `this` は使わず、receiver 名と borrow 種別を明示します。

```nct
impl File {
    pub func open(path: StringView): File!Error {
        ...
    }

    pub method (file: &+Self).write(data: StringView): void!Error {
        ...
    }
}
```

`func` は `File.open(path)` のように型から呼びます。`method` は `file.write(data)` のように値から呼びます。`File.write(&+file, data)` のような UFCS 呼び出しは初期仕様では採用しません。

抽象化が必要な場合は、継承階層ではなく `trait` を使います。

```nct
trait Writer {
    method (writer: &+Self).write(data: StringView): void!Error
}
```

`enum` は有限個の variant を持つ型です。variant の分岐には `match` を使い、各 arm は `is Pattern { ... }` で書きます。`else` がない enum match は網羅性を要求します。

```nct
match error {
    is AppError.missing_path {
        ...
    }
    is AppError.open_failed(path) {
        ...
    }
}
```

目指す方向は、古典的な OOP ではなく、値型、明示的な所有権、明確なモジュール境界によって大きなプログラムを構成する言語です。

### 高級言語として保つ

ユーザーが書くコードは高級言語であり、ARM64 命令や Mach-O の詳細を意識しない形にします。

```nct
import std.io.print

program(): i32 {
    print("Hello")
    return 0
}
```

低レベルの処理はコンパイラと標準ライブラリが引き受けます。

### ビルトイン関数を極力作らない

`print`、`exit`、ファイル操作、文字列操作などは、言語仕様に組み込まず標準ライブラリで提供します。

```nct
func print(msg: string): void {
    ...
}
```

コンパイラは `print` という名前を特別扱いしません。言語仕様を小さく保ち、標準ライブラリを通常の言語機能で拡張できる構造を優先します。

### 標準ライブラリが低レベル実装を持つ

標準ライブラリだけは、ARM64 アセンブリへ降りるための仕組みを持ちます。

```nct
func print(msg: string): void {
    asm {
        // ARM64 assembly
    }
}
```

`asm` は高級言語と OS / CPU の低レベル機能を接続するための escape hatch です。現在の構想では、標準ライブラリでの使用を主対象とします。一般ユーザーコードでは使用を禁止する、または強く制限する可能性があります。

制限する理由は次の通りです。

- 型安全性を維持するため
- 最適化の余地を壊さないため
- ABI や呼び出し規約の破壊を防ぐため

### 自己完結性を優先する

このプロジェクトでは、一般的なコンパイラ実装で使われる外部ツールチェーンを前提にしません。

採用しない方針:

- `.s` を出力して `as` に渡す
- `clang` でリンクする
- `ld` に Mach-O 生成を任せる
- Xcode Command Line Tools の存在を前提にする
- 外部ランタイムライブラリに依存する

目標とする流れ:

```text
source.nct
    |
    v
nocter
    |
    v
Mach-O executable
```

### 既存言語の参照と Nocter の独自性

Nocter は、奇抜な構文や独自概念を増やすことではなく、完成度の高い自己完結型言語処理系を作ることを優先します。基本的な言語設計では既存言語の成功している要素を参照し、独自性は Nocter の目的に直結する部分へ集中させます。

参照する領域:

- Rust: `struct`、`impl`、`trait`、所有権、借用、低レベル機能の隔離
- Zig: 低依存、明示的 allocator、隠れた制御フローや隠れたメモリ確保を避ける設計
- Go: 読みやすいモジュール構成、継承なしでプログラムを組み立てる設計、単純なツール体験
- Swift: 値型中心の API 設計、`protocol` 的な抽象化、Apple 環境に馴染む標準ライブラリ設計

そのまま採用しない領域:

- Rust の高度なライフタイム構文を初期段階から全面採用すること
- Zig の構文や comptime モデルをそのまま再現すること
- Go の GC やランタイム前提の並行処理モデル
- Swift の巨大なランタイムや Apple ツールチェーン依存

Nocter の独自性を置く領域:

- `.nct` から ARM64 機械語と Mach-O 実行ファイルを直接生成するコンパイル経路
- `clang`、`as`、`ld`、Xcode Command Line Tools に依存しない完全自己完結性
- `.nocter/` に `nocter` コマンドと標準ライブラリをまとめる配布モデル
- 標準ライブラリだけが低レベルへ降りる、制限付き `asm` の設計
- GC なしで、所有権、借用、region / arena によってメモリ安全性を目指す設計
- Apple Silicon macOS / Mach-O に対象を絞り、汎用性より完成度を優先する実装

つまり Nocter は、言語表面では堅実さを優先し、コンパイル経路、配布モデル、標準ライブラリと `asm` の境界、GC に頼らないメモリモデルで独自性を出します。

## コンパイラの責務

コンパイラ本体は、次の処理を自前で実装します。

- Lexer
- Parser
- AST 生成
- 型チェック
- IR 生成（必要な場合）
- ARM64 命令生成
- Mach-O 実行ファイル生成
- 最小限のリンカ機能（必要な場合）

外部アセンブラや外部リンカには依存しません。ARM64 命令のエンコードと Mach-O ファイルの構築はコンパイラが直接行います。

## 標準ライブラリ

標準ライブラリは、配布物では `.nocter/std/` に配置します。

構成例:

```text
.nocter/
    nocter
    std/
        prelude.nct
        io.nct
        string.nct
        process.nct
```

利用者は必要な機能を import して使います。

```nct
import std.io.print
```

標準ライブラリは高級言語で記述し、必要な箇所だけ `asm` によって ARM64 命令へ接続します。

## ランタイム

現時点では、独立したランタイムライブラリを持たない方針です。標準ライブラリの `asm` が macOS との最小限の橋渡しを行います。

GC は採用しません。Nocter は実行時ガベージコレクタにメモリ管理を任せる言語ではなく、コンパイル時に所有権、参照の寿命、破棄責任を検査する言語を目指します。

想定する層構造:

```text
高級言語
    |
    v
標準ライブラリ
    |
    v
asm
    |
    v
ARM64 命令
    |
    v
Mach-O
```

## メモリ管理

Nocter のメモリ管理方針は、GC なし、所有権あり、静的検査ありです。

完全にコンパイル時だけで全メモリ量を決めることは、一般用途では現実的ではありません。入力サイズ、分岐、再帰、ファイル読み込み、ユーザー操作によって、必要なメモリ量は実行時に決まります。そのため Nocter は、動的メモリ確保そのものを禁止するのではなく、動的メモリの所有者、寿命、解放責任をコンパイラが検査できる形に制限します。

基本方針:

- 不変束縛は `let`、可変束縛は `var` で宣言する
- 値は可能な限りスタックまたは静的領域に配置する
- ヒープ確保は明示的な allocator または region を通して行う
- 所有権を持つ値だけがメモリを解放できる
- 型はデフォルトで move-only とし、`copy struct` だけ暗黙コピーを許可する
- 非copy値の代入・引数渡し・return には `move` を明示する
- readonly borrow は `&T` として表現する
- readwrite borrow は `&+T` として表現し、同時に他の readonly / readwrite borrow と共存できない
- `&+` は単一トークンとして扱い、単項 `+x` は採用しない
- スコープ終了時に破棄処理を挿入する
- 破棄処理は `impl` 内の専用 `drop` 構文で定義する
- use-after-free、double-free、dangling pointer を型チェック段階で防ぐ

借用の基本規則:

- `&value` は readonly borrow を作る
- `&+value` は readwrite borrow を作る
- `&+value` は `var` 束縛や書き込み可能な場所からだけ作れる
- 複数の readonly borrow は同時に存在できる
- readonly borrow が生きている間、同じ値への readwrite borrow は作れない
- readwrite borrow が生きている間、同じ値への他の readonly / readwrite borrow は作れない
- 借用中の値は `move` できない
- 借用中の値は `drop` できない
- 借用は参照先より長生きできない
- 初期仕様では lifetime 注釈を採用しない

例:

```nct
func allocate_buffer(): void {
    var buffer = Buffer.alloc(1024)
    fill_buffer(&+buffer)
    inspect_buffer(&buffer)
}
```

`buffer` は所有権を持つ値です。スコープを抜けると破棄されます。所有型を別の変数へ渡す場合は、`move` を明示します。

```nct
let a = Buffer.alloc(1024)
let b = move a
```

`move` 後の `a` は使用できません。これにより二重解放を防ぎます。

通常の関数に borrow を渡す場合は、呼び出し側で `&` / `&+` を明示します。

```nct
func inspect(file: &File): void {
    ...
}

inspect(&file)
```

`method` receiver だけは自動 borrow します。

```nct
impl File {
    pub method (file: &+Self).write(text: StringView): void!IOError {
        ...
    }
}

try file.write("hello")
```

コピー可能な値型は `copy struct` で宣言します。`copy struct` は全フィールドがcopy可能である必要があり、`drop` を定義できません。

```nct
copy struct Point {
    pub x: Int
    pub y: Int
}

let p1 = Point{x: 1, y: 2}
let p2 = p1
```

所有値の破棄はスコープ終了時に自動で行います。`drop` は trait ではなく、`impl` 内に置ける専用構文です。明示的に早く破棄したい場合は `drop value` 文を使います。

```nct
impl File {
    drop(file: &+Self): void {
        std.os.close(file.fd).ignore()
    }
}

var file = try File.open(path)
drop file
```

一時的な大量確保には、region または arena を使う設計を検討します。

```nct
region temp {
    let source = read_file(temp, "main.nct")
    let tokens = lex(temp, source)
}
```

`region` を抜けると、その中で確保したメモリをまとめて破棄します。コンパイラは、`region` 内の参照が外側へ漏れないことを検査します。

## 文字列

文字列リテラルの型は `StringView` とします。

```nct
let name = "Nocter" // StringView
```

`StringView` は、UTF-8 として妥当な文字列を指す非所有 view です。実体は Mach-O 内の静的データ、または別の所有オブジェクトが持つバッファです。`StringView` 自身は所有権を持たず、drop も発生しません。

`String` は所有する文字列型です。標準ライブラリ側では `Buffer<u8>` などを使って実装し、スコープ終了時に内部バッファを破棄します。`String` は move-only とし、暗黙 copy は行いません。

```nct
let view: StringView = "README.md"
var owned = try String.copy(allocator, view)

open(view)
open(owned.view())

func open(path: StringView): File!IOError {
    ...
}
```

文字列リテラルをグローバルな `String` として扱う方針は採用しません。`String` は所有型なので、リテラルから `String` が必要な場合は `String.copy(...)` で明示的に確保します。

連続した要素列の非所有ビューには `View<T>` / `WriteView<T>` を使います。`View<T>` は readonly view、`WriteView<T>` は readwrite view です。`StringView` は文字列専用の view として扱います。

```nct
func checksum(bytes: View<u8>): u32
func read_into(file: &+File, output: WriteView<u8>): usize!IOError

impl StringView {
    pub method (text: Self).bytes(): View<u8>
}
```

`View<u8>` は任意のバイト列を表し、UTF-8 であるとは限りません。`StringView` からは `View<u8>` を得られますが、`View<u8>` から `StringView` を作る場合は UTF-8 検証を必要とします。

`"abc"` を `&String` として渡す暗黙変換は採用しません。`&String` は実在する所有 `String` への borrow であり、文字列リテラルは `StringView` として扱います。

## 配列とコレクション

固定長配列は `Array<T, N>` と書きます。

```nct
let header: Array<u8, 4> = [0x7F, 0x45, 0x4C, 0x46]
let numbers = [1, 2, 3] // Array<Int, 3>
```

配列リテラルは `[a, b, c]` です。文脈型があればその要素型と長さに従い、文脈がなければ要素から型を推論します。整数リテラルだけで構成される場合は `Int` を使います。

所有する可変長バッファは標準ライブラリの `Buffer<T>` で表します。`Buffer<T>` は言語組み込みではなく、標準ライブラリ型です。

```nct
var bytes = try Buffer<u8>.with_capacity(allocator, 4096)
try bytes.push(10)

let read: View<u8> = bytes.view()
let write: WriteView<u8> = bytes.write_view()
```

非所有 view は `View<T>` / `WriteView<T>` を使います。

```nct
View<T>       // readonly contiguous view
WriteView<T> // readwrite contiguous view
```

index 演算子 `x[i]` は境界チェックを行います。範囲外の場合は trap します。範囲外を値として扱いたい場合は `get(i)` を使います。

```nct
let first = read[0]      // 範囲外なら trap
let maybe = read.get(0)  // u8?
```

長さは特別なフィールドではなく、通常メソッド `len()` として扱います。

```nct
let count = read.len()
```

## 型システム

Nocter は静的型付け言語として設計します。

普段使いの整数型として `Int` を採用します。`Int` は `i32` の alias です。整数リテラルは文脈型があればその整数型になり、文脈がなければ `Int` になります。

```nct
let count = 10      // Int
let size: u64 = 10  // u64
```

固定幅整数型として `i8`、`i16`、`i32`、`i64`、`u8`、`u16`、`u32`、`u64` を持ちます。非リテラルの整数値同士は暗黙変換しません。

例:

```nct
func print(msg: string): void
```

戻り値を持たない関数は `void` を返します。失敗を表す値には、例外ではなく fallible type `T!E` を使います。

`T!E` は成功時に `T`、失敗時に `E` を返す型です。fallible 関数内では `return value` が成功、`fail error` が失敗を表します。

`try` は fallible value の成功値、または optional value の present value を取り出す構文です。fallible が失敗した場合は現在の関数から同じ error で失敗し、optional が `none` の場合は現在の optional 関数から `none` を返します。例外やスタック巻き戻しではありません。

```nct
func open(path: StringView): File!IOError {
    if failed {
        fail IOError.not_found(path)
    }

    return file
}
```

fallible value をその場で処理する場合は `match` の `ok` / `fail` arm を使います。

```nct
match open(path) {
    is ok(file) {
        ...
    }
    is fail(error) {
        ...
    }
}
```

値が存在しない可能性は `T?` で表します。`Option<T>` という名前付き型を特別扱いしません。optional 関数では `return value` が present、`return none` が absent を表します。

```nct
func env(name: StringView): StringView? {
    if missing {
        return none
    }

    return value
}

if env("HOME") is some(home) {
    use(home)
}
```

optional value には default operator `??` を使えます。右結合で、必要な場合だけ右辺を評価します。

```nct
let port = env_int("PORT") ?? config.default_port ?? 8080
```

bool 条件の値選択には三項条件演算子 `a ? b : c` を使えます。optional default とは別の演算子です。

```nct
let label = count == 0 ? "empty" : "ready"
```

初期仕様では statement 中心にします。`if`、`match`、block `{ ... }` は値を返しません。関数の成功終了は `return`、fallible の失敗は `fail`、optional の absent は `return none` で明示します。

```nct
func max(a: Int, b: Int): Int {
    if a > b {
        return a
    }

    return b
}
```

値として条件分岐したい場合は三項条件演算子を使います。

```nct
let value = use_left ? left : right
```

文末セミコロンは初期仕様では採用しません。1 行 1 文を基本にし、改行または `}` で文を区切ります。

ループはまず `while`、`loop`、`break`、`continue` を採用します。`while` の条件は `bool` です。`loop` は無限ループです。`break value` のようにループから値を返す構文は初期仕様では採用しません。

```nct
var i: usize = 0

while i < bytes.len() {
    let byte = bytes[i]

    if byte == 0 {
        break
    }

    i += 1
}
```

`for item in expr` は初期仕様では採用しません。`iter` や `next` などの普通の名前をコンパイラが特別扱いしない反復プロトコルを設計してから導入します。

ジェネリクスは `<T>` を使います。制約は `T: Trait`、複数制約は `T: A + B` と書きます。

```nct
struct Buffer<T> {
    ...
}

func first<T>(items: View<T>): T? {
    ...
}

func write_line<W: Writer>(writer: &+W, text: StringView): void!IOError {
    try writer.write(text)
    try writer.write("\n")
    return
}
```

ジェネリクスはコンパイル時に具体化します。`Buffer<Int>` と `Buffer<String>` はそれぞれ具体型として扱い、実行時の型情報や共通ランタイムに依存しません。初期仕様では `dyn Trait` のような動的 trait object は採用しません。

単一の pattern だけを見たい場合は `if expr is Pattern` を使います。

```nct
if open(path) is ok(file) {
    use(file)
}
```

型推論などの機能は今後検討しますが、初期段階では明示的で単純な型システムを優先します。所有権、借用、破棄規則を型チェックに統合する場合も、実装可能な小さい規則から段階的に導入します。

## 実装ロードマップ

現時点の実装順序案です。

1. Lexer
2. Parser
3. AST
4. 型チェック
5. ARM64 命令エンコーダ
6. Mach-O 生成
7. `program` のみ実行可能にする
8. 文字列リテラル配置
9. `exit`
10. `asm`
11. `print`
12. `import`
13. `struct`
14. `if` / `match`
15. `while` / `loop` / `break` / `continue`
16. 所有型のコピー禁止
17. `move`
18. `drop`
19. `&T`
20. `&+T`
21. allocator
22. region / arena
23. 標準ライブラリ拡充

この順序は固定ではありません。ただし、`clang` や `ld` に一時的に逃がして Hello World を早く出すことより、最終設計と矛盾しない経路で進めることを優先します。

## 非目標

現在の非目標は次の通りです。

- 汎用クロスコンパイラにすること
- 複数 OS を同時にサポートすること
- C 言語ツールチェーンの薄いラッパーにすること
- 外部ランタイムを前提にすること
- GC を前提にすること
- class 継承を言語設計の中心にすること
- 最短手順で Hello World だけを出すこと

Nocter は、自己完結した ARM64 macOS 専用言語処理系として成立することを優先します。

## プロジェクトの価値観

このプロジェクトでは、実装の短さより成果物全体の一貫性を重視します。

特に重視するもの:

- 自己完結性
- 言語としての一貫性
- 美しい設計
- 静的型付け、値中心、モジュール指向
- GC に頼らないメモリ安全性
- 標準ライブラリを言語自身で記述できること
- コンパイラ単体で実行ファイル生成まで完結すること

実装量が増えても、これらの価値を崩す設計は避けます。
