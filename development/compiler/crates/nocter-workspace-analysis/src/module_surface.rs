use std::path::Path;

use nocter_compile_input::{ModuleIdentity, ModuleSourceKind};
use nocter_computation::{
    ComputationError, ComputationKey, Database, Fingerprint, Query, QueryValue,
};
use nocter_discovery::{DiscoveredModule, DiscoveredUnit};
use nocter_model::CompilationTarget;

use crate::source_syntax;

#[derive(Clone)]
struct ModuleSurfaceKey {
    target: CompilationTarget,
    module: ModuleIdentity,
    sources: Box<[ModuleSourceKey]>,
}

impl ModuleSurfaceKey {
    fn new(target: CompilationTarget, module: &DiscoveredModule) -> Self {
        let mut sources = module
            .sources()
            .iter()
            .map(|source| ModuleSourceKey {
                path: source.canonical_path().into(),
                kind: source.kind(),
            })
            .collect::<Vec<_>>();
        sources.sort_unstable_by(|left, right| {
            source_kind_code(left.kind)
                .cmp(&source_kind_code(right.kind))
                .then_with(|| left.path.cmp(&right.path))
        });
        Self {
            target,
            module: module.identity().clone(),
            sources: sources.into_boxed_slice(),
        }
    }
}

impl ComputationKey for ModuleSurfaceKey {
    fn stable_bytes(&self) -> Box<[u8]> {
        let mut identity = Vec::new();
        encode(self.target.name().as_bytes(), &mut identity);
        encode(self.module.package().as_str().as_bytes(), &mut identity);
        for segment in self.module.path() {
            encode(segment.as_bytes(), &mut identity);
        }
        identity.push(0xff);
        for source in &self.sources {
            identity.push(source_kind_code(source.kind));
            encode(source.path.as_bytes(), &mut identity);
        }
        identity.into_boxed_slice()
    }
}

#[derive(Clone)]
struct ModuleSourceKey {
    path: Box<str>,
    kind: ModuleSourceKind,
}

struct ModuleSurfaceQuery;

struct ModuleSurfaceProduct {
    fingerprint: Fingerprint,
}

impl ModuleSurfaceProduct {
    const fn semantic_fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl QueryValue for ModuleSurfaceProduct {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl Query for ModuleSurfaceQuery {
    type Key = ModuleSurfaceKey;
    type Value = ModuleSurfaceProduct;

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let mut semantic = key.stable_bytes().into_vec();
        for source in &key.sources {
            let surface = source_syntax::declaration_surface(database, Path::new(&*source.path))?;
            let (tag, bytes) = surface.semantic_bytes();
            semantic.push(tag);
            encode(bytes, &mut semantic);
        }
        Ok(ModuleSurfaceProduct {
            fingerprint: Fingerprint::from_bytes(&semantic),
        })
    }
}

/// Composes every discovered module's source-neutral declaration surface for this scope.
pub(super) fn fingerprint(
    database: &Database,
    unit: &DiscoveredUnit,
) -> Result<Fingerprint, ComputationError> {
    let mut modules = unit.modules().iter().collect::<Vec<_>>();
    modules.sort_unstable_by_key(|module| module.identity());
    let mut semantic = Vec::new();
    for module in modules {
        let surface =
            database.query::<ModuleSurfaceQuery>(ModuleSurfaceKey::new(unit.target(), module))?;
        semantic.extend_from_slice(&surface.semantic_fingerprint().digest());
    }
    Ok(Fingerprint::from_bytes(&semantic))
}

const fn source_kind_code(kind: ModuleSourceKind) -> u8 {
    match kind {
        ModuleSourceKind::Root => 0,
        ModuleSourceKind::SingleFile => 1,
        ModuleSourceKind::Implementation => 2,
    }
}

fn encode(bytes: &[u8], output: &mut Vec<u8>) {
    output.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    output.extend_from_slice(bytes);
}

#[cfg(test)]
pub(super) fn execution_count(database: &Database) -> u64 {
    database.execution_count::<ModuleSurfaceQuery>()
}

#[cfg(test)]
pub(super) fn reuse_count(database: &Database) -> u64 {
    database.reuse_count::<ModuleSurfaceQuery>()
}
