//! Definition and implementation locations from the sealed v2 declaration and call selections.
use super::{EvidenceIntegrityError, SemanticLocation};
use nocter_checking::value_access::{CallKind, ExpressionKind};
use nocter_compile_input::ModuleSourceKind;
use nocter_model::{AccessCodeId, ValueFunctionOrigin};
use nocter_session::AnalyzedUnit;
use nocter_source::{ByteOffset, SourceId, TextRange};
use nocter_source_index::SourceOrigin;
use nocter_syntax::SyntaxOrigin;
use std::collections::BTreeMap;

const INVALID: EvidenceIntegrityError = EvidenceIntegrityError::InvalidValueAccessProjection;

#[derive(Clone, Copy, Debug, Default)]
struct Sites {
    contract: Option<SemanticLocation>,
    body: Option<SemanticLocation>,
}

#[derive(Debug, Default)]
pub(in crate::query) struct Navigation {
    targets: BTreeMap<AccessCodeId, Sites>,
    sources: BTreeMap<SourceId, BTreeMap<ByteOffset, (TextRange, AccessCodeId)>>,
}

impl Navigation {
    pub(in crate::query) fn prepare(
        analysis: &AnalyzedUnit,
    ) -> Result<Option<Self>, EvidenceIntegrityError> {
        let Some(accepted) = analysis
            .value_access_analysis()
            .and_then(|value| value.accepted())
        else {
            return Ok(None);
        };
        let input = analysis.unit().compile_input().map_err(|_| INVALID)?;
        let headers = accepted.headers().bind(input).map_err(|_| INVALID)?;
        let current = headers.current().map_err(|_| INVALID)?;
        let checked = accepted.unit().bind(&current).map_err(|_| INVALID)?;
        let mut navigation = Self::default();
        for occurrence in headers.code_origins() {
            let (code, role, origin) = occurrence.map_err(|_| INVALID)?;
            let site = match role {
                ModuleSourceKind::Root => &mut navigation.targets.entry(code).or_default().contract,
                ModuleSourceKind::Implementation | ModuleSourceKind::SingleFile => {
                    &mut navigation.targets.entry(code).or_default().body
                }
            };
            let span = origin.span();
            if site
                .replace(SemanticLocation {
                    source: span.source(),
                    range: span.range(),
                })
                .is_some()
            {
                return Err(INVALID);
            }
            navigation.record(origin, code)?;
        }
        for (owner, _) in accepted.unit().bodies() {
            let typed = checked.body(owner).ok_or(INVALID)?.typed();
            for (call, origin) in typed.direct_call_origins() {
                let expression = typed
                    .query()
                    .body()
                    .expressions()
                    .get(call)
                    .ok_or(INVALID)?;
                let ExpressionKind::Call {
                    kind: CallKind::Direct,
                    target: ValueFunctionOrigin::Declaration(code),
                    ..
                } = expression.kind
                else {
                    return Err(INVALID);
                };
                let SyntaxOrigin::Token(token) = origin else {
                    return Err(INVALID);
                };
                let origin =
                    SourceOrigin::from_token(typed.syntax().tree(), token).map_err(|_| INVALID)?;
                navigation.record(origin, code)?;
            }
        }
        Ok(Some(navigation))
    }

    fn record(
        &mut self,
        origin: SourceOrigin,
        code: AccessCodeId,
    ) -> Result<(), EvidenceIntegrityError> {
        if !self.targets.contains_key(&code) {
            return Err(INVALID);
        }
        let span = origin.span();
        let range = span.range();
        let sites = self.sources.entry(span.source()).or_default();
        match sites.insert(range.start(), (range, code)) {
            Some(previous) if previous != (range, code) => Err(INVALID),
            _ => Ok(()),
        }
    }

    pub(super) fn location(
        &self,
        source: SourceId,
        offset: ByteOffset,
        implementation: bool,
    ) -> Option<SemanticLocation> {
        let (_, &(range, code)) = self.sources.get(&source)?.range(..=offset).next_back()?;
        if offset >= range.end() {
            return None;
        }
        let sites = self.targets.get(&code)?;
        if implementation {
            sites.body.or(sites.contract)
        } else {
            sites.contract.or(sites.body)
        }
    }
}
