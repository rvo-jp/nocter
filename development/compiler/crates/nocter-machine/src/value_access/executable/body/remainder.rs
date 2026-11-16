//! Structural cleanup and transport share the checked sparse exclusions, never another ledger.
use super::{Builder, Failure, mir};
use crate::{
    MachineAddress, MachineAddressId, MachineAddressRoot, MachineAddressStep,
    MachineArithmeticCheck, MachineBinaryOperation, MachineBranchTarget, MachineCall,
    MachineCallAllocation, MachineCallTarget, MachineLayoutKind, MachineOperationKind,
    MachineTerminator, MachineValueId,
};
use nocter_model::{MirLocalId, ValueField};
use nocter_runtime_contract::RuntimeDataId;
use nocter_runtime_contract::{RuntimePrimitive, RuntimeTypeRepresentation};

#[derive(Clone)]
struct Exclusion {
    path: Vec<mir::Projection>,
    conditions: Vec<MachineValueId>,
    prefix: Option<MirLocalId>,
}

impl Builder<'_> {
    pub(super) fn dispose_array_remainder(
        &mut self,
        source: &mir::Place,
        missing: &[mir::RemainderExclusion],
    ) -> Result<(), Failure> {
        if !self.context.cleanup.contains_key(&source.data) {
            return Ok(());
        }
        let exclusions = missing
            .iter()
            .map(|exclusion| {
                let (hole, prefix) = match exclusion {
                    mir::RemainderExclusion::Value(hole) => (hole, None),
                    mir::RemainderExclusion::ArrayPrefix { array, endpoint } => {
                        (array, Some(*endpoint))
                    }
                };
                if hole.slot != source.slot
                    || hole.root != source.root
                    || !hole.projections.starts_with(&source.projections)
                {
                    return Err(Self::invalid());
                }
                Ok(Exclusion {
                    path: hole.projections[source.projections.len()..].to_vec(),
                    conditions: Vec::new(),
                    prefix,
                })
            })
            .collect::<Result<Vec<_>, Failure>>()?;
        let address = self.place(source)?;
        let pointer = self.value(
            source.data,
            true,
            MachineOperationKind::AddressOf { source: address },
        )?;
        self.visit_remaining(source.data, pointer, &exclusions, None)
    }

    pub(super) fn copy_remaining_fields(
        &mut self,
        logical: &mir::Place,
        source: MachineAddressId,
        destination: MachineAddressId,
        missing: &[Box<[mir::Projection]>],
    ) -> Result<(), Failure> {
        let exclusions = missing
            .iter()
            .filter(|hole| hole.starts_with(&logical.projections))
            .map(|hole| Exclusion {
                path: hole[logical.projections.len()..].to_vec(),
                conditions: Vec::new(),
                prefix: None,
            })
            .collect::<Vec<_>>();
        if exclusions.is_empty() {
            return Err(Self::invalid());
        }
        let source = self.value(
            logical.data,
            true,
            MachineOperationKind::AddressOf { source },
        )?;
        let destination = self.value(
            logical.data,
            true,
            MachineOperationKind::AddressOf {
                source: destination,
            },
        )?;
        self.visit_remaining(logical.data, source, &exclusions, Some(destination))
    }

    fn remainder_address(
        &mut self,
        data: RuntimeDataId,
        pointer: MachineValueId,
        offset: u64,
    ) -> Result<MachineAddressId, Failure> {
        let layout = self.context.layouts.get(data).ok_or_else(Self::invalid)?;
        Ok(self.address(MachineAddress::new(
            data,
            layout.size(),
            layout.alignment(),
            MachineAddressRoot::Pointer { value: pointer },
            [MachineAddressStep::Offset(offset)],
        )))
    }
    fn remainder_member(
        &mut self,
        data: RuntimeDataId,
        pointer: MachineValueId,
        offset: u64,
    ) -> Result<MachineValueId, Failure> {
        let source = self.remainder_address(data, pointer, offset)?;
        self.value(data, true, MachineOperationKind::AddressOf { source })
    }
    fn visit_complete(
        &mut self,
        data: RuntimeDataId,
        source: MachineValueId,
        destination: Option<MachineValueId>,
    ) -> Result<(), Failure> {
        if let Some(destination) = destination {
            let source = self.remainder_address(data, source, 0)?;
            let target = self.remainder_address(data, destination, 0)?;
            let value = self.value(data, false, MachineOperationKind::Load { source })?;
            self.emit(MachineOperationKind::Store {
                destination: target,
                value,
            });
        } else {
            let target = *self.context.cleanup.get(&data).ok_or_else(Self::invalid)?;
            self.emit(MachineOperationKind::Call(MachineCall::new(
                MachineCallTarget::Direct(target),
                [source],
                MachineCallAllocation::Inherit,
                None,
            )));
        }
        Ok(())
    }

    fn visit_remaining(
        &mut self,
        data: RuntimeDataId,
        pointer: MachineValueId,
        exclusions: &[Exclusion],
        destination: Option<MachineValueId>,
    ) -> Result<(), Failure> {
        if destination.is_none() && !self.context.cleanup.contains_key(&data) {
            return Ok(());
        }
        if destination.is_some()
            && self
                .context
                .layouts
                .get(data)
                .ok_or_else(Self::invalid)?
                .size()
                == 0
        {
            return Ok(());
        }
        // An unconditional whole-node exclusion needs neither a helper nor a traversal.
        if exclusions
            .iter()
            .any(|hole| hole.path.is_empty() && hole.prefix.is_none() && hole.conditions.is_empty())
        {
            return Ok(());
        }
        let full: Vec<_> = exclusions
            .iter()
            .filter(|hole| hole.path.is_empty() && hole.prefix.is_none())
            .collect();
        let end = (!full.is_empty()).then(|| self.block());
        for hole in full {
            let next_hole = self.block();
            for &condition in &hole.conditions {
                let next = self.block();
                self.terminate(MachineTerminator::Branch {
                    condition,
                    then_target: MachineBranchTarget::new(next, []),
                    else_target: MachineBranchTarget::new(next_hole, []),
                })?;
                self.current = next;
            }
            self.terminate(MachineTerminator::Goto(MachineBranchTarget::new(
                end.ok_or_else(Self::invalid)?,
                [],
            )))?;
            self.current = next_hole;
        }
        let partial: Vec<_> = exclusions
            .iter()
            .filter(|hole| !hole.path.is_empty() || hole.prefix.is_some())
            .cloned()
            .collect();
        if partial.is_empty() {
            self.visit_complete(data, pointer, destination)?;
        } else {
            self.visit_partial_shape(data, pointer, &partial, destination)?;
        }
        if let Some(end) = end {
            self.terminate(MachineTerminator::Goto(MachineBranchTarget::new(end, [])))?;
            self.current = end;
        }
        Ok(())
    }

    fn visit_partial_shape(
        &mut self,
        data: RuntimeDataId,
        pointer: MachineValueId,
        partial: &[Exclusion],
        destination: Option<MachineValueId>,
    ) -> Result<(), Failure> {
        match self
            .context
            .layouts
            .get(data)
            .ok_or_else(Self::invalid)?
            .kind()
            .clone()
        {
            MachineLayoutKind::FixedArray {
                element,
                length,
                stride,
            } => {
                self.visit_partial_array(element, length, stride, pointer, partial, destination)?;
            }
            MachineLayoutKind::Struct { fields } => {
                let Some(RuntimeTypeRepresentation::ValueAggregate { fields: schema }) = self
                    .context
                    .program
                    .runtime()
                    .type_representations()
                    .get(data)
                else {
                    return Err(Self::invalid());
                };
                let schema = schema.clone();
                if schema.len() != fields.len() {
                    return Err(Self::invalid());
                }
                for (field, logical) in fields.iter().zip(schema.iter()).rev() {
                    let mut children = Vec::new();
                    for hole in partial {
                        if hole.path.first() == Some(&mir::Projection::Field(logical.field())) {
                            children.push(Exclusion {
                                path: hole.path[1..].to_vec(),
                                conditions: hole.conditions.clone(),
                                prefix: hole.prefix,
                            });
                        }
                    }
                    let child = self.remainder_member(field.ty(), pointer, field.offset())?;
                    let target = destination
                        .map(|pointer| self.remainder_member(field.ty(), pointer, field.offset()))
                        .transpose()?;
                    self.visit_remaining(field.ty(), child, &children, target)?;
                }
            }
            _ => return Err(Self::invalid()),
        }
        Ok(())
    }

    fn visit_partial_array(
        &mut self,
        element: RuntimeDataId,
        length: u64,
        stride: u64,
        pointer: MachineValueId,
        exclusions: &[Exclusion],
        destination: Option<MachineValueId>,
    ) -> Result<(), Failure> {
        let traversal = self.begin_array(length)?;
        let boolean = self
            .context
            .program
            .runtime()
            .types()
            .primitive(RuntimePrimitive::Bool)
            .ok_or_else(Self::invalid)?;
        let mut children = Vec::new();
        for hole in exclusions {
            let (index, operation, path, prefix) = if hole.path.is_empty() {
                let endpoint = hole.prefix.ok_or_else(Self::invalid)?;
                (
                    self.retained_successor(endpoint, length)?,
                    MachineBinaryOperation::Less,
                    Vec::new(),
                    None,
                )
            } else {
                let index = match hole.path.first() {
                    Some(mir::Projection::Index(slot)) => self.retained_index(*slot, length)?,
                    Some(mir::Projection::Field(ValueField::Array(index))) if *index < length => {
                        self.size_constant(*index)?
                    }
                    _ => return Err(Self::invalid()),
                };
                (
                    index,
                    MachineBinaryOperation::Equal,
                    hole.path[1..].to_vec(),
                    hole.prefix,
                )
            };
            let condition = self.value(
                boolean,
                false,
                MachineOperationKind::Binary {
                    operation,
                    left: traversal.index,
                    right: index,
                    check: MachineArithmeticCheck::NotRequired,
                },
            )?;
            let mut conditions = hole.conditions.clone();
            conditions.push(condition);
            children.push(Exclusion {
                path,
                conditions,
                prefix,
            });
        }
        let address = self.array_address(element, pointer, traversal.index, stride)?;
        let child = self.value(
            element,
            true,
            MachineOperationKind::AddressOf { source: address },
        )?;
        let target = destination
            .map(|pointer| {
                let address = self.array_address(element, pointer, traversal.index, stride)?;
                self.value(
                    element,
                    true,
                    MachineOperationKind::AddressOf { source: address },
                )
            })
            .transpose()?;
        self.visit_remaining(element, child, &children, target)?;
        self.end_array(traversal)
    }
}
