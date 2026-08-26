use std::collections::BTreeMap;
use std::fmt;

use nocter_model::PackageTargetKind;
use nocter_source::SourceFile;
use nocter_syntax::{
    Keyword, NodeId, NodeKind, SyntaxElement, SyntaxTree, TokenKind,
    child_node_iter as child_nodes, decode_string_literal, direct_node,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyDeclaration {
    field: NodeId,
    source: DependencySource,
}

impl DependencyDeclaration {
    #[must_use]
    pub const fn field(&self) -> NodeId {
        self.field
    }

    #[must_use]
    pub const fn source(&self) -> &DependencySource {
        &self.source
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyLock {
    Git(AuthoredString),
    Sha256(AuthoredString),
}

impl DependencyLock {
    /// Removes syntax identity from an already validated authored lock.
    #[must_use]
    pub fn exact(&self) -> crate::ExactDependencyLock {
        match self {
            Self::Git(authored) => crate::ExactDependencyLock::validated(
                crate::ExactDependencyLockKind::Git,
                authored.value().trim_start_matches("git:"),
            ),
            Self::Sha256(authored) => crate::ExactDependencyLock::validated(
                crate::ExactDependencyLockKind::Sha256,
                authored.value().trim_start_matches("sha256:"),
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
    locks: BTreeMap<Box<str>, DependencyLock>,
    lock_directive: Option<NodeId>,
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
    pub const fn locks(&self) -> &BTreeMap<Box<str>, DependencyLock> {
        &self.locks
    }

    #[must_use]
    pub const fn lock_directive(&self) -> Option<NodeId> {
        self.lock_directive
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
    InvalidLockFormat,
    UnknownLockDependency,
    PathDependencyLock,
    LockKindMismatch,
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
    let mut locks = None;
    let mut lock_directive = None;
    let mut targets = Vec::new();
    let mut target_order = 0_u32;

    for declaration in child_nodes(tree, tree.root_id()) {
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
            "lock" => {
                set_once(
                    &mut locks,
                    decode_lock(source, tree, declaration)?,
                    declaration,
                    "lock",
                )?;
                lock_directive = Some(declaration);
            }
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
    let locks = locks.unwrap_or_default();
    validate_locks(&dependencies, &locks)?;
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
        locks,
        lock_directive,
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
        let dependency_record = required_record(tree, field)?;
        let fields = unique_fields(source, tree, dependency_record)?;
        let git = optional_string(source, tree, &fields, "git")?;
        let revision = optional_string(source, tree, &fields, "revision")?;
        let archive = optional_string(source, tree, &fields, "archive")?;
        let path = optional_string(source, tree, &fields, "path")?;
        for name in fields.keys() {
            if !matches!(name.as_ref(), "git" | "revision" | "archive" | "path") {
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
                    Some(alias),
                ));
            }
        };
        if result
            .insert(
                alias.clone(),
                DependencyDeclaration {
                    field,
                    source: dependency_source,
                },
            )
            .is_some()
        {
            return Err(error(
                field,
                PackageDeclarationRule::DuplicateField,
                Some(alias),
            ));
        }
    }
    Ok(result)
}

fn decode_lock(
    source: &SourceFile,
    tree: &SyntaxTree,
    declaration: NodeId,
) -> Result<BTreeMap<Box<str>, DependencyLock>, PackageDeclarationError> {
    let record = required_record(tree, declaration)?;
    let fields = unique_fields(source, tree, record)?;
    for name in fields.keys() {
        if !matches!(name.as_ref(), "format" | "dependencies") {
            return Err(error(
                fields[name],
                PackageDeclarationRule::UnknownField,
                Some(name.clone()),
            ));
        }
    }
    let format_field = required_field(&fields, declaration, "format")?;
    let format = integer_value(source, tree, format_field)?;
    if format != "1" {
        return Err(error(
            format_field,
            PackageDeclarationRule::InvalidLockFormat,
            Some(format.into()),
        ));
    }
    let dependencies_field = required_field(&fields, declaration, "dependencies")?;
    let dependencies_record = required_record(tree, dependencies_field)?;
    let mut locks = BTreeMap::new();
    for field in direct_fields(tree, dependencies_record) {
        let alias = field_name(source, tree, field)?;
        if alias.as_ref() == "std" {
            return Err(error(
                field,
                PackageDeclarationRule::ReservedStandardDependency,
                Some(alias),
            ));
        }
        let authored = authored_string(source, tree, field)?;
        let lock = if valid_prefixed_hex(authored.value(), "git:", 40) {
            DependencyLock::Git(authored)
        } else if valid_prefixed_hex(authored.value(), "sha256:", 64) {
            DependencyLock::Sha256(authored)
        } else {
            return Err(error(
                field,
                PackageDeclarationRule::InvalidLockFormat,
                Some(alias),
            ));
        };
        if locks.insert(alias.clone(), lock).is_some() {
            return Err(error(
                field,
                PackageDeclarationRule::DuplicateField,
                Some(alias),
            ));
        }
    }
    Ok(locks)
}

fn validate_locks(
    dependencies: &BTreeMap<Box<str>, DependencyDeclaration>,
    locks: &BTreeMap<Box<str>, DependencyLock>,
) -> Result<(), PackageDeclarationError> {
    for (alias, lock) in locks {
        let Some(dependency) = dependencies.get(alias) else {
            return Err(error(
                lock_literal(lock),
                PackageDeclarationRule::UnknownLockDependency,
                Some(alias.clone()),
            ));
        };
        let valid = match (dependency.source(), lock) {
            (DependencySource::Git { .. }, DependencyLock::Git(_))
            | (DependencySource::Archive { .. }, DependencyLock::Sha256(_)) => true,
            (DependencySource::Path { .. }, _) => {
                return Err(error(
                    lock_literal(lock),
                    PackageDeclarationRule::PathDependencyLock,
                    Some(alias.clone()),
                ));
            }
            _ => false,
        };
        if !valid {
            return Err(error(
                lock_literal(lock),
                PackageDeclarationRule::LockKindMismatch,
                Some(alias.clone()),
            ));
        }
    }
    Ok(())
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

fn integer_value<'source>(
    source: &'source SourceFile,
    tree: &SyntaxTree,
    subject: NodeId,
) -> Result<&'source str, PackageDeclarationError> {
    value_node(tree, subject)
        .into_iter()
        .flat_map(|value| tree.children(value))
        .find_map(|element| match element {
            SyntaxElement::Token(token) if token.kind() == TokenKind::IntegerLiteral => {
                source.text_at(token.range())
            }
            SyntaxElement::Node(_) | SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
        })
        .ok_or_else(|| error(subject, PackageDeclarationRule::InvalidLockFormat, None))
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

fn valid_prefixed_hex(value: &str, prefix: &str, digits: usize) -> bool {
    value.strip_prefix(prefix).is_some_and(|value| {
        value.len() == digits && value.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn lock_literal(lock: &DependencyLock) -> NodeId {
    match lock {
        DependencyLock::Git(value) | DependencyLock::Sha256(value) => value.literal(),
    }
}

fn direct_fields(tree: &SyntaxTree, record: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    child_nodes(tree, record).filter(|child| {
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
               git_dep: { git: \"https://example.test/a.git\", revision: \"main\", },\n\
               archive_dep: { archive: \"https://example.test/a.tar.gz\", },\n\
               local_dep: { path: \"./local\", },\n\
             }\n\
             #lock: {\n\
               format: 1,\n\
               dependencies: {\n\
                 archive_dep: \"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\n\
                 git_dep: \"git:0123456789abcdef0123456789abcdef01234567\",\n\
               },\n\
             }\n\
             #executable: { name: \"app\", }\n\
             #test: { name: \"unit\", module: \"./tests/unit\", }\n",
        )
        .unwrap();

        assert_eq!(declaration.name().value(), "app");
        assert_eq!(declaration.version().value(), "1.2.3");
        assert_eq!(declaration.dependencies().len(), 3);
        assert_eq!(declaration.locks().len(), 2);
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
    fn rejects_inconsistent_source_and_lock_shapes() {
        let mixed =
            decode("#dependencies: { bad: { git: \"u\", revision: \"r\", path: \"p\", }, }\n")
                .unwrap_err();
        assert_eq!(
            mixed.rule(),
            PackageDeclarationRule::InvalidDependencySource
        );

        let wrong_lock = decode(
            "#dependencies: { archive_dep: { archive: \"u\", }, }\n\
             #lock: { format: 1, dependencies: { archive_dep: \"git:0123456789abcdef0123456789abcdef01234567\", }, }\n",
        )
        .unwrap_err();
        assert_eq!(wrong_lock.rule(), PackageDeclarationRule::LockKindMismatch);
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
