use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nocter_computation::{
    ComputationError, ComputationKey, ComputationRevision, Database, Fingerprint, Input,
    InputRetention, Query, QueryValue,
};
use nocter_filesystem::SourceOverlay;
use nocter_source::{SourceFile, SourceMap, SourceName};
use nocter_syntax::{
    ParseGoal, ParsedSyntax, SourceSyntaxError, SourceSyntaxProvider, parse_reusable,
};

/// Exact set of paths whose source bytes are owned by the current editor overlay.
struct OverlayDomainInput;

impl Input for OverlayDomainInput {
    type Key = ();
    type Value = OverlayDomain;

    const RETENTION: InputRetention = InputRetention::RevisionDerived;
}

struct OverlayDomain {
    paths: BTreeSet<SourcePath>,
    fingerprint: Fingerprint,
}

impl OverlayDomain {
    fn new(paths: BTreeSet<SourcePath>) -> Self {
        let mut identity = Vec::new();
        for path in &paths {
            let bytes = path.0.as_bytes();
            identity.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
            identity.extend_from_slice(bytes);
        }
        Self {
            paths,
            fingerprint: Fingerprint::from_bytes(&identity),
        }
    }
}

impl QueryValue for OverlayDomain {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

struct OverlayTextInput;

impl Input for OverlayTextInput {
    type Key = SourcePath;
    type Value = OverlayText;

    const RETENTION: InputRetention = InputRetention::RevisionDerived;
}

struct OverlayText {
    bytes: Arc<[u8]>,
    fingerprint: Fingerprint,
}

impl OverlayText {
    fn new(bytes: &[u8]) -> Self {
        Self {
            bytes: bytes.into(),
            fingerprint: Fingerprint::from_bytes(bytes),
        }
    }
}

impl QueryValue for OverlayText {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

struct FilesystemEpochInput;

impl Input for FilesystemEpochInput {
    type Key = ();
    type Value = FilesystemEpoch;

    const RETENTION: InputRetention = InputRetention::RevisionDerived;
}

struct FilesystemEpoch {
    fingerprint: Fingerprint,
}

impl FilesystemEpoch {
    fn new(epoch: u64) -> Self {
        Self {
            fingerprint: Fingerprint::from_bytes(&epoch.to_be_bytes()),
        }
    }
}

impl QueryValue for FilesystemEpoch {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

/// Stable identity of one canonical source path across workspace revisions.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
struct SourcePath(Box<str>);

impl SourcePath {
    fn new(path: &Path) -> Self {
        Self(path.to_string_lossy().into_owned().into_boxed_str())
    }

