use super::format_source;
use crate::source::SourceMap;

fn format_text(text: &str) -> String {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let output = format_source(&sources, source);
    assert_eq!(output.diagnostics, Vec::new());
    output.formatted.unwrap()
}

#[test]
fn formats_top_level_items_and_blocks() {
    let formatted = format_text(
        r#"use std/prelude
pub   func   main(  ):i32{
let x:i32=1+2*3
if x>3{return x}else{return 0}
}
"#,
    );

    assert_eq!(
        formatted,
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
        )
    );
}

#[test]
fn formats_type_and_data_declarations() {
    let formatted = format_text(
        r#"pub(nocter) type Path= [u8]
copy struct Pair<T>{pub left:T,right:T}
enum AppError{missing_path,open_failed(path:str)}
trait Writer{method(out:&+Self).write(text:str):void!}
"#,
    );

    assert_eq!(
        formatted,
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
            "    open_failed(path: str),\n",
            "}\n",
            "\n",
            "trait Writer {\n",
            "    method (out: &+Self).write(text: str): void!\n",
            "}\n",
        )
    );
}

#[test]
fn formats_control_flow_and_postfix_expressions() {
    let formatted = format_text(
        r#"func main():i32!{
var file=File.open(path) catch error {return 1}
for i in 0..<10{file.write("x")?}
switch error{is AppError.missing_path{return 1}else{return file.size() as i32}}
}
"#,
    );

    assert_eq!(
        formatted,
        concat!(
            "func main(): i32! {\n",
            "    var file = File.open(path) catch error {\n",
            "        return 1\n",
            "    }\n",
            "    for i in 0..<10 {\n",
            "        file.write(\"x\")?\n",
            "    }\n",
            "    switch error {\n",
            "        is AppError.missing_path {\n",
            "            return 1\n",
            "        }\n",
            "        else {\n",
            "            return file.size() as i32\n",
            "        }\n",
            "    }\n",
            "}\n",
        )
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
