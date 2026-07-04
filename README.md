# Nocter

Nocter は、人間が読みやすい静的型付け高級言語を設計し、まず ARM64 macOS 向けのネイティブ実行ファイルへ直接コンパイルすることを目指すコンパイラプロジェクトです。

言語としては、静的型付け・値中心・モジュール指向・低依存システム言語を目指します。class 継承を中心にしたオブジェクト指向言語ではなく、`struct`、関数、モジュール、所有権、借用、標準ライブラリを軸にします。

言語名は Nocter、ソースファイルの拡張子は `.nct` です。

文法と意味論の詳細は [SPEC.md](SPEC.md) を入口として、[`spec/`](spec/) 配下に章別で記録します。仕様上の採用事項は README の概要より SPEC を優先します。

最重要方針は、外部ツールやランタイムへの依存をなくすことです。最終的には、ホスト環境ごとの `.nocter-<host>/` ディレクトリだけを配布すれば利用できる状態を目指します。ユーザーはこのディレクトリを `~/.nocter-arm64-macos` などに配置し、PATH に通します。

```text
~/.nocter-arm64-macos/
    nocter
    std/
    targets/
        arm64-macos/
            std/
```

利用者は `clang`、`as`、`ld`、Xcode Command Line Tools、外部ランタイムライブラリを必要としません。コンパイラ自身が、字句解析から Mach-O 実行ファイルの生成までを一貫して担います。

## ディレクトリ構成

このリポジトリでは、コンパイラの実装と利用者へ配布する完成品を分けます。

```text
README.md
    ユーザー向けの全容

SPEC.md
    ユーザー向けの言語仕様書の入口

spec/
    ユーザー向けの言語仕様書
    00-overview.md
    01-modules-imports.md
    02-values-types.md
    03-control-flow.md
    04-errors-optionals.md
    05-ownership-borrowing-drop.md
    06-memory-region-allocator.md
    07-strings-arrays-views-pointers.md
    08-generics-traits-methods.md
    09-abi-layout.md
    10-targets-distribution.md
    11-stdlib-primitives-os.md

src/
    コンパイラ本体の実装
    README.md
        コンパイラ開発者向けの実装設計書

.nocter-arm64-macos/
    nocter
    std/
        io.nct
        mem.nct
        os.nct
        ptr.nct
    targets/
        arm64-macos/
            std/
                process.nct
                os/
                    macos.nct
        x64-linux/
            std/
        arm64-linux/
            std/
        x64-windows/
            std/
        arm64-windows/
            std/
```

`README.md` は Nocter の目的、対象環境、配布形態、設計思想を説明する入口です。`SPEC.md` は Nocter を書く人向けの言語仕様書の目次であり、詳細な仕様は `spec/` に章別で置きます。`src/README.md` はコンパイラ開発者向けの内部設計書であり、ユーザー向け文書には実装内部の詳細を入れすぎない方針とします。

`src/` は開発用のソースツリーです。`.nocter-arm64-macos/` は現在の開発環境向けの完成品配置先であり、コンパイラ本体と標準ライブラリを含みます。このディレクトリは生成物・配布物なので git 管理しません。

ユーザーは配布された `.nocter-arm64-macos/` を `~/.nocter-arm64-macos` などへ配置し、次のように PATH を通します。

```sh
export PATH="$HOME/.nocter-arm64-macos:$PATH"
```

標準ライブラリは `NOCTER_HOME` が指定されていればそこから探し、指定がなければ実行中の `nocter` コマンドが置かれたディレクトリから探します。`std.*` の解決では、active target overlay の `targets/<target>/std/` を先に探し、見つからなければ共通 `std/` を探します。

## 対象環境

初期ターゲットは Apple Silicon Mac に限定します。

- CPU: Apple Silicon / ARM64
- OS: macOS
- 出力形式: Mach-O executable

短期的には Intel Mac、Linux、Windows、他 CPU アーキテクチャへの対応を実装対象に含めません。対象を限定することで設計を単純にし、ARM64 macOS 向けコンパイラとしての完成度を優先します。

