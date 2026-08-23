use nocter_source_index::SyntaxOrigin;
use nocter_syntax::NodeId;

/// Stable source-level rule for authored source composition and module import topology.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TopologyRule {
    InvalidSourceInclude,
    ModuleImportCycle,
}

impl TopologyRule {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidSourceInclude => "E0270",
            Self::ModuleImportCycle => "E0271",
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidSourceInclude => "source include crosses a directory-module boundary",
            Self::ModuleImportCycle => "module imports form a dependency cycle",
        }
    }

    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::InvalidSourceInclude => {
                "include a source in the same directory module, or use the target directory module"
            }
            Self::ModuleImportCycle => "remove at least one module import in this cycle",
        }
    }

    #[must_use]
    pub const fn related_message(self) -> Option<&'static str> {
        match self {
            Self::InvalidSourceInclude => None,
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
    pub fn invalid_source_include(declaration: NodeId) -> Self {
        Self {
            rule: TopologyRule::InvalidSourceInclude,
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
