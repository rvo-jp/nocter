<div align="center">
  <img src="./assets/logo.svg" alt="Nocter logo" width="128">
  <h1>Nocter</h1>
  <p>
    <img src="https://img.shields.io/badge/target-arm64--darwin-blue" alt="Target: arm64-darwin">
  </p>
</div>

Nocter は、人間が読みやすく、AI も読み書きしやすい静的型付け高級言語を設計し、まず ARM64 macOS 向けのネイティブ実行ファイルへ直接コンパイルすることを目指すコンパイラプロジェクトです。

言語としては、静的型付け・値中心・モジュール指向・低依存システム言語を目指します。class 継承を中心にしたオブジェクト指向言語ではなく、`struct`、関数、モジュール、所有権、借用、標準ライブラリを軸にします。

言語名は Nocter、ソースファイルの拡張子は `.nct` です。

文法と意味論の詳細は [`spec/`](spec/README.md) に章別で記録します。仕様上の採用事項は README の概要より `spec/` を優先します。

最重要方針は、外部ツールやランタイムへの依存をなくすことです。最終的には、ホスト環境ごとの `nocter-v<version>-<host>.tar.gz` を配布し、展開された `.nocter/` ディレクトリを `~/.nocter/` として配置すれば利用できる状態を目指します。

```text
~/.nocter/
    nocter
    VERSION
    MANIFEST.json
    std/
    targets/
        arm64-darwin/
            std/
```

利用者は `clang`、`as`、`ld`、Xcode Command Line Tools、外部ランタイムライブラリを必要としません。コンパイラ自身が、字句解析から Mach-O 実行ファイルの生成までを一貫して担います。

## コンパイラ実装言語

v0 のコンパイラ本体は Rust で実装します。Rust と Cargo は開発時だけの依存であり、利用者向けの配布物には Rust toolchain を含めず、利用者にも要求しません。利用者が受け取る完成品は、`.nocter/nocter`、`VERSION`、`MANIFEST.json`、`std/`、`targets/` です。

Rust を採用する理由は、所有権、pattern matching、バイナリデータ処理、エラー処理がコンパイラ実装に向いており、C/C++ よりメモリ安全性を保ちやすく、Zig より成熟した実装事例とエコシステムが多いためです。

ただし、Nocter の出力経路を Rust ecosystem に任せるわけではありません。LLVM、`clang`、`as`、`ld`、外部 linker wrapper は使わず、ARM64 instruction encoder、Mach-O writer、Nocter ABI lowering はコンパイラ内部で実装します。Rust 実装は self-hosting までの bootstrap layer とし、長期的には Nocter で Nocter compiler を書ける状態を目指します。

## ディレクトリ構成

このリポジトリでは、コンパイラの実装と利用者へ配布する完成品を分けます。

```text
README.md
    ユーザー向けの全容

spec/
    ユーザー向けの言語仕様書
    README.md
    guides/
        ai.md
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
    12-diagnostics.md
    13-lexical-grammar.md
    14-tooling-editor-integration.md
    15-command-line-interface.md
    16-source-style-formatting.md
    examples/
        valid/
        invalid/

compiler/
    README.md
        コンパイラ開発者向けの入口
    TODO.md
        コンパイラ作業の短期 handoff
    docs/
        architecture.md
        implementation-status.md
        backend-v0.md
        roadmap.md
    Cargo.toml
        Rust 製コンパイラ実装の crate manifest
        開発時のみ使用し、利用者向け配布物には含めない
    Cargo.lock
        Rust 製コンパイラ実装の lockfile
    rust-toolchain.toml
        コンパイラ開発用 Rust toolchain 設定
    src/
        コンパイラ本体の実装

.nocter/
    nocter
    VERSION
    MANIFEST.json
    std/
        prelude.nct
        fmt.nct
        io.nct
        mem.nct
        os.nct
        ptr.nct
        string.nct
    targets/
        arm64-darwin/
            std/
                io_impl.nct
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

`README.md` は Nocter の目的、対象環境、配布形態、設計思想を説明する入口です。`spec/README.md` は Nocter を書く人向けの言語仕様書の目次であり、詳細な仕様は `spec/` に章別で置きます。`compiler/README.md` はコンパイラ開発者向けの入口であり、実装状況や内部設計は `compiler/docs/` に置きます。

`compiler/` は Rust 製 bootstrap compiler の開発用ソースツリーです。`.nocter/` は現在の開発環境向けの完成品配置先であり、コンパイラ本体と標準ライブラリを含みます。このディレクトリは生成物・配布物なので git 管理しません。

ユーザーは `nocter-v<version>-arm64-darwin.tar.gz` を展開し、生成された `.nocter/` をホームディレクトリなどに配置して、次のように PATH を通します。

```sh
export PATH="$HOME/.nocter:$PATH"
```

標準ライブラリは `NOCTER_HOME` が指定されていればそこから探し、指定がなければ実行中の `nocter` コマンドの実体パスを解決し、その親ディレクトリを Nocter home として使います。`cwd/.nocter` や `~/.nocter` は自動探索しません。`std/...` の解決では、active target overlay の `targets/<target>/std/` を先に探し、見つからなければ共通 `std/` を探します。

## 対象環境

初期ターゲットは Apple Silicon Mac に限定します。

- CPU: Apple Silicon / ARM64
- OS: macOS
- 出力形式: Mach-O executable

短期的には Intel Mac、Linux、Windows、他 CPU アーキテクチャへの対応を実装対象に含めません。対象を限定することで設計を単純にし、ARM64 macOS 向けコンパイラとしての完成度を優先します。

ただし、長期的にはクロスコンパイルと他 OS / 他アーキテクチャへ拡張できる基盤を残します。ターゲット依存部分は、命令エンコード、実行ファイル形式、primitive lowering、標準ライブラリの OS 境界に閉じ込めます。言語仕様、型システム、所有権、借用、region、標準ライブラリの上位 API はターゲット非依存に保ちます。

初期ターゲット名は `arm64-darwin` とします。将来 target の外枠として、`x64-linux`、`arm64-linux`、`x64-windows`、`arm64-windows` を予約します。これらは認識する target 名として扱いますが、backend、実行ファイル writer、primitive set、target std overlay が揃うまでは実装済み target とは見なしません。

初期段階では実際のクロスコンパイルは無効にし、`arm64-darwin` を既定 target とします。ただし、コンパイラ内部では host と target を分けます。配布アーカイブ `nocter-v<version>-arm64-darwin.tar.gz` は ARM64 macOS 上で動く `nocter` を含み、展開 root は常に `.nocter/` です。その中の `targets/arm64-darwin/` が ARM64 macOS 向けの target overlay です。

将来のクロスコンパイルでは、同じ Nocter home の中に出力先 target を追加します。

```text
~/.nocter/
    nocter
    VERSION
    MANIFEST.json
    std/
    targets/
        arm64-darwin/
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
nocter build app.nct -o app
nocter build app.nct --entry start
nocter run app.nct
nocter run app.nct --entry start
nocter app.nct
nocter app.nct --entry start
nocter check app.nct
nocter check app.nct --entry start
nocter check app.nct --format json
nocter check app.nct --entry start --format json
nocter fmt app.nct
nocter fmt --check app.nct
nocter tokens app.nct --format json
nocter ast app.nct --format json
nocter --version
nocter doctor
nocter lsp
nocter build app.nct --target arm64-darwin
nocter build app.nct --target x64-linux
```

`build` は1つの root `.nct` file を受け取り、`-o path` で出力 executable path を指定します。`run` は一時 Mach-O executable を生成して実行し、終了後に削除します。`nocter app.nct` は quick trial 用の短縮形で、明示形は `nocter run app.nct` です。`--entry name` は root file の top-level `func name()` を executable entry として選びます。省略時は `main` です。`fmt` は指定された1つの `.nct` source file だけを整形し、import graph は辿りません。

RAM-only 実行や JIT 実行は v0 では採用しません。`run` も `build` と同じ parser、type checker、ownership checker、ARM64 code generator、Mach-O writer を通ります。違いは、成果物を project に残すか、一時 executable として実行後に削除するかだけです。

`--target` を省略した場合は、host と同じ target を使います。初期実装で実際に出力できる target は `arm64-darwin` のみです。予約済み target を指定した場合は、target 名を認識した上で未実装エラーにします。

```text
error: target x64-linux is recognized but not implemented
```

`VERSION` は release version を1行で持ちます。`MANIFEST.json` は release、host、default target、実装済み target、compiler path、標準ライブラリ path、archive 情報を持つ tool 向け metadata です。v1 では checksum は持たず、release pipeline と hash 検証方針が決まってから追加します。

```text
.nocter/
    nocter
    VERSION
    MANIFEST.json
    std/
    targets/