ただし、長期的にはクロスコンパイルと他 OS / 他アーキテクチャへ拡張できる基盤を残します。ターゲット依存部分は、命令エンコード、実行ファイル形式、primitive lowering、標準ライブラリの OS 境界に閉じ込めます。言語仕様、型システム、所有権、借用、region、標準ライブラリの上位 API はターゲット非依存に保ちます。

初期ターゲット名は `arm64-macos` とします。将来 target の外枠として、`x64-linux`、`arm64-linux`、`x64-windows`、`arm64-windows` を予約します。これらは認識する target 名として扱いますが、backend、実行ファイル writer、primitive set、target std overlay が揃うまでは実装済み target とは見なしません。

初期段階では実際のクロスコンパイルは無効にし、`arm64-macos` を既定 target とします。ただし、コンパイラ内部では host と target を分けます。`.nocter-arm64-macos/` は ARM64 macOS 上で動く `nocter` を含む host package であり、その中の `targets/arm64-macos/` が ARM64 macOS 向けの target overlay です。

将来のクロスコンパイルでは、同じ host package の中に出力先 target を追加します。

```text
.nocter-arm64-macos/
    nocter
    std/
    targets/
        arm64-macos/
            std/
        x64-linux/
            std/
        arm64-linux/
            std/
        x64-windows/
            std/
        arm64-windows/
            std/
```

想定コマンド:

```sh
nocter build app.nct
nocter build app.nct --target arm64-macos
nocter build app.nct --target x64-linux
```

`--target` を省略した場合は、host と同じ target を使います。初期実装で実際に出力できる target は `arm64-macos` のみです。予約済み target を指定した場合は、target 名を認識した上で未実装エラーにします。

```text
error: target x64-linux is recognized but not implemented
```

## 設計方針

### パス由来モジュール

Nocter には `module` 宣言を置きません。モジュール名は import root からの相対ファイルパスで決まります。

```text
examples/word_count.nct                                  => examples.word_count
.nocter-arm64-macos/std/io.nct                           => std.io
.nocter-arm64-macos/targets/arm64-macos/std/os/macos.nct => std.os.macos
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

### 標準ライブラリが primitive 境界を持つ

標準ライブラリだけは、OS / CPU の低レベル機能へ降りるための型付き `primitive` 宣言を持ちます。任意の ARM64 アセンブリを文字列として書く `asm` は初期仕様では採用しません。

```nct
pub copy struct SyscallResult {
    pub value: usize
    pub errno: i32
}

pub primitive syscall3(
    number: usize,
    a0: usize,
    a1: usize,
    a2: usize,
): SyscallResult

