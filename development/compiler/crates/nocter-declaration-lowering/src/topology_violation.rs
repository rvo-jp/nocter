use nocter_syntax::NodeId;
use nocter_syntax::SyntaxOrigin;

/// Stable source-level rule for authored source composition and module import topology.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TopologyRule {
    InvalidSourceVisibility,
    ModuleImportCycle,
    PackageDirectiveOutsideRoot,
}

impl TopologyRule {
    pub const ALL: [Self; 3] = [
        Self::InvalidSourceVisibility,
        Self::ModuleImportCycle,
        Self::PackageDirectiveOutsideRoot,
    ];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidSourceVisibility => "E0270",
            Self::ModuleImportCycle => "E0271",
            Self::PackageDirectiveOutsideRoot => "E0276",
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidSourceVisibility => "source see crosses a directory-module boundary",
            Self::ModuleImportCycle => "module imports form a dependency cycle",
            Self::PackageDirectiveOutsideRoot => {
                "package directives are permitted only in the package root index.nct"
            }
        }
    }

    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::InvalidSourceVisibility => {
                "see a source in the same directory module, or use the target directory module"
            }
            Self::ModuleImportCycle => "remove at least one module import in this cycle",
            Self::PackageDirectiveOutsideRoot => {
                "move this directive to the index.nct that contains #package"
            }
        }
    }

    #[must_use]
    pub const fn related_message(self) -> Option<&'static str> {
        match self {
            Self::InvalidSourceVisibility | Self::PackageDirectiveOutsideRoot => None,
            Self::ModuleImportCycle => Some("another import in this cycle is here"),
        }
    }
}

/// Exact syntax subjects for one authored topology-rule violation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopologyViolation {
    rule: TopologyRule,
    primary: SyntaxOrigin,
    related: Box<[SyntaxOrigin]>,
}

impl TopologyViolation {
    #[must_use]
    pub fn invalid_source_see(declaration: NodeId) -> Self {
        Self {
            rule: TopologyRule::InvalidSourceVisibility,
            primary: SyntaxOrigin::Node(declaration),
            related: Box::new([]),
        }
    }

    #[must_use]
    pub fn package_directive_outside_root(declaration: NodeId) -> Self {
        Self {
            rule: TopologyRule::PackageDirectiveOutsideRoot,
            primary: SyntaxOrigin::Node(declaration),
            related: Box::new([]),
        }
    }

    pub(crate) fn module_import_cycle(imports: Vec<NodeId>) -> Option<Self> {
        let mut imports = imports.into_iter().map(SyntaxOrigin::Node);
        let primary = imports.next()?;
        Some(Self {
            rule: TopologyRule::ModuleImportCycle,
            primary,
            related: imports.collect::<Vec<_>>().into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn rule(&self) -> TopologyRule {
        self.rule
    }

    #[must_use]
    pub const fn primary(&self) -> SyntaxOrigin {
        self.primary
    }

    #[must_use]
    pub const fn related(&self) -> &[SyntaxOrigin] {
        &self.related
    }
}
