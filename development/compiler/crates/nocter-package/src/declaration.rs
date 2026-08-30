use std::collections::BTreeMap;
use std::fmt;

use nocter_model::PackageTargetKind;
use nocter_source::SourceFile;
use nocter_syntax::{
    Keyword, NodeId, NodeKind, SyntaxElement, SyntaxTree, TokenKind, child_node_iter,
    decode_string_literal, direct_node,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredString {
    value: Box<str>,
    literal: NodeId,
}

impl AuthoredString {
    #[must_use]
    pub const fn value(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub const fn literal(&self) -> NodeId {
        self.literal
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencySource {
    Git {
        url: AuthoredString,
        revision: AuthoredString,
    },
    Archive {
        url: AuthoredString,
    },
    Path {
        path: AuthoredString,
    },
}

impl DependencySource {
    #[must_use]
    pub const fn exact_lock_kind(&self) -> Option<crate::ExactDependencyLockKind> {
        match self {
            Self::Git { .. } => Some(crate::ExactDependencyLockKind::Git),
            Self::Archive { .. } => Some(crate::ExactDependencyLockKind::Sha256),
            Self::Path { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyDeclaration {
    record: NodeId,
    source: DependencySource,
    selection: Option<DependencyExactSelection>,
}

impl DependencyDeclaration {
    #[must_use]
    pub const fn record(&self) -> NodeId {
        self.record
    }

    #[must_use]
    pub const fn source(&self) -> &DependencySource {
        &self.source
    }

    #[must_use]
    pub const fn selection(&self) -> Option<&DependencyExactSelection> {
        self.selection.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyExactSelection {
    GitCommit(AuthoredString),
    ArchiveSha256(AuthoredString),
}

impl DependencyExactSelection {
    /// Removes syntax identity from an already validated authored exact selection.
    #[must_use]
    pub fn exact(&self) -> crate::ExactDependencyLock {
        match self {
            Self::GitCommit(authored) => crate::ExactDependencyLock::validated(
                crate::ExactDependencyLockKind::Git,
                authored.value(),
            ),
            Self::ArchiveSha256(authored) => crate::ExactDependencyLock::validated(
                crate::ExactDependencyLockKind::Sha256,
                authored.value(),
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageTargetDeclaration {
    declaration: NodeId,
    name: AuthoredString,
    kind: PackageTargetKind,
    order: u32,
    module: Box<[Box<str>]>,
}

impl PackageTargetDeclaration {
    #[must_use]
    pub const fn declaration(&self) -> NodeId {
        self.declaration
    }

    #[must_use]
    pub const fn name(&self) -> &AuthoredString {
        &self.name
    }

    #[must_use]
    pub const fn kind(&self) -> PackageTargetKind {
        self.kind
    }

    #[must_use]
    pub const fn order(&self) -> u32 {
        self.order
    }

    #[must_use]
    pub const fn module(&self) -> &[Box<str>] {
        &self.module
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageDeclaration {
    name: AuthoredString,
    version: AuthoredString,
    dependencies: BTreeMap<Box<str>, DependencyDeclaration>,
    targets: Box<[PackageTargetDeclaration]>,
}

impl PackageDeclaration {
    #[must_use]
    pub const fn name(&self) -> &AuthoredString {
        &self.name
    }

    #[must_use]
    pub const fn version(&self) -> &AuthoredString {
        &self.version
    }

    #[must_use]
    pub const fn dependencies(&self) -> &BTreeMap<Box<str>, DependencyDeclaration> {
        &self.dependencies
    }

    #[must_use]
    pub const fn targets(&self) -> &[PackageTargetDeclaration] {
        &self.targets
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageDeclarationError {
    subject: NodeId,
    rule: PackageDeclarationRule,
    name: Option<Box<str>>,
}

impl PackageDeclarationError {
    #[must_use]
    pub const fn subject(&self) -> NodeId {
        self.subject
    }

    #[must_use]
    pub const fn rule(&self) -> PackageDeclarationRule {
        self.rule
    }

    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageDeclarationRule {
    SyntaxErrorsPresent,
    InvalidDirective,
    DuplicateDirective,
    ExpectedString,
    ExpectedRecord,
    UnknownField,
    DuplicateField,
    MissingField,
    InvalidDependencyAlias,
    ReservedStandardDependency,
    InvalidDependencySource,
    InvalidTargetName,
    InvalidModulePath,
    InvalidGitCommit,
    InvalidArchiveDigest,
    UnexpectedDependencySelection,
    TargetOrderOverflow,
    MissingPackageDirective,
}

impl fmt::Display for PackageDeclarationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid package declaration: {:?}", self.rule)?;
        if let Some(name) = &self.name {
            write!(formatter, " ({name})")?;
        }
        Ok(())
    }
}

impl std::error::Error for PackageDeclarationError {}

/// Decodes every data-bearing package directive exactly once.
///
/// # Errors
///
/// Rejects malformed, duplicate, unknown, or mutually inconsistent declaration data. Parse
/// diagnostics must be handled before this semantic package boundary.
pub fn decode_package_declaration(
    source: &SourceFile,
    tree: &SyntaxTree,
) -> Result<PackageDeclaration, PackageDeclarationError> {
    if tree.has_errors()
        || tree
            .node(tree.root_id())
            .is_none_or(|root| root.kind() != NodeKind::SourceFile)
    {
        return Err(error(
            tree.root_id(),
            PackageDeclarationRule::SyntaxErrorsPresent,
            None,
        ));
    }

    let mut package = None;
    let mut dependencies = None;
    let mut targets = Vec::new();
    let mut target_order = 0_u32;

    for declaration in child_node_iter(tree, tree.root_id()) {
        if tree
            .node(declaration)
            .is_none_or(|node| node.kind() != NodeKind::PackageDirective)
        {
            continue;
        }
        let directive = directive_name(source, tree, declaration)
            .ok_or_else(|| error(declaration, PackageDeclarationRule::InvalidDirective, None))?;
        match directive.as_ref() {
            "package" => set_once(
                &mut package,
                decode_package_header(source, tree, declaration)?,
                declaration,
                "package",
            )?,
            "dependencies" => set_once(
                &mut dependencies,
                decode_dependencies(source, tree, declaration)?,
                declaration,
                "dependencies",
            )?,
            "executable" | "test" => {
                let kind = if directive.as_ref() == "executable" {
                    PackageTargetKind::Executable
                } else {
                    PackageTargetKind::Test
                };
                targets.push(decode_target(
                    source,
                    tree,
                    declaration,
                    kind,
                    target_order,
                )?);
                target_order = target_order.checked_add(1).ok_or_else(|| {
                    error(
                        declaration,
                        PackageDeclarationRule::TargetOrderOverflow,
                        None,
                    )
                })?;
            }
            _ => {
                return Err(error(
                    declaration,
                    PackageDeclarationRule::InvalidDirective,
                    Some(directive),
                ));
            }
        }
    }

    let dependencies = dependencies.unwrap_or_default();
    let (name, version) = package.ok_or_else(|| {
        error(
            tree.root_id(),
            PackageDeclarationRule::MissingPackageDirective,
            Some("package".into()),
        )
    })?;
    Ok(PackageDeclaration {
        name,
        version,
        dependencies,
        targets: targets.into_boxed_slice(),
    })
}

fn decode_package_header(
    source: &SourceFile,
    tree: &SyntaxTree,
    declaration: NodeId,
) -> Result<(AuthoredString, AuthoredString), PackageDeclarationError> {
    let record = required_record(tree, declaration)?;
    let fields = unique_fields(source, tree, record)?;
    for name in fields.keys() {
        if !matches!(name.as_ref(), "name" | "version") {
            return Err(error(
                fields[name],
                PackageDeclarationRule::UnknownField,
                Some(name.clone()),
            ));
        }
    }
    let name = required_field(&fields, declaration, "name")?;
    let version = required_field(&fields, declaration, "version")?;
    Ok((
        authored_string(source, tree, name)?,
        authored_string(source, tree, version)?,
    ))
}

fn decode_dependencies(
    source: &SourceFile,
    tree: &SyntaxTree,
    declaration: NodeId,
) -> Result<BTreeMap<Box<str>, DependencyDeclaration>, PackageDeclarationError> {
    let record = required_record(tree, declaration)?;
    let mut result = BTreeMap::new();
    for field in direct_fields(tree, record) {
        let alias = field_name(source, tree, field)?;
        if !valid_module_segment(&alias) {
            return Err(error(
                field,
                PackageDeclarationRule::InvalidDependencyAlias,
                Some(alias),
            ));
        }
        if alias.as_ref() == "std" {
            return Err(error(
                field,
                PackageDeclarationRule::ReservedStandardDependency,
                Some(alias),
            ));
        }
        let dependency = decode_dependency(source, tree, field, &alias)?;
        if result.insert(alias.clone(), dependency).is_some() {
            return Err(error(
                field,
                PackageDeclarationRule::DuplicateField,
                Some(alias),
            ));
        }
    }
    Ok(result)
}

fn decode_dependency(
    source: &SourceFile,
    tree: &SyntaxTree,
    field: NodeId,
    alias: &str,
) -> Result<DependencyDeclaration, PackageDeclarationError> {
    let record = required_record(tree, field)?;
    let fields = unique_fields(source, tree, record)?;
    let git = optional_string(source, tree, &fields, "git")?;
    let revision = optional_string(source, tree, &fields, "revision")?;
    let archive = optional_string(source, tree, &fields, "archive")?;
    let path = optional_string(source, tree, &fields, "path")?;
    let commit = optional_string(source, tree, &fields, "commit")?;
    let sha256 = optional_string(source, tree, &fields, "sha256")?;
    for name in fields.keys() {
        if !matches!(
            name.as_ref(),
            "git" | "revision" | "archive" | "path" | "commit" | "sha256"
        ) {
            return Err(error(
                fields[name],
                PackageDeclarationRule::UnknownField,
                Some(name.clone()),
            ));
        }
    }
    let dependency_source = match (git, revision, archive, path) {
        (Some(url), Some(revision), None, None) => DependencySource::Git { url, revision },
        (None, None, Some(url), None) => DependencySource::Archive { url },
        (None, None, None, Some(path)) => DependencySource::Path { path },
        _ => {
            return Err(error(
                field,
                PackageDeclarationRule::InvalidDependencySource,
                Some(alias.into()),
            ));
        }
    };
    let selection = match (&dependency_source, commit, sha256) {
        (DependencySource::Git { .. }, Some(commit), None) => {
            if !valid_hex(commit.value(), 40) {
                return Err(error(
                    commit.literal(),
                    PackageDeclarationRule::InvalidGitCommit,
                    Some(alias.into()),
                ));
            }
            Some(DependencyExactSelection::GitCommit(commit))
        }
        (DependencySource::Archive { .. }, None, Some(sha256)) => {
            if !valid_hex(sha256.value(), 64) {
                return Err(error(
                    sha256.literal(),
                    PackageDeclarationRule::InvalidArchiveDigest,
                    Some(alias.into()),
                ));
            }
            Some(DependencyExactSelection::ArchiveSha256(sha256))
        }
        (
            DependencySource::Git { .. }
            | DependencySource::Archive { .. }
            | DependencySource::Path { .. },
            None,
            None,
        ) => None,
        _ => {
            return Err(error(
                field,
                PackageDeclarationRule::UnexpectedDependencySelection,
                Some(alias.into()),
            ));
        }
    };
    Ok(DependencyDeclaration {
        record,
        source: dependency_source,
        selection,
    })
}

fn decode_target(
    source: &SourceFile,
    tree: &SyntaxTree,
    declaration: NodeId,
    kind: PackageTargetKind,
    order: u32,
) -> Result<PackageTargetDeclaration, PackageDeclarationError> {
    let record = required_record(tree, declaration)?;
    let fields = unique_fields(source, tree, record)?;
    for name in fields.keys() {
        if !matches!(name.as_ref(), "name" | "module") {
            return Err(error(
                fields[name],
                PackageDeclarationRule::UnknownField,
                Some(name.clone()),
            ));
        }
    }
    let name_field = required_field(&fields, declaration, "name")?;
    let name = authored_string(source, tree, name_field)?;
    if name.value().is_empty() {
        return Err(error(
            name.literal(),
            PackageDeclarationRule::InvalidTargetName,
            None,
        ));
    }
    let module = if let Some(field) = fields.get("module") {
        let authored = authored_string(source, tree, *field)?;
        parse_module_path(authored.value()).ok_or_else(|| {
            error(
                authored.literal(),
                PackageDeclarationRule::InvalidModulePath,
                Some(authored.value.clone()),
            )
        })?
    } else if kind == PackageTargetKind::Executable {
        Box::new([])
    } else {
        return Err(error(
            declaration,
            PackageDeclarationRule::MissingField,
            Some("module".into()),
        ));
    };
    Ok(PackageTargetDeclaration {
        declaration,
        name,
        kind,
        order,
        module,
    })
}

fn set_once<T>(
    slot: &mut Option<T>,
    value: T,
    subject: NodeId,
    name: &'static str,
) -> Result<(), PackageDeclarationError> {
    if slot.replace(value).is_some() {
        Err(error(
            subject,
            PackageDeclarationRule::DuplicateDirective,
            Some(name.into()),
        ))
    } else {
        Ok(())
    }
}

fn unique_fields(
    source: &SourceFile,
    tree: &SyntaxTree,
    record: NodeId,
) -> Result<BTreeMap<Box<str>, NodeId>, PackageDeclarationError> {
    let mut fields = BTreeMap::new();
    for field in direct_fields(tree, record) {
        let name = field_name(source, tree, field)?;
        if fields.insert(name.clone(), field).is_some() {
            return Err(error(
                field,
                PackageDeclarationRule::DuplicateField,
                Some(name),
            ));
        }
    }
    Ok(fields)
}

fn required_field(
    fields: &BTreeMap<Box<str>, NodeId>,
    subject: NodeId,
    name: &'static str,
) -> Result<NodeId, PackageDeclarationError> {
    fields.get(name).copied().ok_or_else(|| {
        error(
            subject,
            PackageDeclarationRule::MissingField,
            Some(name.into()),
        )
    })
}

fn optional_string(
    source: &SourceFile,
    tree: &SyntaxTree,
    fields: &BTreeMap<Box<str>, NodeId>,
    name: &str,
) -> Result<Option<AuthoredString>, PackageDeclarationError> {
    fields
        .get(name)
        .map(|field| authored_string(source, tree, *field))
        .transpose()
}

fn authored_string(
    source: &SourceFile,
    tree: &SyntaxTree,
    subject: NodeId,
) -> Result<AuthoredString, PackageDeclarationError> {
    let literal = value_node(tree, subject)
        .and_then(|value| direct_node(tree, value, NodeKind::StringLiteral))
        .ok_or_else(|| error(subject, PackageDeclarationRule::ExpectedString, None))?;
    let value = decode_string_literal(source, tree, literal)
        .ok_or_else(|| error(literal, PackageDeclarationRule::ExpectedString, None))?;
    Ok(AuthoredString { value, literal })
}

fn required_record(tree: &SyntaxTree, subject: NodeId) -> Result<NodeId, PackageDeclarationError> {
    value_node(tree, subject)
        .and_then(|value| direct_node(tree, value, NodeKind::DirectiveRecord))
        .ok_or_else(|| error(subject, PackageDeclarationRule::ExpectedRecord, None))
}

fn value_node(tree: &SyntaxTree, subject: NodeId) -> Option<NodeId> {
    direct_node(tree, subject, NodeKind::DirectiveValue)
}

fn directive_name(source: &SourceFile, tree: &SyntaxTree, declaration: NodeId) -> Option<Box<str>> {
    tree.children(declaration).iter().find_map(|element| {
        let SyntaxElement::Token(token) = element else {
            return None;
        };
        match token.kind() {
            TokenKind::Identifier => source.text_at(token.range()).map(Into::into),
            TokenKind::Keyword(Keyword::Test) => Some("test".into()),
            _ => None,
        }
    })
}

fn field_name(
    source: &SourceFile,
    tree: &SyntaxTree,
    field: NodeId,
) -> Result<Box<str>, PackageDeclarationError> {
    tree.children(field)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) if token.kind() == TokenKind::Identifier => {
                source.text_at(token.range()).map(Into::into)
            }
            SyntaxElement::Node(_) | SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
        })
        .ok_or_else(|| error(field, PackageDeclarationRule::InvalidDirective, None))
}

fn parse_module_path(authored: &str) -> Option<Box<[Box<str>]>> {
    if authored == "." {
        return Some(Box::new([]));
    }
    let relative = authored.strip_prefix("./")?;
    if relative.is_empty() {
        return None;
    }
    relative
        .split('/')
        .map(|segment| valid_module_segment(segment).then(|| Box::<str>::from(segment)))
        .collect::<Option<Vec<_>>>()
        .map(Vec::into_boxed_slice)
}

fn valid_module_segment(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    !bytes.is_empty()
        && segment != "_"
        && !bytes[0].is_ascii_digit()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
        && Keyword::from_spelling(segment).is_none()
}

fn valid_hex(value: &str, digits: usize) -> bool {
    value.len() == digits && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn direct_fields(tree: &SyntaxTree, record: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    child_node_iter(tree, record).filter(|child| {
        tree.node(*child)
            .is_some_and(|node| node.kind() == NodeKind::DirectiveField)
    })
}

fn error(
    subject: NodeId,
    rule: PackageDeclarationRule,
    name: Option<Box<str>>,
) -> PackageDeclarationError {
    PackageDeclarationError {
        subject,
        rule,
        name,
    }
}

#[cfg(test)]
mod tests {
    use nocter_source::{SourceMap, SourceName};
    use nocter_syntax::{ParseGoal, parse};

    use super::*;

    fn decode(text: &str) -> Result<PackageDeclaration, PackageDeclarationError> {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(SourceName::new("/package/index.nct"), text.as_bytes())
            .unwrap();
        let source = sources.get(source).unwrap();
        let syntax = parse(source, ParseGoal::SourceFile);
        decode_package_declaration(source, &syntax)
    }

    #[test]
    fn decodes_complete_package_data_and_exact_target_order() {
        let declaration = decode(
            "#package: { name: \"app\", version: \"1.2.3\", }\n\
             #dependencies: {\n\
               git_dep: { git: \"https://example.test/a.git\", revision: \"main\", commit: \"0123456789abcdef0123456789abcdef01234567\", },\n\
               archive_dep: { archive: \"https://example.test/a.tar.gz\", sha256: \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\", },\n\
               local_dep: { path: \"./local\", },\n\
             }\n\
             #executable: { name: \"app\", }\n\
             #test: { name: \"unit\", module: \"./tests/unit\", }\n",
        )
        .unwrap();

        assert_eq!(declaration.name().value(), "app");
        assert_eq!(declaration.version().value(), "1.2.3");
        assert_eq!(declaration.dependencies().len(), 3);
        assert!(declaration.dependencies()["git_dep"].selection().is_some());
        assert!(
            declaration.dependencies()["archive_dep"]
                .selection()
                .is_some()
        );
        assert!(
            declaration.dependencies()["local_dep"]
                .selection()
                .is_none()
        );
        assert_eq!(declaration.targets().len(), 2);
        assert_eq!(
            declaration.targets()[0].kind(),
            PackageTargetKind::Executable
        );
        assert_eq!(declaration.targets()[0].order(), 0);
        assert!(declaration.targets()[0].module().is_empty());
        assert_eq!(declaration.targets()[1].kind(), PackageTargetKind::Test);
        assert_eq!(declaration.targets()[1].order(), 1);
        assert_eq!(
            declaration.targets()[1]
                .module()
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<_>>(),
            ["tests", "unit"]
        );
    }

    #[test]
    fn rejects_inconsistent_source_and_exact_selection_shapes() {
        let mixed =
            decode("#dependencies: { bad: { git: \"u\", revision: \"r\", path: \"p\", }, }\n")
                .unwrap_err();
        assert_eq!(
            mixed.rule(),
            PackageDeclarationRule::InvalidDependencySource
        );

        let wrong_selection = decode(
            "#dependencies: { archive_dep: { archive: \"u\", commit: \"0123456789abcdef0123456789abcdef01234567\", }, }\n",
        )
        .unwrap_err();
        assert_eq!(
            wrong_selection.rule(),
            PackageDeclarationRule::UnexpectedDependencySelection
        );

        let malformed_commit = decode(
            "#dependencies: { git_dep: { git: \"u\", revision: \"r\", commit: \"bad\", }, }\n",
        )
        .unwrap_err();
        assert_eq!(
            malformed_commit.rule(),
            PackageDeclarationRule::InvalidGitCommit
        );

        let malformed_digest =
            decode("#dependencies: { archive_dep: { archive: \"u\", sha256: \"bad\", }, }\n")
                .unwrap_err();
        assert_eq!(
            malformed_digest.rule(),
            PackageDeclarationRule::InvalidArchiveDigest
        );

        let selected_path = decode(
            "#dependencies: { local: { path: \"../local\", sha256: \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\", }, }\n",
        )
        .unwrap_err();
        assert_eq!(
            selected_path.rule(),
            PackageDeclarationRule::UnexpectedDependencySelection
        );

        let legacy_lock = decode(
            "#package: { name: \"app\", version: \"0.0.0\", }\n#lock: { format: 1, dependencies: {}, }\n",
        )
        .unwrap_err();
        assert_eq!(legacy_lock.rule(), PackageDeclarationRule::InvalidDirective);
    }

    #[test]
    fn rejects_reserved_std_and_invalid_target_modules() {
        let reserved = decode("#dependencies: { std: { path: \"./std\", }, }\n").unwrap_err();
        assert_eq!(
            reserved.rule(),
            PackageDeclarationRule::ReservedStandardDependency
        );

        let module = decode("#executable: { name: \"app\", module: \"../app\", }\n").unwrap_err();
        assert_eq!(module.rule(), PackageDeclarationRule::InvalidModulePath);
    }

    #[test]
    fn scalar_fields_never_search_inside_nested_records() {
        let name =
            decode("#package: { name: { nested: \"app\", }, version: \"0.1.0\", }\n").unwrap_err();
        assert_eq!(name.rule(), PackageDeclarationRule::ExpectedString);

        let target = decode(
            "#package: { name: \"app\", version: \"0.1.0\", }\n#executable: { name: { nested: \"app\", }, }\n",
        )
        .unwrap_err();
        assert_eq!(target.rule(), PackageDeclarationRule::ExpectedString);
    }
}
