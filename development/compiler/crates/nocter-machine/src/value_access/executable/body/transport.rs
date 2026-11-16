//! Guarded physical movement uses separate layouts and schema-shared helper bodies.
use super::super::transport::{Alternative, CallableCase, Field, Helper, Pair, Plan};
use super::{Builder, Context, Failure};
use crate::identity::MachineId;
use crate::{
    MachineAddress, MachineAddressId, MachineAddressRoot, MachineAddressStep, MachineBlockId,
    MachineBranchTarget, MachineCall, MachineCallAllocation, MachineCallTarget, MachineFunction,
    MachineFunctionKind, MachineOperationKind, MachineStackId, MachineStackObject,
    MachineStackPurpose, MachineTerminator,
};

pub(crate) fn build(
    context: &Context<'_>,
    pair: Pair,
    helper: &Helper,
) -> Result<MachineFunction, Failure> {
    let mut builder = Builder::new(context, helper.function);
    let mut roots = Vec::new();
    for (position, data) in [pair.0, pair.1].into_iter().enumerate() {
        let stack = MachineStackId::new(position);
        builder.stack.push(MachineStackObject::address(
            data,
            context.layouts.target(),
            MachineStackPurpose::Parameter { position },
        ));
        builder.parameters.push(stack);
        let layout = context.layouts.get(data).ok_or(Failure::InvalidRoot)?;
        roots.push(builder.address(MachineAddress::new(
            data,
            layout.size(),
            layout.alignment(),
            MachineAddressRoot::Stack(stack),
            [MachineAddressStep::Dereference],
        )));
    }
    let (source, target) = (roots[0], roots[1]);
    match &helper.plan {
        Plan::Callable {
            source_choice,
            cases,
        } => {
            builder.transport_callable(source, target, pair, *source_choice, cases)?;
        }
        Plan::Outcome { primary, alternate } => {
            let condition = builder.primary_condition(source, pair.0)?;
            let present = builder.block();
            let absent = builder.block();
            let done = builder.block();
            builder.terminate(MachineTerminator::Branch {
                condition,
                then_target: MachineBranchTarget::new(present, []),
                else_target: MachineBranchTarget::new(absent, []),
            })?;
            for (block, is_primary, alternative) in
                [(present, true, primary), (absent, false, alternate)]
            {
                builder.current = block;
                if matches!(alternative, Alternative::Excluded) {
                    builder.terminate(MachineTerminator::Unreachable)?;
                    continue;
                }
                if let Alternative::Payload(field) = alternative {
                    builder.transport_field(source, target, field)?;
                }
                builder.store_outcome_tag(target, pair.1, is_primary)?;
                builder.go(done)?;
            }
            builder.current = done;
        }
        Plan::Aggregate(fields) => {
            for field in fields {
                builder.transport_field(source, target, field)?;
            }
        }
        Plan::Array {
            pair: element,
            length,
            source_stride,
            target_stride,
        } => {
            let left = builder.value(pair.0, true, MachineOperationKind::AddressOf { source })?;
            let right = builder.value(
                pair.1,
                true,
                MachineOperationKind::AddressOf { source: target },
            )?;
            let traversal = builder.begin_array(*length)?;
            let source = builder.array_address(element.0, left, traversal.index, *source_stride)?;
            let target =
                builder.array_address(element.1, right, traversal.index, *target_stride)?;
            builder.transport_value(*element, source, target)?;
            builder.end_array(traversal)?;
        }
    }
    builder.terminate(MachineTerminator::Return(None))?;
    let abi = crate::transport::address_signatures([pair.0, pair.1], context.layouts)
        .map_err(Failure::Abi)?;
    builder.finish(MachineFunctionKind::Callable(abi), MachineBlockId::new(0))
}

impl Builder<'_> {
    fn transport_callable(
        &mut self,
        source: MachineAddressId,
        target: MachineAddressId,
        pair: Pair,
        source_choice: bool,
        cases: &[CallableCase],
    ) -> Result<(), Failure> {
        let action = |builder: &mut Builder<'_>, index: usize| {
            let case = cases.get(index).ok_or(Failure::InvalidRoot)?;
            builder.transport_field(source, target, &case.payload)?;
            builder.store_callable_tag(target, pair.1, case.target_tag)
        };
        if source_choice {
            self.select_callable_case(source, pair.0, action)
        } else {
            if cases.len() != 1 {
                return Err(Failure::InvalidRoot);
            }
            action(self, 0)
        }
    }
    fn transport_field(
        &mut self,
        source: MachineAddressId,
        target: MachineAddressId,
        field: &Field,
    ) -> Result<(), Failure> {
        let from = self.offset_address(source, field.pair.0, field.source)?;
        let to = self.offset_address(target, field.pair.1, field.target)?;
        self.transport_value(field.pair, from, to)
    }
    pub(super) fn transport_value(
        &mut self,
        pair: Pair,
        source: MachineAddressId,
        target: MachineAddressId,
    ) -> Result<(), Failure> {
        if self.context.transports.copies.contains(&pair) {
            let from = self.context.layouts.get(pair.0).ok_or_else(Self::invalid)?;
            let to = self.context.layouts.get(pair.1).ok_or_else(Self::invalid)?;
            if from.size() != to.size() || from.alignment() != to.alignment() {
                return Err(Self::invalid());
            }
            if from.size() != 0 {
                let value = self.value(pair.0, false, MachineOperationKind::Load { source })?;
                self.emit(MachineOperationKind::Store {
                    destination: target,
                    value,
                });
            }
            return Ok(());
        }
        let helper = self
            .context
            .transports
            .helpers
            .get(&pair)
            .ok_or_else(Self::invalid)?
            .function;
        let from = self.value(pair.0, true, MachineOperationKind::AddressOf { source })?;
        let to = self.value(
            pair.1,
            true,
            MachineOperationKind::AddressOf { source: target },
        )?;
        self.emit(MachineOperationKind::Call(MachineCall::new(
            MachineCallTarget::Direct(helper),
            [from, to],
            MachineCallAllocation::Inherit,
            None,
        )));
        Ok(())
    }
}