```

```json
{
  "schema": "nocter.manifest",
  "schema_version": 1,
  "release": "0.1.0",
  "host": "arm64-darwin",
  "default_target": "arm64-darwin",
  "compiler": {
    "path": "nocter"
  },
  "std": {
    "path": "std"
  },
  "implemented_targets": [
    {
      "name": "arm64-darwin",
      "std_path": "targets/arm64-darwin/std",
      "backend": "arm64",
      "executable": "macho",
      "os": "darwin"
    }
  ],
  "archive": {
    "name": "nocter-v0.1.0-arm64-darwin.tar.gz",
    "root": ".nocter"
  }
}
```

`nocter --version` は compiler release、host、default target を表示します。`nocter doctor` は Nocter home を解決し、`VERSION`、`MANIFEST.json`、`std/`、`targets/<target>/` の整合性を検査します。

build profile は安全性を変えません。将来 debug / release や最適化 option を持つ場合でも、bounds check、整数 overflow check、division by zero check、shift range check、invalid bool / enum tag check、`unreachable()` 到達 check は常に有効です。release build が速くなる場合は、compiler が check 不要を証明して削除できた場合だけです。unchecked arithmetic、unchecked indexing、unchecked enum-tag operation は v0 の一般ユーザー API として公開しません。

## 設計方針

### パス由来モジュール

Nocter には `module` 宣言を置きません。1つの `.nct` ファイルが1つの module になり、module identity は canonical file path から決まります。

```text
examples/word_count.nct                                  => examples/word_count
~/.nocter/std/io.nct                                     => std/io
~/.nocter/targets/arm64-darwin/std/os/macos.nct           => std/os/macos
```

ファイルパスを唯一の情報源にすることで、ファイル位置とモジュール宣言の不一致を防ぎます。

import は明示的な名前指定を基本にします。`./` または `../` で始まる path は現在ファイルから見た `.nct` を探し、それ以外の path は active Nocter home、通常は `~/.nocter/` 内から探します。

```nct
from std/mem import Allocator
from std/io import File, stdout, stderr
from std/io import File as StdFile
from ./config import AppConfig
pub from std/string import String

import std/io as io
```

`pub from` は、import した公開名を現在 module の公開 API として再公開します。prelude や façade module で使います。`pub(nocter)` の名前は通常公開 API として `pub from` できません。

ワイルドカード import、bare import、namespace alias re-export、absolute path、import path 内の `.nct` 拡張子は初期仕様では採用しません。

user project module は、compiler が内部的にファイル先頭へ synthetic `use std/prelude` を持つものとして扱います。source text は書き換えず、diagnostic や formatter は元の source を基準にします。synthetic prelude は user project module ごとに独立して適用され、`.nocter/std/` と `targets/<target>/std/` には適用しません。明示的な `use std/prelude` は書いてもよいですが、user project module では冗長です。

prelude は小さく保ちます。`Int` のような基本 alias、`Error` / `ErrorCode`、所有文字列 `String` のような ubiquitous な標準ライブラリ型だけを置きます。`str`、`error`、`[T]`、`[+T]` は compiler built-in の型構文です。`File`、`Allocator`、`print`、`stdout`、`stderr`、`args`、`env`、`cwd`、`exit`、`abort` は domain module から明示 import します。project-wide prelude 設定は初期仕様では採用しません。

v0 では package manifest と project root discovery を採用しません。compiler に渡した `.nct` が root file です。executable の entry point は compiler の entry setting で選ばれ、v0 の既定値は root file の top-level `func main()` です。`--entry start` を指定した場合は root file の top-level `func start()` を選びます。`main` や `start` は予約語や built-in ではなく、通常の関数名です。compiler は root file から import graph を辿り、到達した `.nct` ファイル全体を1つの compile unit として name resolution、type checking、ownership checking、code generation します。separate compilation、incremental build、package registry、lockfile、workspace は v0 では扱いません。

```text
project/
    app.nct
    src/
        config.nct
        parser.nct
```

```sh
nocter build app.nct -o app
```

```nct
// app.nct
from std/io import print
from ./src/config import Config

func main(): i32! {
    let config = Config.default()
    print(config.name)?
    return 0
}
```

```nct
// src/config.nct
pub struct Config {
    pub name: str
}

impl Config {
    pub func default(): Config {
        return Config{
            name: "Nocter",
        }
    }
}
```

モジュール内の定義はデフォルトで private です。他モジュールから import できる API には `pub` を付けます。Nocter 配布物内部だけに公開する API には `pub(nocter)` を付けます。`struct` のフィールドと `impl` 内の関数もデフォルト private です。

```nct
pub struct File {
    fd: i32
}

impl File {
    pub func open(path: str): File! {
        ...
    }
}
```

```nct
pub(nocter) primitive from_addr<T>(address: usize): *T
```

`pub(nocter)` は active Nocter home 内、つまり共通 `std/` と `targets/<target>/std/` の module だけで書けます。公開先も active Nocter home 内だけです。user project からは import できません。`nocter` は `pub(nocter)` の中だけで意味を持つ contextual な scope 名で、通常の予約語ではありません。

v0 では attribute 構文を採用しません。`@inline`、`@repr(...)`、`@target(...)`、`@test`、`@deprecated` のような構文はありません。layout は Nocter ABI v0、target 分岐は `~/.nocter/targets/<target>/std/` の overlay、低レベル境界は active Nocter home 内の typed `primitive`、visibility は `pub` / `pub(nocter)` で表します。`@` は将来の attribute-like syntax 用に予約しますが、v0 の source では string literal、byte literal、comment の外に書けません。

### ソース形式と字句規則

`.nct` source file は UTF-8 とします。改行は LF と CRLF を受け付け、compiler 内部では LF に正規化します。識別子は ASCII の `[A-Za-z_][A-Za-z0-9_]*` に限定し、予約語と単独の `_` は通常名として使えません。module の file / directory name は snake_case identifier にします。

comment は `// line comment` と `/* block comment */` を採用します。doc comment は次の宣言や binding に付く `/// line doc` / `/** block doc */` と、file 全体に付く `//! file doc` / `/*! file doc */` を採用します。`////`、`/**/`、`/*** ... */` は通常 comment として扱います。通常 comment は実装メモ、doc comment は将来の hover、API docs、LSP 用の説明として扱います。block comment の入れ子は v0 では採用しません。

文末セミコロンは採用しません。文は改行または `}` で区切ります。空白は token の区切りにだけ使い、indent に構文上の意味はありません。

source style は formatter が統一します。compiler は空白や改行に寛容にし、style 違反を compile error にはしません。`nocter fmt app.nct` は指定された1ファイルを公式 style に書き戻し、`nocter fmt --check app.nct` は CI や editor integration 用に差分有無だけを検査します。formatter output が仕様書、README、`example.nct` の正準表記です。current formatter v0 は comment を安全に保持する実装をまだ持たないため、comment を含む file は書き換えず diagnostic を出します。

初期 style は、indent 4 spaces、`a: Type` は `:` の後だけ空白、`name = value` と binary operator は前後に空白、`func(arg)` と `file.write(arg)` は callee / receiver に密着、block の `{` は同じ行、fallible type は `T!`、fallible optional success は `T?!` とします。

整数リテラルは decimal、hex `0xFF`、binary `0b1010` を採用し、桁区切り `_` を `1_000` や `0xFF_FF` のように数字の間で使えます。float literal は v0 では採用しません。

文字列リテラルは `"..."`、byte literal は `b'...'` です。裸の `'...'` は v0 では採用せず、将来の `Char` 設計用に空けます。escape は `\n`、`\r`、`\t`、`\0`、`\\`、`\"`、`\'`、`\xNN` を初期仕様とします。

### 静的型付け・値中心・モジュール指向

Nocter は、class を言語の中心に置きません。データは `struct`、振る舞いは関数、名前空間と再利用単位はモジュールで表現します。

```nct
struct File {
    fd: i32
}

func write(file: &+File, data: str): void! {
    ...
}
```

struct の値は `Type{ field: value, ... }` で作ります。`init`、`new`、constructor 専用構文は作りません。

```nct
pub struct User {
    pub id: u64
    name: String
}

let user = User{
    id: 1,
    name: String.copy(allocator, "alice")?,
}
```

struct literal は全 field を1回ずつ初期化します。field の順序は自由ですが、未知 field、重複 field、未初期化 field はコンパイルエラーです。private field は同じ module 内でしか初期化できません。初期化ロジックや検証が必要な場合は、通常の associated function を使います。

```nct
impl User {
    pub func create(id: u64, name: String): User {
        return User{
            id: id,
            name: move name,
        }
    }
}
```

field default value、struct update syntax、positional struct、tuple struct、constructor overloading は v0 では採用しません。

`impl` 内の `func` は型に関連付く associated function です。`impl` 内の `method` は receiver を持つメソッドです。`self` / `this` は使わず、receiver 名と borrow 種別を明示します。

```nct
impl File {
    pub func open(path: str): File! {
        ...
    }

    pub method (file: &+Self).write(data: str): void! {
        ...
    }
}
```

`func` は `File.open(path)` のように型から呼びます。`method` は `file.write(data)` のように値から呼びます。`File.write(&+file, data)` のような UFCS 呼び出しは初期仕様では採用しません。

method lookup は v0 では小さく保ちます。receiver が concrete nominal type の場合は、その型の inherent method だけを探します。trait method は concrete value の extension method としては扱いません。receiver が generic type parameter の場合だけ、明示された `T: Trait` bound の method を呼び出せます。inherent method を優先し、候補が複数になる場合は compile error です。曖昧性を解消するための `Trait.method(value, args)` や `<Type as Trait>` 形式は v0 では採用しません。

