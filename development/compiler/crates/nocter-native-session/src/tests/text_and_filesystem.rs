use super::*;

#[test]
fn standard_string_concat_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        concat!(
            "func main(): i32 {\n",
            "    let text = String.concat(\"No\", \"cter\")\n",
            "    if (&text as &str) == \"Nocter\" { return 42 }\n",
            "    return 1\n",
            "}\n",
        ),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    assert!(!image.image().bytes().is_empty());
}

#[test]
fn unicode_scalar_values_cross_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        r#"noalloc func scalar_iteration_is_exact(): bool {
    let text: &str = "Aλ😀"
    if text.len() != 7 || text.char_count() != 3 { return false }
    var chars = text.chars()
    let first = chars.next() otherwise { return false }
    let second = chars.next() otherwise { return false }
    let third = chars.next() otherwise { return false }
    if first != 'A' || second != 'λ' || third != '\u{1F600}' { return false }
    let _extra = chars.next() otherwise { return true }
    return false
}

func main(): i32 {
    let face: char = '\u{1F600}'
    if face.code_point() != 128512 { return 1 }
    if face.utf8_len() != 4 || face.is_ascii() { return 2 }
    let digit = char.from_u32(57) otherwise { return 3 }
    if !digit.is_ascii_digit() || digit != '9' { return 4 }
    if !'\u{3000}'.is_whitespace() || '\u{3001}'.is_whitespace() { return 10 }
    if !'Ω'.is_alphabetic() || '3'.is_alphabetic() { return 11 }
    if !'ß'.is_lowercase() || !'İ'.is_uppercase() { return 12 }
    if !'٥'.is_decimal_digit() || '²'.is_decimal_digit() { return 13 }
    if !(digit < face) { return 5 }
    if !scalar_iteration_is_exact() { return 6 }
    var text = String.copy("A")
    text.push('λ')
    text.try_push('\u{1F600}') catch _ { return 7 }
    if (&text as &str) != "Aλ😀" { return 8 }
    let _surrogate = char.from_u32(55296) otherwise { return 0 }
    return 9
}
"#,
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_test(image.image(), &package_root.0, "unicode-scalars");
}

#[test]
fn standard_text_transformations_cross_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        r#"use std/ptr

func main(): i32 {
    let text: &str = " \tNocter\r\n"
    if text.trim_ascii_start() != "Nocter\r\n" { return 1 }
    if text.trim_ascii_end() != " \tNocter" { return 2 }
    if text.trim_ascii() != "Nocter" { return 3 }

    let whitespace: &str = "\t \r\n"
    let empty_start = whitespace.trim_ascii_start()
    let empty_end = whitespace.trim_ascii_end()
    let empty_both = whitespace.trim_ascii()
    if empty_start.len() != 0 || empty_end.len() != 0 || empty_both.len() != 0 { return 4 }
    let whitespace_end = ptr.addr(whitespace.ptr()) + whitespace.len()
    if ptr.addr(empty_start.ptr()) != whitespace_end
        || ptr.addr(empty_end.ptr()) != whitespace_end
        || ptr.addr(empty_both.ptr()) != whitespace_end { return 5 }

    let unicode: &str = "\xE3\x80\x80\xC2\xA0Nocter\xC2\x85\xE3\x80\x80"
    let unicode_start = unicode.trim_start()
    let unicode_end = unicode.trim_end()
    let unicode_both = unicode.trim()
    if unicode_start != "Nocter\xC2\x85\xE3\x80\x80" { return 11 }
    if unicode_end != "\xE3\x80\x80\xC2\xA0Nocter" { return 12 }
    if unicode_both != "Nocter" { return 13 }
    let unicode_addr = ptr.addr(unicode.ptr())
    if ptr.addr(unicode_start.ptr()) != unicode_addr + 5
        || ptr.addr(unicode_end.ptr()) != unicode_addr
        || ptr.addr(unicode_both.ptr()) != unicode_addr + 5 { return 15 }
    let zero_width_space: &str = "Nocter\xE2\x80\x8B"
    if zero_width_space.trim() != zero_width_space { return 14 }

    let repeated = "é".repeat(3)
    if (&repeated as &str) != "ééé" { return 6 }
    let replaced = "aaaa/é".replace_all("aa", "b") catch _ { return 7 }
    if (&replaced as &str) != "bb/é" { return 8 }
    let _invalid = "x".replace_all("", "y") catch failure {
        if failure.has_code("std.str.empty_pattern") { return 0 }
        return 9
    }
    return 10
}
"#,
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_test(image.image(), &package_root.0, "text-transformations");
}

