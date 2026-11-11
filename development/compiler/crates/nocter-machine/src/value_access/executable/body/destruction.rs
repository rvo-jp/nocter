//! Schema-selected complete cleanup helpers consume existing resource responsibility.
use super::{Builder, Context, Failure};
use crate::identity::MachineId;
use crate::{
    MachineAddress, MachineAddressRoot, MachineAddressStep, MachineBlockId, MachineCall,
    MachineCallAllocation, MachineCallTarget, MachineFunction, MachineFunctionId,
    MachineFunctionKind, MachineLayoutKind, MachineOperationKind, MachineStackId,
    MachineStackObject, MachineStackPurpose, MachineTerminator,
};
use nocter_model::TypeId;

pub(crate) fn build(
    context: &Context<'_>,
    data: TypeId,
    owner: MachineFunctionId,
) -> Result<MachineFunction, Failure> {
    let mut builder = Builder::new(context, owner);
    let target = context.layouts.target();
    builder.stack.push(MachineStackObject::address(
        data,
        target,
        MachineStackPurpose::Parameter { position: 0 },
    ));
    builder.parameters.push(MachineStackId::new(0));
    let layout = context.layouts.get(data).ok_or(Failure::InvalidRoot)?;
    match layout.kind() {
        MachineLayoutKind::ErrorHandle => {
            let place = builder.address(MachineAddress::new(
                data,
                layout.size(),
                layout.alignment(),
                MachineAddressRoot::Stack(MachineStackId::new(0)),
                [MachineAddressStep::Dereference],
            ));
            builder.emit(MachineOperationKind::ReleaseError { place });
        }
        MachineLayoutKind::Struct { fields } => {
            for field in fields.iter().rev() {
                cleanup_field(&mut builder, field.ty(), field.offset())?;
            }
        }
        MachineLayoutKind::StaticCallable {
            environment: Some(environment),
        } => cleanup_field(&mut builder, *environment, 0)?,
        MachineLayoutKind::FixedArray {
            element,
            length,
            stride,
        } => {
            builder.cleanup_array(*element, *length, *stride)?;
        }
        _ => return Err(Failure::InvalidRoot),
    }
    builder.terminate(MachineTerminator::Return(None))?;
    let abi = crate::transport::address_signature(data, context.layouts).map_err(Failure::Abi)?;
    builder.finish(MachineFunctionKind::Callable(abi), MachineBlockId::new(0))
}
fn cleanup_field(builder: &mut Builder<'_>, data: TypeId, offset: u64) -> Result<(), Failure> {
    let Some(&target) = builder.context.cleanup.get(&data) else {
        return Ok(());
    };
    let layout = builder
        .context
        .layouts
        .get(data)
        .ok_or(Failure::InvalidRoot)?;
    let place = builder.address(MachineAddress::new(
        data,
        layout.size(),
        layout.alignment(),
        MachineAddressRoot::Stack(MachineStackId::new(0)),
        [
            MachineAddressStep::Dereference,
            MachineAddressStep::Offset(offset),
        ],
    ));
    let pointer = builder.value(
        data,
        true,
        MachineOperationKind::AddressOf { source: place },
    )?;
    builder.emit(MachineOperationKind::Call(MachineCall::new(
        MachineCallTarget::Direct(target),
        [pointer],
        MachineCallAllocation::Inherit,
        None,
    )));
    Ok(())
}

impl Builder<'_> {
    pub(super) fn cleanup_array(
        &mut self,
        data: TypeId,
        length: u64,
        stride: u64,
    ) -> Result<(), Failure> {
        if length == 0 {
            return Err(Self::invalid());
        }
        let slot = MachineStackId::new(0);
        let container = self.stack.first().ok_or_else(Self::invalid)?.ty();
        let layout = self
            .context
            .layouts
            .get(container)
            .ok_or_else(Self::invalid)?;
        let base = self.address(MachineAddress::new(
            container,
            layout.size(),
            layout.alignment(),
            MachineAddressRoot::Stack(slot),
            [MachineAddressStep::Dereference],
        ));
        let pointer = self.value(
            container,
            true,
            MachineOperationKind::AddressOf { source: base },
        )?;
        self.cleanup_range(data, pointer, 0, length, stride)
    }
    pub(super) fn dispose_array_elements(
        &mut self,
        source: &nocter_mir::value_access::Place,
        start: u64,
        end: u64,
    ) -> Result<(), Failure> {
        let layout = self
            .context
            .layouts
            .get(source.data)
            .ok_or_else(Self::invalid)?;
        let MachineLayoutKind::FixedArray {
            element,
            length,
            stride,
        } = *layout.kind()
        else {
            return Err(Self::invalid());
        };
        if start >= end || end > length {
            return Err(Self::invalid());
        }
        if !self.context.cleanup.contains_key(&element) {
            return Ok(());
        }
        let place = self.place(source)?;
        let pointer = self.value(
            source.data,
            true,
            MachineOperationKind::AddressOf { source: place },
        )?;
        self.cleanup_range(element, pointer, start, end, stride)
    }
    fn cleanup_range(
        &mut self,
        data: TypeId,
        pointer: crate::MachineValueId,
        start: u64,
        end: u64,
        stride: u64,
    ) -> Result<(), Failure> {
        let target = *self.context.cleanup.get(&data).ok_or_else(Self::invalid)?;
        let traversal = self.begin_array_range(start, end)?;

        let address = self.array_address(data, pointer, traversal.index, stride)?;
        let pointer = self.value(
            data,
            true,
            MachineOperationKind::AddressOf { source: address },
        )?;
        self.emit(MachineOperationKind::Call(MachineCall::new(
            MachineCallTarget::Direct(target),
            [pointer],
            MachineCallAllocation::Inherit,
            None,
        )));
        self.end_array(traversal)
    }
}
