use std::error::Error;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use nocter_computation::{
    ComputationError, ComputationKey, Database, Fingerprint, Query, QueryValue,
};
use nocter_filesystem::SourceOverlay;
use nocter_source::{SourceFile, SourceMap, SourceName};
use nocter_syntax::{
    ParseGoal, ParsedSyntax, SourceSyntaxError, SourceSyntaxProvider, parse_reusable,
};

/// Stable identity of one canonical source path across workspace revisions.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
struct SourcePath(Box<str>);

impl SourcePath {
    fn new(path: &Path) -> Self {
        Self(path.to_string_lossy().into_owned().into_boxed_str())
    }
}

#[derive(Clone)]
struct SourceSyntaxKey {
    path: SourcePath,
    goal: ParseGoal,
    text: Arc<str>,
    text_fingerprint: Fingerprint,
}

impl SourceSyntaxKey {
    fn new(source: &SourceFile, goal: ParseGoal) -> Self {
        Self {
            path: SourcePath::new(Path::new(source.name().as_str())),
            goal,
            text: Arc::from(source.text()),
            text_fingerprint: Fingerprint::from_bytes(source.text().as_bytes()),
        }
    }
}

impl ComputationKey for SourceSyntaxKey {
    fn stable_bytes(&self) -> Box<[u8]> {
        let path = self.path.0.as_bytes();
        let goal = self.goal.as_str().as_bytes();
        let mut identity = Vec::with_capacity(path.len() + goal.len() + 34);
        identity.extend_from_slice(path);
        identity.push(0);
        identity.extend_from_slice(goal);
        identity.push(0);
        identity.extend_from_slice(&self.text_fingerprint.digest());
        identity.into_boxed_slice()
    }
}

struct SourceSyntaxQuery;

struct SourceSyntaxProduct {
    result: Result<Arc<ParsedSyntax>, Arc<str>>,
    fingerprint: Fingerprint,
}

impl SourceSyntaxProduct {
    fn parse(key: &SourceSyntaxKey) -> Self {
        let mut sources = SourceMap::new();
        let source_id =
            match sources.add_bytes(SourceName::new(key.path.0.as_ref()), key.text.as_bytes()) {
                Ok(source_id) => source_id,
                Err(error) => {
                    let message: Arc<str> = error.to_string().into();
                    return Self {
                        fingerprint: tagged_fingerprint(1, message.as_bytes()),
                        result: Err(message),
                    };
                }
            };
        let source = sources
            .get(source_id)
            .expect("a newly allocated source identity resolves in its owner");
        let syntax = Arc::new(parse_reusable(source, key.goal));
        let mut semantic_input =
            Vec::with_capacity(source.text().len() + key.goal.as_str().len() + 1);
        semantic_input.extend_from_slice(key.goal.as_str().as_bytes());
        semantic_input.push(0);
        semantic_input.extend_from_slice(source.text().as_bytes());
        Self {
            fingerprint: tagged_fingerprint(0, &semantic_input),
            result: Ok(syntax),
        }
    }
}

impl QueryValue for SourceSyntaxProduct {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl Query for SourceSyntaxQuery {
    type Key = SourceSyntaxKey;
    type Value = SourceSyntaxProduct;

    fn execute(_database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        Ok(SourceSyntaxProduct::parse(key))
    }
}

pub(super) struct SourceDeclarationSurfaceProduct {
    bodies: Box<[nocter_syntax::BodySyntaxSurface]>,
    fingerprint: Fingerprint,
}

impl SourceDeclarationSurfaceProduct {
    /// Returns the source-neutral declaration identity used by a containing module surface.
    pub(super) const fn semantic_fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    /// Returns exact per-body inputs keyed by stable declaration-surface locators.
    pub(super) fn body_surfaces(&self) -> &[nocter_syntax::BodySyntaxSurface] {
        &self.bodies
    }
}

impl QueryValue for SourceDeclarationSurfaceProduct {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

struct SourceDeclarationSurfaceQuery;

impl Query for SourceDeclarationSurfaceQuery {
    type Key = SourceSyntaxKey;
    type Value = SourceDeclarationSurfaceProduct;

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let syntax = database.query::<SourceSyntaxQuery>(key.clone())?;
        let (bodies, fingerprint) = match &syntax.result {
            Ok(syntax) => {
                let projection = syntax.declaration_projection();
                let bodies = projection.body_surfaces().to_vec().into_boxed_slice();
                let surface = projection.into_surface();
                let fingerprint = tagged_fingerprint(0, surface.canonical_bytes());
                (bodies, fingerprint)
            }
            Err(message) => (
                Vec::new().into_boxed_slice(),
                tagged_fingerprint(1, message.as_bytes()),
            ),
        };
        Ok(SourceDeclarationSurfaceProduct {
            bodies,
            fingerprint,
        })
    }
}

pub(crate) fn declaration_surface(
    database: &Database,
    source: &SourceFile,
    goal: ParseGoal,
) -> Result<Arc<SourceDeclarationSurfaceProduct>, ComputationError> {
    database.query::<SourceDeclarationSurfaceQuery>(SourceSyntaxKey::new(source, goal))
}

fn tagged_fingerprint(tag: u8, bytes: &[u8]) -> Fingerprint {
    let mut input = Vec::with_capacity(bytes.len() + 1);
    input.push(tag);
    input.extend_from_slice(bytes);
    Fingerprint::from_bytes(&input)
}

/// Computes source-token identity without duplicating overlay bytes into the query database.
pub(crate) fn source_view_fingerprint(
    overlay: &SourceOverlay,
    filesystem_epoch: u64,
) -> Fingerprint {
    let mut source_identity = Vec::new();
    source_identity.extend_from_slice(&filesystem_epoch.to_be_bytes());
    for (path, source) in overlay.sources() {
        let path = path.to_string_lossy();
        source_identity.extend_from_slice(&(path.len() as u64).to_be_bytes());
        source_identity.extend_from_slice(path.as_bytes());
        source_identity.extend_from_slice(&Fingerprint::from_bytes(source.bytes()).digest());
    }
    Fingerprint::from_bytes(&source_identity)
}

pub(crate) struct ComputedSourceSyntax<'database> {
    database: &'database Database,
    source_overlay: SourceOverlay,
}

