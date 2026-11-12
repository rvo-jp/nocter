//! Authored root audiences are independent of direct physical lookup and implementation locations.
mod signatures;
use super::Failure;
use crate::{DeclarationSurface, SurfaceDeclarationId};
use nocter_compile_input::{ModuleIdentity, ModuleSourceKind};
use nocter_diagnostics::{DiagnosticCode, SourceDiagnostic};
use nocter_model::PackageIdentity;
use nocter_source_index::SourceOrigin;
use nocter_syntax::{BoundSyntax, NodeId, NodeKind, direct_node};
use std::collections::BTreeMap;

/// A normalized outward audience, not permission to see a private physical source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModuleAudience {
    Public,
    Package(PackageIdentity),
    Descendants(ModuleIdentity),
}
impl ModuleAudience {
    #[must_use]
    pub fn contains(&self, caller: &ModuleIdentity) -> bool {
        match self {
            Self::Public => true,
            Self::Package(package) => caller.package() == package,
            Self::Descendants(boundary) => {
                caller.package() == boundary.package() && caller.path().starts_with(boundary.path())
            }
        }
    }
    #[must_use]
    pub fn is_within(&self, target: &Self) -> bool {
        match (self, target) {
            (_, Self::Public) => true,
            (Self::Public, _) => false,
            (Self::Package(a), Self::Package(b)) => a == b,
            (Self::Descendants(a), Self::Package(b)) => a.package() == b,
            (Self::Package(a), Self::Descendants(b)) => b.path().is_empty() && a == b.package(),
            (Self::Descendants(a), Self::Descendants(b)) => {
                a.package() == b.package() && a.path().starts_with(b.path())
            }
        }
    }
    fn intersection(self, owner: &Self) -> Option<Self> {
        if self.is_within(owner) {
            Some(self)
        } else if owner.is_within(&self) {
            Some(owner.clone())
        } else {
            None
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisibilityRule {
    InvalidPath,
    AbovePackageRoot,
    SignatureExposure,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisibilityDiagnostic {
    pub(super) rule: VisibilityRule,
    pub(super) source: SourceDiagnostic,
}
impl VisibilityDiagnostic {
    #[must_use]
    pub const fn rule(&self) -> VisibilityRule {
        self.rule
    }
    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}
#[derive(Debug)]
pub(super) struct ModuleAudiences {
    declarations: Box<[Option<ModuleAudience>]>,
    imports: BTreeMap<NodeId, ModuleAudience>,
}
impl ModuleAudiences {
    pub fn prepare(surface: &DeclarationSurface<'_>) -> Result<Self, Failure> {
        let mut declarations: Vec<Option<ModuleAudience>> =
            vec![None; surface.declarations().len()];
        for (index, declaration) in surface.declarations().iter().enumerate() {
            let source = &surface.sources()[declaration.source().index()];
            if source.kind() != ModuleSourceKind::Root {
                continue;
            }
            let syntax = BoundSyntax::new(
                surface
                    .source_map()
                    .get(source.syntax().source())
                    .ok_or(Failure::InconsistentSyntax)?,
                source.syntax(),
            )
            .ok_or(Failure::InconsistentSyntax)?;
            let audience = resolve(
                surface.modules(),
                source.module(),
                syntax,
                declaration.visibility(),
            )?;
            let audience = if let Some(owner) = declaration.owner() {
                audience
                    .intersection(
                        declarations[owner.index()]
                            .as_ref()
                            .ok_or(Failure::InconsistentSyntax)?,
                    )
                    .ok_or(Failure::InconsistentSyntax)?
            } else {
                audience
            };
            declarations[index] = Some(audience);
        }
        let mut imports = BTreeMap::new();
        for import in surface.imports() {
            let source = &surface.sources()[import.source().index()];
            if source.kind() != ModuleSourceKind::Root {
                continue;
            }
            let syntax = BoundSyntax::new(
                surface
                    .source_map()
                    .get(source.syntax().source())
                    .ok_or(Failure::InconsistentSyntax)?,
                source.syntax(),
            )
            .ok_or(Failure::InconsistentSyntax)?;
            let audience = resolve(
                surface.modules(),
                source.module(),
                syntax,
                direct_node(syntax.tree(), import.node(), NodeKind::Visibility),
            )?;
            imports.insert(import.node(), audience);
        }
        Ok(Self {
            declarations: declarations.into(),
            imports,
        })
    }
    pub fn declaration(&self, raw: SurfaceDeclarationId) -> Option<&ModuleAudience> {
        self.declarations.get(raw.index())?.as_ref()
    }
    pub fn import(&self, node: NodeId) -> Option<&ModuleAudience> {
        self.imports.get(&node)
    }
}
fn resolve(
    modules: &[ModuleIdentity],
    declaring: &ModuleIdentity,
    syntax: BoundSyntax<'_>,
    directive: Option<NodeId>,
) -> Result<ModuleAudience, Failure> {
    let Some(directive) = directive else {
        return Ok(ModuleAudience::Public);
    };
    let literal = direct_node(syntax.tree(), directive, NodeKind::StringLiteral)
        .ok_or(Failure::InconsistentSyntax)?;
    let value =
        nocter_syntax::decode_string_literal(syntax, literal).ok_or(Failure::InconsistentSyntax)?;
    let fail = |rule| reject(syntax, literal, rule);
    let value: &str = &value;
    if value == "/" {
        return Ok(ModuleAudience::Package(declaring.package().clone()));
    }
    let boundary = if value == "./" {
        declaring
    } else {
        let mut rest = value;
        let mut steps = 0;
        while let Some(next) = rest.strip_prefix("../") {
            steps += 1;
            rest = next;
        }
        if steps == 0 || !rest.is_empty() {
            return Err(fail(VisibilityRule::InvalidPath));
        }
        crate::visibility::ancestor_module(modules, declaring, steps).map_err(|error| {
            fail(match error {
                crate::visibility::AncestorBoundaryError::AbovePackageRoot => {
                    VisibilityRule::AbovePackageRoot
                }
                crate::visibility::AncestorBoundaryError::NotModule => VisibilityRule::InvalidPath,
            })
        })?
    };
    if boundary.path().is_empty() {
        Ok(ModuleAudience::Package(boundary.package().clone()))
    } else {
        Ok(ModuleAudience::Descendants(boundary.clone()))
    }
}
fn reject(syntax: BoundSyntax<'_>, node: NodeId, rule: VisibilityRule) -> Failure {
    let Ok(primary) = SourceOrigin::from_node(syntax.tree(), node) else {
        return Failure::InconsistentSyntax;
    };
    let message = match rule {
        VisibilityRule::InvalidPath => "visibility requires /, ./ or complete ../ components",
        VisibilityRule::AbovePackageRoot => {
            "visibility cannot select an ancestor above the package root"
        }
        VisibilityRule::SignatureExposure => {
            unreachable!("signature exposure requires declaration evidence")
        }
    };
    Failure::Visibility(Box::new(VisibilityDiagnostic {
        rule,
        source: SourceDiagnostic::new(DiagnosticCode::E0239, message, primary, [], None::<&str>),
    }))
}

#[cfg(test)]
mod tests;
