use nocter_declarations::Visibility;
use nocter_model::{ModuleId, PackageId};

use crate::{ModuleIdentity, ReservedDeclarations};

pub(super) fn module_index_by_identity(
    reserved: &ReservedDeclarations<'_>,
    identity: &ModuleIdentity,
) -> Option<usize> {
    reserved.modules.binary_search(identity).ok()
}

pub(super) fn module_index_by_id(
    reserved: &ReservedDeclarations<'_>,
    module: ModuleId,
) -> Option<usize> {
    reserved
        .module_ids
        .iter()
        .position(|candidate| *candidate == module)
}

pub(super) fn visible_from(
    reserved: &ReservedDeclarations<'_>,
    visibility: Visibility,
    from: ModuleId,
    declaring_module: ModuleId,
) -> bool {
    match visibility {
        Visibility::Private => from == declaring_module,
        Visibility::Public => true,
        Visibility::Package(package) => package_for_module(reserved, from) == Some(package),
        Visibility::Descendants(boundary) => {
            let Some(boundary_index) = module_index_by_id(reserved, boundary) else {
                return false;
            };
            let Some(from_index) = module_index_by_id(reserved, from) else {
                return false;
            };
            let boundary = &reserved.modules[boundary_index];
            let from = &reserved.modules[from_index];
            boundary.package() == from.package() && from.path().starts_with(boundary.path())
        }
    }
}

pub(super) fn visibility_is_within(
    reserved: &ReservedDeclarations<'_>,
    proposed: Visibility,
    target: Visibility,
) -> bool {
    match (proposed, target) {
        (Visibility::Private, _) | (_, Visibility::Public) => true,
        (_, Visibility::Private)
        | (Visibility::Public, _)
        | (Visibility::Package(_), Visibility::Descendants(_)) => false,
        (Visibility::Package(left), Visibility::Package(right)) => left == right,
        (Visibility::Descendants(boundary), Visibility::Package(package)) => {
            package_for_module(reserved, boundary) == Some(package)
        }
        (Visibility::Descendants(proposed), Visibility::Descendants(target)) => {
            let Some(proposed_index) = module_index_by_id(reserved, proposed) else {
                return false;
            };
            let Some(target_index) = module_index_by_id(reserved, target) else {
                return false;
            };
            let proposed = &reserved.modules[proposed_index];
            let target = &reserved.modules[target_index];
            proposed.package() == target.package() && proposed.path().starts_with(target.path())
        }
    }
}

fn package_for_module(reserved: &ReservedDeclarations<'_>, module: ModuleId) -> Option<PackageId> {
    let module_index = module_index_by_id(reserved, module)?;
    let package_identity = reserved.modules[module_index].package();
    reserved
        .packages
        .iter()
        .position(|package| package.identity() == package_identity)
        .and_then(|index| reserved.package_ids.get(index))
        .copied()
}