pub primitive trap(): never
pub primitive unreachable(): never
```

`primitive` は高級言語とコンパイラ内蔵の低レベル実装を接続するための境界です。初期仕様では Nocter home の共通 `std/` と active target overlay の `std/` 内だけで宣言できます。一般ユーザーコードは `primitive` を宣言できません。

初期 `arm64-macos` target primitive set v0 は、`syscall0` から `syscall6`、`trap`、`unreachable` だけです。別枠として、target 非依存の `std.ptr` core pointer primitive を持ちます。`print`、`exit`、file 操作、allocator、`String`、`Buffer` は primitive にしません。これらは標準ライブラリの通常 API として実装します。

任意 `asm` ではなく型付き `primitive` に絞る理由は次の通りです。

- 型安全性を維持するため
- 最適化の余地を壊さないため
- ABI や呼び出し規約の破壊を防ぐため
- 標準ライブラリの低レベル境界を小さく監査可能にするため
- 標準ライブラリ機能の追加ごとに compiler primitive を増やさないため

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
- `.nocter-<host>/` に `nocter` コマンドと標準ライブラリをまとめる配布モデル
- 標準ライブラリだけが低レベルへ降りる、型付き `primitive` 境界の設計
- GC なしで、所有権、借用、region、明示 allocator によってメモリ安全性を目指す設計
- Apple Silicon macOS / Mach-O に対象を絞り、汎用性より完成度を優先する実装
- 初期実装を `arm64-macos` に絞りつつ、target 依存部分を分離する設計

つまり Nocter は、言語表面では堅実さを優先し、コンパイル経路、配布モデル、標準ライブラリと `primitive` の境界、GC に頼らないメモリモデルで独自性を出します。

## コンパイラの責務

コンパイラ本体は、次の処理を自前で実装します。

- Lexer
- Parser
- AST 生成
- 型チェック
- IR 生成（必要な場合）
- Nocter ABI v0 に基づくデータレイアウトと呼び出し規約
- target 別の命令生成
- target 別の実行ファイル生成
- 最小限のリンカ機能（必要な場合）

外部アセンブラや外部リンカには依存しません。初期ターゲットでは ARM64 命令のエンコードと Mach-O ファイルの構築をコンパイラが直接行います。将来ターゲットを追加する場合も、外部ツールチェーンに依存せず、target backend を追加する方針です。

## Nocter ABI

Nocter は C ABI 互換を目指しません。通常の Nocter 関数、method、`drop`、`primitive` は Nocter 独自の ABI を使います。

初期 ABI は `Nocter ABI v0` とし、対象は `arm64-macos` だけです。

基本方針:

- 64-bit word、little-endian、stack 16-byte alignment
- `x0-x7` を引数と直接戻り値に使う
- `x8` を indirect return pointer に使う
- `x19-x28` は callee-saved
- `struct` は宣言順 layout、field reordering なし
- `enum`、`T?`、`T!E` は `u32` tag と payload で表す
- `StringView`、`View<T>`、`WriteView<T>` は `ptr + len` の 2 word layout
- 16 bytes 以下の値は直接渡し、16 bytes を超える値は pointer 経由で渡す
- `drop` は `x0 = &+Self`、戻り値なし
- `primitive` も Nocter ABI の境界を通り、OS syscall ABI は backend 内に隠す

C 連携が必要になった場合は、将来 `extern "c"` のような別 ABI を明示的に追加します。C ABI へ暗黙に寄せると、`T?`、`T!E`、move-only、drop、region の設計が歪むためです。

## 標準ライブラリ

標準ライブラリは、配布物では `.nocter-<host>/std/` と `.nocter-<host>/targets/<target>/std/` に配置します。現在の開発環境では `.nocter-arm64-macos/std/` と `.nocter-arm64-macos/targets/arm64-macos/std/` を使います。

共通 `std/` は target 非依存の API を置く場所です。`targets/<target>/std/` は syscall、process ABI、trap、低レベル allocator 境界など、target に依存する標準ライブラリ実装を置く場所です。どちらの物理配置から読まれても、ユーザーが import するモジュール名は `std.*` のままです。

構成例:

```text
.nocter-arm64-macos/
    nocter
    std/
        prelude.nct
        io.nct
        mem.nct
        os.nct
        ptr.nct
        string.nct
    targets/
        arm64-macos/
            std/
                process.nct
                os/
                    macos.nct
        x64-linux/
            std/
        arm64-linux/
            std/
        x64-windows/
            std/
        arm64-windows/
            std/
```

利用者は必要な機能を import して使います。

```nct
import std.io.print
```

標準ライブラリは原則として Nocter で記述します。初期 `arm64-macos` では、OS syscall、trap、unreachable のように Nocter だけでは表現できない箇所だけ `primitive` 宣言によってコンパイラ内蔵の低レベル実装へ接続します。allocator は primitive ではなく、標準ライブラリの通常 API として扱います。

## OS Error Model

OS error は target 固有の raw error を common std の公開 error へ変換し、最後に domain error としてユーザーへ見せます。

採用する層構造:

```text
std.os.macos
    SyscallResult
    Errno
    syscall number
    macOS errno mapping

std.os
    Platform
    OSErrorKind
    OSError

std.io / std.process
    IOError
    File
    print
    exit
```

`SyscallResult` と `Errno` は target overlay の低レベル型です。通常のユーザー向け API はこれらを返さず、`std.os.OSError` や `std.io.IOError` へ変換します。

```text
std.os.macos.syscall3
    -> SyscallResult
    -> Errno
    -> std.os.OSError
    -> std.io.IOError
