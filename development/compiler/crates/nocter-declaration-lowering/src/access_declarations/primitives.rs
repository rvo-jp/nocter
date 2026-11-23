//! Toolchain-selected module/name authority, followed by exact closed signature validation.
use super::{CodeSource, Failure, headers::Header};
use nocter_compile_input::{ModuleIdentity, ToolchainInput};
use nocter_declarations::value_access::{CodeKind, Program};
use nocter_diagnostics::{DiagnosticCode, SourceDiagnostic};
use nocter_model::AccessCodeId;
use nocter_runtime_contract::PrimitiveRole;
use nocter_source_index::SourceOrigin;
use nocter_syntax::{BoundSyntax, SyntaxOrigin, SyntaxToken};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrimitiveAuthorityError {
    RepeatedRole(PrimitiveRole),
    RepeatedName(ModuleIdentity, Box<str>),
}
/// Explicit host authority for one selected module, never derived from authored source paths.
#[derive(Clone, Debug)]
pub struct PrimitiveAuthority {
    module: ModuleIdentity,
    roles: BTreeMap<Box<str>, PrimitiveRole>,
}
impl PrimitiveAuthority {
    /// The logical module explicitly selected by the host for this construction authority.
    #[must_use]
    pub const fn module(&self) -> &ModuleIdentity {
        &self.module
    }
    /// # Errors
    /// Rejects a conflicting toolchain catalog, including conflicts outside the selected module.
    pub fn for_module(
        toolchain: &ToolchainInput,
        module: &ModuleIdentity,
    ) -> Result<Self, PrimitiveAuthorityError> {
        let mut roles = BTreeMap::new();
        validate_catalog(toolchain)?;
        for locator in toolchain.primitive_roles() {
            if module.package() == toolchain.standard_package() && locator.module() == module {
                roles.insert(locator.name().into(), locator.role());
            }
        }
        Ok(Self {
            module: module.clone(),
            roles,
        })
    }
}
pub(super) fn validate_catalog(toolchain: &ToolchainInput) -> Result<(), PrimitiveAuthorityError> {
    let mut seen_roles = BTreeSet::new();
    let mut seen_names = BTreeSet::new();
    for locator in toolchain.primitive_roles() {
        if !seen_roles.insert(locator.role()) {
            return Err(PrimitiveAuthorityError::RepeatedRole(locator.role()));
        }
        let key = (locator.module().clone(), Box::<str>::from(locator.name()));
        if !seen_names.insert(key.clone()) {
            return Err(PrimitiveAuthorityError::RepeatedName(key.0, key.1));
        }
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrimitiveRule {
    Unauthorized,
    MissingMarker,
    UnsupportedRole,
    InputCount,
    Input(usize),
    Result,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrimitiveDiagnostic {
    rule: PrimitiveRule,
    source: SourceDiagnostic,
}
impl PrimitiveDiagnostic {
    #[must_use]
    pub const fn rule(&self) -> PrimitiveRule {
        self.rule
    }
    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Binding {
    Selected(PrimitiveRole),
    Checked(PrimitiveRole),
}
/// Immutable snapshots reveal only bindings whose exact declaration contract has been checked.
#[derive(Clone, Debug, Default)]
pub struct PrimitiveBindings {
    bindings: Arc<BTreeMap<AccessCodeId, Binding>>,
}
impl PrimitiveBindings {
    #[must_use]
    pub fn role(&self, code: AccessCodeId) -> Option<PrimitiveRole> {
        match self.bindings.get(&code)? {
            Binding::Checked(role) => Some(*role),
            Binding::Selected(_) => None,
        }
    }
    pub(super) fn select(
        &mut self,
        syntax: BoundSyntax<'_>,
        authority: Option<&PrimitiveAuthority>,
        id: AccessCodeId,
        header: &Header,
    ) -> Result<(), Failure> {
        self.select_role(
            syntax,
            selected_role(syntax, authority, header.name),
            id,
            header,
        )
    }
    pub(super) fn select_role(
        &mut self,
        syntax: BoundSyntax<'_>,
        role: Option<PrimitiveRole>,
        id: AccessCodeId,
        header: &Header,
    ) -> Result<(), Failure> {
        let origin = SyntaxOrigin::Token(header.name);
        if header.kind != CodeKind::Primitive {
            return if role.is_some() {
                Err(missing_marker(syntax, header.name))
            } else {
                Ok(())
            };
        }
        let Some(role) = role else {
            return Err(reject(
                syntax,
                PrimitiveRule::Unauthorized,
                origin,
                "this primitive declaration is not selected by the toolchain for this module",
            ));
        };
        if role.value_signature().is_none() {
            return Err(reject(
                syntax,
                PrimitiveRule::UnsupportedRole,
                origin,
                "this primitive role has no qualified value-model signature",
            ));
        }
        Arc::make_mut(&mut self.bindings).insert(id, Binding::Selected(role));
        Ok(())
    }
    pub(super) fn needs_validation(&self, code: AccessCodeId) -> bool {
        matches!(self.bindings.get(&code), Some(Binding::Selected(_)))
    }
    pub(super) fn all_checked(&self) -> bool {
        self.bindings
            .values()
            .all(|binding| matches!(binding, Binding::Checked(_)))
    }
    pub(super) fn has_same_selection(&self, retained: &Self) -> bool {
        self.bindings.len() == retained.bindings.len()
            && self.bindings.iter().all(|(code, binding)| {
                let (Binding::Selected(role) | Binding::Checked(role)) = binding;
                retained.role(*code) == Some(*role)
            })
    }
    pub(super) fn validate(
        &mut self,
        syntax: BoundSyntax<'_>,
        program: &Program,
        id: AccessCodeId,
        source: &CodeSource,
    ) -> Result<(), Failure> {
        let Some(Binding::Selected(role)) = self.bindings.get(&id).copied() else {
            return Ok(());
        };
        let signature = role.value_signature().ok_or(Failure::InconsistentSyntax)?;
        let code = program.codes().get(id).ok_or(Failure::InconsistentSyntax)?;
        if code.parameters().len() != signature.inputs().len() {
            let message = format!(
                "primitive requires exactly {} inputs",
                signature.inputs().len()
            );
            return Err(reject(
                syntax,
                PrimitiveRule::InputCount,
                source.origin(),
                &message,
            ));
        }
        for (index, (&actual, expected)) in
            code.parameters().iter().zip(signature.inputs()).enumerate()
        {
            let actual = program
                .parameters()
                .get(actual)
                .ok_or(Failure::InconsistentSyntax)?;
            if program.data_kind(actual.data()) != Some(expected.data())
                || actual.operation() != expected.operation()
            {
                let origin = SyntaxOrigin::Token(
                    *source
                        .parameters()
                        .get(index)
                        .ok_or(Failure::InconsistentSyntax)?,
                );
                let message = format!(
                    "primitive input requires {:?} operation and data type {}",
                    expected.operation(),
                    expected.data().as_str()
                );
                return Err(reject(
                    syntax,
                    PrimitiveRule::Input(index),
                    origin,
                    &message,
                ));
            }
        }
        if !signature
            .result()
            .matches_type(code.result(), |data| program.type_kind(data))
        {
            let message = format!(
                "primitive result must have data type {}",
                signature.result()
            );
            return Err(reject(
                syntax,
                PrimitiveRule::Result,
                SyntaxOrigin::Node(source.result_origin()),
                &message,
            ));
        }
        Arc::make_mut(&mut self.bindings).insert(id, Binding::Checked(role));
        Ok(())
    }
}
fn selected_role(
    syntax: BoundSyntax<'_>,
    authority: Option<&PrimitiveAuthority>,
    name: SyntaxToken,
) -> Option<PrimitiveRole> {
    authority
        .and_then(|authority| authority.roles.get(super::text(syntax, name)))
        .copied()
}
pub(super) fn validate_record(
    syntax: BoundSyntax<'_>,
    authority: Option<&PrimitiveAuthority>,
    name: SyntaxToken,
) -> Result<(), Failure> {
    if selected_role(syntax, authority, name).is_some() {
        return Err(missing_marker(syntax, name));
    }
    Ok(())
}
fn missing_marker(syntax: BoundSyntax<'_>, name: SyntaxToken) -> Failure {
    reject(
        syntax,
        PrimitiveRule::MissingMarker,
        SyntaxOrigin::Token(name),
        "the selected primitive must be declared bodyless with primitive func",
    )
}
fn reject(
    syntax: BoundSyntax<'_>,
    rule: PrimitiveRule,
    origin: SyntaxOrigin,
    message: &str,
) -> Failure {
    let origin = match origin {
        SyntaxOrigin::Node(node) => SourceOrigin::from_node(syntax.tree(), node).ok(),
        SyntaxOrigin::Token(token) => SourceOrigin::from_token(syntax.tree(), token).ok(),
    };
    let Some(origin) = origin else {
        return Failure::InconsistentSyntax;
    };
    let code = match rule {
        PrimitiveRule::Unauthorized
        | PrimitiveRule::MissingMarker
        | PrimitiveRule::UnsupportedRole => DiagnosticCode::E0236,
        _ => DiagnosticCode::E0291,
    };
    Failure::Primitive(Box::new(PrimitiveDiagnostic {
        rule,
        source: SourceDiagnostic::new(code, message, origin, Vec::new(), None::<&str>),
    }))
}
