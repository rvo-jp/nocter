use nocter_model::{ModuleId, PackageId, Symbol};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PackageTargetKind {
    Executable,
    Test,
}

/// One package target in explicit package-file declaration order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageTarget {
    package: PackageId,
    module: ModuleId,
    name: Symbol,
    kind: PackageTargetKind,
    declaration_order: u32,
}

impl PackageTarget {
    #[must_use]
    pub const fn new(
        package: PackageId,
        module: ModuleId,
        name: Symbol,
        kind: PackageTargetKind,
        declaration_order: u32,
    ) -> Self {
        Self {
            package,
            module,
            name,
            kind,
            declaration_order,
        }
    }

    #[must_use]
    pub const fn package(self) -> PackageId {
        self.package
    }

    #[must_use]
    pub const fn module(self) -> ModuleId {
        self.module
    }

    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn kind(self) -> PackageTargetKind {
        self.kind
    }

    #[must_use]
    pub const fn declaration_order(self) -> u32 {
        self.declaration_order
    }
}
