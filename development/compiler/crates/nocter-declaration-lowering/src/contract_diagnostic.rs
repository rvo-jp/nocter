use std::fmt;

use nocter_source_index::SourceOrigin;
use nocter_syntax::NodeId;

use crate::{
    CallableContractError, DeclarationSurface, DiagnosticNote, SourceDiagnostic,
    diagnostic::origin_from_trees,
};

/// Stable source-level rule for public contract and private body joining.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CallableContractRule {
    MissingBody,
    MismatchedBody,
    DuplicateBody,
    InvalidBodyOmission,
}

impl CallableContractRule {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::MissingBody => "E0250",
            Self::MismatchedBody => "E0251",
            Self::DuplicateBody => "E0252",
            Self::InvalidBodyOmission => "E0253",
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::MissingBody => "public callable contract has no implementation body",
            Self::MismatchedBody => {
                "private implementation body does not match its public contract"
            }
            Self::DuplicateBody => "public callable contract has more than one implementation body",
            Self::InvalidBodyOmission => {
                "callable omits its body outside an eligible public contract"
            }
        }
    }

    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::MissingBody => "add one exact private implementation body in the same module",
            Self::MismatchedBody => {
                "make the private body header exactly match the public contract"
            }
            Self::DuplicateBody => "keep exactly one matching private implementation body",
            Self::InvalidBodyOmission => {
                "write the body inline or declare an eligible public root contract"
            }
        }
    }

    #[must_use]
    pub const fn related_message(self) -> Option<&'static str> {
        match self {
            Self::MismatchedBody | Self::DuplicateBody => Some("public contract is declared here"),
            Self::MissingBody | Self::InvalidBodyOmission => None,
        }
    }
}

/// A callable-contract rule projected to exact source syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableContractDiagnostic {
    rule: CallableContractRule,
    source: Box<SourceDiagnostic>,
}

impl CallableContractDiagnostic {
    pub(crate) fn project(
        error: CallableContractError,
        surface: &DeclarationSurface<'_>,
    ) -> Result<Self, CallableContractError> {
        let rule = rule(error).ok_or(error)?;
        let primary = origin(surface, primary_node(error)).ok_or(error)?;
        let notes = related_node(error)
            .zip(rule.related_message())
            .map(|(node, message)| {
                origin(surface, node)
                    .map(|origin| DiagnosticNote::new(message, origin))
                    .ok_or(error)
            })
            .transpose()?
            .into_iter()
            .collect::<Vec<_>>();
        let source = SourceDiagnostic::new(
            rule.code(),
            rule.message(),
            primary,
            notes,
            Some(rule.help()),
        );
        Ok(Self {
            rule,
            source: Box::new(source),
        })
    }

    #[must_use]
    pub const fn rule(&self) -> CallableContractRule {
        self.rule
    }

    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}

impl fmt::Display for CallableContractDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}",
            self.source.code(),
            self.source.message()
        )
    }
}

impl std::error::Error for CallableContractDiagnostic {}

const fn rule(error: CallableContractError) -> Option<CallableContractRule> {
    match error {
        CallableContractError::MissingBody(_) => Some(CallableContractRule::MissingBody),
        CallableContractError::MismatchedBody { .. }
        | CallableContractError::UnmatchedImplementationEntry(_) => {
            Some(CallableContractRule::MismatchedBody)
        }
        CallableContractError::DuplicateBody { .. } => Some(CallableContractRule::DuplicateBody),
        CallableContractError::InvalidBodyOmission(_) => {
            Some(CallableContractRule::InvalidBodyOmission)
        }
        CallableContractError::InconsistentSurface(_) => None,
    }
}

const fn primary_node(error: CallableContractError) -> NodeId {
    match error {
        CallableContractError::MissingBody(node)
        | CallableContractError::InvalidBodyOmission(node)
        | CallableContractError::UnmatchedImplementationEntry(node)
        | CallableContractError::InconsistentSurface(node) => node,
        CallableContractError::MismatchedBody { body, .. }
        | CallableContractError::DuplicateBody { body, .. } => body,
    }
}

const fn related_node(error: CallableContractError) -> Option<NodeId> {
    match error {
        CallableContractError::MismatchedBody { contract, .. }
        | CallableContractError::DuplicateBody { contract, .. } => Some(contract),
        CallableContractError::MissingBody(_)
        | CallableContractError::InvalidBodyOmission(_)
        | CallableContractError::UnmatchedImplementationEntry(_)
        | CallableContractError::InconsistentSurface(_) => None,
    }
}

fn origin(surface: &DeclarationSurface<'_>, node: NodeId) -> Option<SourceOrigin> {
    origin_from_trees(
        surface.sources().iter().map(crate::SurfaceSource::syntax),
        node,
    )
}

#[cfg(test)]
mod tests {
    use nocter_source::{SourceMap, SourceName};
    use nocter_syntax::{ParseGoal, SyntaxTree, parse};

    use super::{CallableContractDiagnostic, CallableContractRule};
    use crate::test_support::source_use;
    use crate::{
        CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
        PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode,
        analyze_callable_contracts, collect_declaration_surface,
    };

    #[test]
    fn mismatch_projects_body_as_primary_and_contract_as_related() {
        let mut sources = SourceMap::new();
        let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let root_id = add_source(
            &mut sources,
            "/app/index.nct",
            "use ./parse\n\npub func parse(text: &str): usize\n",
        );
        let implementation_id = add_source(
            &mut sources,
            "/app/parse.nct",
            "func parse(text: usize): usize { text }\n",
        );
        let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
        let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
        let implementation = parse_source(&sources, implementation_id, ParseGoal::ModuleSource);
        let package = PackageInput::new(
            PackageIdentity::new("workspace:app"),
            "app",
            PackageMode::Declared,
            Some(PackageDeclarationInput::new("/app/nocter.nct", &manifest)),
        );
        let module = ModuleInput::new(
            ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new()),
            vec![
                ModuleSourceInput::new(
                    "/app/parse.nct",
                    ModuleSourceKind::Implementation,
                    &implementation,
                ),
                ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ],
        );
        let input = CompileUnitInput::new(
            &sources,
            vec![package],
            vec![module],
            vec![source_use(&root, 0, "/app/parse.nct")],
        );
        let surface = collect_declaration_surface(&input).unwrap();

        let error = analyze_callable_contracts(&surface).unwrap_err();
        let diagnostic = CallableContractDiagnostic::project(error, &surface).unwrap();

        assert_eq!(diagnostic.rule(), CallableContractRule::MismatchedBody);
        assert_eq!(diagnostic.source().code(), "E0251");
        assert_eq!(diagnostic.source().primary().source(), implementation_id);
        assert_eq!(diagnostic.source().notes().len(), 1);
        assert_eq!(diagnostic.source().notes()[0].origin().source(), root_id);
        assert_eq!(
            diagnostic.source().notes()[0].message(),
            "public contract is declared here"
        );
    }

    fn add_source(sources: &mut SourceMap, name: &str, text: &str) -> nocter_source::SourceId {
        sources
            .add_bytes(SourceName::new(name), text.as_bytes())
            .unwrap()
    }

    fn parse_source(
        sources: &SourceMap,
        source: nocter_source::SourceId,
        goal: ParseGoal,
    ) -> SyntaxTree {
        let tree = parse(sources.get(source).unwrap(), goal);
        assert!(!tree.has_errors(), "{:#?}", tree.diagnostics());
        tree
    }
}