関数、associated function、method、primitive の呼び出しは v0 では位置引数のみです。引数は書いた順に parameter へ対応し、個数は完全一致します。各引数は通常の文脈型付け、所有権、move、copy、borrow 規則で parameter 型に適合する必要があります。

```nct
func copy(allocator: &+Allocator, source: str): String! {
    ...
}

let text = String.copy(&+allocator, "hello")?
```

parameter は関数本体内では immutable binding として扱います。`var` parameter は v0 では採用しません。owned parameter は関数本体が所有し、move-only parameter は `move name` で別の所有先へ移せます。移されなかった owned parameter は関数 scope 終了時に破棄されます。

```nct
func rename(user: &+User, name: String): void {
    user.name = move name
}

func normalize(value: i32): i32 {
    var current = value

    if current < 0 {
        current = -current
    }

    return current
}
```

`&T` parameter は readonly borrow、`&+T` parameter は readwrite borrow です。`&+T` parameter の binding 自体は再代入できませんが、参照先は変更できます。

```nct
func increment(value: &+Counter): void {
    value.count += 1
}
```

名前付き引数、default parameter、variadic function、function / method overload は v0 では採用しません。引数の多い API は設定用 `struct` を渡します。

```nct
pub struct OpenOptions {
    pub read: bool
    pub write: bool
    pub create: bool
}

let file = File.open_with(path, OpenOptions{
    read: true,
    write: false,
    create: false,
})?
```

複数行の parameter list と argument list では trailing comma を許可します。単一行 list の trailing comma は v0 では許可しません。

```nct
pub func copy(
    allocator: &+Allocator,
    source: str,
): String! {
    ...
}
```

抽象化が必要な場合は、継承階層ではなく `trait` を使います。

```nct
trait Writer {
    method (writer: &+Self).write(data: str): void!
}
```

`impl Trait for Type` は、trait を定義した module または実装対象の nominal type を定義した module でだけ書けます。外部 trait を外部 type へ実装することはできません。同じ trait と type の組み合わせに対する実装は、読み込まれた program 全体で1つだけです。

`enum` は有限個の variant を持つ型です。statement として variant を分岐する場合は `match` を使い、各 arm は `Pattern { ... }` で書きます。fallback には最後の arm として `else { ... }` を使います。v0 では網羅性チェックを延期するため、`else` がない `match` は終端文として扱いません。値を返す enum pattern 分岐には `?{}` を使います。

payload を持たない variant は `Enum.variant`、payload を持つ variant は `Enum.variant(args...)` で作ります。variant constructor は enum 宣言から生まれる構文上の値生成手段であり、通常の関数名や特別な識別子ではありません。unqualified variant constructor は v0 では採用しません。

```nct
let state = ScanState.inside_word
let error = AppError.open_failed(path)
```

```nct
match error {
    AppError.missing_path {
        ...
    }
    AppError.open_failed(path) {
        ...
    }
    else {
        ...
    }
}
```

```nct
return error ?{
    AppError.missing_path : missing_code()
    AppError.open_failed(path) : code_for(path)
    : unknown_code()
}
```

目指す方向は、古典的な OOP ではなく、値型、明示的な所有権、明確なモジュール境界によって大きなプログラムを構成する言語です。

### 高級言語として保つ

ユーザーが書くコードは高級言語であり、ARM64 命令や Mach-O の詳細を意識しない形にします。

```nct
from std/io import print

func main(): i32! {
    print("Hello")?
    return 0
}
```

低レベルの処理はコンパイラと標準ライブラリが引き受けます。

標準の entry function は `func main(): i32!` です。`--entry name` を指定した場合は `func name(): i32!` が同じ役割を持ちます。成功時は返した `i32` が process exit status になり、失敗時は compiler-generated entry wrapper が built-in `error` を stderr へ出力して status `1` で終了します。stderr 出力自体が失敗した場合、その失敗は無視して status `1` で終了します。simple infallible entry point 用に `func main(): void` と `func main(): i32` も v0 では受け付けます。entry function parameters は採用しません。command-line arguments、environment、current working directory、process termination は `std/process` の通常 API で扱います。

```nct
from std/process import args

func main(): i32! {
    let argv = args()?

    if argv.len() < 2 {
        return 1
    }

    return 0
}
```

compiler が生成する低レベル entry code は target の process entry 情報、例えば `argc`、`argv`、`envp` 相当の情報を受け取ります。ユーザーコードはそれらや Mach-O entry ABI を意識せず、`std/process.args()` や `std/process.env(name)` を使います。

### ビルトイン関数を極力作らない

`print`、`args`、`env`、`cwd`、`exit`、`abort`、ファイル操作、文字列操作などは、言語仕様に組み込まず標準ライブラリで提供します。

```nct
pub func print(text: str): void! {
    ...
}
```

コンパイラは `print` という名前を特別扱いしません。言語仕様を小さく保ち、標準ライブラリを通常の言語機能で拡張できる構造を優先します。

### 標準ライブラリが primitive 境界を持つ

標準ライブラリだけは、OS / CPU の低レベル機能へ降りるための型付き `primitive` 宣言を持ちます。任意の ARM64 アセンブリを文字列として書く `asm` は初期仕様では採用しません。

```nct
pub(nocter) copy struct SyscallResult {
    pub value: usize
    pub errno: i32
}

pub(nocter) primitive syscall3(
    number: usize,
    a0: usize,
    a1: usize,
    a2: usize,
): SyscallResult

pub(nocter) primitive trap(): never
pub(nocter) primitive unreachable(): never
```

`trap()` は target が定める illegal instruction、breakpoint、または同等の復帰不能停止へ下げます。`unreachable()` は到達不能の明示で、到達した場合は trap します。どちらも `never` を返し、stack unwinding は行いません。

`primitive` は高級言語とコンパイラ内蔵の低レベル実装を接続するための境界です。初期仕様では Nocter home の共通 `std/` と active target overlay の `std/` 内だけで宣言できます。一般ユーザーコードは `primitive` を宣言できません。一般ユーザーコードから呼べる primitive は、標準ライブラリが明示的に `pub` で公開した小さな API だけです。target syscall primitive のような低レベル境界は `pub(nocter)` にします。

v0 では `unsafe` keyword、`unsafe` block、`unsafe func` を採用しません。一般ユーザーコードは常に safe Nocter code です。低レベル実装の trusted boundary は active Nocter home 内、つまり共通 `std/` と `targets/<target>/std/` に限定します。trusted module も通常の型チェック、所有権、borrow、drop 検査を受けます。

初期 `arm64-darwin` target primitive set v0 は、`syscall0` から `syscall6`、`trap`、`unreachable` だけです。別枠として、target 非依存の `std/ptr` core pointer primitive を持ちます。`print`、`exit`、`abort`、file 操作、allocator、`String`、`Buffer` は primitive にしません。これらは標準ライブラリの通常 API として実装します。

将来の `open_file_raw`、`write_fd_raw`、`mmap_raw` のような typed wrapper も compiler primitive ではなく、target overlay または common std の通常 API として定義します。compiler は OS API 名を特別扱いせず、既存 primitive の module path、名前、正確な signature を検証します。標準ライブラリ機能を増やす通常手段は Nocter code と target overlay であり、compiler primitive の追加ではありません。user project module は v0 でも長期方針でも primitive declaration boundary の外側に置きます。

任意 `asm` ではなく型付き `primitive` に絞る理由は次の通りです。

- 型安全性を維持するため
- 最適化の余地を壊さないため
- ABI や呼び出し規約の破壊を防ぐため
- 標準ライブラリの低レベル境界を小さく監査可能にするため
- 標準ライブラリ機能の追加ごとに compiler primitive を増やさないため

### 自己完結性を優先する

このプロジェクトでは、一般的なコンパイラ実装で使われる外部ツールチェーンを前提にしません。

採用しない方針:

- LLVM を codegen backend として使う
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
- host-specific archive に `nocter` コマンドと標準ライブラリをまとめ、標準配置先を `~/.nocter/` に統一する配布モデル
- 標準ライブラリだけが低レベルへ降りる、型付き `primitive` 境界の設計
- GC なしで、所有権、借用、region、明示 allocator によってメモリ安全性を目指す設計
- Apple Silicon macOS / Mach-O に対象を絞り、汎用性より完成度を優先する実装
- 初期実装を `arm64-darwin` に絞りつつ、target 依存部分を分離する設計

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

初期 ABI は `Nocter ABI v0` とし、対象は `arm64-darwin` だけです。

基本方針:

- 64-bit word、little-endian、stack 16-byte alignment
- `x0-x7` を引数と直接戻り値に使う
- `x8` を indirect return pointer に使う
- `x19-x28` は callee-saved
- `struct` は宣言順 layout、field reordering なし
- `enum`、`T?`、`T!` は `u32` tag と payload で表す
- `str`、`[T]`、`[+T]` は `ptr + len` の 2 word layout
- 16 bytes 以下の値は直接渡し、16 bytes を超える値は pointer 経由で渡す
- `drop` は `x0 = &+Self`、戻り値なし
- `primitive` も Nocter ABI の境界を通り、OS syscall ABI は backend 内に隠す