#[test]
fn standard_directory_stream_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        r#"use std/fs.FileType
use std/fs

async func open_and_drop(): void! {
    let stream = await fs.read_dir(".")?
    return
}

async func open_fails_with(path: &str, code: &str): bool {
    let _stream = await fs.read_dir(path) catch failure {
        return failure.has_code(code)
    }
    return false
}

async func closed_stream_has_no_entry(stream: &+fs.ReadDir): bool! {
    let _entry = await stream.next()? otherwise { return true }
    return false
}

async func inspect_directory(): i32! {
    let stream = await fs.read_dir(".")?
    var saw_file = false
    var saw_directory = false
    var saw_symlink = false
    var batch_count: usize = 0
    for await entry in move stream {
        let name = entry.file_name()
        let path: &str = entry.path()
        if name == "." || name == ".." { return 2 }
        if name == "regular.txt" {
            if entry.file_type() is FileType.regular { saw_file = true }
            if !saw_file { return 3 }
            if path != "./regular.txt" { return 4 }
        }
        if name == "nested" {
            if entry.file_type() is FileType.directory { saw_directory = true }
            if !saw_directory { return 5 }
        }
        if name == "link" {
            if entry.file_type() is FileType.symlink { saw_symlink = true }
            if !saw_symlink { return 6 }
        }
        if name.starts_with("batch-") { batch_count += 1 }
    }
    if !saw_file || !saw_directory || !saw_symlink || batch_count != 700 { return 7 }
    let followed = await fs.metadata("link")?
    if !followed.is_file() { return 46 }
    let link_metadata = await fs.symlink_metadata("link")?
    var link_is_symlink = false
    if link_metadata.file_type() is FileType.symlink { link_is_symlink = true }
    if !link_is_symlink { return 47 }
    await fs.symlink("regular.txt", "created-link")?
    await fs.remove_file("created-link")?
    if !await open_fails_with("missing", "std.io.not_found") { return 8 }
    if !await open_fails_with("regular.txt", "std.io.not_directory") { return 9 }

    var attempts: usize = 0
    while attempts < 512 {
        await open_and_drop()?
        attempts += 1
    }

    var closed = await fs.read_dir(".")?
    await closed.close()?
    if !await closed_stream_has_no_entry(&+closed)? { return 42 }

    await fs.create_dir_all("async-created/one/two")?
    let created = await fs.metadata("async-created/one/two")?
    if !created.is_directory() { return 43 }
    await fs.remove_dir("async-created/one/two")?
    await fs.remove_dir("async-created/one")?
    await fs.remove_dir("async-created")?
    await fs.create_dir_all("") catch failure {
        if !failure.has_code("std.io.invalid_input") { return 44 }
        return 42
    }
    return 45
}

async func main(): i32 {
    return await inspect_directory() catch _ { return 12 }
}
"#,
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    assert!(
        unit.syntax_diagnostics().is_empty(),
        "directory stream fixture has syntax diagnostics: {:#?}",
        unit.syntax_diagnostics()
    );

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_directory_stream(image.image(), &package_root.0, 42);
}