impl<'database> ComputedSourceSyntax<'database> {
    pub(crate) fn new(database: &'database Database, source_overlay: &SourceOverlay) -> Self {
        Self {
            database,
            source_overlay: source_overlay.clone(),
        }
    }

    fn validate_source(&self, source: &SourceFile) -> Result<(), SourceSyntaxError> {
        let requested_path = Path::new(source.name().as_str());
        let observed = self
            .source_overlay
            .observe_file(requested_path)
            .map_err(|error| {
                SourceSyntaxError::new(SourceAdmissionError::Observation {
                    path: requested_path.to_path_buf(),
                    error,
                })
            })?
            .ok_or_else(|| {
                SourceSyntaxError::new(SourceAdmissionError::Missing(requested_path.to_path_buf()))
            })?;
        let observed_name = observed.canonical_path().to_string_lossy();
        if observed_name != source.name().as_str() {
            return Err(SourceSyntaxError::new(SourceAdmissionError::Identity {
                expected: observed.canonical_path().to_path_buf(),
                received: requested_path.to_path_buf(),
            }));
        }
        if !source
            .matches_source_bytes(observed.bytes())
            .map_err(SourceSyntaxError::new)?
        {
            return Err(SourceSyntaxError::new(SourceAdmissionError::Contents(
                requested_path.to_path_buf(),
            )));
        }
        Ok(())
    }
}

impl SourceSyntaxProvider for ComputedSourceSyntax<'_> {
    fn parsed_syntax(
        &mut self,
        source: &SourceFile,
        goal: ParseGoal,
    ) -> Result<Arc<ParsedSyntax>, SourceSyntaxError> {
        self.validate_source(source)?;
        let product = self
            .database
            .query::<SourceSyntaxQuery>(SourceSyntaxKey::new(source, goal))
            .map_err(SourceSyntaxError::new)?;
        let syntax = product
            .result
            .as_ref()
            .map_err(|message| SourceSyntaxError::new(SourceProductError(Arc::clone(message))))?;
        debug_assert!(syntax.matches(source));
        Ok(Arc::clone(syntax))
    }
}

#[derive(Debug)]
enum SourceAdmissionError {
    Observation {
        path: std::path::PathBuf,
        error: std::io::Error,
    },
    Missing(std::path::PathBuf),
    Identity {
        expected: std::path::PathBuf,
        received: std::path::PathBuf,
    },
    Contents(std::path::PathBuf),
}

impl fmt::Display for SourceAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observation { path, error } => {
                write!(
                    formatter,
                    "failed to observe admitted source {}: {error}",
                    path.display()
                )
            }
            Self::Missing(path) => {
                write!(
                    formatter,
                    "source is absent from the admitted overlay: {}",
                    path.display()
                )
            }
            Self::Identity { expected, received } => write!(
                formatter,
                "source identity {} does not match admitted canonical path {}",
                received.display(),
                expected.display(),
            ),
            Self::Contents(path) => write!(
                formatter,
                "source contents do not match the admitted overlay: {}",
                path.display(),
            ),
        }
    }
}

impl Error for SourceAdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Observation { error, .. } => Some(error),
            Self::Missing(_) | Self::Identity { .. } | Self::Contents(_) => None,
        }
    }
}

#[derive(Debug)]
struct SourceProductError(Arc<str>);