C 連携が必要になった場合は、将来 `extern "c"` のような別 ABI を明示的に追加します。C ABI へ暗黙に寄せると、`T?`、`T!`、move-only、drop、region の設計が歪むためです。

## 標準ライブラリ

標準ライブラリは、ユーザー環境では `~/.nocter/std/` と `~/.nocter/targets/<target>/std/` に配置します。配布アーカイブの host は archive 名で表し、payload root は常に `.nocter/` です。現在の開発環境でも `.nocter/std/` と `.nocter/targets/arm64-darwin/std/` を使います。

共通 `std/` は target 非依存の API を置く場所です。`targets/<target>/std/` は syscall、process ABI、trap、低レベル allocator 境界など、target に依存する標準ライブラリ実装を置く場所です。どちらの物理配置から読まれても、ユーザーが import する path は `std/...` のままです。

構成例:

```text
~/.nocter/
    nocter
    VERSION
    MANIFEST.json
    std/
        prelude.nct
        fmt.nct
        io.nct
        mem.nct
        os.nct
        ptr.nct
        string.nct
    targets/
        arm64-darwin/
            std/
                io_impl.nct
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
from std/io import print
```

標準ライブラリは原則として Nocter で記述します。初期 `arm64-darwin` では、OS syscall、trap、unreachable のように Nocter だけでは表現できない箇所だけ `primitive` 宣言によってコンパイラ内蔵の低レベル実装へ接続します。allocator は primitive ではなく、標準ライブラリの通常 API として扱います。

## OS Error Model

OS error は target 固有の raw error を common std の公開 record へ変換し、最後に built-in `error` payload としてユーザーへ見せます。

採用する層構造:

```text
std/os/macos
    SyscallResult
    Errno
    syscall number
    macOS errno mapping

std/os
    Platform
    OSErrorKind
    OSError

compiler built-in
    error

std/prelude
    ErrorCode
    Error

std/io / std/process
    File
    stdout
    stderr
    print
    args
    env
    cwd
    exit
    abort
```

`SyscallResult` と `Errno` は target overlay の低レベル型です。通常のユーザー向け API はこれらを返さず、`std/os` の `OSError` を経由して built-in `error` へ変換します。

```text
std/os/macos.syscall3
    -> SyscallResult
    -> Errno
    -> std/os.OSError
    -> error
```

common `std/os` には `Errno` という名前を置きません。Windows は errno ではないため、公開 API は `OSError` に統一します。`OSError.code` は macOS / Linux では errno、Windows では将来 Win32 error code など target が定める raw code になります。

`ErrorCode` は標準ライブラリが提供する `str` alias です。compiler は `ErrorCode` という名前を知らず、`Error.new("std.io.not_found", message)` のような標準ライブラリ constructor が built-in `error` payload の primitive code 表現へ変換します。`ErrorCode` は open な文字列コードなので、ユーザーやライブラリ作者は `"app.config.missing_key"` のような独自コードを追加できます。

`std/io` の初期公開 API は、`File`、`stdout`、`stderr`、`print`、byte read/write、text write に限定します。`print` は compiler built-in ではなく、標準ライブラリ関数です。

```nct
pub struct File {
    ...
}

impl File {
    pub func open(path: str): File!
    pub method (file: &+Self).read(buffer: [+u8]): usize!
    pub method (file: &+Self).write(bytes: [u8]): void!
    pub method (file: &+Self).write_text(text: str): void!

    drop File(file: &+Self) {
        ...
    }
}

pub func stdout(): File
pub func stderr(): File
pub func print(text: str): void!
```

`File.open(path)` は v0 では既存ファイルを読み取り用に開きます。作成、追記、truncate、seek、directory traversal、buffered I/O、async I/O、path object、encoding conversion は初期仕様では採用しません。

`read(buffer)` は読み込んだ byte 数を返し、通常ファイルでは `0` が EOF です。`write(bytes)` は byte view 全体を書き切るか `error` で失敗します。`write_text(text)` は `str` の UTF-8 bytes をそのまま書きます。`print(text)` は `stdout()` へ text を書き、改行は追加しません。

`File` は内部的に owned handle と borrowed process standard stream を区別します。`File.open(path)` で得た `File` の drop は handle を閉じますが、`stdout()` / `stderr()` で得た `File` の drop は process の標準出力 / 標準エラーを閉じません。`drop File` は失敗できないため、close error は v0 では無視します。将来必要なら明示的な `close` API を追加します。

`std/io` は共通の user-facing module です。raw file descriptor や syscall との接続は active target overlay の `std/io_impl` に置き、`pub(nocter)` helper として `std/io` からだけ使います。利用者は `std/io_impl` を import せず、`File`、`stdout`、`stderr`、`print`、`File.open/read/write/write_text` を通じて I/O を扱います。

`std/process` の `args(): [str]!`、`env(name): str?!`、`cwd(): str!`、`exit(code): never`、`abort(): never` は標準ライブラリ API です。compiler primitive ではありません。`args` / `env` / `cwd` / `exit` / `abort` という名前を compiler は特別扱いしません。

`str?!` は、処理そのものは `error` で失敗しえますが、成功した場合の値は optional であることを表します。`args()`、`env(name)`、`cwd()` が返す `str` は process context storage を指す readonly view であり、呼び出し側は所有しません。owned `String` が必要な場合は明示的に copy します。target 実装は OS 由来の `argv` / `envp` / cwd を UTF-8 として検証し、`str` にできない場合は `"std.process.invalid_encoding"` で失敗します。

`exit` / `abort` は target overlay の syscall や process termination boundary を使って実装し、万一 OS の終了操作から戻った場合は `trap()` します。`exit` / `abort` は caller scope の Nocter cleanup を実行しません。cleanup が必要な場合は、呼び出し前に明示します。

`std/process` はユーザー向け module path ですが、process context や process termination は process ABI に依存するため、初期実装では active target overlay 側に物理配置します。利用者は配置を意識せず `from std/process import args`、`from std/process import env`、`from std/process import exit` のように使います。

## ランタイム

現時点では、独立したランタイムライブラリを持たない方針です。標準ライブラリの `primitive` 宣言が初期ターゲット `arm64-darwin` と最小限の橋渡しを行います。将来は `~/.nocter/targets/<target>/std/` に target ごとの OS 境界の primitive 実装を追加します。

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
- v0 の `move` operand は local / parameter binding 名だけに限定する
- field move、index move、partial move は v0 では採用しない
- `move name` 後、binding は uninitialized state になる
- `drop name` 後、binding は uninitialized state になる
- `var` binding 全体だけは move / drop 後に再初期化できる
- `let` binding、field 単位、index 単位の再初期化は v0 では採用しない
- parameter は immutable binding とし、`var` parameter は採用しない
- owned parameter は関数本体が所有し、未 move の owned parameter は関数 scope 終了時に破棄する
- 既存の move-only binding を返す場合は `return move value` と書く
- struct literal、enum variant、関数呼び出し結果などの新規生成値は `return expr` で返せる
- borrow / view を返す場合は、参照元が返却後も生きていることをコンパイラが検査する
- readonly borrow は `&T` として表現する
- readwrite borrow は `&+T` として表現し、同時に他の readonly / readwrite borrow と共存できない
- `&+` は単一トークンとして扱い、単項 `+x` は採用しない
- スコープ終了時に破棄処理を挿入する
- 破棄処理は `impl` 内の専用 `drop` member で定義する
- use-after-free、double-free、dangling pointer を型チェック段階で防ぐ
- raw pointer は address-carrying value として扱い、初期仕様では dereference を一般ユーザーに提供しない
- `unsafe` は v0 では採用せず、低レベル境界は active Nocter home 内の trusted module に閉じる

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
- borrow は作成位置から、その borrow-like value の最後の source-level 使用位置まで有効です
- borrow binding の lexical scope が続いていても、その後に使われないなら borrow はそこで終了できます
- method receiver borrow は通常 call の間だけ有効ですが、receiver 由来の borrow-like value を返す場合は、その戻り値の最後の使用位置まで有効です
- `str`、`[T]`、`[+T]`、`ViewIter<T>`、borrow-like value を含む aggregate も同じ live range / provenance check を受けます
- direct named field については限定的な field-sensitive borrow を行います
- whole value の borrow は全 field と競合しますが、互いに disjoint な named field 同士は同時に扱えます
- array index、collection index、`[T]` element、pointer dereference、method call result、enum payload は v0 では field-sensitive に扱いません

初期化と代入の基本規則:

- local variable は必ず初期化する
- `let` / `var` は initializer 必須
- `let` は再代入できない
- `var` は再代入できる
- assignment は statement であり、値を返さない
- assignment target は writable place である必要がある
- v0 の writable place は `var` binding、writable place から到達できる field、`&+T` borrow から到達できる field
- 再代入では、右辺の評価に成功してから古い値を `drop` し、新しい値を格納する
- 右辺の postfix `?` が失敗した場合、古い値は置き換えられず、通常の failure 伝播と scope-end `drop` に従う
- active borrow と競合する場所には再代入できない
- 非copy値を既存の値から代入する場合は `move` が必要
- copy 型は通常の代入で copy する
- field assignment は partial move ではなく overwrite として扱う
- move 後の `var` binding 再初期化では古い値を `drop` しない
- 再初期化の右辺が postfix `?` で失敗した場合、binding は uninitialized のまま
- 分岐後に binding を使うには、すべての到達経路で initialized である必要がある
- compiler は binding の状態を `initialized` / `uninitialized` / `maybe initialized` として追跡する
- `maybe initialized` binding は直接使えない
- scope 終了時の `maybe initialized` binding には compiler が条件付き drop を生成する
- chained assignment は v0 では採用しない
- `+=` などの複合代入は v0 では数値型の writable place のみに許可する
- `[+T]`、array、collection への index assignment は v0 では延期する

