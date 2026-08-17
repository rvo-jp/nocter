//! Shared source fixture for compiler boundary and integration tests.

use nocter_declaration_lowering::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode,
    PackageTargetResolutionInput, UseResolutionInput, UseTargetInput,
};
use nocter_model::CompilationTarget;
use nocter_source::{SourceId, SourceMap, SourceName};
use nocter_syntax::{NodeKind, ParseGoal, SyntaxElement, SyntaxTree, parse};

const ERROR_SOURCE: &str = "pub(/) primitive new_error(code: &str, message: &str): error\n";
const MEM_SOURCE: &str = "\
pub(/) primitive current_allocator_state(): usize
pub(/) primitive current_allocator_kind(): usize
pub(/) primitive allocation_abort_raw(): never
";
const PTR_SOURCE: &str = "\
pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref<T>(value: &T): *T
pub primitive from_ref_mut<T>(value: &+T): *T
pub(/) primitive from_addr<T>(address: usize): *T
pub(/) primitive pointee_size<T>(pointer: *T): usize
pub(/) primitive pointee_align<T>(pointer: *T): usize
pub(/) primitive copy_str_to_ptr(destination: *u8, offset: usize, text: &str): void
pub(/) primitive copy_ptr_to_ptr(destination: *u8, source: *u8, byte_count: usize): void
pub(/) primitive store_u8_to_ptr(destination: *u8, offset: usize, value: u8): void
pub(/) primitive store_value_to_ptr<T>(destination: *T, offset: usize, value: T): void
pub(/) primitive drop_value_at_ptr<T>(pointer: *T, offset: usize): void
pub(/) primitive take_value_at_ptr<T>(pointer: *T, offset: usize): T
pub(/) primitive str_from_raw_parts(pointer: *u8, len: usize): &str
pub(/) primitive slice_from_raw_parts(pointer: *u8, len: usize): &[u8]
pub(/) primitive slice_from_raw_parts_mut(pointer: *u8, len: usize): &+[u8]
pub(/) primitive slice_from_raw_parts_value<T>(pointer: *T, len: usize): &[T]
pub(/) primitive slice_from_raw_parts_value_mut<T>(pointer: *T, len: usize): &+[T]
";
const STRING_SOURCE: &str = "\
pub(/) primitive bytes_from_str(value: &str): &[u8]
pub(/) primitive str_subview_unchecked(text: &str, start: usize, len: usize): &str from text
";
const SLICE_SOURCE: &str = "\
pub(/) primitive slice_len_raw<T>(value: &[T]): usize
pub(/) primitive slice_ptr_addr_raw<T>(value: &[T]): usize
";
const STR_SOURCE: &str = "\
pub(/) primitive str_len_raw(value: &str): usize
pub(/) primitive str_ptr_addr_raw(value: &str): usize
";
const PROCESS_SOURCE: &str = "\
#target: \"arm64-darwin\"
pub(/) primitive exit_raw(code: i32): never
#target: \"arm64-darwin\"
pub(/) primitive arg_count_raw(): usize
#target: \"arm64-darwin\"
pub(/) primitive arg_raw(index: usize): &str
#target: \"arm64-darwin\"
pub(/) primitive env_count_raw(): usize
#target: \"arm64-darwin\"
pub(/) primitive env_name_raw(index: usize): &str
#target: \"arm64-darwin\"
pub(/) primitive env_value_raw(index: usize): &str
";
const IO_SOURCE: &str = "\
#target: \"arm64-darwin\"
pub(/) primitive open_read_raw(path: *u8): i32!
#target: \"arm64-darwin\"
pub(/) primitive create_raw(path: *u8): i32!
#target: \"arm64-darwin\"
pub(/) primitive append_raw(path: *u8): i32!
#target: \"arm64-darwin\"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
#target: \"arm64-darwin\"
pub(/) primitive write_bytes_raw(fd: i32, bytes: &[u8]): void!
#target: \"arm64-darwin\"
pub(/) primitive read_bytes_raw(fd: i32, buffer: &+[u8]): usize!
#target: \"arm64-darwin\"
pub(/) primitive close_fd_raw(fd: i32): void
";
const INTERNAL_OS_SOURCE: &str = "\
#target: \"arm64-darwin\"
pub(/) copy struct SyscallResult {
    pub value: usize
    pub errno: i32
}
#target: \"arm64-darwin\"
pub(/) primitive syscall0(number: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive syscall1(number: usize, a0: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive syscall2(number: usize, a0: usize, a1: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive syscall3(number: usize, a0: usize, a1: usize, a2: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive syscall4(number: usize, a0: usize, a1: usize, a2: usize, a3: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive syscall5(number: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive syscall6(number: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive trap(): never
#target: \"arm64-darwin\"
pub(/) primitive unreachable(): never
";

struct FixtureModule {
    path: Box<[Box<str>]>,
    source_path: Box<str>,
    syntax: SyntaxTree,
}

/// Complete source input containing an application and the minimum compiler-selected standard
/// surface required to construct an `arm64-darwin` target program.
pub struct CompilerFixture {
    sources: SourceMap,
    app_manifest: Option<SyntaxTree>,
    std_manifest: SyntaxTree,
    app: SyntaxTree,
    standard: SyntaxTree,
    prelude: SyntaxTree,
    modules: Vec<FixtureModule>,
    app_standard_uses: Vec<Box<[Box<str>]>>,
}

impl CompilerFixture {
    #[must_use]
    pub fn new() -> Self {
        Self::with_app("func main(): i32! { return 0 }\n")
    }

    #[must_use]
    pub fn with_app(app_source: &str) -> Self {
        Self::build(app_source, None)
    }

    #[must_use]
    pub fn with_tests(app_source: &str) -> Self {
        Self::build(
            app_source,
            Some("#test: { name: \"unit\", module: \".\", }\n"),
        )
    }

    #[must_use]
    pub fn with_app_standard_uses(app_source: &str, modules: &[&[&str]]) -> Self {
        let mut fixture = Self::with_app(app_source);
        fixture.app_standard_uses = modules
            .iter()
            .map(|segments| {
                segments
                    .iter()
                    .map(|segment| Box::<str>::from(*segment))
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            })
            .collect();
        fixture
    }

    fn build(app_source: &str, app_manifest_source: Option<&str>) -> Self {
        let mut sources = SourceMap::new();
        let std_manifest = add_parsed(&mut sources, "/std/nocter.nct", "", ParseGoal::PackageFile);
        let app_manifest = app_manifest_source.map(|text| {
            add_parsed(
                &mut sources,
                "/app/nocter.nct",
                text,
                ParseGoal::PackageFile,
            )
        });
        let app_path = if app_manifest.is_some() {
            "/app/index.nct"
        } else {
            "/tmp/app.nct"
        };
        let app = add_parsed(&mut sources, app_path, app_source, ParseGoal::ModuleSource);
        let standard = add_parsed(&mut sources, "/std/index.nct", "", ParseGoal::ModuleSource);
        let prelude = add_parsed(
            &mut sources,
            "/std/prelude/index.nct",
            "",
            ParseGoal::ModuleSource,
        );
        let modules = [
            (&["error"][..], ERROR_SOURCE),
            (&["mem"][..], MEM_SOURCE),
            (&["ptr"][..], PTR_SOURCE),
            (&["string"][..], STRING_SOURCE),
            (&["slice"][..], SLICE_SOURCE),
            (&["str"][..], STR_SOURCE),
            (&["process"][..], PROCESS_SOURCE),
            (&["io"][..], IO_SOURCE),
            (&["internal", "os"][..], INTERNAL_OS_SOURCE),
        ]
        .into_iter()
        .map(|(path, text)| {
            let source_path = format!("/std/{}/index.nct", path.join("/"));
            let syntax = add_parsed(&mut sources, &source_path, text, ParseGoal::ModuleSource);
            FixtureModule {
                path: path
                    .iter()
                    .map(|segment| Box::<str>::from(*segment))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                source_path: source_path.into_boxed_str(),
                syntax,
            }
        })
        .collect();
        Self {
            sources,
            app_manifest,
            std_manifest,
            app,
            standard,
            prelude,
            modules,
            app_standard_uses: Vec::new(),
        }
    }

    /// Returns a borrowing compile-unit input and the standard prelude module.
    ///
    /// # Panics
    ///
    /// Panics when an internally authored fixture manifest lacks its required target directive.
    #[must_use]
    pub fn input(&self) -> (CompileUnitInput<'_>, ModuleIdentity) {
        let app_package = PackageIdentity::new("workspace:app");
        let standard_package = PackageIdentity::new("toolchain:std");
        let app_declaration = self
            .app_manifest
            .as_ref()
            .map(|manifest| PackageDeclarationInput::new("/app/nocter.nct", manifest));
        let packages = vec![
            PackageInput::new(
                app_package.clone(),
                "app",
                if app_declaration.is_some() {
                    PackageMode::Declared
                } else {
                    PackageMode::SingleFile
                },
                app_declaration,
            ),
            package(
                standard_package.clone(),
                "std",
                "/std/nocter.nct",
                &self.std_manifest,
            ),
        ];
        let mut modules = vec![
            ModuleInput::new(
                ModuleIdentity::new(app_package.clone(), Vec::<&str>::new()),
                vec![ModuleSourceInput::new(
                    if self.app_manifest.is_some() {
                        "/app/index.nct"
                    } else {
                        "/tmp/app.nct"
                    },
                    if self.app_manifest.is_some() {
                        ModuleSourceKind::Root
                    } else {
                        ModuleSourceKind::SingleFile
                    },
                    &self.app,
                )],
            ),
            module(
                standard_package.clone(),
                &[],
                "/std/index.nct",
                &self.standard,
            ),
            module(
                standard_package.clone(),
                &["prelude"],
                "/std/prelude/index.nct",
                &self.prelude,
            ),
        ];
        modules.extend(self.modules.iter().map(|fixture| {
            let segments = fixture.path.iter().map(AsRef::as_ref).collect::<Vec<_>>();
            module(
                standard_package.clone(),
                &segments,
                &fixture.source_path,
                &fixture.syntax,
            )
        }));
        let use_resolutions = self.app_use_resolutions(&standard_package);
        let prelude = ModuleIdentity::new(standard_package, ["prelude"]);
        let mut input = CompileUnitInput::new(
            CompilationTarget::Arm64Darwin,
            &self.sources,
            packages,
            modules,
            use_resolutions,
        );
        if let Some(manifest) = &self.app_manifest {
            let declaration = manifest
                .children(manifest.root_id())
                .iter()
                .find_map(|element| match element {
                    SyntaxElement::Node(node)
                        if manifest
                            .node(*node)
                            .is_some_and(|node| node.kind() == NodeKind::PackageDirective) =>
                    {
                        Some(*node)
                    }
                    SyntaxElement::Node(_)
                    | SyntaxElement::Token(_)
                    | SyntaxElement::Missing(_) => None,
                })
                .expect("test fixture manifest has no target directive");
            input = input.with_package_target_resolutions(vec![PackageTargetResolutionInput::new(
                declaration,
                ModuleIdentity::new(app_package, Vec::<&str>::new()),
            )]);
        }
        (input, prelude)
    }

    fn app_use_resolutions(&self, standard: &PackageIdentity) -> Vec<UseResolutionInput> {
        let nodes = use_declarations(&self.app);
        assert_eq!(nodes.len(), self.app_standard_uses.len());
        nodes
            .into_iter()
            .zip(&self.app_standard_uses)
            .map(|(declaration, path)| {
                UseResolutionInput::new(
                    declaration,
                    UseTargetInput::Module(ModuleIdentity::new(
                        standard.clone(),
                        path.iter().map(AsRef::as_ref),
                    )),
                )
            })
            .collect()
    }
}

impl Default for CompilerFixture {
    fn default() -> Self {
        Self::new()
    }
}

fn use_declarations(tree: &SyntaxTree) -> Vec<nocter_syntax::NodeId> {
    let mut declarations = Vec::new();
    let mut pending = vec![tree.root_id()];
    while let Some(node) = pending.pop() {
        if tree
            .node(node)
            .is_some_and(|node| node.kind() == NodeKind::UseDeclaration)
        {
            declarations.push(node);
        }
        for child in tree.children(node).iter().rev() {
            if let SyntaxElement::Node(child) = child {
                pending.push(*child);
            }
        }
    }
    declarations.sort_unstable_by_key(|node| {
        tree.node(*node)
            .map(nocter_syntax::SyntaxNode::range)
            .map(nocter_source::TextRange::start)
    });
    declarations
}

fn add_parsed(sources: &mut SourceMap, name: &str, text: &str, goal: ParseGoal) -> SyntaxTree {
    let source = sources
        .add_bytes(SourceName::new(name), text.as_bytes())
        .unwrap();
    parsed(sources, source, goal)
}

fn parsed(sources: &SourceMap, source: SourceId, goal: ParseGoal) -> SyntaxTree {
    let tree = parse(sources.get(source).unwrap(), goal);
    assert!(!tree.has_errors(), "{:#?}", tree.diagnostics());
    tree
}

fn package<'syntax>(
    identity: PackageIdentity,
    name: &str,
    path: &str,
    manifest: &'syntax SyntaxTree,
) -> PackageInput<'syntax> {
    PackageInput::new(
        identity,
        name,
        PackageMode::Declared,
        Some(PackageDeclarationInput::new(path, manifest)),
    )
}

fn module<'syntax>(
    package: PackageIdentity,
    path: &[&str],
    source_path: &str,
    source: &'syntax SyntaxTree,
) -> ModuleInput<'syntax> {
    ModuleInput::new(
        ModuleIdentity::new(package, path.iter().copied()),
        vec![ModuleSourceInput::new(
            source_path,
            ModuleSourceKind::Root,
            source,
        )],
    )
}
