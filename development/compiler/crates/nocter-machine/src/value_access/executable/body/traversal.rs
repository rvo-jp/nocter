//! One homogeneous countdown traversal. The callback body selects fill or complete-element cleanup.
use super::{Builder, Failure};
use crate::{
    MachineAddress, MachineAddressId, MachineAddressRoot, MachineAddressStep,
    MachineArithmeticCheck, MachineBinaryOperation, MachineBlockId, MachineBranchTarget,
    MachineConstant, MachineOperationKind, MachineTerminator, MachineValueId,
};
use nocter_model::TypeId;
use nocter_runtime_contract::RuntimePrimitive;

pub(super) struct Traversal {
    pub(super) index: MachineValueId,
    continuation: MachineBranchTarget,
    exit: MachineBlockId,
}
impl Builder<'_> {
    pub(super) fn primitive(&self, role: RuntimePrimitive) -> Result<TypeId, Failure> {
        self.context
            .program
            .runtime()
            .types()
            .primitive(role)
            .ok_or_else(Self::invalid)
    }
    pub(super) fn size_constant(&mut self, value: u64) -> Result<MachineValueId, Failure> {
        self.value(
            self.primitive(RuntimePrimitive::Usize)?,
            false,
            MachineOperationKind::Constant(MachineConstant::Integer(i128::from(value))),
        )
    }
    pub(super) fn size_binary(
        &mut self,
        operation: MachineBinaryOperation,
        left: MachineValueId,
        right: MachineValueId,
    ) -> Result<MachineValueId, Failure> {
        self.value(
            self.primitive(RuntimePrimitive::Usize)?,
            false,
            MachineOperationKind::Binary {
                operation,
                left,
                right,
                check: MachineArithmeticCheck::InternalInvariant,
            },
        )
    }
    pub(super) fn begin_array(&mut self, length: u64) -> Result<Traversal, Failure> {
        let start = self.size_constant(0)?;
        let end = self.size_constant(length)?;
        self.begin_array_range(start, end)
    }
    pub(super) fn begin_array_range(
        &mut self,
        lower: MachineValueId,
        limit: MachineValueId,
    ) -> Result<Traversal, Failure> {
        let one = self.size_constant(1)?;
        let header = self.block();
        let body = self.block();
        let exit = self.block();
        let remaining = self.parameter(header, self.primitive(RuntimePrimitive::Usize)?)?;
        self.terminate(MachineTerminator::Goto(MachineBranchTarget::new(
            header,
            [limit],
        )))?;
        self.current = header;
        let condition = self.value(
            self.primitive(RuntimePrimitive::Bool)?,
            false,
            MachineOperationKind::Binary {
                operation: MachineBinaryOperation::Less,
                left: lower,
                right: remaining,
                check: MachineArithmeticCheck::NotRequired,
            },
        )?;
        self.terminate(MachineTerminator::Branch {
            condition,
            then_target: MachineBranchTarget::new(body, []),
            else_target: MachineBranchTarget::new(exit, []),
        })?;
        self.current = body;
        let index = self.size_binary(MachineBinaryOperation::Subtract, remaining, one)?;
        Ok(Traversal {
            index,
            continuation: MachineBranchTarget::new(header, [index]),
            exit,
        })
    }
    pub(super) fn end_array(&mut self, traversal: Traversal) -> Result<(), Failure> {
        self.terminate(MachineTerminator::Goto(traversal.continuation))?;
        self.current = traversal.exit;
        Ok(())
    }
    pub(super) fn array_address(
        &mut self,
        data: TypeId,
        pointer: MachineValueId,
        index: MachineValueId,
        stride: u64,
    ) -> Result<MachineAddressId, Failure> {
        let stride = self.size_constant(stride)?;
        let offset = self.size_binary(MachineBinaryOperation::Multiply, index, stride)?;
        let element = self.context.layouts.get(data).ok_or_else(Self::invalid)?;
        Ok(self.address(MachineAddress::new(
            data,
            element.size(),
            element.alignment(),
            MachineAddressRoot::Pointer { value: pointer },
            [MachineAddressStep::OffsetValue(offset)],
        )))
    }
}