評価順序と一時値の基本規則:

- 式は左から右に評価する
- 関数引数も左から右に評価する
- `method` 呼び出しでは receiver を最初に評価する
- struct literal の field initializer は、literal に書いた順に評価する
- `??` と三項条件演算子は必要な側だけ評価する
- 一時値は原則として文末で生成の逆順に `drop` する
- 一時値の所有権が local binding、owned parameter、構築中の aggregate、代入先、return value に移った場合、その一時値自体は caller 側の文末では `drop` しない
- block、`if` body、`match` arm、loop body は scope を作る
- scope 終了時は local 変数を宣言の逆順で `drop` する
- postfix `?` / `return` / `break` / `continue` で途中離脱する場合は、現在の statement で生成済みの一時値を先に `drop` し、その後に離脱で抜ける scope の `drop` を実行する
- 一時値から作った borrow や view を文の外へ逃がせない
- local owned value、temporary owned value、owned parameter から作った borrow / view は返せない
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

`move` または明示 `drop` 後の binding は uninitialized state になります。`var` binding 全体だけは再初期化できます。`let` binding や field だけの再初期化は v0 ではできません。

```nct
var text = String.new()
consume(move text)

text = String.new()
consume(move text)
```

再初期化では古い値を `drop` しません。右辺の postfix `?` が失敗した場合、binding は uninitialized のままです。分岐後に使う binding は、すべての到達経路で initialized でなければなりません。

```nct
var file = File.open(path)?
close(move file)

file.read() // error

file = File.open(other_path)?
file.read()?
```

control flow の合流では、全 path で initialized なら initialized、全 path で uninitialized なら uninitialized、path に差があれば maybe initialized です。maybe initialized binding は読み取り、borrow、move、field access、明示 `drop` に使えません。ただし scope end では compiler が条件付き drop を生成します。

```nct
var file = File.open(path)?

if should_close {
    close(move file)
}

// file is maybe initialized here.
// Direct use is an error, but scope end is safe.
```

`move` は予約語による unary expression で、所有権を移すことだけを表します。v0 では operand を local / parameter binding 名に限定します。

```nct
let b = move a
consume(move text)
return move value
```

copy 型に `move` を使うこと、field / index から直接 move すること、新規生成値に `move` を付けることは v0 ではエラーです。

```nct
move object.field  // error
move array[index]  // error
move make_value()  // error
move copy_value    // error
```

field を差し替えたい場合は、所有値全体を新しい binding に move して、その binding を変更します。

```nct
func rename(user: User, name: String): User {
    var next = move user
    next.name = move name
    return move next
}
```

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
    pub method (file: &+Self).write_text(text: str): void! {
        ...
    }
}

file.write_text("hello")?
```

新規生成された owned temporary は、その1回の method call に限って `&+Self` receiver として使えます。

```nct
(File.open(path)?).write_text("hello")?
```

`File.open(path)` が失敗した場合、`File` temporary は存在しません。`write_text` が失敗した場合、生成済みの temporary `File` を drop してから failure を伝播します。成功した場合は statement 終端で temporary `File` を drop します。

再代入は、古い所有値を安全に破棄してから新しい値を入れます。

```nct
var file = File.open(path)?

file = File.open(other_path)?
```

field assignment も同じ overwrite 規則です。右辺を先に評価し、成功した後に古い field 値を `drop` して新しい値を格納します。右辺の postfix `?` が失敗した場合、左辺は変更しません。

```nct
var user = move old_user
user.name = move new_name
```

`let` binding、`&T` 経由の field、active borrow と競合する場所には代入できません。direct named field では disjoint field を区別しますが、whole value の borrow や同じ field の borrow とは競合します。assignment は値を返さないため、`a = b = c` のような chained assignment は v0 では採用しません。

非copy値を別の変数から移す場合は `move` を使います。

```nct
var a = File.open(path_a)?
var b = File.open(path_b)?

a = move b
```

一時的な所有値から view を取り出して外へ残すことは禁止します。

```nct
let view = (String.copy(allocator, "abc")?).view() // error
```

所有値を束縛してから view を作ります。

```nct
var text = String.copy(allocator, "abc")?
let view = text.view()
```

戻り値でも同じ所有権規則を使います。copy 型は `return value` で返せます。既存の move-only binding を返す場合は `return move value` が必要です。

```nct
func take_user(user: User): User {
    return move user
}

func make_user(name: String): User {
    return User{
        name: move name,
    }
}
```

`return move user` 後、その binding は無効です。`return` で関数を抜ける時、返却値以外の live local owned value は逆順に `drop` されます。

borrow / view を返す場合は、参照元が返却後も生きている必要があります。v0 では source-level lifetime 注釈を持たず、コンパイラが provenance を追跡できる範囲に制限します。

```nct
func greeting(): str {
    return "hello" // OK: static storage
}

func bad(allocator: &+Allocator): str! {
    var text = String.copy(allocator, "hello")?
    return text.view() // error: local owned value is dropped at return
}
```

コピー可能な値型は `copy struct` で宣言します。`copy struct` は全フィールドがcopy可能である必要があり、`drop` を定義できません。

```nct
copy struct Point {
    pub x: i32
    pub y: i32
}

let p1 = Point{x: 1, y: 2}
let p2 = p1
```

所有値の破棄はスコープ終了時に自動で行います。破棄処理の定義には、trait ではなく `impl` 内の専用 `drop` member を使います。`drop` member は戻り値型を書かず、`pub` も付けません。明示的に早く破棄したい場合は `drop name` 文を使います。

```nct
import std/os as os

impl File {
    drop(file: &+Self) {
        os.close(file.fd).ignore()
    }
}

var file = File.open(path)?
drop file
```

`drop name` の operand は v0 では local / parameter binding 名だけです。initialized な move-only owned binding だけを明示 drop できます。copy 型、borrow、maybe initialized、uninitialized binding は明示 drop できません。`drop name` 後、その binding は uninitialized state になります。`var` binding はその後に再初期化できます。

```nct
var file = File.open(path)?
drop file

file = File.open(other)?
file.read()?
```

`drop object.field`、`drop array[index]`、`drop make_value()` は v0 では採用しません。

一時的な大量確保には、言語構文として `region` を使います。`region` は allocator から短命な一時領域を作り、block 終了時にその領域の確保をまとめて解放する仕組みです。

```nct
region scratch using allocator {
    let source = read_file(scratch.allocator(), "main.nct")?
    let tokens = lex(scratch.allocator(), source.view())?
}
```

`scratch` は region に付けた block-local binding 名であり、特別な名前ではありません。`temp`、`work`、`arena` など別の名前も使えます。

`scratch.allocator()` は region allocator を取り出す標準ライブラリ API の例です。コンパイラは `allocator` という名前を特別扱いするのではなく、region から派生した allocator value の provenance を追跡します。

`region` を抜けると、まず block 内の所有値を通常通り逆順に `drop` し、その後で region allocator が残りの region 確保をまとめて解放します。`return`、`break`、`continue` で region block を抜ける場合も同じ cleanup を行います。`never` 呼び出しは stack unwinding ではないため、呼び出し元 region の cleanup を暗黙には保証しません。

コンパイラは、region 内で確保した所有値、region 由来の borrow、`str` / `[T]` などの view が region の外へ漏れないことを検査します。copy 値でも、region 由来の参照や backing storage を含む場合は外へ持ち出せません。純粋な `i32` や統計値のように region へ依存しない copy 値だけを外へ持ち出せます。

`Allocator`、`Layout`、`RawBuffer` は `std/mem` の普通の公開 API として定義します。コンパイラは `Allocator` という名前を特別扱いしません。特別なのは `region ... using ...` 構文と、region 由来 allocator の provenance tracking だけです。

```nct
from std/mem import Allocator, Layout, RawBuffer

import std/mem as mem

