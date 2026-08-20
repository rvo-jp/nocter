use nocter_machine::{
    MachineAddressId, MachineAddressRoot, MachineAddressStep, MachineFunction, MachineIndex,
    MachineIndexBound, MachineValueId,
};

use crate::{
    Arm64FunctionFrame, Arm64SelectedInstruction, Arm64SelectedMemoryAddress,
    Arm64SelectedRegister, Arm64SelectedStackAddress, Arm64SelectionError, Arm64ValuePlan,
    Arm64ValueStorage,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedAddressRoot {
    Stack(Arm64SelectedStackAddress),
    Pointer(Arm64SelectedRegister),
    View {
        pointer: Arm64SelectedRegister,
        length: Arm64SelectedRegister,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedIndex {
    Constant(u64),
    Register(Arm64SelectedRegister),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedIndexBound {
    Fixed(u64),
    CurrentView,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64SelectedAddressStep {
    Offset(u64),
    Dereference,
    ViewDereference {
        pointer_offset: u64,
        length_offset: u64,
    },
    Index {
        index: Arm64SelectedIndex,
        stride: u64,
        bound: Arm64SelectedIndexBound,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64SelectedAddressCalculation {
    root: Arm64SelectedAddressRoot,
    steps: Box<[Arm64SelectedAddressStep]>,
}

impl Arm64SelectedAddressCalculation {
    #[must_use]
    pub const fn root(&self) -> Arm64SelectedAddressRoot {
        self.root
    }

    #[must_use]
    pub const fn steps(&self) -> &[Arm64SelectedAddressStep] {
        &self.steps
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Arm64SelectedAddress {
    Stack(Arm64SelectedStackAddress),
    Runtime(Arm64SelectedAddressCalculation),
}

/// Dense target-selected address plan for every checked machine address in one function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64SelectedAddressPlan {
    addresses: Box<[Arm64SelectedAddress]>,
}

impl Arm64SelectedAddressPlan {
    /// Normalizes static paths to checked frame addresses and runtime paths to selected registers
    /// and representation-owned projection steps.
    pub(crate) fn build(
        function: &MachineFunction,
        values: &Arm64ValuePlan,
        frame: &Arm64FunctionFrame,
    ) -> Result<Self, Arm64SelectionError> {
        let mut addresses = Vec::with_capacity(function.body().addresses().len());
        for (address_id, address) in function.body().addresses() {
            if address_id.index() != addresses.len() {
                return Err(Arm64SelectionError::NonDenseAddress(address_id));
            }
            addresses.push(select_address(address, values, frame)?);
        }
        Ok(Self {
            addresses: addresses.into_boxed_slice(),
        })
    }

    pub(crate) fn use_address(
        &self,
        address: MachineAddressId,
        selected: &mut Vec<Arm64SelectedInstruction>,
    ) -> Result<Arm64SelectedMemoryAddress, Arm64SelectionError> {
        match self
            .addresses
            .get(address.index())
            .ok_or(Arm64SelectionError::UnknownAddress(address))?
        {
            Arm64SelectedAddress::Stack(address) => Ok(Arm64SelectedMemoryAddress::Stack(*address)),
            Arm64SelectedAddress::Runtime(_) => {
                selected.push(Arm64SelectedInstruction::ResolveAddress(address));
                Ok(Arm64SelectedMemoryAddress::Register {
                    base: Arm64SelectedRegister::Fixed(runtime_address_register()),
                    offset: 0,
                })
            }
        }
    }

    pub(crate) fn calculation(
        &self,
        address: MachineAddressId,
    ) -> Option<&Arm64SelectedAddressCalculation> {
        match self.addresses.get(address.index())? {
            Arm64SelectedAddress::Stack(_) => None,
            Arm64SelectedAddress::Runtime(calculation) => Some(calculation),
        }
    }
}

fn select_address(
    address: &nocter_machine::MachineAddress,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
) -> Result<Arm64SelectedAddress, Arm64SelectionError> {
    if let Some(address) = select_static_stack(address, frame)? {
        return Ok(Arm64SelectedAddress::Stack(address));
    }
    let (root, mut current_view) = select_root(address.root(), values, frame)?;
    let mut steps = Vec::with_capacity(address.steps().len());
    for step in address.steps() {
        let selected = match *step {
            MachineAddressStep::Offset(offset) => Arm64SelectedAddressStep::Offset(offset),
            MachineAddressStep::Dereference => {
                current_view = false;
                Arm64SelectedAddressStep::Dereference
            }
            MachineAddressStep::ViewDereference {
                pointer_offset,
                length_offset,
            } => {
                current_view = true;
                Arm64SelectedAddressStep::ViewDereference {
                    pointer_offset,
                    length_offset,
                }
            }
            MachineAddressStep::Index {
                index,
                stride,
                bound,
            } => {
                let bound = match bound {
                    MachineIndexBound::Fixed(length) => Arm64SelectedIndexBound::Fixed(length),
                    MachineIndexBound::CurrentView if current_view => {
                        Arm64SelectedIndexBound::CurrentView
                    }
                    MachineIndexBound::CurrentView => {
                        return Err(Arm64SelectionError::ProjectedAddress);
                    }
                };
                current_view = false;
                Arm64SelectedAddressStep::Index {
                    index: match index {
                        MachineIndex::Constant(index) => Arm64SelectedIndex::Constant(index),
                        MachineIndex::Value(value) => {
                            Arm64SelectedIndex::Register(one_word(values, value)?)
                        }
                    },
                    stride,
                    bound,
                }
            }
        };
        steps.push(selected);
    }
    if current_view {
        return Err(Arm64SelectionError::ProjectedAddress);
    }
    Ok(Arm64SelectedAddress::Runtime(
        Arm64SelectedAddressCalculation {
            root,
            steps: steps.into_boxed_slice(),
        },
    ))
}

fn select_static_stack(
    address: &nocter_machine::MachineAddress,
    frame: &Arm64FunctionFrame,
) -> Result<Option<Arm64SelectedStackAddress>, Arm64SelectionError> {
    let MachineAddressRoot::Stack(stack) = address.root() else {
        return Ok(None);
    };
    let mut offset = 0_u64;
    for step in address.steps() {
        let MachineAddressStep::Offset(additional) = step else {
            return Ok(None);
        };
        offset = offset
            .checked_add(*additional)
            .ok_or(Arm64SelectionError::AddressOverflow)?;
    }
    Ok(Some(crate::memory_selection::frame_stack(
        frame, stack, offset,
    )?))
}

fn select_root(
    root: MachineAddressRoot,
    values: &Arm64ValuePlan,
    frame: &Arm64FunctionFrame,
) -> Result<(Arm64SelectedAddressRoot, bool), Arm64SelectionError> {
    match root {
        MachineAddressRoot::Stack(stack) => Ok((
            Arm64SelectedAddressRoot::Stack(crate::memory_selection::frame_stack(frame, stack, 0)?),
            false,
        )),
        MachineAddressRoot::Pointer { value } => Ok((
            Arm64SelectedAddressRoot::Pointer(one_word(values, value)?),
            false,
        )),
        MachineAddressRoot::View {
            value,
            pointer_offset,
            length_offset,
        } => {
            let registers = direct_registers(values, value)?;
            let pointer =
                registers[crate::memory_selection::direct_lane(pointer_offset, registers.len())?];
            let length =
                registers[crate::memory_selection::direct_lane(length_offset, registers.len())?];
            Ok((
                Arm64SelectedAddressRoot::View {
                    pointer: Arm64SelectedRegister::Virtual(pointer),
                    length: Arm64SelectedRegister::Virtual(length),
                },
                true,
            ))
        }
    }
}

fn one_word(
    values: &Arm64ValuePlan,
    value: MachineValueId,
) -> Result<Arm64SelectedRegister, Arm64SelectionError> {
    match direct_registers(values, value)? {
        [register] => Ok(Arm64SelectedRegister::Virtual(*register)),
        _ => Err(Arm64SelectionError::ExpectedOneWord(value)),
    }
}

fn direct_registers(
    values: &Arm64ValuePlan,
    value: MachineValueId,
) -> Result<&[crate::Arm64VirtualRegister], Arm64SelectionError> {
    match values
        .value(value)
        .ok_or(Arm64SelectionError::UnknownValue(value))?
    {
        Arm64ValueStorage::Direct(registers) => Ok(registers),
        Arm64ValueStorage::Omitted | Arm64ValueStorage::Memory { .. } => {
            Err(Arm64SelectionError::MemoryValue(value))
        }
    }
}

pub(crate) fn runtime_address_register() -> crate::Arm64Register {
    crate::Arm64NocterAbi::argument_register(0)
        .expect("the ABI reserves x0 as the runtime-address boundary register")
}
