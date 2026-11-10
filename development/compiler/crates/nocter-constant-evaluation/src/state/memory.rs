use super::{Place, Rule, Slot, Storage, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
#[derive(Clone, Debug)]
pub(super) struct Address {
    pub frame: usize,
    pub slot: usize,
    pub fields: Vec<usize>,
}
#[derive(Clone)]
pub(super) enum Cell {
    Data(Arc<Value>),
    Alias(Address),
}
#[derive(Default)]
pub(super) struct Frame {
    pub slots: Arc<[Slot]>,
    pub cells: BTreeMap<usize, Cell>,
    pub owned: BTreeMap<usize, BTreeSet<Vec<usize>>>,
}
#[derive(Default)]
pub(super) struct Memory {
    pub frames: Vec<Frame>,
}
type Result<T> = std::result::Result<T, Rule>;
impl Memory {
    pub fn address(&self, frame: usize, place: &Place) -> Result<Address> {
        let mut address = Address {
            frame,
            slot: place.slot,
            fields: place.fields.to_vec(),
        };
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert((address.frame, address.slot)) {
                return Err(Rule::InvalidPlan);
            }
            let current = self.frames.get(address.frame).ok_or(Rule::InvalidPlan)?;
            let storage = current
                .slots
                .get(address.slot)
                .ok_or(Rule::InvalidPlan)?
                .storage;
            match current.cells.get(&address.slot) {
                Some(Cell::Alias(base)) => {
                    if storage != Storage::Alias {
                        return Err(Rule::InvalidPlan);
                    }
                    let mut fields = base.fields.clone();
                    fields.extend(address.fields);
                    address = Address {
                        frame: base.frame,
                        slot: base.slot,
                        fields,
                    };
                }
                _ if storage == Storage::Alias => return Err(Rule::InvalidPlan),
                _ => return Ok(address),
            }
        }
    }
    pub fn read(&self, address: &Address) -> Result<Arc<Value>> {
        let Some(Cell::Data(value)) = self
            .frames
            .get(address.frame)
            .and_then(|f| f.cells.get(&address.slot))
        else {
            return Err(Rule::InvalidPlan);
        };
        let mut value = value.clone();
        for &field in &address.fields {
            let Value::Record(values) = value.as_ref() else {
                return Err(Rule::InvalidPlan);
            };
            value = values.get(field).ok_or(Rule::InvalidPlan)?.clone();
        }
        Ok(value)
    }
    pub fn store(&mut self, address: &Address, value: Arc<Value>) -> Result<()> {
        let frame = self
            .frames
            .get_mut(address.frame)
            .ok_or(Rule::InvalidPlan)?;
        if address.fields.is_empty() {
            frame.cells.insert(address.slot, Cell::Data(value));
            return Ok(());
        }
        let Some(Cell::Data(root)) = frame.cells.get_mut(&address.slot) else {
            return Err(Rule::InvalidPlan);
        };
        let mut root = root;
        for &field in &address.fields[..address.fields.len() - 1] {
            let Value::Record(values) = Arc::make_mut(root) else {
                return Err(Rule::InvalidPlan);
            };
            root = values.get_mut(field).ok_or(Rule::InvalidPlan)?;
        }
        let Value::Record(values) = Arc::make_mut(root) else {
            return Err(Rule::InvalidPlan);
        };
        *values
            .get_mut(*address.fields.last().ok_or(Rule::InvalidPlan)?)
            .ok_or(Rule::InvalidPlan)? = value;
        Ok(())
    }
    pub fn construct(&mut self, address: &Address) -> Result<()> {
        let frame = self
            .frames
            .get_mut(address.frame)
            .ok_or(Rule::InvalidPlan)?;
        if address.fields.is_empty() {
            if frame.owned.insert(address.slot, BTreeSet::new()).is_some() {
                return Err(Rule::InvalidPlan);
            }
        } else {
            let holes = frame
                .owned
                .get_mut(&address.slot)
                .ok_or(Rule::InvalidPlan)?;
            if holes
                .iter()
                .any(|hole| hole != &address.fields && address.fields.starts_with(hole))
            {
                return Err(Rule::InvalidPlan);
            }
            holes.retain(|hole| !hole.starts_with(&address.fields));
        }
        Ok(())
    }
    pub fn retire(&mut self, address: &Address, partial: bool) -> Result<()> {
        let frame = self
            .frames
            .get_mut(address.frame)
            .ok_or(Rule::InvalidPlan)?;
        let holes = frame
            .owned
            .get_mut(&address.slot)
            .ok_or(Rule::InvalidPlan)?;
        if holes
            .iter()
            .any(|h| address.fields.starts_with(h) || (!partial && h.starts_with(&address.fields)))
        {
            return Err(Rule::InvalidPlan);
        }
        if address.fields.is_empty() {
            frame.owned.remove(&address.slot);
        } else {
            holes.retain(|h| !h.starts_with(&address.fields));
            holes.insert(address.fields.clone());
        }
        Ok(())
    }
}