var allocator = mem.page_allocator()
let buffer = mem.alloc(&+allocator, 4096, 16)?
mem.free(&+allocator, move buffer)
```

確保失敗は `"std.mem.out_of_memory"` などを持つ `error` として扱います。OOM を暗黙に `panic` / `abort` しません。`RawBuffer` は低レベル API であり、通常のコードでは `String` や `Buffer<T>` のような所有型を使います。

## Raw Pointer と Address API

Nocter は raw pointer 型 `*T` を持ちます。`*T` は所有権でも borrow でもなく、単なる address-carrying value です。

基本規則:

- `*T` は copy
- `*T` は non-null
- null が必要なら `*T?`
- `*T` は lifetime を延長しない
- `*T` は read / write 権限を表さない
- 初期仕様では一般ユーザーコードに raw pointer dereference を提供しない
- `unsafe` block で dereference を有効化する仕組みは v0 では採用しない

pointer と address の変換は `std/ptr` に置きます。

```nct
pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref<T>(value: &T): *T
pub primitive from_ref_mut<T>(value: &+T): *T
pub(nocter) primitive from_addr<T>(address: usize): *T
```

`usize` から pointer を作る `from_addr<T>(address: usize): *T` は `pub(nocter)` です。初期仕様では共通 `std/` と active target overlay の trusted module だけが使える制限 API で、一般ユーザーコードからは呼べません。

`[T]`、`[+T]`、`str` は `ptr()` と `len()` を持ちます。標準ライブラリ内の trusted module が syscall に buffer を渡す場合は、`ptr()` で raw pointer を取り、`std/ptr` の `addr(...)` で `usize` へ変換します。一般ユーザーコードは syscall primitive を直接呼べません。

```nct
import std/ptr as ptr
import std/os/macos as os

let bytes = text.bytes()
let result = os.syscall3(
    SYS_write,
    fd as usize,
    ptr.addr(bytes.ptr()),
    bytes.len(),
)
```

`std/ptr` の関数は target 非依存の core pointer primitive です。OS 境界の `std/os/macos` の `syscall0..6` とは別扱いです。これは raw pointer 型そのものに必要な最小操作であり、`print`、`exit`、`abort`、file 操作、allocator、`String`、`Buffer` を compiler primitive にするものではありません。

trusted module が public API の不変条件を破った場合、それは標準ライブラリまたは compiler のバグとして扱います。一般ユーザーコードが `unsafe` に opt-in した結果とは扱いません。`unsafe` と `trusted` は v0 では予約語にしません。

## 文字列

文字列リテラルの型は compiler built-in の `str` とします。

```nct
let name = "Nocter" // str
```

`str` は import なしで使える built-in 型です。UTF-8 として妥当な文字列を指す非所有 view で、実体は Mach-O 内の静的データ、または別の所有オブジェクトが持つバッファです。`str` 自身は所有権を持たず、drop も発生しません。

`String` は所有する文字列型です。標準ライブラリ側では `Buffer<u8>` などを使って実装し、スコープ終了時に内部バッファを破棄します。`String` は move-only とし、暗黙 copy は行いません。

```nct
let view: str = "README.md"
var owned = String.copy(allocator, view)?

open(view)
open(owned.view())

func open(path: str): File! {
    ...
}
```

文字列リテラルをグローバルな `String` として扱う方針は採用しません。`String` は所有型なので、リテラルから `String` が必要な場合は `String.copy(...)` で明示的に確保します。

連続した要素列の非所有ビューには `[T]` / `[+T]` を使います。`[T]` は readonly view、`[+T]` は readwrite view です。`str` は文字列専用の view として扱います。

```nct
func checksum(bytes: [u8]): u32
func read_into(file: &+File, output: [+u8]): usize!

impl str {
    pub method (text: Self).ptr(): *u8
    pub method (text: Self).len(): usize
    pub method (text: Self).bytes(): [u8]
}
```

`[u8]` は任意のバイト列を表し、UTF-8 であるとは限りません。`str` からは `[u8]` を得られますが、`[u8]` から `str` を作る場合は UTF-8 検証を必要とします。

`"abc"` を `&String` として渡す暗黙変換は採用しません。`&String` は実在する所有 `String` への borrow であり、文字列リテラルは `str` として扱います。

byte literal は `b'...'` と書き、型は `u8` です。裸の `'...'` は初期仕様では採用せず、将来の `Char` 用に空けておきます。

```nct
let a: u8 = b'a'
let newline: u8 = b'\n'
let raw: u8 = b'\xFF'
```

文字列リテラルと byte literal では、`\n`、`\r`、`\t`、`\0`、`\\`、`\"`、`\'`、`\xNN` を使えます。文字列リテラルは UTF-8 の `str` です。byte literal は escape 解決後にちょうど 1 byte でなければなりません。

## 配列とコレクション

固定長配列は `[T; N]` と書きます。

```nct
let header: [u8; 4] = [0x7F, 0x45, 0x4C, 0x46]
let numbers = [1, 2, 3] // [i32; 3]
```

配列リテラルは `[a, b, c]` です。文脈型があればその要素型と長さに従い、文脈がなければ要素から型を推論します。整数リテラルだけで構成される場合は `i32` を使います。

所有する可変長バッファは標準ライブラリの `Buffer<T>` で表します。`Buffer<T>` は言語組み込みではなく、標準ライブラリ型です。

```nct
var bytes = Buffer<u8>.with_capacity(allocator, 4096)?
bytes.push(10)?

let read: [u8] = bytes.view()
let write: [+u8] = bytes.write_view()
```

非所有 view は compiler built-in の `[T]` / `[+T]` を使います。

```nct
[T]       // readonly contiguous view
[+T] // readwrite contiguous view
```

borrow と view は、コンパイラ内部の hidden provenance を持ちます。provenance は実行時値ではなく、ABI や `ptr + len` layout に影響しません。`&T`、`&+T`、`str`、`[T]`、`[+T]`、`ViewIter<T>`、これらを含む aggregate が borrow-like value です。

v0 の provenance source kind:

```text
static
local
param_borrow
owned_param
region
unknown
```

`static` は返却・保存できます。`local`、`owned_param`、`region` は外へ逃がせません。`param_borrow` は返却できますが、呼び出し側では元の borrow より長く使えません。`unknown` は安全側に倒し、safe v0 code では関数から返したり長寿命の場所へ保存したりできません。

`[+T]` は `&+T` と同じく readwrite permission と排他性を持ちます。`[T]` と `str` は readonly permission を持ちます。

```nct
func ok(): str {
    return "hello" // static
}

func bad(allocator: &+Allocator): str! {
    var text = String.copy(allocator, "hello")?
    return text.view() // error: local
}

func slice(input: str): str {
    return input // param_borrow-like provenance
}
```

index 演算子 `x[i]` は境界チェックを行います。範囲外の場合は trap します。この境界チェックは debug / release に関係なく常に有効です。範囲外を値として扱いたい場合は `get(i)` を使います。

```nct
let first = read[0]      // 範囲外なら trap
let maybe = read.get(0)  // u8?
```

長さは特別なフィールドではなく、通常メソッド `len()` として扱います。

```nct
let count = read.len()
```

collection 用の操作は標準ライブラリの通常メソッドとして用意します。`len()`、`get()`、`ptr()`、`view()`、`write_view()` は collection / view の基本 API です。v0 の collection iteration は `[T]` から `ViewIter<T>` を作る readonly borrow iteration です。`iter()`、`next()`、`ViewIter<T>` は普通の標準ライブラリ API であり、`for` 構文が名前で特別扱いすることはありません。

```nct
var iter = read.iter()

while let byte = iter.next() {
    consume(byte)
}
```

`ViewIter<T>.next()` は `(&T)?` を返します。これは optional readonly borrow であり、optional value への borrow ではありません。`[+T]` の mutable element iteration と、collection から要素を move する owned iteration は v0 では採用しません。

## 型システム

Nocter は静的型付け言語として設計します。

compiler built-in は、構文と最小 primitive 型に限定します。

```text
bool
i8 i16 i32 i64
u8 u16 u32 u64
usize isize
str
error
void
never

*T
&T
&+T
[T]
[+T]
T?
T!
T?!
[T; N]
(T)
```

fallible type の公式表記は `T!` です。成功値が optional の fallible type は `T?!` と書きます。

型構文の括弧はグルーピングだけを行います。例えば `(&T)?` は optional readonly borrow、`&(T?)` は optional value への readonly borrow です。`T?!` は成功値が optional の fallible type です。

`str`、`error`、`[T]`、`[+T]` は compiler built-in の基礎型です。`String`、`Error`、`ErrorCode`、`ViewIter<T>`、`Allocator`、`File`、`print`、`args`、`env`、`cwd`、`exit`、`abort` は compiler built-in ではありません。

整数リテラルは decimal、hex `0x...`、binary `0b...` を持ち、桁区切り `_` を使えます。文脈型があればその整数型になり、文脈がなければ `i32` になります。float literal は v0 では採用しません。

```nct
let count = 10      // i32
let size: u64 = 10  // u64
```

`Int` は compiler built-in ではありません。`std/prelude` が提供する通常の alias です。user project module では synthetic prelude により利用できます。標準ライブラリ内部では `from std/prelude import Int` のように明示 import します。

```nct
let count: Int = 10 // Int is an alias of i32
```

型 alias は `type` で宣言します。alias は完全に同じ型の別名であり、新しい別型を作りません。

```nct
pub type Int = i32
pub type Bytes = [u8]
pub type Map<K, V> = HashMap<K, V>
```

`type` は top-level 宣言です。通常の定義と同じく private が既定で、公開する場合は `pub type`、Nocter 配布物内部だけに公開する場合は `pub(nocter) type` と書きます。generic alias は許可します。

```nct
let x: Int = 10
let y: i32 = x // OK: Int は i32
```

alias は ABI、layout、所有権、copy/drop 判定を変えません。alias に対する `impl` は禁止します。

```nct
impl Int {
    ...
}
// error: alias には impl できない
```