#[test]
fn standard_symbolic_link_targets_cross_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    std::fs::write(package_root.0.join("item"), b"value").unwrap();
    std::fs::hard_link(
        package_root.0.join("item"),
        package_root.0.join("hard-link"),
    )
    .unwrap();
    package_root.source(
        "main.nct",
        r#"use std/fs

async func main(): i32 {
    await fs.write_text("item", "value") catch _ { return 1 }
    await fs.create_dir("target") catch _ { return 9 }
    await fs.symlink("target/../item", "link") catch _ { return 2 }
    let target = await fs.read_link("link") catch _ { return 2 }
    let text: &str = &target
    if text != "target/../item" { return 3 }
    let canonical = await fs.canonicalize("link") catch _ { return 5 }
    let name = canonical.file_name() otherwise { return 6 }
    if name != "item" { return 7 }
    let copied = await fs.copy("item", "copy") catch _ { return 11 }
    if copied != 5 { return 12 }
    let copied_text = await fs.read_to_string("copy") catch _ { return 13 }
    if &copied_text != "value" { return 14 }
    var alias_rejected = false
    let _alias_copy = await fs.copy("item", "link") catch failure {
        if !failure.has_code("std.fs.same_file") { return 15 }
        alias_rejected = true
        0
    }
    if !alias_rejected { return 19 }
    var hard_link_rejected = false
    let _hard_link_copy = await fs.copy("item", "hard-link") catch failure {
        if !failure.has_code("std.fs.same_file") { return 20 }
        hard_link_rejected = true
        0
    }
    if !hard_link_rejected { return 21 }
    let preserved = await fs.read_to_string("item") catch _ { return 16 }
    if &preserved != "value" { return 17 }
    await fs.create_dir_all("recursive/first/second") catch _ { return 23 }
    await fs.write_text("recursive/first/second/leaf", "leaf") catch _ { return 24 }
    let walker = await fs.walk_dir("recursive") catch _ { return 28 }
    let walked = await count_walk(move walker) catch _ { return 29 }
    if walked != 3 { return 30 }
    await fs.remove_dir_all("recursive") catch _ { return 25 }
    let recursive_exists = await fs.exists("recursive") catch _ { return 26 }
    if recursive_exists { return 27 }
    await fs.remove_file("link") catch _ { return 4 }
    await fs.remove_file("item") catch _ { return 8 }
    await fs.remove_file("copy") catch _ { return 18 }
    await fs.remove_file("hard-link") catch _ { return 22 }
    await fs.remove_dir("target") catch _ { return 10 }
    return 0
}

async func count_walk(walker: fs.WalkDir): usize! {
    var walked: usize = 0
    for await entry in move walker {
        let _kind = entry.file_type()
        let _ = move entry
        walked += 1
    }
    return walked
}
"#,
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_test(image.image(), &package_root.0, "symbolic-link-targets");
}

#[test]
fn public_path_and_directory_lifecycle_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        r#"use std/fs
use std/fs.FileType
use std/path.Utf8Path

blocking func metadata_fails_with(path: &str, code: &str): bool {
    let _details = fs.metadata_blocking(path) catch failure {
        return failure.has_code(code)
    }
    return false
}

blocking func main(): i32! {
    let target = Utf8Path.new("workspace/cache/items.json")?
    let parent = target.parent() otherwise { return 1 }
    fs.create_dir_all_blocking(parent)?
    fs.write_text_blocking(&target, "value")?

    let file_name = target.file_name() otherwise { return 2 }
    let stem = target.file_stem() otherwise { return 3 }
    let extension = target.extension() otherwise { return 4 }
    if file_name != "items.json" { return 5 }
    if stem != "items" { return 6 }
    if extension != "json" { return 7 }

    let canonical = fs.canonicalize_blocking(&target)?
    let canonical_name = canonical.file_name() otherwise { return 13 }
    if canonical_name != "items.json" { return 14 }

    let copied = fs.copy_blocking(&target, "copied.json")?
    if copied != 5 { return 15 }
    let copied_text = fs.read_to_string_blocking("copied.json")?
    if &copied_text != "value" { return 16 }
    var self_copy_rejected = false
    let _self_copy = fs.copy_blocking(&target, &target) catch failure {
        if !failure.has_code("std.fs.same_file") { return 17 }
        self_copy_rejected = true
        0
    }
    if !self_copy_rejected { return 19 }
    let preserved = fs.read_to_string_blocking(&target)?
    if &preserved != "value" { return 18 }
    fs.remove_file_blocking("copied.json")?

    fs.remove_file_blocking(&target)?
    fs.remove_dir_blocking("workspace/cache")?
    fs.remove_dir_blocking("workspace")?

    var dangling_rejected = false
    fs.create_dir_blocking("dangling-root")?
    fs.symlink_blocking("missing", "dangling-root/link")?
    let dangling_target = fs.read_link_blocking("dangling-root/link")?
    if (&dangling_target as &str) != "missing" { return 12 }
    fs.create_dir_all_blocking("dangling-root/link/child") catch failure {
        dangling_rejected = failure.has_code("std.io.not_directory")
    }
    if !dangling_rejected { return 8 }

    var symlink_remove_rejected = false
    fs.remove_dir_blocking("dangling-root/link") catch failure {
        symlink_remove_rejected = failure.has_code("std.io.not_directory")
    }
    if !symlink_remove_rejected { return 9 }
    let dangling = fs.symlink_metadata_blocking("dangling-root/link")?
    var dangling_is_symlink = false
    if dangling.file_type() is FileType.symlink { dangling_is_symlink = true }
    if !dangling_is_symlink { return 10 }
    if !metadata_fails_with("dangling-root/link", "std.io.not_found") { return 11 }
    fs.remove_file_blocking("dangling-root/link")?
    fs.remove_dir_blocking("dangling-root")?

    fs.create_dir_blocking("real-root")?
    fs.symlink_blocking("real-root", "linked-root")?
    fs.create_dir_all_blocking("linked-root/child")?
    fs.remove_dir_blocking("linked-root/child")?
    fs.remove_file_blocking("linked-root")?
    fs.remove_dir_blocking("real-root")?
    return 0
}
"#,
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    assert!(
        unit.syntax_diagnostics().is_empty(),
        "public path and directory fixture has syntax diagnostics: {:#?}",
        unit.syntax_diagnostics()
    );

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_test(image.image(), &package_root.0, "public-path-directory");
}

