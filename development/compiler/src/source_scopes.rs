//! Semantic package and directory-module ownership for loaded physical sources.

use crate::package::{PackageGraph, PackageId};
use crate::resolve::ImportAccess;
use crate::source::{SourceId, SourceMap};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceScope {
    package: PackageId,
    module: Vec<String>,
    standard_library: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SourceScopeMap {
    scopes: HashMap<SourceId, SourceScope>,
}

impl SourceScopeMap {
    pub(crate) fn new(
        sources: &SourceMap,
        source_ids: impl IntoIterator<Item = SourceId>,
        graph: Option<&PackageGraph>,
        standard_library_root: Option<&Path>,
    ) -> Self {
        let standard_library_root = standard_library_root.map(canonicalize);
        let scopes = source_ids
            .into_iter()
            .filter_map(|source| {
                let path = sources.get(source)?.absolute_path()?;
                let (package, root, standard_library) =
                    package_scope(path, graph, standard_library_root.as_deref())?;
                let relative = path.strip_prefix(&root).ok()?;
                let module = crate::source_layout::logical_module_path(relative)?
                    .components()
                    .map(|component| component.as_os_str().to_str().map(str::to_string))
                    .collect::<Option<Vec<_>>>()?;
                Some((
                    source,
                    SourceScope {
                        package,
                        module,
                        standard_library,
                    },
                ))
            })
            .collect();
        Self { scopes }
    }

    pub(crate) fn access(
        &self,
        declaration_source: SourceId,
        use_source: SourceId,
    ) -> ImportAccess {
        let Some(declaration) = self.scopes.get(&declaration_source) else {
            return ImportAccess::Public;
        };
        let Some(use_scope) = self.scopes.get(&use_source) else {
            return ImportAccess::Public;
        };
        if declaration.package != use_scope.package {
            return ImportAccess::Public;
        }
        let common = declaration
            .module
            .iter()
            .zip(&use_scope.module)
            .take_while(|(left, right)| left == right)
            .count();
        let required = declaration.module.len().saturating_sub(common);
        let Ok(required_parent_levels) = u16::try_from(required) else {
            return ImportAccess::Public;
        };
        ImportAccess::Package {
            required_parent_levels,
        }
    }

    pub(crate) fn is_standard_library(&self, source: SourceId) -> bool {
        self.scopes
            .get(&source)
            .is_some_and(|scope| scope.standard_library)
    }

    pub(crate) fn standard_library_module_path(&self, source: SourceId) -> Option<String> {
        let scope = self.scopes.get(&source)?;
        scope.standard_library.then(|| {
            if scope.module.is_empty() {
                "std".to_string()
            } else {
                format!("std/{}", scope.module.join("/"))
            }
        })
    }

    pub(crate) fn reexport_does_not_widen(
        &self,
        target_visibility: crate::ast::Visibility,
        target_source: SourceId,
        reexport_visibility: crate::ast::Visibility,
        reexport_source: SourceId,
    ) -> bool {
        use crate::ast::Visibility;
        if target_visibility == Visibility::Public {
            return true;
        }
        if reexport_visibility == Visibility::Public || target_visibility == Visibility::Private {
            return false;
        }
        let Some((target_package, target_boundary)) =
            self.boundary(target_visibility, target_source)
        else {
            return false;
        };
        let Some((reexport_package, reexport_boundary)) =
            self.boundary(reexport_visibility, reexport_source)
        else {
            return false;
        };
        target_package == reexport_package && reexport_boundary.starts_with(&target_boundary)
    }

    fn boundary(
        &self,
        visibility: crate::ast::Visibility,
        source: SourceId,
    ) -> Option<(PackageId, Vec<String>)> {
        use crate::ast::Visibility;
        let scope = self.scopes.get(&source)?;
        let boundary = match visibility {
            Visibility::Package => Vec::new(),
            Visibility::ModuleTree(parents) => {
                let retained = scope.module.len().checked_sub(usize::from(parents))?;
                scope.module[..retained].to_vec()
            }
            Visibility::Private | Visibility::Public => return None,
        };
        Some((scope.package.clone(), boundary))
    }
}

fn package_scope(
    path: &Path,
    graph: Option<&PackageGraph>,
    standard_library_root: Option<&Path>,
) -> Option<(PackageId, PathBuf, bool)> {
    if let Some(root) = standard_library_root.filter(|root| path.starts_with(root)) {
        let root = canonicalize(root);
        let package = graph
            .and_then(PackageGraph::standard_library)
            .filter(|package| path.starts_with(package.root()))
            .map(|package| package.id().clone())
            .unwrap_or_else(|| PackageId::standard_library(&root, None));
        return Some((package, root, true));
    }
    if let Some((graph, package)) = graph.and_then(|graph| {
        graph
            .package_containing(path)
            .map(|package| (graph, package))
    }) {
        return Some((
            package.id().clone(),
            canonicalize(package.root()),
            graph.is_standard_library_package(package.id()),
        ));
    }
    let root = path.parent().and_then(|parent| {
        parent
            .ancestors()
            .find(|directory| directory.join("nocter.nct").is_file())
            .map(canonicalize)
    })?;
    Some((PackageId::root(&root), root, false))
}

fn canonicalize(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
