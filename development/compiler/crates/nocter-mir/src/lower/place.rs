use nocter_checking::{PlaceProjection, PlaceRoot};
use nocter_model::{MirPlaceId, PlaceId, TypeId};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{MirPlaceRoot, MirProjection, MirProjectionKind};

pub(super) struct LoweredPlacePath {
    pub(super) root: MirPlaceRoot,
    pub(super) projections: Vec<MirProjection>,
    pub(super) ty: TypeId,
}

impl LoweredPlacePath {
    pub(super) fn push(&mut self, kind: MirProjectionKind, ty: TypeId) {
        self.projections.push(MirProjection::new(kind, ty));
        self.ty = ty;
    }
}

impl FunctionLowerer<'_> {
    pub(super) fn lower_place(&mut self, place: PlaceId) -> Result<MirPlaceId, MirLoweringError> {
        if let Some(lowered) = self.places.get(&place).copied() {
            return Ok(lowered);
        }
        let checked = self
            .body
            .places()
            .get(place)
            .cloned()
            .ok_or(MirLoweringError::UnknownPlace(place))?;
        let root = match checked.root() {
            PlaceRoot::Parameter(parameter) => MirPlaceRoot::Local(
                *self
                    .parameters
                    .get(&parameter)
                    .ok_or(MirLoweringError::MissingInput(parameter))?,
            ),
            PlaceRoot::Local(local) => MirPlaceRoot::Local(self.ensure_local(local)?),
            PlaceRoot::Capture(capture) => {
                let mut path = self.lower_capture_path(capture)?;
                path.projections.reserve(checked.projections().len());
                return self.finish_lower_place(place, &checked, path, 0);
            }
            PlaceRoot::Value(value) => {
                let checked_value = self
                    .body
                    .nodes()
                    .get(value)
                    .ok_or(MirLoweringError::InvalidProjectionTypes(place))?;
                let value_ty = self.concrete_type(checked_value.ty())?;
                if !matches!(
                    checked.projections().first(),
                    Some(PlaceProjection::BorrowDeref { .. })
                ) {
                    let lowered_value = self.require_value(value)?;
                    let storage = self.materialize_value_storage(value, lowered_value)?;
                    let stored = self
                        .builder
                        .place(storage)
                        .ok_or(MirLoweringError::InvalidProjectionTypes(place))?;
                    let path = LoweredPlacePath {
                        root: stored.root(),
                        projections: stored.projections().to_vec(),
                        ty: value_ty,
                    };
                    return self.finish_lower_place(place, &checked, path, 0);
                }
                let (capability, referent) = self.borrow_shape(place, value_ty)?;
                let Some(PlaceProjection::BorrowDeref {
                    capability: projected,
                    ..
                }) = checked.projections().first()
                else {
                    return Err(MirLoweringError::InvalidProjectionTypes(place));
                };
                if *projected != capability {
                    return Err(MirLoweringError::InvalidProjectionTypes(place));
                }
                let value = self.require_value(value)?;
                let path = LoweredPlacePath {
                    root: MirPlaceRoot::Dereference { value, capability },
                    projections: Vec::with_capacity(checked.projections().len() - 1),
                    ty: referent,
                };
                return self.finish_lower_place(place, &checked, path, 1);
            }
        };
        let root_ty = match root {
            MirPlaceRoot::Local(local) => self
                .builder
                .local_type(local)
                .ok_or(MirLoweringError::UnknownPlace(place))?,
            MirPlaceRoot::Dereference { .. } => unreachable!("checked roots start from locals"),
        };
        let path = LoweredPlacePath {
            root,
            projections: Vec::with_capacity(checked.projections().len()),
            ty: root_ty,
        };
        self.finish_lower_place(place, &checked, path, 0)
    }

    fn finish_lower_place(
        &mut self,
        place: PlaceId,
        checked: &nocter_checking::CheckedPlace,
        mut path: LoweredPlacePath,
        projection_start: usize,
    ) -> Result<MirPlaceId, MirLoweringError> {
        for projection in checked.projections().iter().skip(projection_start) {
            let ty = self.concrete_type(projection.ty())?;
            match projection {
                PlaceProjection::Field { field, .. } => {
                    path.push(MirProjectionKind::Field(*field), ty);
                }
                PlaceProjection::TupleElement { index, .. } => {
                    path.push(MirProjectionKind::TupleElement(*index), ty);
                }
                PlaceProjection::BorrowDeref { capability, .. } => {
                    path.push(MirProjectionKind::BorrowDereference(*capability), ty);
                }
                PlaceProjection::BuiltinIndex { index, .. } => {
                    let index = self.require_value(*index)?;
                    path.push(MirProjectionKind::DynamicIndex(index), ty);
                }
                PlaceProjection::CoercedBuiltinIndex {
                    index,
                    receiver_coercion,
                    ..
                } => {
                    self.lower_coerced_builtin_index(
                        place,
                        &mut path,
                        *index,
                        receiver_coercion,
                        ty,
                    )?;
                }
                PlaceProjection::SelectedIndex {
                    index,
                    operation,
                    receiver_coercion,
                    ..
                } => self.lower_selected_index(
                    place,
                    &mut path,
                    *index,
                    operation,
                    receiver_coercion.as_ref(),
                    ty,
                )?,
            }
        }
        let ty = self.concrete_type(checked.ty())?;
        if path.ty != ty {
            return Err(MirLoweringError::InvalidProjectionTypes(place));
        }
        let lowered = self.builder.add_place(path.root, path.projections, ty);
        if self.places.insert(place, lowered).is_some() {
            return Err(MirLoweringError::InvalidProjectionTypes(place));
        }
        Ok(lowered)
    }
}