```

common `std.os` には `Errno` という名前を置きません。Windows は errno ではないため、公開 API は `OSError` に統一します。`OSError.code` は macOS / Linux では errno、Windows では将来 Win32 error code など target が定める raw code になります。

`std.process.exit(code): never` は標準ライブラリ API です。compiler primitive ではありません。target overlay の syscall を使って実装し、万一 OS の exit 操作から戻った場合は `trap()` します。

`std.process` はユーザー向け module 名ですが、`exit` は process ABI に依存するため、初期実装では target overlay 側に物理配置します。利用者は配置を意識せず `import std.process.exit` で使います。

## ランタイム

現時点では、独立したランタイムライブラリを持たない方針です。標準ライブラリの `primitive` 宣言が初期ターゲット `arm64-macos` と最小限の橋渡しを行います。将来は `.nocter-<host>/targets/<target>/std/` に target ごとの OS 境界の primitive 実装を追加します。

GC は採用しません。Nocter は実行時ガベージコレクタにメモリ管理を任せる言語ではなく、コンパイル時に所有権、参照の寿命、破棄責任を検査する言語を目指します。

想定する層構造:

```text
高級言語
    |
    v
標準ライブラリ
    |
    v
primitive
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
- raw pointer は address-carrying value として扱い、初期仕様では dereference を一般ユーザーに提供しない

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

初期化と代入の基本規則:

- local variable は必ず初期化する
- `let` / `var` は initializer 必須
- `let` は再代入できない
- `var` は再代入できる
- 再代入では、右辺の評価に成功してから古い値を `drop` し、新しい値を格納する
- 右辺の `try` が失敗した場合、古い値は置き換えられず、通常の `try` 伝播と scope-end `drop` に従う
- 借用中の値には再代入できない
- 非copy値を既存の値から代入する場合は `move` が必要
- copy 型は通常の代入で copy する
- フィールド代入も同じ所有権規則に従う
- `+=` などの複合代入も writable place と借用規則に従う

評価順序と一時値の基本規則:

- 式は左から右に評価する
- 関数引数も左から右に評価する
- `method` 呼び出しでは receiver を最初に評価する
- `??` と三項条件演算子は必要な側だけ評価する
- 一時値は原則として文末で `drop` する
- block、`if` body、`match` arm、loop body は scope を作る
- scope 終了時は local 変数を宣言の逆順で `drop` する
- `try` / `return` / `fail` / `break` / `continue` で途中離脱する場合も、離脱で抜ける scope の `drop` を実行する
- 一時値から作った borrow や view を文の外へ逃がせない
- 初期仕様では一時値の lifetime extension を採用しない

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

再代入は、古い所有値を安全に破棄してから新しい値を入れます。

```nct
var file = try File.open(path)

file = try File.open(other_path)
```

非copy値を別の変数から移す場合は `move` を使います。

```nct
var a = try File.open(path_a)
var b = try File.open(path_b)

a = move b
```

一時的な所有値から view を取り出して外へ残すことは禁止します。

```nct
let view = (try String.copy(allocator, "abc")).view() // error
```

所有値を束縛してから view を作ります。

```nct
var text = try String.copy(allocator, "abc")
let view = text.view()
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

所有値の破棄はスコープ終了時に自動で行います。`drop` は trait ではなく、`impl` 内に置ける専用構文です。`drop` は戻り値型を書かず、`pub` も付けません。明示的に早く破棄したい場合は `drop value` 文を使います。

```nct
impl File {
    drop(file: &+Self) {
        std.os.close(file.fd).ignore()
    }
}

var file = try File.open(path)
drop file
```

一時的な大量確保には、言語構文として `region` を使います。`region` は allocator から短命な一時領域を作り、block 終了時にその領域の確保をまとめて解放する仕組みです。

```nct
region scratch using allocator {
    let source = try read_file(scratch.allocator(), "main.nct")
    let tokens = try lex(scratch.allocator(), source.view())
}
```

`scratch` は region に付けた block-local binding 名であり、特別な名前ではありません。`temp`、`work`、`arena` など別の名前も使えます。

`scratch.allocator()` は region allocator を取り出す標準ライブラリ API の例です。コンパイラは `allocator` という名前を特別扱いするのではなく、region から派生した allocator value の provenance を追跡します。

`region` を抜けると、まず block 内の所有値を通常通り逆順に `drop` し、その後で region allocator が残りの region 確保をまとめて解放します。`return`、`fail`、`break`、`continue` で region block を抜ける場合も同じ cleanup を行います。`never` 呼び出しは stack unwinding ではないため、呼び出し元 region の cleanup を暗黙には保証しません。

コンパイラは、region 内で確保した所有値、region 由来の borrow、`StringView` / `View<T>` などの view が region の外へ漏れないことを検査します。copy 値でも、region 由来の参照や backing storage を含む場合は外へ持ち出せません。純粋な `Int` や統計値のように region へ依存しない copy 値だけを外へ持ち出せます。

`Allocator`、`AllocError`、`Layout`、`RawBuffer` は `std.mem` の普通の公開 API として定義します。コンパイラは `Allocator` という名前を特別扱いしません。特別なのは `region ... using ...` 構文と、region 由来 allocator の provenance tracking だけです。

```nct
import std.mem as mem
import std.mem.{Allocator, AllocError, Layout, RawBuffer}

