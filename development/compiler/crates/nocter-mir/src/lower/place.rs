use nocter_checking::{PlaceProjection, PlaceRoot};
use nocter_model::{MirPlaceId, PlaceId};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{MirPlaceRoot, MirProjection, MirProjectionKind};

impl FunctionLowerer<'_> {
    pub(super) fn lower_place(&mut self, place: PlaceId) -> Result<MirPlaceId, MirLoweringError> {
        let checked = self
            .body
            .places()
            .get(place)
            .cloned()
            .ok_or(MirLoweringError::UnknownPlace(place))?;
        if checked.projections().len() != checked.projection_types().len() {
            return Err(MirLoweringError::InvalidProjectionTypes(place));
        }
        let root = match checked.root() {
            PlaceRoot::Parameter(parameter) => MirPlaceRoot::Local(
                *self
                    .parameters
                    .get(&parameter)
                    .ok_or(MirLoweringError::MissingInput(parameter))?,
            ),
            PlaceRoot::Local(local) => MirPlaceRoot::Local(self.ensure_local(local)?),
            PlaceRoot::Capture(_) => {
                return Err(MirLoweringError::UnsupportedPlaceProjection(place));
            }
        };
        let mut projections = Vec::with_capacity(checked.projections().len());
        for (projection, source_ty) in checked.projections().iter().zip(checked.projection_types())
        {
            let ty = self.concrete_type(*source_ty)?;
            let kind = match projection {
                PlaceProjection::Field(field) => MirProjectionKind::Field(*field),
                PlaceProjection::BorrowDeref { capability } => {
                    MirProjectionKind::BorrowDereference(*capability)
                }
                PlaceProjection::BuiltinIndex { index } => {
                    MirProjectionKind::DynamicIndex(self.require_value(*index)?)
                }
                PlaceProjection::CoercedBuiltinIndex { .. }
                | PlaceProjection::SelectedIndex { .. } => {
                    return Err(MirLoweringError::UnsupportedPlaceProjection(place));
                }
            };
            projections.push(MirProjection::new(kind, ty));
        }
        let ty = self.concrete_type(checked.ty())?;
        Ok(self.builder.add_place(root, projections, ty))
    }
}
