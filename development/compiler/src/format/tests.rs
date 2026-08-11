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
fn formats_closures_and_explicit_captures_stably() {
    assert_formats_stably(
        r#"func main(source: i32, count: i32): void {
let callback=(&source,&+count;value:i32):bool {
value>source
}
return
}
"#,
        r#"func main(source: i32, count: i32): void {
    let callback = (&source, &+count; value: i32): bool { value > source }
    return
}
"#,
    );
}

#[test]
fn formats_top_level_items_and_blocks() {
    assert_formats_stably(
        r#"pub   func   main(  ):i32{
let x:i32=1+2*3
if x>3{return x}else{return 0}
}

"#,
        concat!(
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
fn formats_source_backed_callable_contracts_stably() {
    assert_formats_stably(
        r#"pub func parse(text:&str):Value! from text
instance Value{pub method &self.render():String}
construct Value{pub default func new():Self
pub literal [](...items:i32):Self}
coerce Value{pub &self as &str from self}
"#,
        r#"pub func parse(text: &str): Value! from text

instance Value {
    pub method &self.render(): String
}

construct Value {
    pub default func new(): Self

    pub literal [](...items: i32): Self
}

coerce Value {
    pub &self as &str from self
}
"#,
    );
}

#[test]
fn formats_equality_operator_declarations_and_requirements_stably() {
    assert_formats_stably(
        r#"struct Text{value:i32}
instance Text{pub operator(&self==other:&Self):bool{return self.value==other.value}}
func equal<T>(left:&T,right:&T):bool where(&T==&T):bool{return left==right}
"#,
        r#"struct Text {
    value: i32,
}

instance Text {
    pub operator (&self == other: &Self): bool {
        return self.value == other.value
    }
}

func equal<T>(left: &T, right: &T): bool where (&T == &T): bool {
    return left == right
}
"#,
    );
}

#[test]
fn formats_index_operator_declarations_stably() {
    assert_formats_stably(
        "instance Buffer<T>{pub operator(&self[index:usize]):&T from self{return &self.values[index]}pub operator(&+self[index:usize]):&+T{return &+self.values[index]}}\n",
        "instance Buffer<T> {\n    pub operator (&self[index: usize]): &T from self {\n        return &self.values[index]\n    }\n\n    pub operator (&+self[index: usize]): &+T {\n        return &+self.values[index]\n    }\n}\n",
    );
}

#[test]
fn formats_borrow_coercions_stably() {
    assert_formats_stably(
        r#"coerce Vec<T>{pub &self as &[T] from self{return self.view()}
&+self as &+[T] from self{return self.view_mut()}}
"#,
        r#"coerce Vec<T> {
    pub &self as &[T] from self {
        return self.view()
    }

    &+self as &+[T] from self {
        return self.view_mut()
    }
}
"#,
    );
}

