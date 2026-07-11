use super::format_source;
use crate::source::SourceMap;

fn format_text(text: &str) -> String {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let output = format_source(&sources, source);
    assert_eq!(output.diagnostics, Vec::new());
    output.formatted.unwrap()
}

fn assert_formats_stably(input: &str, expected: &str) {
    let formatted = format_text(input);
    assert_eq!(formatted, expected);
    assert_eq!(format_text(&formatted), formatted);
}

#[test]
fn formats_top_level_items_and_blocks() {
    assert_formats_stably(
        r#"use std/prelude
pub   func   main(  ):i32{
let x:i32=1+2*3
if x>3{return x}else{return 0}
}
"#,
        concat!(
            "use std/prelude\n",
            "\n",
            "pub func main(): i32 {\n",
            "    let x: i32 = 1 + 2 * 3\n",
            "    if x > 3 {\n",
            "        return x\n",
            "    } else {\n",
            "        return 0\n",
            "    }\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_type_and_data_declarations() {
    assert_formats_stably(
        r#"pub(nocter) type Path= [u8]
copy struct Pair<T>{pub left:T,right:T}
enum AppError{missing_path,open_failed(path:&str)}
trait Writer{method(out:&+Self).write(text:&str):void!}
"#,
        concat!(
            "pub(nocter) type Path = [u8]\n",
            "\n",
            "copy struct Pair<T> {\n",
            "    pub left: T,\n",
            "    right: T,\n",
            "}\n",
            "\n",
            "enum AppError {\n",
            "    missing_path,\n",
            "    open_failed(path: &str),\n",
            "}\n",
            "\n",
            "trait Writer {\n",
            "    method (out: &+Self).write(text: &str): void!\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_control_flow_and_postfix_expressions() {
    assert_formats_stably(
        r#"func main():i32!{
var file=File.open(path) catch error {return 1}
for i in 0..<10{file.write("x")?}
match error{AppError.missing_path{return 1}else{return file.size() as i32}}
}
"#,
        concat!(
            "func main(): i32! {\n",
            "    var file = File.open(path) catch error {\n",
            "        return 1\n",
            "    }\n",
            "    for i in 0..<10 {\n",
            "        file.write(\"x\")?\n",
            "    }\n",
            "    match error {\n",
            "        AppError.missing_path {\n",
            "            return 1\n",
            "        }\n",
            "        else {\n",
            "            return file.size() as i32\n",
            "        }\n",
            "    }\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_pattern_conditional_expression() {
    assert_formats_stably(
        r#"enum AppError{missing_path,open_failed(path:&str)}
func code(error:AppError):i32{return error ?{AppError.missing_path:1
AppError.open_failed(path):2
:0}}
"#,
        concat!(
            "enum AppError {\n",
            "    missing_path,\n",
            "    open_failed(path: &str),\n",
            "}\n",
            "\n",
            "func code(error: AppError): i32 {\n",
            "    return error ?{\n",
            "        AppError.missing_path : 1\n",
            "        AppError.open_failed(path) : 2\n",
            "        : 0\n",
            "    }\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_optional_fallible_types_stably() {
    assert_formats_stably(
        r#"func env(name:&str):&str?!{return none}
func maybe_open(path:&str):File?{return none}
"#,
        concat!(
            "func env(name: &str): &str?! {\n",
            "    return none\n",
            "}\n",
            "\n",
            "func maybe_open(path: &str): File? {\n",
            "    return none\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_imports_impls_and_literals_stably() {
    assert_formats_stably(
        r#"from std/io import print as write,File
import std/process as process
impl Writer for File{pub method(file:&+Self).write(text:&str):void!{let bytes=[1,2,3]
var point=Point{x:1,y:2}
while var item=next(){print(item)}
}}
"#,
        concat!(
            "from std/io import print as write, File\n",
            "\n",
            "import std/process as process\n",
            "\n",
            "impl Writer for File {\n",
            "    pub method (file: &+Self).write(text: &str): void! {\n",
            "        let bytes = [1, 2, 3]\n",
            "        var point = Point { x: 1, y: 2 }\n",
            "        while var item = next() {\n",
            "            print(item)\n",
            "        }\n",
            "    }\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_multi_line_string_with_comment_markers_stably() {
    assert_formats_stably(
        concat!(
            "func main(): i32 {\n",
            "    let text = \"\"\"\n",
            "        not // a comment\n",
            "        not /* a comment */ either\n",
            "        \"\"\"\n",
            "    return 0\n",
            "}\n",
        ),
        concat!(
            "func main(): i32 {\n",
            "    let text = \"\"\"\n",
            "        not // a comment\n",
            "        not /* a comment */ either\n",
            "        \"\"\"\n",
            "    return 0\n",
            "}\n",
        ),
    );
}

#[test]
fn rejects_comments_until_formatter_preserves_them() {
    let mut sources = SourceMap::new();
    let source = sources.add_source(
        "app.nct",
        None,
        "func main(): i32 { // keep me\n    return 0\n}\n",
    );

    let output = format_source(&sources, source);

    assert!(output.formatted.is_none());
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(output.diagnostics[0].code, "E0601");
}