型安全な別型が必要な場合は alias ではなく `struct` を使います。v0 では専用の `newtype` 構文は採用しません。

```nct
pub copy struct UserId {
    pub value: u64
}
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

let signed: i32 = 10
let unsigned = signed as u64 // error

let big: u64 = 300
let small = big as u8      // error
let checked = u8.checked(big)   // u8?
let truncated = u8.truncate(big) // u8
```

通常の整数演算で overflow した場合は trap します。wrapping 演算は通常演算ではなく、明示 API で扱います。division by zero と、型の bit 幅以上の shift も trap します。これらの安全チェックは debug / release に関係なく常に有効です。compiler が trap 条件が起こらないことを証明できる場合だけ check を削除できます。`bool` と整数の暗黙変換は行いません。

比較と論理演算は、初期仕様では小さく保ちます。

- `==` / `!=` は初期仕様では `bool`、整数型、`str`、payload を持たない enum に限定する
- ordering 比較 `<` / `<=` / `>` / `>=` は同じ数値型同士に限定する
- shift 演算 `<<` / `>>` は左辺を整数値、右辺を整数の shift count とし、結果は左辺の型にする
- `&&` / `||` は `bool` 専用で短絡評価する
- `!expr` は `bool` 専用
- 単項 `-expr` は符号付き整数型専用
- 単項 `+expr` は採用しない
- ユーザー定義 operator overload は初期仕様では採用しない
- `String == String` や `String == str` は std 側の演算子定義が必要になるため v0 では保留する
- `==` を struct に自動生成しない
- payload を持たない enum の比較は許可する
- payload を持つ enum の値全体の比較は初期仕様では採用せず、statement 分岐には `match` / `if expr is Pattern`、値選択には `?{}` を使う

演算子の優先順位は、call / method / index / field を最も高くし、`??`、三項条件演算子、`?{}` を低くします。`&&`、`||`、`??`、三項条件演算子、`?{}` は必要な側だけ評価します。

```nct
if count > 0 && state == ScanState.inside_word {
    ...
}
```

例:

```nct
func log(msg: str): void
```

戻り値を持たない関数は `void` を返します。失敗を表す値には、例外ではなく fallible type `T!` を使います。

`T!` は成功時に `T`、失敗時に built-in `error` を返す型です。fallible 関数内では `return value` が成功を表します。ただし `return error_value` のように返す値が `error` 型の場合は失敗を表します。曖昧さを避けるため、`error!` は関数 return type として使えません。

`error` は型位置で意味を持つ compiler built-in 構文です。import で解決される通常名ではなく、ユーザー定義の型名として再定義できません。一方で、値の束縛名としての `error` は通常のローカル名です。`catch error { ... }` の `error` は慣習的な束縛名であり、`catch err { ... }` のような別名も有効です。

postfix `?` は fallible value または optional value を現在の関数へ伝播する構文です。`T!` に使うと成功値 `T` を取り出し、失敗時は現在の fallible 関数から同じ `error` で失敗します。`T?` に使うと present 値 `T` を取り出し、`none` 時は現在の optional return layer から `none` を返します。例外やスタック巻き戻しではありません。

postfix `!` は fallible value または optional value を強制的に取り出す構文です。成功または present の場合は `T` を返します。失敗または `none` の場合は即座に復帰不能停止します。通常コードでは `?`、`catch`、`if let`、`let ... else`、`??` を優先し、`!` はテスト、プロトタイプ、復旧不能な前提に限定します。

fallible failure return、`trap`、`abort` は別の仕組みです。

```text
return error_value = T! を通る回復可能エラー
trap               = プログラムバグ、契約違反、compiler check 失敗による復帰不能停止
abort              = cleanup なしの即時 process termination
```

fallible failure return は通常の `return` と同じく、離脱する scope の通常 cleanup を実行します。`trap` と `abort` は `never` を返し、stack unwinding を行いません。`panic` と unwind は v0 では採用しません。

```nct
func open(path: str): File! {
    if failed {
        return Error.new("std.io.not_found", "file not found")
    }

    return file
}
```

fallible type は `T!` と書きます。成功時は `T`、失敗時は built-in type の `error` を返します。`Error` と `ErrorCode` は標準ライブラリが提供する通常名であり、compiler はこれらの大文字名を特別扱いしません。`ErrorCode` は `str` の alias で、`Error.new(...)` などの標準ライブラリ API を通じて、built-in `error` payload の primitive code 表現へ変換されます。

fallible value を伝播する場合は postfix `?` を使います。失敗側を別の `error` に置き換えて離脱したい場合は `catch error { ... }` を使います。復旧不能な前提として強制的に成功値を取り出す場合は postfix `!` を使えます。

```nct
func read_all(allocator: &+Allocator, path: str): String! {
    var file = File.open(path) catch error {
        return Error.new("std.io.open_failed", error.message)
    }

    var text = file.read_to_string(allocator) catch error {
        return Error.new("std.io.read_failed", error.message)
    }

    return move text
}
```

`catch` は例外処理ではありません。`expr catch error { ... }` は `expr` が失敗した場合だけ `catch` block を実行します。初期仕様では `catch` block は `return`、`break`、`continue`、`never` を返す関数呼び出しなどで現在の制御フローを離脱し、通常の末尾到達はできません。`catch` は `T!` 専用で、`T?` には使いません。

fallible value は `match` で分解しません。`match` は enum 専用に戻し、success / failure pattern は採用しません。fallible value は postfix `?` または `catch` で扱います。

値が存在しない可能性は `T?` で表します。`Option<T>` という名前付き型を特別扱いしません。optional 関数では `return value` が present、`return none` が absent を表します。

```nct
func lookup(name: str): str? {
    if missing {
        return none
    }

    return value
}

func require_home(): str? {
    if let home = lookup("HOME") {
        return home
    }

    return none
}
```

optional と fallible は合成できます。`T?!` は、失敗しうる処理の成功値が optional であることを表します。`process.env("HOME")?` は fallible layer だけを外すため型は `str?` です。さらに optional return layer を持つ関数内では、もう一度 `?` を使って `none` を伝播できます。

```nct
if let home = process.env("HOME")? {
    use(home)
}
```

optional value の absence を伝播する場合も postfix `?` を使えます。`T?` に対する `expr?` は present なら `T` を取り出し、`none` なら現在の optional return layer から `none` を返します。現在の関数の return type が `none` を運べる場合に有効です。present / absent で分岐する場合は `if let` / `if var`、値がなければ現在の制御フローを抜ける場合は `let ... else` / `var ... else`、default value を選ぶ場合は `??` を使います。

optional を値として使う前に、値がない場合だけ早期離脱したいときは `let ... else` を使います。

```nct
let home = lookup("HOME") else {
    return none
}

use(home)
```

```nct
let config = find_config(path) else {
    return Error.new("app.config.missing", path)
}

load(config)
```

`let name = expr else { ... }` は `expr: T?` が present の場合に `name: T` を束縛し、その後の文へ進みます。`none` の場合は `else` block を実行します。`else` block は `return`、`return none`、`break`、`continue`、`never` を返す関数呼び出し、停止しない `loop` などで現在の制御フローを必ず離脱し、通常の末尾到達はできません。つまり `else` block は `never` 型です。

`var name = expr else { ... }` も使えます。present の値を mutable binding として取り出します。`let ... else` / `var ... else` は declaration statement であり、式ではありません。`else` block で代替値を返す用途には使いません。absence を値で補う場合は `??` を使います。

borrowed optional projection も使えます。

```nct
let name = &maybe_name else {
    return none
}

inspect(name)
```

```nct
var name = &+maybe_name else {
    return none
}

name.push("!")
```

`let name = &place else { ... }` は `place: T?` から `name: &T` を作り、optional 自体は move / copy しません。`var name = &+place else { ... }` は writable な `place: T?` から `name: &+T` を作ります。`let name = &+place else { ... }` と `var name = &place else { ... }` は v0 では採用しません。projection borrow が生きている間、source optional は move、代入、再初期化、明示 `drop` できません。

optional value には default operator `??` を使えます。右結合で、必要な場合だけ右辺を評価します。

```nct
let port = env_int("PORT") ?? config.default_port ?? 8080
```

`T?` も `match` では分解しません。local に present / absent を分岐したい場合は `if let` / `if var` を使います。optional を繰り返し取り出す場合は `while let` / `while var` を使います。

```nct
if let home = env("HOME") {
    consume(home)
} else {
    use_default_home()
}
```

```nct
if var text = maybe_text {
    text.push("!")
    consume(move text)
}
```

```nct
var iter = bytes.iter()

while let byte = iter.next() {
    consume(byte)
}
```

`if let` は optional が present の場合に中身を immutable binding として束縛し、`if var` は mutable binding として束縛します。`none` の場合は `else` body を実行します。`else` は省略できます。`else if let` / `else if var` も使えます。`if var` は元の optional へ値を書き戻す構文ではありません。

`if let value = maybe` / `if var value = maybe` は optional value を通常の所有権規則で評価します。move-only optional binding を直接使う場合、元の binding は消費されます。optional を消費せず中身だけ借用したい場合は borrowed optional projection を使います。

```nct
var maybe_name = get_name()

if let name = &maybe_name {
    inspect(name) // name: &String
}
```

