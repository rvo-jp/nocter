//! A homogeneous fill is a finite Machine CFG, not N generated writes or resource owners.
use super::{Builder, Failure};
use crate::{
    MachineAddress, MachineAddressRoot, MachineAddressStep, MachineArithmeticCheck,
    MachineBinaryOperation, MachineBranchTarget, MachineConstant, MachineLayoutKind,
    MachineOperationKind, MachineTerminator, MachineValueId,
};
use nocter_mir::value_access as mir;
use nocter_model::TypeId;
use nocter_runtime_contract::RuntimePrimitive;

impl Builder<'_> {
    fn size_constant(&mut self, value: u64) -> Result<MachineValueId, Failure> {
        let data = self
            .context
            .program
            .runtime()
            .types()
            .primitive(RuntimePrimitive::Usize)
            .ok_or_else(Self::invalid)?;
        self.value(
            data,
            false,
            MachineOperationKind::Constant(MachineConstant::Integer(i128::from(value))),
        )
    }
    pub(super) fn repeat(
        &mut self,
        source: &mir::Place,
        destination: &mir::Place,
    ) -> Result<(), Failure> {
        let layout = self
            .context
            .layouts
            .get(destination.data)
            .ok_or_else(Self::invalid)?;
        if layout.size() == 0 {
            return Ok(());
        }
        let MachineLayoutKind::FixedArray {
            element,
            length,
            stride,
        } = layout.kind()
        else {
            return Err(Self::invalid());
        };
        if *element != source.data || *length <= 1 {
            return Err(Self::invalid());
        }
        let (length, stride) = (*length, *stride);
        let payload = self.load(source)?;
        let base = self.place(destination)?;
        let pointer = self.value(
            destination.data,
            true,
            MachineOperationKind::AddressOf { source: base },
        )?;
        self.fill(source.data, payload, pointer, length, stride)
    }
    fn fill(
        &mut self,
        data: TypeId,
        payload: MachineValueId,
        pointer: MachineValueId,
        length: u64,
        stride: u64,
    ) -> Result<(), Failure> {
        let limit = self.size_constant(length)?;
        let zero = self.size_constant(0)?;
        let one = self.size_constant(1)?;
        let stride = self.size_constant(stride)?;
        let sizes = self
            .context
            .program
            .runtime()
            .types()
            .primitive(RuntimePrimitive::Usize)
            .ok_or_else(Self::invalid)?;
        let boolean = self
            .context
            .program
            .runtime()
            .types()
            .primitive(RuntimePrimitive::Bool)
            .ok_or_else(Self::invalid)?;
        let header = self.block();
        let body = self.block();
        let exit = self.block();
        let index = self.parameter(header, sizes)?;
        self.terminate(MachineTerminator::Goto(MachineBranchTarget::new(
            header,
            [zero],
        )))?;
        self.current = header;
        let condition = self.value(
            boolean,
            false,
            MachineOperationKind::Binary {
                operation: MachineBinaryOperation::Less,
                left: index,
                right: limit,
                check: MachineArithmeticCheck::NotRequired,
            },
        )?;
        self.terminate(MachineTerminator::Branch {
            condition,
            then_target: MachineBranchTarget::new(body, []),
            else_target: MachineBranchTarget::new(exit, []),
        })?;
        self.current = body;
        let offset = self.value(
            sizes,
            false,
            MachineOperationKind::Binary {
                operation: MachineBinaryOperation::Multiply,
                left: index,
                right: stride,
                check: MachineArithmeticCheck::InternalInvariant,
            },
        )?;
        let element = self.context.layouts.get(data).ok_or_else(Self::invalid)?;
        let address = self.address(MachineAddress::new(
            data,
            element.size(),
            element.alignment(),
            MachineAddressRoot::Pointer { value: pointer },
            [MachineAddressStep::OffsetValue(offset)],
        ));
        self.emit(MachineOperationKind::Store {
            destination: address,
            value: payload,
        });
        let next = self.value(
            sizes,
            false,
            MachineOperationKind::Binary {
                operation: MachineBinaryOperation::Add,
                left: index,
                right: one,
                check: MachineArithmeticCheck::InternalInvariant,
            },
        )?;
        self.terminate(MachineTerminator::Goto(MachineBranchTarget::new(
            header,
            [next],
        )))?;
        self.current = exit;
        Ok(())
    }
}
