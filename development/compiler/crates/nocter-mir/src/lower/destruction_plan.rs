use nocter_checking::{ConcreteDestructionKind, ConcreteDestructionPlan};
use nocter_model::BodyNodeId;

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{
    MirCaptureDestruction, MirDestructionKind, MirDestructionPlan, MirFieldDestruction,
    MirPayloadDestruction, MirVariantDestruction,
};

impl FunctionLowerer<'_> {
    /// Resolves one concrete checking plan into the self-contained recipe required when cleanup
    /// executes inside a transferred compiler-owned value rather than in this caller's CFG.
    pub(super) fn lower_deferred_destruction(
        &self,
        owner: BodyNodeId,
        plan: &ConcreteDestructionPlan,
    ) -> Result<MirDestructionPlan, MirLoweringError> {
        let kind = match plan.kind() {
            ConcreteDestructionKind::Struct { drop, fields } => MirDestructionKind::Struct {
                drop: drop
                    .as_ref()
                    .map(|drop| self.require_drop_item(owner, drop))
                    .transpose()?,
                fields: fields
                    .iter()
                    .map(|field| {
                        Ok(MirFieldDestruction::new(
                            field.field(),
                            self.lower_deferred_destruction(owner, field.plan())?,
                        ))
                    })
                    .collect::<Result<Vec<_>, MirLoweringError>>()?
                    .into_boxed_slice(),
            },
            ConcreteDestructionKind::Enum { drop, variants } => MirDestructionKind::Enum {
                drop: drop
                    .as_ref()
                    .map(|drop| self.require_drop_item(owner, drop))
                    .transpose()?,
                variants: variants
                    .iter()
                    .map(|variant| {
                        Ok(MirVariantDestruction::new(
                            variant.variant(),
                            variant
                                .payload()
                                .iter()
                                .map(|payload| {
                                    Ok(MirPayloadDestruction::new(
                                        payload.parameter(),
                                        self.lower_deferred_destruction(owner, payload.plan())?,
                                    ))
                                })
                                .collect::<Result<Vec<_>, MirLoweringError>>()?,
                        ))
                    })
                    .collect::<Result<Vec<_>, MirLoweringError>>()?
                    .into_boxed_slice(),
            },
            ConcreteDestructionKind::FixedArray { length, element } => {
                MirDestructionKind::FixedArray {
                    length: *length,
                    element: Box::new(self.lower_deferred_destruction(owner, element)?),
                }
            }
            ConcreteDestructionKind::Optional(payload) => MirDestructionKind::Optional(Box::new(
                self.lower_deferred_destruction(owner, payload)?,
            )),
            ConcreteDestructionKind::Fallible(payload) => MirDestructionKind::Fallible(Box::new(
                self.lower_deferred_destruction(owner, payload)?,
            )),
            ConcreteDestructionKind::Closure(captures) => MirDestructionKind::Closure(
                captures
                    .iter()
                    .map(|capture| {
                        Ok(MirCaptureDestruction::new(
                            capture.capture(),
                            self.lower_deferred_destruction(owner, capture.plan())?,
                        ))
                    })
                    .collect::<Result<Vec<_>, MirLoweringError>>()?
                    .into_boxed_slice(),
            ),
            ConcreteDestructionKind::Opaque {
                definition, plan, ..
            } => MirDestructionKind::Opaque {
                definition: *definition,
                plan: Box::new(self.lower_deferred_destruction(owner, plan)?),
            },
        };
        Ok(MirDestructionPlan::new(plan.ty(), kind))
    }

    fn require_drop_item(
        &self,
        owner: BodyNodeId,
        selection: &nocter_checking::DropSelection,
    ) -> Result<nocter_model::ExecutableItemId, MirLoweringError> {
        self.item
            .body()
            .drop_item(selection)
            .ok_or(MirLoweringError::InvalidCleanup(owner))
    }
}
