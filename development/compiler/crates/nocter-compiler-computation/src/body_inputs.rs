use std::collections::BTreeSet;
use std::path::Path;

use nocter_computation::{ComputationError, Database};
use nocter_discovery::DiscoveredUnit;

use crate::semantic::BodySourcePublication;

/// Collects exact body inputs from source-syntax products already demanded by module surfaces.
pub(crate) fn collect(
    database: &Database,
    unit: &DiscoveredUnit,
) -> Result<Vec<BodySourcePublication>, ComputationError> {
    let mut sources = BTreeSet::new();
    for source in unit
        .modules()
        .iter()
        .flat_map(|module| module.sources().iter())
    {
        sources.insert(source.canonical_path());
    }
    let mut publications = Vec::new();
    for path in sources {
        let surface = crate::source_syntax::declaration_surface(database, Path::new(path))?;
        publications.extend(
            surface
                .body_surfaces()
                .iter()
                .map(|body| BodySourcePublication::new(path, body)),
        );
    }
    Ok(publications)
}