#[test]
fn standard_streaming_lines_cross_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        include_str!("../../../../tests/fixtures/native/blocking_buffering.nct"),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    assert!(
        unit.syntax_diagnostics().is_empty(),
        "streaming line fixture has syntax diagnostics: {:#?}",
        unit.syntax_diagnostics()
    );

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_streaming_lines(image.image(), &package_root.0, 42);
}

#[test]
fn standard_input_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        r#"use std/io.BlockingReader
use std/io
use std/vec.Vec

func drop_stdin_wrapper(): void {
    let wrapper = io.stdin()
    return
}

blocking func main(): i32! {
    var input = io.stdin()
    let bytes = input.read_to_end_blocking()?
    if bytes.len() != 7 { return 1 }
    if bytes[0] != 97 || bytes[1] != 108 || bytes[2] != 112 { return 2 }
    if bytes[3] != 104 || bytes[4] != 97 || bytes[5] != 10 || bytes[6] != 255 { return 3 }

    input.close()
    var empty: Vec<u8> = Vec.empty()
    let _ = input.read_blocking(&+empty) catch closed_failure {
        if !closed_failure.has_code("std.io.closed") { return 4 }

        var after_close = io.stdin()
        if after_close.read_blocking(&+empty)? != 0 { return 5 }
        after_close.close()

        drop_stdin_wrapper()
        var after_drop = io.stdin()
        if after_drop.read_blocking(&+empty)? != 0 { return 6 }
        return 42
    }
    return 7
}
"#,
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    assert!(
        unit.syntax_diagnostics().is_empty(),
        "standard input fixture has syntax diagnostics: {:#?}",
        unit.syntax_diagnostics()
    );

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_standard_input(image.image(), &package_root.0, b"alpha\n\xff", 42);
}

#[test]
fn standard_buffered_input_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        r#"use std/io
use std/io/buffer.BlockingBufReader
use std/string.String

blocking func main(): i32! {
    var input = BlockingBufReader.with_capacity(io.stdin(), 3)
    var line = String.with_capacity(64)
    let original_capacity = line.capacity()

    if !input.read_line_into_blocking(&+line)? || (&line as &str) != "" { return 1 }
    if !input.read_line_into_blocking(&+line)? || (&line as &str) != "alpha" { return 2 }
    if !input.read_line_into_blocking(&+line)? || (&line as &str) != "lone\rbeta" { return 3 }
    if !input.read_line_into_blocking(&+line)? || (&line as &str) != "😀 split" { return 4 }
    let final_line = input.read_line_blocking()? otherwise { return 5 }
    if (&final_line as &str) != "final" { return 6 }
    if input.read_line_into_blocking(&+line)? { return 7 }
    if (&line as &str) != "" { return 8 }
    if line.capacity() != original_capacity { return 9 }
    let _after_eof = input.read_line_blocking()? otherwise { return 42 }
    return 10
}
"#,
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    assert!(
        unit.syntax_diagnostics().is_empty(),
        "buffered standard input fixture has syntax diagnostics: {:#?}",
        unit.syntax_diagnostics()
    );

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_standard_input(
        image.image(),
        &package_root.0,
        b"\nalpha\r\nlone\rbeta\n\xf0\x9f\x98\x80 split\nfinal",
        42,
    );
}