    fn as_path(&self) -> &Path {
        Path::new(self.0.as_ref())
    }
}

impl ComputationKey for SourcePath {
    fn stable_bytes(&self) -> Box<[u8]> {
        self.0.as_bytes().into()
    }
}

struct SourceTextQuery;

struct SourceTextProduct {
    result: Result<Arc<[u8]>, Arc<str>>,
    fingerprint: Fingerprint,
}

impl SourceTextProduct {
    fn read(path: &Path) -> Self {
        match std::fs::read(path) {
            Ok(bytes) => Self {
                fingerprint: tagged_fingerprint(0, &bytes),
                result: Ok(bytes.into()),
            },
            Err(error) => {
                let message: Arc<str> = error.to_string().into();
                Self {
                    fingerprint: tagged_fingerprint(1, message.as_bytes()),
                    result: Err(message),
                }
            }
        }
    }
}

impl QueryValue for SourceTextProduct {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl Query for SourceTextQuery {
    type Key = SourcePath;
    type Value = SourceTextProduct;

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let domain = database.input::<OverlayDomainInput>(&())?;
        if domain.paths.contains(key) {
            let text = database.input::<OverlayTextInput>(key)?;
            return Ok(Self::Value {
                result: Ok(Arc::clone(&text.bytes)),
                fingerprint: tagged_fingerprint(0, &text.bytes),
            });
        }
        let _ = database.input::<FilesystemEpochInput>(&())?;
        Ok(SourceTextProduct::read(key.as_path()))
    }
}

#[derive(Clone)]
struct SourceSyntaxKey {
    path: SourcePath,
    goal: ParseGoal,
}

impl SourceSyntaxKey {
    fn new(path: &Path, goal: ParseGoal) -> Self {
        Self {
            path: SourcePath::new(path),
            goal,
        }
    }
}

impl ComputationKey for SourceSyntaxKey {
    fn stable_bytes(&self) -> Box<[u8]> {
        let path = self.path.stable_bytes();
        let goal = self.goal.as_str().as_bytes();
        let mut identity = Vec::with_capacity(path.len() + goal.len() + 1);
        identity.extend_from_slice(&path);
        identity.push(0);
        identity.extend_from_slice(goal);
        identity.into_boxed_slice()
    }
}

struct SourceSyntaxQuery;

struct SourceSyntaxProduct {
    result: Result<Arc<ParsedSyntax>, Arc<str>>,
    fingerprint: Fingerprint,
}

impl SourceSyntaxProduct {
    fn parse(path: &SourcePath, goal: ParseGoal, text: &SourceTextProduct) -> Self {
        let bytes = match &text.result {
            Ok(bytes) => bytes,
            Err(message) => {
                return Self {
                    fingerprint: tagged_fingerprint(1, message.as_bytes()),
                    result: Err(Arc::clone(message)),
                };
            }
        };
        let mut sources = SourceMap::new();
        let source_id = match sources.add_bytes(SourceName::new(path.0.as_ref()), bytes) {
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
        let syntax = Arc::new(parse_reusable(source, goal));
        let mut semantic_input = Vec::with_capacity(source.text().len() + goal.as_str().len() + 1);
        semantic_input.extend_from_slice(goal.as_str().as_bytes());
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

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let text = database.query::<SourceTextQuery>(key.path.clone())?;
        Ok(SourceSyntaxProduct::parse(&key.path, key.goal, &text))
    }
}

pub(super) struct SourceDeclarationSurfaceProduct {
    surface: Result<Arc<nocter_syntax::DeclarationSyntaxSurface>, Arc<str>>,
    bodies: Box<[nocter_syntax::BodySyntaxSurface]>,
    fingerprint: Fingerprint,
}

impl SourceDeclarationSurfaceProduct {
    /// Returns the exact source-neutral input used by a containing module surface.
    pub(super) fn semantic_bytes(&self) -> (u8, &[u8]) {
        match &self.surface {
            Ok(surface) => (0, surface.canonical_bytes()),
            Err(message) => (1, message.as_bytes()),
        }
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
        let (surface, bodies, fingerprint) = match &syntax.result {
            Ok(syntax) => {
                let projection = syntax.declaration_projection();
                let bodies = projection.body_surfaces().to_vec().into_boxed_slice();
                let surface = Arc::new(projection.into_surface());
                let fingerprint = tagged_fingerprint(0, surface.canonical_bytes());
                (Ok(surface), bodies, fingerprint)
            }
            Err(message) => (
                Err(Arc::clone(message)),
                Vec::new().into_boxed_slice(),
                tagged_fingerprint(1, message.as_bytes()),
            ),
        };
        Ok(SourceDeclarationSurfaceProduct {
            surface,
            bodies,
            fingerprint,
        })
    }
}

pub(crate) fn declaration_surface(
    database: &Database,
    path: &Path,
) -> Result<Arc<SourceDeclarationSurfaceProduct>, ComputationError> {
    database
        .query::<SourceDeclarationSurfaceQuery>(SourceSyntaxKey::new(path, ParseGoal::SourceFile))
}

fn tagged_fingerprint(tag: u8, bytes: &[u8]) -> Fingerprint {
    let mut input = Vec::with_capacity(bytes.len() + 1);
    input.push(tag);
    input.extend_from_slice(bytes);
    Fingerprint::from_bytes(&input)
}

/// Prepared source inputs whose identity and publication are derived from one representation.
pub(crate) struct SourceRevisionPublication {
    domain: OverlayDomain,
    epoch: FilesystemEpoch,
    texts: Vec<(SourcePath, OverlayText)>,
    fingerprint: Fingerprint,
}

impl SourceRevisionPublication {
    pub(crate) fn new(overlay: &SourceOverlay, filesystem_epoch: u64) -> Self {
        let texts = overlay
            .sources()
            .map(|(path, source)| (SourcePath::new(path), OverlayText::new(source.bytes())))
            .collect::<Vec<_>>();
        let domain = OverlayDomain::new(texts.iter().map(|(path, _)| path.clone()).collect());
        let epoch = FilesystemEpoch::new(filesystem_epoch);
        let mut source_identity = Vec::new();
        source_identity.extend_from_slice(&domain.fingerprint.digest());
        source_identity.extend_from_slice(&epoch.fingerprint.digest());
        for (_, text) in &texts {
            source_identity.extend_from_slice(&text.fingerprint.digest());
        }
        Self {
            domain,
            epoch,
            texts,
            fingerprint: Fingerprint::from_bytes(&source_identity),
        }
    }

    pub(crate) const fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    pub(crate) fn publish(
        self,
        database: &mut Database,
    ) -> Result<ComputationRevision, ComputationError> {
        let mut revision = database.advance_revision()?;
        revision.set::<OverlayDomainInput>(&(), self.domain);
        revision.set::<FilesystemEpochInput>(&(), self.epoch);
        for (path, text) in self.texts {
            revision.set::<OverlayTextInput>(&path, text);
        }
        Ok(revision.commit())
    }
}

pub(crate) struct ComputedSourceSyntax<'database> {
    database: &'database Database,
}

impl<'database> ComputedSourceSyntax<'database> {
    pub(crate) const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

impl SourceSyntaxProvider for ComputedSourceSyntax<'_> {
    fn parsed_syntax(
        &mut self,
        source: &SourceFile,
        goal: ParseGoal,
    ) -> Result<Arc<ParsedSyntax>, SourceSyntaxError> {
        let path = PathBuf::from(source.name().as_str());
        let product = self
            .database
            .query::<SourceSyntaxQuery>(SourceSyntaxKey::new(&path, goal))
            .map_err(SourceSyntaxError::new)?;
        let syntax = product
            .result
            .as_ref()
            .map_err(|message| SourceSyntaxError::new(SourceProductError(Arc::clone(message))))?;
        if !syntax.matches(source) {
            return Err(SourceSyntaxError::new(SourceProductError(
                "source syntax was requested with bytes outside the admitted overlay".into(),
            )));
        }
        Ok(Arc::clone(syntax))
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

pub(crate) fn source_text_execution_count(database: &Database) -> u64 {
    database.execution_count::<SourceTextQuery>()
}

pub(crate) fn declaration_surface_execution_count(database: &Database) -> u64 {
    database.execution_count::<SourceDeclarationSurfaceQuery>()
}