var allocator = mem.page_allocator()
let buffer = try mem.alloc(&+allocator, 4096, 16)
mem.free(&+allocator, move buffer)
```

確保失敗は `AllocError` を使う fallible value として扱います。OOM を暗黙に `panic` / `abort` しません。`RawBuffer` は低レベル API であり、通常のコードでは `String` や `Buffer<T>` のような所有型を使います。

## Raw Pointer と Address API

Nocter は raw pointer 型 `*T` を持ちます。`*T` は所有権でも borrow でもなく、単なる address-carrying value です。

基本規則:

- `*T` は copy
- `*T` は non-null
- null が必要なら `*T?`
- `*T` は lifetime を延長しない
- `*T` は read / write 権限を表さない
- 初期仕様では一般ユーザーコードに raw pointer dereference を提供しない

pointer と address の変換は `std.ptr` に置きます。

```nct
pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref<T>(value: &T): *T
pub primitive from_ref_mut<T>(value: &+T): *T
```

`usize` から pointer を作る `from_addr<T>(address: usize): *T` は、初期仕様では共通 `std/` と active target overlay の内部だけで使える制限 API です。一般ユーザーコードからは呼べません。

`View<T>`、`WriteView<T>`、`StringView` は `ptr()` と `len()` を持ちます。syscall に buffer を渡す場合は、`ptr()` で raw pointer を取り、`std.ptr.addr(...)` で `usize` へ変換します。

```nct
import std.ptr
import std.os.macos as os

let bytes = text.bytes()
let result = os.syscall3(
    SYS_write,
    fd as usize,
    ptr.addr(bytes.ptr()),
    bytes.len(),
)
```

`std.ptr` の関数は target 非依存の core pointer primitive です。OS 境界の `std.os.macos.syscall0..6` とは別扱いです。これは raw pointer 型そのものに必要な最小操作であり、`print`、`exit`、file 操作、allocator、`String`、`Buffer` を compiler primitive にするものではありません。

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
    pub method (text: Self).ptr(): *u8
    pub method (text: Self).len(): usize
    pub method (text: Self).bytes(): View<u8>
}
```

`View<u8>` は任意のバイト列を表し、UTF-8 であるとは限りません。`StringView` からは `View<u8>` を得られますが、`View<u8>` から `StringView` を作る場合は UTF-8 検証を必要とします。

`"abc"` を `&String` として渡す暗黙変換は採用しません。`&String` は実在する所有 `String` への borrow であり、文字列リテラルは `StringView` として扱います。

byte literal は `b'...'` と書き、型は `u8` です。裸の `'...'` は初期仕様では採用せず、将来の `Char` 用に空けておきます。

```nct
let a: u8 = b'a'
let newline: u8 = b'\n'
let raw: u8 = b'\xFF'
```

文字列リテラルと byte literal では、`\n`、`\r`、`\t`、`\0`、`\\`、`\"`、`\'`、`\xNN` を使えます。文字列リテラルは UTF-8 の `StringView` です。byte literal は escape 解決後にちょうど 1 byte でなければなりません。

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

collection 用の操作は標準ライブラリの通常メソッドとして用意します。`len()`、`get()`、`ptr()`、`view()`、`write_view()` は collection / view の基本 API です。将来的に `iter()` / `next()` を用意する場合も、それらは普通のメソッドであり、`for` 構文が名前で特別扱いすることはありません。

```nct
var iter = read.iter()

loop {
    if let byte = iter.next() {
        use(byte)
    } else {
        break
    }
}
```

