//! Structural complement of one tuple destination; ownership and address freedom are separate.
use super::{Place, Projection};
use nocter_model::ValueField;
use nocter_runtime_contract::{RuntimeDataId, RuntimeEnvironment, RuntimeTypeRepresentation};

pub(super) fn preserved(
    runtime: &RuntimeEnvironment,
    before: RuntimeDataId,
    destination: &Place,
) -> Option<Box<[(Place, Place)]>> {
    if destination.projections.is_empty() {
        return None;
    }
    let mut left = before;
    let mut right = destination.root;
    let mut prefix = Vec::new();
    let mut retained = Vec::new();
    for projection in &destination.projections {
        let Projection::Field(ValueField::Tuple(selected)) = projection else {
            return None;
        };
        let (
            Some(RuntimeTypeRepresentation::ValueAggregate { fields: old }),
            Some(RuntimeTypeRepresentation::ValueAggregate { fields: new }),
        ) = (
            runtime.type_representations().get(left),
            runtime.type_representations().get(right),
        )
        else {
            return None;
        };
        if old.len() != new.len() || *selected >= old.len() {
            return None;
        }
        for (index, (old, new)) in old.iter().zip(new).enumerate() {
            if old.field() != ValueField::Tuple(index) || new.field() != old.field() {
                return None;
            }
            if index == *selected {
                continue;
            }
            if old.ty() != new.ty() {
                return None;
            }
            let mut path = prefix.clone();
            path.push(Projection::Field(ValueField::Tuple(index)));
            let source = Place {
                slot: destination.slot,
                root: before,
                projections: path.into(),
                data: old.ty(),
            };
            let target = Place {
                root: destination.root,
                ..source.clone()
            };
            retained.push((source, target));
        }
        left = old[*selected].ty();
        right = new[*selected].ty();
        prefix.push(*projection);
    }
    (right == destination.data).then(|| retained.into())
}
