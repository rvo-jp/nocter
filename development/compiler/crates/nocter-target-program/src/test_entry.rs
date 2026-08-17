use std::fmt;

use nocter_declarations::PackageTargetKind;
use nocter_model::{BodyId, ModuleId, PackageId, PackageTargetId, Symbol, TestId};

use crate::TargetProgram;

/// One source-declared case directly owned by a selected test module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedTest {
    declaration: TestId,
    name: Symbol,
    body: BodyId,
}

impl SelectedTest {
    #[must_use]
    pub const fn declaration(self) -> TestId {
        self.declaration
    }

    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn body(self) -> BodyId {
        self.body
    }
}

/// The ordered compiler-owned runner roots for one package test target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedTestTarget {
    target: PackageTargetId,
    package: PackageId,
    module: ModuleId,
    tests: Box<[SelectedTest]>,
}

impl SelectedTestTarget {
    #[must_use]
    pub const fn target(&self) -> PackageTargetId {
        self.target
    }

    #[must_use]
    pub const fn package(&self) -> PackageId {
        self.package
    }

    #[must_use]
    pub const fn module(&self) -> ModuleId {
        self.module
    }

    #[must_use]
    pub const fn tests(&self) -> &[SelectedTest] {
        &self.tests
    }
}

/// Failure to derive compiler-owned test runners from a target program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TestSelectionError {
    UnknownTarget(PackageTargetId),
    NotTestTarget(PackageTargetId),
    InvalidTestDeclaration(TestId),
}

impl fmt::Display for TestSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTarget(_) => {
                formatter.write_str("test selection names an unknown package target")
            }
            Self::NotTestTarget(_) => formatter.write_str("selected package target is not a test"),
            Self::InvalidTestDeclaration(_) => {
                formatter.write_str("selected module contains an invalid test declaration")
            }
        }
    }
}

impl std::error::Error for TestSelectionError {}

/// Selects only tests directly declared by one package test target's module.
///
/// Arena order is declaration source order. Imported modules and dependencies are never scanned.
///
/// # Errors
///
/// Returns a typed selection or checked-program integrity failure.
pub fn select_test_target(
    program: &TargetProgram,
    selected: PackageTargetId,
) -> Result<SelectedTestTarget, TestSelectionError> {
    let graph = program.checked().graph();
    let target = graph
        .package_targets()
        .get(selected)
        .ok_or(TestSelectionError::UnknownTarget(selected))?;
    if target.kind() != PackageTargetKind::Test {
        return Err(TestSelectionError::NotTestTarget(selected));
    }
    let mut tests = Vec::new();
    for (id, declaration) in graph.declarations().tests().iter() {
        let Some(site) = graph.declaration_sites().get(declaration.site()) else {
            return Err(TestSelectionError::InvalidTestDeclaration(id));
        };
        if site.module() != target.module() {
            continue;
        }
        if graph
            .declarations()
            .bodies()
            .get(declaration.body())
            .is_none()
        {
            return Err(TestSelectionError::InvalidTestDeclaration(id));
        }
        tests.push(SelectedTest {
            declaration: id,
            name: declaration.name(),
            body: declaration.body(),
        });
    }
    Ok(SelectedTestTarget {
        target: selected,
        package: target.package(),
        module: target.module(),
        tests: tests.into_boxed_slice(),
    })
}