## 型システム

Nocter は静的型付け言語として設計します。

普段使いの整数型として `Int` を採用します。`Int` は `i32` の alias です。整数リテラルは文脈型があればその整数型になり、文脈がなければ `Int` になります。

```nct
let count = 10      // Int
let size: u64 = 10  // u64
```

固定幅整数型として `i8`、`i16`、`i32`、`i64`、`u8`、`u16`、`u32`、`u64` を持ちます。非リテラルの整数値同士は暗黙変換しません。

整数演算は同じ型同士で行います。整数リテラルだけは文脈型に合わせますが、変数や式の結果は暗黙変換しません。

```nct
let a: u32 = 10
let b: u64 = 20

let c = a + b          // error
let d = (a as u64) + b // OK
```

明示変換は `expr as Type` と書きます。`as` は lossless な安全変換だけに使います。narrowing、符号変更、切り捨ては `as` では許可せず、明示 API を使います。

```nct
let x: u32 = 10
let y: u64 = x as u64  // OK

let signed: Int = 10
let unsigned = signed as u64 // error

let big: u64 = 300
let small = big as u8      // error
let checked = u8.checked(big)   // u8?
let truncated = u8.truncate(big) // u8
```

通常の整数演算で overflow した場合は trap します。wrapping 演算は通常演算ではなく、明示 API で扱います。division by zero と、型の bit 幅以上の shift も trap します。`bool` と整数の暗黙変換は行いません。

比較と論理演算は、初期仕様では小さく保ちます。

- `==` / `!=` は同じ型同士に限定する
- ordering 比較 `<` / `<=` / `>` / `>=` は同じ数値型同士に限定する
- `&&` / `||` は `bool` 専用で短絡評価する
- `!expr` は `bool` 専用
- 単項 `-expr` は数値型専用
- 単項 `+expr` は採用しない
- ユーザー定義 operator overload は初期仕様では採用しない
- `==` を struct に自動生成しない
- payload を持たない enum の比較は許可する
- payload を持つ enum の比較は初期仕様では採用せず、`match` / `if expr is Pattern` を使う

演算子の優先順位は、call / method / index / field を最も高くし、`??` と三項条件演算子を低くします。`&&`、`||`、`??`、三項条件演算子は必要な側だけ評価します。

```nct
if count > 0 && state == ScanState.inside_word {
    ...
}
```

例:

```nct
func print(msg: string): void
```

戻り値を持たない関数は `void` を返します。失敗を表す値には、例外ではなく fallible type `T!E` を使います。

`T!E` は成功時に `T`、失敗時に `E` を返す型です。fallible 関数内では `return value` が成功、`fail error` が失敗を表します。

`try` は fallible value の成功値を取り出す構文です。失敗した場合は現在の関数から同じ error で失敗します。例外やスタック巻き戻しではありません。`try` は `T!E` 専用で、`T?` には使いません。

```nct
func open(path: StringView): File!IOError {
    if failed {
        fail IOError.not_found(path)
    }

    return file
}
```

error 型の暗黙変換は行いません。fallible value の失敗側を別の error に変換して離脱したい場合は `try ... catch` を使います。

```nct
func read_all(allocator: &+Allocator, path: StringView): String!AppError {
    var file = try File.open(path) catch error {
        fail AppError.open_failed(path)
    }

    var text = try file.read_to_string(allocator) catch error {
        fail AppError.read_failed(path)
    }

    return move text
}
```

`catch` は例外処理ではありません。`try expr catch error { ... }` は `expr` が失敗した場合だけ `catch` block を実行します。初期仕様では `catch` block は `fail`、`return`、`break`、`continue`、`never` を返す関数呼び出しなどで現在の制御フローを離脱し、通常の末尾到達はできません。`catch` は `T!E` 専用で、`T?` には使いません。

fallible value は `match` で分解しません。`match` は enum 専用に戻し、`ok` / `fail` pattern は採用しません。fallible value は `try` または `try ... catch` で扱います。

値が存在しない可能性は `T?` で表します。`Option<T>` という名前付き型を特別扱いしません。optional 関数では `return value` が present、`return none` が absent を表します。