```nct
var maybe_name = get_name()

if var name = &+maybe_name {
    name.push("!") // name: &+String
}
```

`if let name = &place` は `place: T?` から `name: &T` を作ります。`if var name = &+place` は writable な `place: T?` から `name: &+T` を作ります。どちらも optional 自体を move / copy しません。projection borrow は then body の中だけ有効で、その間 source optional は move、代入、再初期化、明示 `drop` できません。

`while let name = &place` と `while var name = &+place` は v0 では採用しません。borrowed projection は optional を進めたり消費したりしないためです。`ViewIter<T>.next(): (&T)?` のように optional borrow value を返す式は通常の optional として扱えるので、`while let item = iter.next()` は使えます。

bool 条件の値選択には三項条件演算子 `a ? b : c` を使えます。optional default とは別の演算子です。

```nct
let label = count == 0 ? "empty" : "ready"
```

enum pattern の値選択には `?{}` を使います。これは `match` expression ではなく、fallback arm を必須にした enum 専用の式です。

```nct
return error ?{
    AppError.open_failed(path) : code_for(path)
    : unknown_code()
}
```

初期仕様では statement 中心にします。`if`、`match`、block `{ ... }` は値を返しません。関数の成功終了は `return`、fallible の失敗は `return error_value`、optional の absent は `return none` で明示します。

```nct
func max(a: i32, b: i32): i32 {
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
    consume(byte)
}
```

`start..<end` は `start` 以上 `end` 未満を表します。`start` と `end` は loop 開始前に 1 回だけ評価します。step は常に `+1`、loop 変数は immutable binding です。

`while let name = expr` と `while var name = expr` は `T?` 専用の optional loop です。`expr` が present の間だけ body を実行し、`none` になったら loop を終了します。`while let name = &place` / `while var name = &+place` の borrowed optional projection は v0 では採用しません。

```nct
var iter = bytes.iter()

while let byte = iter.next() {
    consume(byte)
}
```

`for item in collection` は初期仕様では採用しません。collection の走査は `for i in 0..<items.len()` と index、または標準ライブラリの通常メソッド `iter()` / `next()` を明示的に使います。

通常復帰しない処理は `never` で表します。`never` は値を持つ型ではなく、呼び出し元へ戻らない制御フローを表す型です。`trap()`、`std/process.abort()`、`std/process.exit(code)`、停止しないイベントループ、到達不能分岐の明示に使います。

`trap` はプログラムバグや runtime check 失敗のための primitive 境界です。compiler も out-of-bounds indexing、整数 overflow、division by zero、invalid enum tag、unreachable 到達などで trap を生成できます。

`abort` と `exit` は標準ライブラリ API であり、compiler primitive ではありません。どちらも caller scope の Nocter cleanup を実行しません。cleanup が必要なら呼び出し前に行います。

`panic` は v0 の言語機能ではなく、標準ライブラリ API としても採用しません。予約語でもありません。ユーザーが `panic` という通常関数を定義しても、言語仕様上の特別な動作はありません。stack unwinding も v0 では採用しません。

```nct
import std/process as process

func require_path(path: str?): str {
    if let value = path {
        return value
    }

    process.abort()
}
```

`never` を返す関数を呼んだ後の同一 block 内の文は到達不能です。Nocter は初期仕様で到達不能コードをコンパイルエラーにします。`never` は値を生成しないため、三項条件演算子や `catch` の分岐で必要な型に収まりますが、変数に格納する値としては存在しません。`void` 以外の関数はすべての到達可能経路で値を返すか、`return none` / `never` などで経路を終端する必要があります。`never` 呼び出しは例外や stack unwinding ではないため、statement-end temporary drop や caller scope の `drop` 実行を暗黙に保証しません。

ジェネリクスは `<T>` を使います。制約は v0 では `T: Trait` だけです。複数制約 `T: A + B`、`where` clause、default type parameter は採用しません。

```nct
struct Buffer<T> {
    ...
}

func first<T>(items: [T]): T? {
    ...
}

func write_line<W: Writer>(writer: &+W, text: str): void! {
    writer.write(text)?
    writer.write("\n")?
    return
}
```

ジェネリクスはコンパイル時に具体化します。`Buffer<i32>` と `Buffer<String>` はそれぞれ具体型として扱い、実行時の型情報や共通ランタイムに依存しません。初期仕様では `dyn Trait` のような動的 trait object は採用しません。

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

## 診断方針

Nocter compiler は、型、所有権、borrow、初期化状態、visibility、import、fallible value、primitive 境界のエラーで、原因、対象、修正方向を説明します。

基本形:

```text
error[E0001]: cannot move `file` while it is borrowed
  --> app.nct:12:18
   |
12 |     close(move file)
   |                ^^^^ move occurs here
   |
note: readonly borrow created here
  --> app.nct:10:16
   |
10 |     inspect(&file)
   |              ^^^^
help: end the borrow before moving `file`
```

v0 では、source-level compiler error に `E0000` 形式の error code を付け、primary span を1つ持たせます。関連する原因箇所がある場合は related span と `note` を出し、修正方向が明確な場合は `help` を出します。parser は最初の構文エラーで止まってよく、型チェック以降は複数の独立エラーを出せますが、cascade error は抑制します。

`pub(nocter)` 違反、borrow 違反、move 後の使用、明示 `drop` 後の使用、maybe initialized binding の使用、`return` 値の success/failure 不一致、`T!` / `T?` ではない値への postfix `?` / `!`、optional return layer のない場所での optional propagation、`catch` の fallthrough、selected entry function の欠落や不正な signature などは専用診断にします。診断文は compiler 内部都合ではなく、Nocter の source-level 概念で説明します。

## エディタ連携

VS Code 拡張機能は初期段階では TextMate grammar による構文ハイライト、comment toggle、bracket matching、auto closing などの薄い表示層として扱います。言語仕様の正は `spec/13-lexical-grammar.md` と各仕様章に置き、拡張機能側で独自の名前解決、型推論、borrow check、import 解決を実装しません。hover などの semantic editor feature は、compiler / LSP が `///`、`/** ... */`、`//!`、`/*! ... */` の doc comment を解析して提供します。

VS Code 拡張機能は別リポジトリ `vscode-nocter` で開発します。想定する拡張機能側の構成は次の通りです。

```text
vscode-nocter/
    package.json
    language-configuration.json
    syntaxes/
        nocter.tmLanguage.json
    snippets/
        nocter.code-snippets
    src/
        extension.ts
```

`nocter check app.nct --format json` は machine-readable diagnostics を出し、VS Code Problems や AI tool が利用できます。formatter は `nocter fmt` を正とし、VS Code 拡張機能や AI tool は独自 formatter を持たず compiler toolchain を呼び出します。JSON stdout は `schema: "nocter.diagnostics"`、`version: 1`、`ok`、`command`、`target`、`root`、`root_absolute_path`、`diagnostics` を持つ単一 object です。各 span は人間向けの `file` と、editor / LSP 用の canonical absolute path である `absolute_path` を持ちます。`nocter lsp` v0 は initialize、shutdown、full-document sync、publishDiagnostics を提供します。VS Code 拡張機能はこれを LSP client として使い、hover / definition / completion は LSP 側の後続機能として追加します。

compiler 内部の source span は UTF-8 byte offset を正とし、CLI 用 JSON では byte offset と UTF-8 byte column を併記します。これは LSP position ではありません。LSP server は client が要求する position encoding に合わせて変換します。VS Code 拡張機能と AI tool は Nocter の意味解析を再実装せず、`nocter check`、`nocter tokens --format json`、`nocter ast --format json`、または `nocter lsp` の結果を使います。

## 実装ロードマップ

現時点の実装順序案です。

1. Lexer
2. Parser
3. AST
4. source span と診断基盤
5. 型チェック
6. ARM64 命令エンコーダ
7. Mach-O 生成
8. root file の selected entry function のみ実行可能にする
9. 文字列リテラル配置
10. `exit`
11. `primitive`
12. `print`
13. `import`
14. `struct`
15. `if` / `match` / `?{}`
16. `while` / `loop` / range `for` / `break` / `continue`
17. 所有型のコピー禁止
18. `move`
19. `drop`
20. `&T`
21. `&+T`
22. allocator
23. region
24. 標準ライブラリ拡充
25. `nocter run app.nct`
26. `nocter check --format json`
27. `nocter fmt app.nct`
28. `nocter tokens app.nct --format json`
29. `nocter ast app.nct --format json`
30. `nocter lsp`

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

Nocter は、まず自己完結した `arm64-darwin` 言語処理系として成立することを優先します。長期的な portability は、初期実装の焦点をぼかさず、target 依存部分を分離することで確保します。

## プロジェクトの価値観

このプロジェクトでは、実装の短さより成果物全体の一貫性を重視します。

特に重視するもの:

- 自己完結性
- 言語としての一貫性
- 美しい設計
- 静的型付け、値中心、モジュール指向
- GC に頼らないメモリ安全性
- 人間と AI の両方が読み書きしやすい正準表記、例、機械可読 diagnostics
- 標準ライブラリを言語自身で記述できること
- コンパイラ単体で実行ファイル生成まで完結すること

実装量が増えても、これらの価値を崩す設計は避けます。
