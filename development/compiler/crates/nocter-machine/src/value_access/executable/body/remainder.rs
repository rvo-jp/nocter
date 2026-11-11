//! Reverse structural cleanup with exclusions from the checked sparse owner, never a new ledger.
use super::{Builder, Failure, mir};
use crate::{
    MachineAddress, MachineAddressRoot, MachineAddressStep, MachineArithmeticCheck,
    MachineBinaryOperation, MachineBranchTarget, MachineCall, MachineCallAllocation,
    MachineCallTarget, MachineLayoutKind, MachineOperationKind, MachineTerminator, MachineValueId,
};
use nocter_model::{MirLocalId, TypeId, ValueField};
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
        self.cleanup_remaining(source.data, pointer, &exclusions)
    }

    fn cleanup_remaining(
        &mut self,
        data: TypeId,
        pointer: MachineValueId,
        exclusions: &[Exclusion],
    ) -> Result<(), Failure> {
        let Some(&target) = self.context.cleanup.get(&data) else {
            return Ok(());
        };
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
            self.emit(MachineOperationKind::Call(MachineCall::new(
                MachineCallTarget::Direct(target),
                [pointer],
                MachineCallAllocation::Inherit,
                None,
            )));
        } else {
            self.cleanup_partial_shape(data, pointer, &partial)?;
        }
        if let Some(end) = end {
            self.terminate(MachineTerminator::Goto(MachineBranchTarget::new(end, [])))?;
            self.current = end;
        }
        Ok(())
    }

    fn cleanup_partial_shape(
        &mut self,
        data: TypeId,
        pointer: MachineValueId,
        partial: &[Exclusion],
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
                self.cleanup_partial_array(element, length, stride, pointer, partial)?;
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
                    let child = self
                        .context
                        .layouts
                        .get(field.ty())
                        .ok_or_else(Self::invalid)?;
                    let address = self.address(MachineAddress::new(
                        field.ty(),
                        child.size(),
                        child.alignment(),
                        MachineAddressRoot::Pointer { value: pointer },
                        [MachineAddressStep::Offset(field.offset())],
                    ));
                    let child_pointer = self.value(
                        field.ty(),
                        true,
                        MachineOperationKind::AddressOf { source: address },
                    )?;
                    self.cleanup_remaining(field.ty(), child_pointer, &children)?;
                }
            }
            _ => return Err(Self::invalid()),
        }
        Ok(())
    }

    fn cleanup_partial_array(
        &mut self,
        element: TypeId,
        length: u64,
        stride: u64,
        pointer: MachineValueId,
        exclusions: &[Exclusion],
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
        self.cleanup_remaining(element, child, &children)?;
        self.end_array(traversal)
    }
}