impl fmt::Display for SourceProductError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for SourceProductError {}

pub(crate) fn execution_count(database: &Database) -> u64 {
    database.execution_count::<SourceSyntaxQuery>()
}

pub(crate) fn reuse_count(database: &Database) -> u64 {
    database.reuse_count::<SourceSyntaxQuery>()
}

pub(crate) fn declaration_surface_execution_count(database: &Database) -> u64 {
    database.execution_count::<SourceDeclarationSurfaceQuery>()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use nocter_filesystem::SourceOverride;

    use super::*;

    fn source(sources: &mut SourceMap, text: &str) -> nocter_source::SourceId {
        sources
            .add_bytes(SourceName::new("/virtual/source.nct"), text.as_bytes())
            .unwrap()
    }

    #[test]
    fn declaration_queries_preserve_the_selected_grammar_and_reuse_its_parse() {
        let text = "func run(take value: i32): i32 { value }";
        let mut sources = SourceMap::new();
        let id = source(&mut sources, text);
        let file = sources.get(id).unwrap();
        let mut overlay = SourceOverlay::builder();
        overlay
            .insert_source(
                PathBuf::from(file.name().as_str()),
                SourceOverride::new(text.as_bytes()),
            )
            .unwrap();
        let database = Database::new();
        let overlay = overlay.finish();
        let mut provider = ComputedSourceSyntax::new(&database, &overlay);
        let parsed = provider.parsed_syntax(file, ParseGoal::V2CoreUnit).unwrap();
        assert!(!parsed.bind(file).unwrap().has_errors());
        let selected = declaration_surface(&database, file, ParseGoal::V2CoreUnit).unwrap();
        assert_eq!(selected.body_surfaces().len(), 1);
        assert_eq!(execution_count(&database), 1);
        assert_eq!(declaration_surface_execution_count(&database), 1);
        let released = declaration_surface(&database, file, ParseGoal::SourceFile).unwrap();
        assert_ne!(
            selected.semantic_fingerprint(),
            released.semantic_fingerprint()
        );
        assert_eq!(execution_count(&database), 2);
        assert_eq!(declaration_surface_execution_count(&database), 2);
        let again = declaration_surface(&database, file, ParseGoal::V2CoreUnit).unwrap();
        assert!(Arc::ptr_eq(&selected, &again));
        assert_eq!(execution_count(&database), 2);
    }

    #[test]
    fn v2_body_edits_reuse_declarations_without_reusing_exact_bodies() {
        let database = Database::new();
        let mut before = SourceMap::new();
        let old = source(&mut before, "func run(take value: i32): i32 { value + 1 }");
        let mut after = SourceMap::new();
        let new = source(&mut after, "func run(take value: i32): i32 { value + 2 }");
        let before =
            declaration_surface(&database, before.get(old).unwrap(), ParseGoal::V2CoreUnit)
                .unwrap();
        let after =
            declaration_surface(&database, after.get(new).unwrap(), ParseGoal::V2CoreUnit).unwrap();
        assert_eq!(before.semantic_fingerprint(), after.semantic_fingerprint());
        assert_eq!(before.body_surfaces().len(), 1);
        assert_eq!(after.body_surfaces().len(), 1);
        assert_eq!(
            before.body_surfaces()[0].locator(),
            after.body_surfaces()[0].locator()
        );
        assert_ne!(
            before.body_surfaces()[0].canonical_bytes(),
            after.body_surfaces()[0].canonical_bytes()
        );
    }

    #[test]
    fn computed_syntax_accepts_only_the_admitted_source_value() {
        let path = PathBuf::from("/virtual/source.nct");
        let mut overlay = SourceOverlay::builder();
        overlay
            .insert_source(
                path.clone(),
                SourceOverride::new(&b"func admitted(): void {}\n"[..]),
            )
            .unwrap();
        let overlay = overlay.finish();
        let database = Database::new();
        let mut provider = ComputedSourceSyntax::new(&database, &overlay);

        let mut admitted_sources = SourceMap::new();
        let admitted_id = admitted_sources
            .add_bytes(
                SourceName::new(path.to_string_lossy()),
                b"func admitted(): void {}\n",
            )
            .unwrap();
        assert!(
            provider
                .parsed_syntax(
                    admitted_sources.get(admitted_id).unwrap(),
                    ParseGoal::SourceFile,
                )
                .is_ok()
        );

        let mut foreign_sources = SourceMap::new();
        let foreign_id = foreign_sources
            .add_bytes(
                SourceName::new(path.to_string_lossy()),
                b"func foreign(): void {}\n",
            )
            .unwrap();
        assert!(
            provider
                .parsed_syntax(
                    foreign_sources.get(foreign_id).unwrap(),
                    ParseGoal::SourceFile,
                )
                .is_err()
        );
    }
}
