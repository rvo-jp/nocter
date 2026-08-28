use std::collections::{BTreeMap, BTreeSet};

use nocter_model::{
    Arena, ArenaBuilder, AssociatedTypeId, CallableId, DeclarationSiteId, InterfaceId,
    RequirementId, Symbol,
};

use crate::{DeclarationArenas, DeclarationProgram, RequirementKind, RequirementSubject};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InterfaceCapabilityIssue {
    Cycle {
        interface: InterfaceId,
        related: InterfaceId,
    },
    MemberCollision {
        interface: InterfaceId,
        first: DeclarationSiteId,
        second: DeclarationSiteId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DependencyVisit {
    Active,
    Complete,
}

/// Effective capabilities of one interface after prerequisite closure.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InterfaceCapability {
    direct_prerequisites: Box<[RequirementId]>,
    prerequisite_paths: Box<[Box<[RequirementId]>]>,
    inherited_interfaces: Box<[InterfaceId]>,
    methods: Box<[CallableId]>,
    associated_types: Box<[AssociatedTypeId]>,
}

impl InterfaceCapability {
    #[must_use]
    pub const fn direct_prerequisites(&self) -> &[RequirementId] {
        &self.direct_prerequisites
    }

    /// Canonical declaration paths for every direct and transitive prerequisite.
    ///
    /// Each path starts at a requirement declared by this interface and ends at the exact
    /// requirement that contributes a predicate. Checking specializes these frozen paths instead
    /// of traversing interface declarations again.
    #[must_use]
    pub const fn prerequisite_paths(&self) -> &[Box<[RequirementId]>] {
        &self.prerequisite_paths
    }

    #[must_use]
    pub const fn inherited_interfaces(&self) -> &[InterfaceId] {
        &self.inherited_interfaces
    }

    #[must_use]
    pub const fn methods(&self) -> &[CallableId] {
        &self.methods
    }

    #[must_use]
    pub const fn associated_types(&self) -> &[AssociatedTypeId] {
        &self.associated_types
    }
}

/// Canonical, declaration-owned interface prerequisite closure.
#[derive(Clone, Debug, Default)]
pub struct InterfaceCapabilityGraph {
    entries: Arena<InterfaceId, InterfaceCapability>,
    issues: Box<[InterfaceCapabilityIssue]>,
}

impl InterfaceCapabilityGraph {
    pub(crate) fn build(program: &DeclarationProgram) -> Self {
        let declarations = program.declarations();
        let direct = declarations
            .interfaces()
            .iter()
            .map(|(interface_id, interface)| {
                let requirements = interface.requirements().to_vec();
                (interface_id, requirements)
            })
            .collect::<BTreeMap<_, _>>();

        let mut issues = Vec::new();
        let mut reported_cycles = BTreeSet::new();
        let mut dependency_visits = BTreeMap::new();
        let mut dependency_path = Vec::new();
        for (interface_id, _) in declarations.interfaces().iter() {
            collect_dependency_cycles(
                declarations,
                &direct,
                interface_id,
                &mut dependency_visits,
                &mut dependency_path,
                &mut reported_cycles,
                &mut issues,
            );
        }
        let mut entries = ArenaBuilder::new();
        for (interface_id, _) in declarations.interfaces().iter() {
            let mut path = Vec::new();
            let mut ordered = Vec::new();
            collect_interfaces(
                declarations,
                &direct,
                interface_id,
                interface_id,
                &mut path,
                &mut ordered,
            );
            let mut seen = BTreeSet::new();
            ordered.retain(|candidate| seen.insert(*candidate));
            let (methods, associated_types) =
                collect_members(declarations, interface_id, &ordered, &mut issues);
            let mut prerequisite_paths = Vec::new();
            collect_prerequisite_paths(
                declarations,
                &direct,
                interface_id,
                &mut vec![interface_id],
                &mut Vec::new(),
                &mut prerequisite_paths,
            );
            let actual = entries.insert(InterfaceCapability {
                direct_prerequisites: direct
                    .get(&interface_id)
                    .cloned()
                    .unwrap_or_default()
                    .into_boxed_slice(),
                prerequisite_paths: prerequisite_paths.into_boxed_slice(),
                inherited_interfaces: ordered.into_boxed_slice(),
                methods: methods.into_boxed_slice(),
                associated_types: associated_types.into_boxed_slice(),
            });
            debug_assert_eq!(actual, interface_id);
        }
        Self {
            entries: entries.finish(),
            issues: issues.into_boxed_slice(),
        }
    }

    #[must_use]
    pub fn get(&self, interface: InterfaceId) -> Option<&InterfaceCapability> {
        self.entries.get(interface)
    }

    pub(crate) const fn issues(&self) -> &[InterfaceCapabilityIssue] {
        &self.issues
    }

    #[must_use]
    pub fn entails(&self, interface: InterfaceId, candidate: InterfaceId) -> bool {
        interface == candidate
            || self
                .get(interface)
                .is_some_and(|entry| entry.inherited_interfaces().contains(&candidate))
    }
}

fn collect_prerequisite_paths(
    declarations: &DeclarationArenas,
    direct: &BTreeMap<InterfaceId, Vec<RequirementId>>,
    current: InterfaceId,
    active: &mut Vec<InterfaceId>,
    prefix: &mut Vec<RequirementId>,
    output: &mut Vec<Box<[RequirementId]>>,
) {
    for requirement in direct.get(&current).into_iter().flatten() {
        prefix.push(*requirement);
        output.push(prefix.clone().into_boxed_slice());
        if let Some(RequirementKind::Interface { application, .. }) = declarations
            .requirements()
            .get(*requirement)
            .map(crate::Requirement::kind)
        {
            let prerequisite = application.interface();
            if !active.contains(&prerequisite) {
                active.push(prerequisite);
                collect_prerequisite_paths(
                    declarations,
                    direct,
                    prerequisite,
                    active,
                    prefix,
                    output,
                );
                active.pop();
            }
        }
        prefix.pop();
    }
}

/// Validates every implication edge, including prerequisites whose subject is an interface
/// parameter rather than contextual `Self`. Member inheritance is intentionally narrower and is
/// collected separately by `collect_interfaces`.
fn collect_dependency_cycles(
    declarations: &DeclarationArenas,
    direct: &BTreeMap<InterfaceId, Vec<RequirementId>>,
    current: InterfaceId,
    visits: &mut BTreeMap<InterfaceId, DependencyVisit>,
    path: &mut Vec<InterfaceId>,
    reported_cycles: &mut BTreeSet<(InterfaceId, InterfaceId)>,
    issues: &mut Vec<InterfaceCapabilityIssue>,
) {
    match visits.get(&current).copied() {
        Some(DependencyVisit::Complete) => return,
        Some(DependencyVisit::Active) => {
            let position = path
                .iter()
                .position(|candidate| *candidate == current)
                .unwrap_or(0);
            let mut cycle = path[position..].to_vec();
            cycle.sort_unstable();
            let first = cycle[0];
            let related = cycle.get(1).copied().unwrap_or(first);
            if reported_cycles.insert((first, related)) {
                issues.push(InterfaceCapabilityIssue::Cycle {
                    interface: first,
                    related,
                });
            }
            return;
        }
        None => {}
    }
    visits.insert(current, DependencyVisit::Active);
    path.push(current);
    for requirement_id in direct.get(&current).into_iter().flatten() {
        let Some(RequirementKind::Interface { application, .. }) = declarations
            .requirements()
            .get(*requirement_id)
            .map(crate::Requirement::kind)
        else {
            continue;
        };
        collect_dependency_cycles(
            declarations,
            direct,
            application.interface(),
            visits,
            path,
            reported_cycles,
            issues,
        );
    }
    path.pop();
    visits.insert(current, DependencyVisit::Complete);
}

#[allow(clippy::too_many_arguments)]
fn collect_interfaces(
    declarations: &DeclarationArenas,
    direct: &BTreeMap<InterfaceId, Vec<RequirementId>>,
    root: InterfaceId,
    current: InterfaceId,
    path: &mut Vec<InterfaceId>,
    ordered: &mut Vec<InterfaceId>,
) {
    if path.contains(&current) {
        return;
    }
    path.push(current);
    for requirement_id in direct.get(&current).into_iter().flatten() {
        let Some(RequirementKind::Interface {
            subject: RequirementSubject::InterfaceSelf(owner),
            application,
            ..
        }) = declarations
            .requirements()
            .get(*requirement_id)
            .map(crate::Requirement::kind)
        else {
            continue;
        };
        if *owner != current {
            continue;
        }
        let prerequisite = application.interface();
        collect_interfaces(declarations, direct, root, prerequisite, path, ordered);
        if prerequisite != root {
            ordered.push(prerequisite);
        }
    }
    path.pop();
}

fn collect_members(
    declarations: &DeclarationArenas,
    interface: InterfaceId,
    prerequisites: &[InterfaceId],
    issues: &mut Vec<InterfaceCapabilityIssue>,
) -> (Vec<CallableId>, Vec<AssociatedTypeId>) {
    let mut methods = BTreeMap::<Symbol, (CallableId, DeclarationSiteId)>::new();
    let mut associated = BTreeMap::<Symbol, (AssociatedTypeId, DeclarationSiteId)>::new();
    for owner in prerequisites.iter().copied().chain([interface]) {
        let Some(declaration) = declarations.interfaces().get(owner) else {
            continue;
        };
        for method_id in declaration.methods() {
            let Some(method) = declarations.callables().get(*method_id) else {
                continue;
            };
            let Some(name) = method.name() else { continue };
            if let Some((_, first)) = associated.get(&name).copied() {
                issues.push(InterfaceCapabilityIssue::MemberCollision {
                    interface,
                    first,
                    second: method.site(),
                });
                continue;
            }
            if let Some((existing, first)) = methods.get(&name).copied() {
                if existing != *method_id {
                    issues.push(InterfaceCapabilityIssue::MemberCollision {
                        interface,
                        first,
                        second: method.site(),
                    });
                }
            } else {
                methods.insert(name, (*method_id, method.site()));
            }
        }
        for associated_id in declaration.associated_types() {
            let Some(member) = declarations.associated_types().get(*associated_id) else {
                continue;
            };
            if let Some((_, first)) = methods.get(&member.name()).copied() {
                issues.push(InterfaceCapabilityIssue::MemberCollision {
                    interface,
                    first,
                    second: member.site(),
                });
                continue;
            }
            if let Some((existing, first)) = associated.get(&member.name()).copied() {
                if existing != *associated_id {
                    issues.push(InterfaceCapabilityIssue::MemberCollision {
                        interface,
                        first,
                        second: member.site(),
                    });
                }
            } else {
                associated.insert(member.name(), (*associated_id, member.site()));
            }
        }
    }
    (
        methods.into_values().map(|(id, _)| id).collect(),
        associated.into_values().map(|(id, _)| id).collect(),
    )
}