#[test]
fn standard_collection_ordering_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source("main.nct", COLLECTION_ORDERING_TEST_SOURCE);
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    assert!(
        unit.syntax_diagnostics().is_empty(),
        "collection ordering fixture has syntax diagnostics: {:#?}",
        unit.syntax_diagnostics()
    );

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_native_test(image.image(), &package_root.0, "collection-ordering");
}

#[test]
fn standard_filesystem_contract_crosses_native_tests() {
    let standard_root = nocter_test_support::standard_library_root();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"directory-records\", module: \"./fs\" }\n");
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    overlay
        .insert_source(
            standard_root.join("fs/directory_phase0_test.nct"),
            SourceOverride::new(DIRECTORY_RECORD_TEST_SOURCE.to_vec()),
        )
        .unwrap();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![resolved_standard(&standard_root, &standard_package)],
            overlay.finish(),
        ),
        vec![
            ModuleIdentity::new(standard_package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(standard_package.clone(), ["fs"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard filesystem tests failed native compilation")
    };
    assert_eq!(cases.len(), 8);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_path_lexical_contract_crosses_native_tests() {
    let standard_root = nocter_test_support::standard_library_root();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"path\", module: \"./path\" }\n");
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![resolved_standard(&standard_root, &standard_package)],
            overlay.finish(),
        ),
        vec![
            ModuleIdentity::new(standard_package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(standard_package.clone(), ["path"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard path tests failed native compilation")
    };
    assert_eq!(cases.len(), 2);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_url_contract_crosses_native_tests() {
    let standard_root = nocter_test_support::standard_library_root();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"url\", module: \"./url\" }\n");
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![resolved_standard(&standard_root, &standard_package)],
            overlay.finish(),
        ),
        vec![
            ModuleIdentity::new(standard_package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(standard_package.clone(), ["url"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard URL tests failed native compilation")
    };
    assert_eq!(cases.len(), 11);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_str_contract_crosses_native_tests() {
    let standard_root = nocter_test_support::standard_library_root();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str(concat!(
        "\n#test: { name: \"str\", module: \"./str\" }\n",
        "#test: { name: \"string\", module: \"./string\" }\n",
    ));
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![resolved_standard(&standard_root, &standard_package)],
            overlay.finish(),
        ),
        vec![
            ModuleIdentity::new(standard_package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(standard_package.clone(), ["str"]),
            ModuleIdentity::new(standard_package.clone(), ["string"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 2);
    let output = TempPackage::new();
    let mut case_count = 0;
    for target in compiled.targets() {
        let NativeTestTargetOutcome::Compiled(cases) = target.outcome() else {
            panic!("standard text tests failed native compilation")
        };
        for case in cases {
            case_count += 1;
            execute_native_test(case.image(), &output.0, case.identity().name());
        }
    }
    assert_eq!(case_count, 12);
}

#[test]
fn standard_hash_contract_crosses_native_tests() {
    let standard_root = nocter_test_support::standard_library_root();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"hash\", module: \"./hash\" }\n");
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![resolved_standard(&standard_root, &standard_package)],
            overlay.finish(),
        ),
        vec![
            ModuleIdentity::new(standard_package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(standard_package.clone(), ["hash"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard hash tests failed native compilation")
    };
    assert_eq!(cases.len(), 5);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_format_contract_crosses_native_tests() {
    let standard_root = nocter_test_support::standard_library_root();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"format\", module: \"./fmt\" }\n");
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![resolved_standard(&standard_root, &standard_package)],
            overlay.finish(),
        ),
        vec![
            ModuleIdentity::new(standard_package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(standard_package.clone(), ["fmt"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard format tests failed native compilation")
    };
    assert_eq!(cases.len(), 2);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}