#[test]
fn formats_native_test_declarations_stably() {
    assert_formats_stably(
        "test   pushes{let value:i32=1 return}\n",
        concat!(
            "test pushes {\n",
            "    let value: i32 = 1\n",
            "    return\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_lexical_regions_stably() {
    assert_formats_stably(
        r#"func main(arena:usize):void{region temp using arena{let value=temp}return}
"#,
        concat!(
            "func main(arena: usize): void {\n",
            "    region temp using arena {\n",
            "        let value = temp\n",
            "    }\n",
            "    return\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_result_provenance_across_callable_forms_stably() {
    assert_formats_stably(
        r#"interface Lookup<T>{pub method &self.get(fallback:&T):&T from self|fallback}
func greeting():&str from static{return "hello"}
primitive allocated_text():&str
"#,
        concat!(
            "interface Lookup<T> {\n",
            "    pub method &self.get(fallback: &T): &T from self | fallback\n",
            "}\n",
            "\n",
            "func greeting(): &str from static {\n",
            "    return \"hello\"\n",
            "}\n",
            "\n",
            "primitive allocated_text(): &str\n",
        ),
    );
}

#[test]
fn formats_interface_default_methods_stably() {
    assert_formats_stably(
        r#"interface Value{pub method &self.value():i32{let result:i32=self.required() return result}pub method &self.required():i32}
"#,
        concat!(
            "interface Value {\n",
            "    pub method &self.value(): i32 {\n",
            "        let result: i32 = self.required()\n",
            "        return result\n",
            "    }\n",
            "    pub method &self.required(): i32\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_associated_types_before_interface_methods_stably() {
    assert_formats_stably(
        r#"interface Source{pub method &+self.next():Self.Item? pub type Item}
conform Source for Buffer<T>{method &+self.next():T?{return none}type Item=T}
"#,
        concat!(
            "interface Source {\n",
            "    pub type Item\n",
            "\n",
            "    pub method &+self.next(): Self.Item?\n",
            "}\n",
            "\n",
            "conform Source for Buffer<T> {\n",
            "    method &+self.next(): T? {\n",
            "        return none\n",
            "    }\n",
            "\n",
            "    type Item = T\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_associated_type_bounds_stably() {
    assert_formats_stably(
        "interface Iterator {}\ninterface Source { pub type Iter:Iterator+&+func():i32 }\n",
        "interface Iterator {}\n\ninterface Source {\n    pub type Iter: Iterator + &+func(): i32\n}\n",
    );
}

#[test]
fn formats_generic_interface_bounds_stably() {
    assert_formats_stably(
        r#"func measure<T>(value:&T):i32 where T: Measure+Display {return value.measure()}
"#,
        "func measure<T>(value: &T): i32 where T: Measure + Display {\n    return value.measure()\n}\n",
    );
}

#[test]
fn formats_nominal_where_predicates_stably() {
    assert_formats_stably(
        r#"struct Box<T>where copy T,T:Readable{}
type ReadableBox<T> =Box<T> where T:Readable
interface Source<T>where T:Readable{pub type Item}
"#,
        concat!(
            "struct Box<T> where copy T, T: Readable {}\n",
            "\n",
            "type ReadableBox<T> = Box<T> where T: Readable\n",
            "\n",
            "interface Source<T> where T: Readable {\n",
            "    pub type Item\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_typed_literal_definitions_and_expressions_stably() {
    assert_formats_stably(
        r#"construct Vec<T>{pub default literal [](...items:T):Self{for item in items{return move item}}}
func build(arena:Arena,other:Vec<i32>):Vec<i32>{return Vec<i32> [1,...other,...&other,...move other,3] using arena}
"#,
        concat!(
            "construct Vec<T> {\n",
            "    pub default literal [](...items: T): Self {\n",
            "        for item in items {\n",
            "            return move item\n",
            "        }\n",
            "    }\n",
            "}\n",
            "\n",
            "func build(arena: Arena, other: Vec<i32>): Vec<i32> {\n",
            "    return Vec<i32> [1, ...other, ...&other, ...move other, 3] using arena\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_construct_declarations_stably() {
    assert_formats_stably(
        r#"construct Vec<T>{pub default literal [](...items:T):Self{return Self.empty()}
pub func new():Self{return make()}
pub func from_iter<I>(iterator:I):Self where I: Source<T> {return Self.new()}}
"#,
        r#"construct Vec<T> {
    pub default literal [](...items: T): Self {
        return Self.empty()
    }

    pub func new(): Self {
        return make()
    }

    pub func from_iter<I>(iterator: I): Self where I: Source<T> {
        return Self.new()
    }
}
"#,
    );
}

#[test]
fn formats_type_and_data_declarations() {
    assert_formats_stably(
        r#"pub(/) type Path= [u8]
copy struct Pair<T> {pub left:T,right:T}
enum AppError {missing_path,open_failed(path:&str)}
"#,
        concat!(
            "pub(/) type Path = [u8]\n",
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
        ),
    );
}

#[test]
fn formats_target_directive_on_primitive() {
    assert_formats_stably(
        r#"#target: "arm64-darwin"
pub(/) primitive write_text_raw(fd:i32,text:&str):void!
"#,
        concat!(
            "#target: \"arm64-darwin\"\n",
            "pub(/) primitive write_text_raw(fd: i32, text: &str): void!\n",
        ),
    );
}

#[test]
fn formats_target_directive_on_function() {
    assert_formats_stably(
        r#"#target: "arm64-darwin"
pub(/) func free_pages(address:usize,size:usize):void{return}
"#,
        concat!(
            "#target: \"arm64-darwin\"\n",
            "pub(/) func free_pages(address: usize, size: usize): void {\n",
            "    return\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_target_directive_on_type_declarations() {
    assert_formats_stably(
        r#"#target: "arm64-darwin"
pub(/) type RawWord=usize
#target: "arm64-darwin"
pub(/) copy struct SyscallResult {pub value:usize,errno:i32}
#target: "arm64-darwin"
pub(/) enum PlatformError {interrupted}
#target: "arm64-darwin"
pub(/) interface PlatformContract {pub method &self.code():i32}
"#,
        concat!(
            "#target: \"arm64-darwin\"\n",
            "pub(/) type RawWord = usize\n",
            "\n",
            "#target: \"arm64-darwin\"\n",
            "pub(/) copy struct SyscallResult {\n",
            "    pub value: usize,\n",
            "    errno: i32,\n",
            "}\n",
            "\n",
            "#target: \"arm64-darwin\"\n",
            "pub(/) enum PlatformError {\n",
            "    interrupted,\n",
            "}\n",
            "\n",
            "#target: \"arm64-darwin\"\n",
            "pub(/) interface PlatformContract {\n",
            "    pub method &self.code(): i32\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_control_flow_and_postfix_expressions() {
    assert_formats_stably(
        r#"func main():i32!{
var file=File.open(path) catch error {return 1}
maybe() catch _ {return 2}
let next=move file
for i in 0..<10{file.write("x")?}
match error{AppError.missing_path{return 1}_{return file.size() as i32}}
}
"#,
        concat!(
            "func main(): i32! {\n",
            "    var file = File.open(path) catch error {\n",
            "        return 1\n",
            "    }\n",
            "    maybe() catch _ {\n",
            "        return 2\n",
            "    }\n",
            "    let next = move file\n",
            "    for i in 0..<10 { file.write(\"x\")? }\n",
            "    match error {\n",
            "        AppError.missing_path {\n",
            "            return 1\n",
            "        }\n",
            "        _ {\n",
            "            return file.size() as i32\n",
            "        }\n",
            "    }\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_match_expression() {
    assert_formats_stably(
        r#"enum AppError {missing_path,open_failed(path:&str)}
func code(error:AppError):i32{return match error{AppError.missing_path{1}
AppError.open_failed(path){2}
_{0}}}
"#,
        concat!(
            "enum AppError {\n",
            "    missing_path,\n",
            "    open_failed(path: &str),\n",
            "}\n",
            "\n",
            "func code(error: AppError): i32 {\n",
            "    return match error {\n",
            "        AppError.missing_path { 1 }\n",
            "        AppError.open_failed(path) { 2 }\n",
            "        _ { 0 }\n",
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
fn formats_static_opaque_result_contracts_stably() {
    assert_formats_stably(
        r#"func values<T>():some Source<T,Item=&T>?{
return none
}
"#,
        r#"func values<T>(): some Source<T, Item = &T>? {
    return none
}
"#,
    );
}

#[test]
fn canonical_type_notation_preserves_prefix_and_postfix_structure() {
    assert_formats_stably(
        concat!(
            "func optional_borrow(): (&Item)? { return none }\n",
            "func borrow_optional(value: Item?): &(Item?) { return &value }\n",
            "func optional_callback(): (func(): Item)? { return none }\n",
            "func borrow_callback(callback: func(): Item): &(func(): Item) { return &callback }\n",
        ),
        concat!(
            "func optional_borrow(): &Item? {\n",
            "    return none\n",
            "}\n",
            "\n",
            "func borrow_optional(value: Item?): &(Item?) {\n",
            "    return &value\n",
            "}\n",
            "\n",
            "func optional_callback(): (func(): Item)? {\n",
            "    return none\n",
            "}\n",
            "\n",
            "func borrow_callback(callback: func(): Item): &(func(): Item) {\n",
            "    return &callback\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_ancestor_visibility_without_named_scopes() {
    assert_formats_stably(
        "pub(./) func child():void{return}\npub(../../) func ancestor():void{return}\npub(/) func package():void{return}\n",
        concat!(
            "pub(./) func child(): void {\n",
            "    return\n",
            "}\n",
            "\n",
            "pub(../../) func ancestor(): void {\n",
            "    return\n",
            "}\n",
            "\n",
            "pub(/) func package(): void {\n",
            "    return\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_builtin_callable_contracts_stably() {
    assert_formats_stably(
        "func apply<F>(callback:F):void where F:&+func(value:i32):i32 {\nreturn\n}\n",
        "func apply<F>(callback: F): void where F: &+func(value: i32): i32 {\n    return\n}\n",
    );
}

#[test]
fn formats_result_provenance_contracts_stably() {
    assert_formats_stably(
        r#"pub func view(factory:&func():&Text from static):&Text from static{return factory()}
interface Factory{pub method &self.view():&Text from self}
conform Factory for Builder{method &self.view():&Text from self{return borrow()}}
construct Text{pub default func new():Self{return make()}pub literal ""(text:&str):Self{return make()}}
"#,
        concat!(
            "pub func view(factory: &func(): &Text from static): &Text from static {\n",
            "    return factory()\n",
            "}\n",
            "\n",
            "interface Factory {\n",
            "    pub method &self.view(): &Text from self\n",
            "}\n",
            "\n",
            "conform Factory for Builder {\n",
            "    method &self.view(): &Text from self {\n",
            "        return borrow()\n",
            "    }\n",
            "}\n",
            "\n",
            "construct Text {\n",
            "    pub default func new(): Self {\n",
            "        return make()\n",
            "    }\n",
            "\n",
            "    pub literal \"\"(text: &str): Self {\n",
            "        return make()\n",
            "    }\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_imports_instances_conformances_and_literals_stably() {
    assert_formats_stably(
        r#"use std/io
use std/io.{print as write,File}
use std/process as process
instance File {pub method &+self.write(text:&str):void!{let bytes=[1,2,3]
let marker=b'\n'
var point=Point {x:1,y:2}
var marker=Marker<i32> {code:42}
while ready(){print(text)}
}}
destruct File(&+self){drop self}
"#,
        concat!(
            "use std/io\n",
            "\n",
            "use std/io.{print as write, File}\n",
            "\n",
            "use std/process as process\n",
            "\n",
            "instance File {\n",
            "    pub method &+self.write(text: &str): void! {\n",
            "        let bytes = [1, 2, 3]\n",
            "        let marker = b'\\n'\n",
            "        var point = Point { x: 1, y: 2 }\n",
            "        var marker = Marker<i32> { code: 42 }\n",
            "        while ready() { print(text) }\n",
            "    }\n",
            "}\n",
            "\n",
            "destruct File(&+self) {\n",
            "    drop self\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_method_generic_parameters_stably() {
    assert_formats_stably(
        "instance Factory {pub method &self.convert<T,U>(value:T):U where T:Readable+Measured{return make(value)}}\n",
        concat!(
            "instance Factory {\n",
            "    pub method &self.convert<T, U>(value: T): U where T: Readable + Measured {\n",
            "        return make(value)\n",
            "    }\n",
            "}\n",
        ),
    );
}

#[test]
fn formats_generic_conformanceementations_with_members_stably() {
    assert_formats_stably(
        "conform Source<T> for Box<T> where T:Readable{method &self.read():T{return self.value}}\n",
        concat!(
            "conform Source<T> for Box<T> where T: Readable {\n",
            "    method &self.read(): T {\n",
            "        return self.value\n",
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
