use std::collections::BTreeSet;

use nocter_model::{ConformanceId, ConstructionId, DropId, InstanceId};

use crate::{DeclarationAnalysisAdmission, DeclarationProgram};

use super::{DeclarationRule, DeclarationValidationReport, DeclarationViolation};

/// Complete result of applying authored declaration rules to one structurally valid graph.
pub(crate) struct DeclarationValidation {
    report: DeclarationValidationReport,
    admission: DeclarationAnalysisAdmission,
    body_analysis: BodyAnalysisCapability,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BodyAnalysisCapability {
    DeclarationsOnly,
    AdmittedBodies,
}

impl DeclarationValidation {
    pub(crate) fn into_parts(
        self,
    ) -> (
        DeclarationValidationReport,
        DeclarationAnalysisAdmission,
        BodyAnalysisCapability,
    ) {
        (self.report, self.admission, self.body_analysis)
    }
}

/// Mutable validation state shared by rule domains during one and only one graph traversal.
pub(super) struct ValidationCollector {
    violations: Vec<DeclarationViolation>,
    rejected_constructions: BTreeSet<ConstructionId>,
    rejected_instances: BTreeSet<InstanceId>,
    rejected_conformances: BTreeSet<ConformanceId>,
    rejected_drops: BTreeSet<DropId>,
    body_analysis_safe: bool,
}

impl ValidationCollector {
    pub(super) const fn new() -> Self {
        Self {
            violations: Vec::new(),
            rejected_constructions: BTreeSet::new(),
            rejected_instances: BTreeSet::new(),
            rejected_conformances: BTreeSet::new(),
            rejected_drops: BTreeSet::new(),
            body_analysis_safe: true,
        }
    }

    pub(super) fn reject_program_fact(&mut self, violation: DeclarationViolation) {
        self.body_analysis_safe = false;
        self.violations.push(violation);
    }

    pub(super) fn report(&mut self, violation: DeclarationViolation) {
        self.violations.push(violation);
    }

    pub(super) fn reject_construction(
        &mut self,
        id: ConstructionId,
        violation: DeclarationViolation,
    ) {
        self.rejected_constructions.insert(id);
        self.violations.push(violation);
    }

    pub(super) fn quarantine_construction(&mut self, id: ConstructionId) {
        self.rejected_constructions.insert(id);
    }

    pub(super) fn reject_instance(&mut self, id: InstanceId, violation: DeclarationViolation) {
        self.rejected_instances.insert(id);
        self.violations.push(violation);
    }

    pub(super) fn reject_conformance(
        &mut self,
        id: ConformanceId,
        violation: DeclarationViolation,
    ) {
        self.rejected_conformances.insert(id);
        self.violations.push(violation);
    }

    pub(super) fn reject_drop(&mut self, id: DropId, violation: DeclarationViolation) {
        self.rejected_drops.insert(id);
        self.violations.push(violation);
    }

    pub(super) fn quarantine_drop(&mut self, id: DropId) {
        self.rejected_drops.insert(id);
    }

    pub(super) fn finish(self, program: &DeclarationProgram) -> DeclarationValidation {
        let declarations = program.declarations();
        let constructions = declarations
            .constructions()
            .iter()
            .map(|(id, _)| id)
            .filter(|id| !self.rejected_constructions.contains(id))
            .collect();
        let instances = declarations
            .instances()
            .iter()
            .map(|(id, _)| id)
            .filter(|id| !self.rejected_instances.contains(id))
            .collect();
        let conformances = declarations
            .conformances()
            .iter()
            .map(|(id, _)| id)
            .filter(|id| !self.rejected_conformances.contains(id))
            .collect();
        let drops = declarations
            .drops()
            .iter()
            .map(|(id, _)| id)
            .filter(|id| !self.rejected_drops.contains(id))
            .collect();
        DeclarationValidation {
            report: DeclarationValidationReport::new(self.violations),
            admission: DeclarationAnalysisAdmission::new(
                constructions,
                instances,
                conformances,
                drops,
            ),
            body_analysis: if self.body_analysis_safe {
                BodyAnalysisCapability::AdmittedBodies
            } else {
                BodyAnalysisCapability::DeclarationsOnly
            },
        }
    }
}

pub(super) fn violation(
    rule: DeclarationRule,
    primary: nocter_model::DeclarationSiteId,
) -> DeclarationViolation {
    DeclarationViolation::new(rule, primary)
}

pub(super) fn related_violation(
    rule: DeclarationRule,
    primary: nocter_model::DeclarationSiteId,
    related: nocter_model::DeclarationSiteId,
) -> DeclarationViolation {
    DeclarationViolation::with_related(rule, primary, related)
}