```nct
func env(name: StringView): StringView? {
    if missing {
        return none
    }

    return value
}

func require_home(): StringView? {
    if let home = env("HOME") {
        return home
    }

    return none
}
```

optional value には default operator `??` を使えます。右結合で、必要な場合だけ右辺を評価します。

```nct
let port = env_int("PORT") ?? config.default_port ?? 8080
```

`T?` も `match` では分解しません。local に present / absent を分岐したい場合は `if let` / `if var` を使います。

```nct
if let home = env("HOME") {
    use(home)
} else {
    use_default_home()
}
```

```nct
if var text = maybe_text {
    text.push("!")
    use(move text)
}
```

`if let` は optional が present の場合に中身を immutable binding として束縛し、`if var` は mutable binding として束縛します。`none` の場合は `else` body を実行します。`else` は省略できます。`else if let` / `else if var` も使えます。`if var` は元の optional へ値を書き戻す構文ではありません。

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

ループは `while`、`loop`、range 専用 `for`、`break`、`continue` を採用します。`while` の条件は `bool` です。`loop` は無限ループです。`break value` のようにループから値を返す構文は初期仕様では採用しません。

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

初期 `for` は half-open range 専用です。

```nct
for i in 0..<bytes.len() {
    let byte = bytes[i]
    use(byte)
}
```

`start..<end` は `start` 以上 `end` 未満を表します。`start` と `end` は loop 開始前に 1 回だけ評価します。step は常に `+1`、loop 変数は immutable binding です。`for item in collection` は初期仕様では採用しません。collection の走査は `for i in 0..<items.len()` と index、または標準ライブラリの通常メソッド `iter()` / `next()` を明示的に使います。

通常復帰しない処理は `never` で表します。`never` は値を持つ型ではなく、呼び出し元へ戻らない制御フローを表す型です。`panic`、`abort`、`exit`、停止しないイベントループ、到達不能分岐の明示に使います。`panic`、`abort`、`exit` は標準ライブラリ API の候補名であり、コンパイラが特別扱いする名前ではありません。

```nct
func panic(message: StringView): never {
    std.process.abort(message)
}

func require_path(path: StringView?): StringView {
    if let value = path {
        return value
    }

    panic("missing path")
}
```

`never` を返す関数を呼んだ後の同一 block 内の文は到達不能です。Nocter は初期仕様で到達不能コードをコンパイルエラーにします。`never` は値を生成しないため、三項条件演算子や `try ... catch` の分岐で必要な型に収まりますが、変数に格納する値としては存在しません。`void` 以外の関数はすべての到達可能経路で値を返すか、`fail` / `return none` / `never` などで経路を終端する必要があります。`never` 呼び出しは例外や stack unwinding ではないため、呼び出し元の `drop` 実行を暗黙に保証しません。

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

単一の enum pattern だけを見たい場合は `if expr is Pattern` を使います。

```nct
if error is AppError.open_failed(path) {
    report(path)
} else if error is AppError.read_failed(path) {
    report(path)
} else {
    report_other(error)
}
```

`if is` の `else` は省略可能です。`else if enum_expr is Pattern` も使えます。payload binding は then body の中だけで有効です。

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
10. `primitive`
11. `print`
12. `import`
13. `struct`
14. `if` / `match`
15. `while` / `loop` / range `for` / `break` / `continue`
16. 所有型のコピー禁止
17. `move`
18. `drop`
19. `&T`
20. `&+T`
21. allocator
22. region
23. 標準ライブラリ拡充

この順序は固定ではありません。ただし、`clang` や `ld` に一時的に逃がして Hello World を早く出すことより、最終設計と矛盾しない経路で進めることを優先します。

## 非目標

現在の非目標は次の通りです。

- 初期段階から汎用クロスコンパイラにすること
- 初期段階で複数 OS / 複数 architecture を同時に実装すること
- C 言語ツールチェーンの薄いラッパーにすること
- 外部ランタイムを前提にすること
- GC を前提にすること
- class 継承を言語設計の中心にすること
- 最短手順で Hello World だけを出すこと

Nocter は、まず自己完結した `arm64-macos` 言語処理系として成立することを優先します。長期的な portability は、初期実装の焦点をぼかさず、target 依存部分を分離することで確保します。

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
