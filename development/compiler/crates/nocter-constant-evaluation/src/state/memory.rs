use super::{
    Data, Place, Rule, Slot, Storage as SlotStorage, Value,
    budget::{Lease, Resident, Storage},
};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, btree_map::Entry},
    ops::Deref,
    sync::Arc,
};
struct PathData {
    fields: Box<[usize]>,
    _allocation: Lease,
}
#[derive(Clone, Default)]
pub(super) struct Path(Option<Arc<PathData>>);
impl Path {
    fn extended(&self, storage: &Storage, tail: &[usize]) -> Result<Self> {
        if tail.is_empty() {
            return Ok(self.clone());
        }
        let length = self
            .len()
            .checked_add(tail.len())
            .ok_or(Rule::StorageLimit)?;
        let allocation = storage.items(length)?;
        let fields = self.iter().chain(tail).copied().collect();
        Ok(Self(Some(Arc::new(PathData {
            fields,
            _allocation: allocation,
        }))))
    }
}
impl Deref for Path {
    type Target = [usize];
    fn deref(&self) -> &[usize] {
        self.0.as_ref().map_or(&[], |p| p.fields.as_ref())
    }
}
impl PartialEq for Path {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}
impl Eq for Path {}
impl PartialOrd for Path {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Path {
    fn cmp(&self, other: &Self) -> Ordering {
        (**self).cmp(&**other)
    }
}
#[derive(Clone, Copy)]
pub(super) enum Root {
    Local { frame: usize, slot: usize },
    Static(nocter_model::ValueStatic),
}
#[derive(Clone)]
pub(super) struct Address {
    pub root: Root,
    pub fields: Path,
}
#[derive(Clone)]
pub(super) enum Cell {
    Data(Value),
    Alias(Address),
}
pub(super) struct Frame {
    pub slots: Arc<[Slot]>,
    pub cells: BTreeMap<usize, Resident<Cell>>,
    pub owned: BTreeMap<usize, Resident<BTreeMap<Path, Resident<()>>>>,
}
impl Frame {
    pub fn new(storage: &Storage, slots: Arc<[Slot]>) -> Result<Resident<Self>> {
        storage.entry(Self {
            slots,
            cells: BTreeMap::new(),
            owned: BTreeMap::new(),
        })
    }
    pub fn set(&mut self, storage: &Storage, slot: usize, cell: Cell) -> Result<()> {
        let role = self.slots.get(slot).ok_or(Rule::InvalidPlan)?.storage;
        match &cell {
            Cell::Data(value) if role != SlotStorage::Alias && value.belongs_to(storage) => {}
            Cell::Alias(_) if role == SlotStorage::Alias => {}
            _ => return Err(Rule::InvalidPlan),
        }
        match self.cells.entry(slot) {
            Entry::Occupied(mut entry) => entry.get_mut().value = cell,
            Entry::Vacant(entry) => {
                entry.insert(storage.entry(cell)?);
            }
        }
        Ok(())
    }
    pub fn own(&mut self, storage: &Storage, slot: usize) -> Result<()> {
        let Entry::Vacant(entry) = self.owned.entry(slot) else {
            return Err(Rule::InvalidPlan);
        };
        entry.insert(storage.entry(BTreeMap::new())?);
        Ok(())
    }
}
impl Address {
    pub fn local(&self) -> Result<(usize, usize)> {
        match self.root {
            Root::Local { frame, slot } => Ok((frame, slot)),
            Root::Static(_) => Err(Rule::InvalidPlan),
        }
    }
}
pub(super) struct Memory {
    globals: BTreeMap<nocter_model::ValueStatic, Resident<Value>>,
    pub frames: Vec<Resident<Frame>>,
    pub storage: Storage,
}
type Result<T> = std::result::Result<T, Rule>;
impl Memory {
    pub fn new(storage: Storage) -> Self {
        Self {
            globals: BTreeMap::new(),
            frames: Vec::new(),
            storage,
        }
    }
    pub fn bind_static(
        &mut self,
        frame: &mut Frame,
        slot: usize,
        id: nocter_model::ValueStatic,
        source: &Value,
    ) -> Result<()> {
        if !self.globals.contains_key(&id) {
            let value = self.storage.import(source)?;
            self.globals.insert(id, self.storage.entry(value)?);
        }
        frame.set(
            &self.storage,
            slot,
            Cell::Alias(Address {
                root: Root::Static(id),
                fields: Path::default(),
            }),
        )
    }
    /// Alias cells store canonical roots at definition time; no chain or cycle can be created.
    pub fn address(&self, frame: usize, place: &Place) -> Result<Address> {
        let current = self.frames.get(frame).ok_or(Rule::InvalidPlan)?;
        let role = current
            .slots
            .get(place.slot)
            .ok_or(Rule::InvalidPlan)?
            .storage;
        match current.cells.get(&place.slot).map(|entry| &entry.value) {
            Some(Cell::Alias(base)) if role == SlotStorage::Alias => {
                if let Root::Local { frame, slot } = base.root {
                    let root = self
                        .frames
                        .get(frame)
                        .and_then(|frame| frame.slots.get(slot))
                        .ok_or(Rule::InvalidPlan)?;
                    if root.storage == SlotStorage::Alias {
                        return Err(Rule::InvalidPlan);
                    }
                }
                Ok(Address {
                    root: base.root,
                    fields: base.fields.extended(&self.storage, &place.fields)?,
                })
            }
            Some(Cell::Alias(_)) => Err(Rule::InvalidPlan),
            _ if role == SlotStorage::Alias => Err(Rule::InvalidPlan),
            _ => Ok(Address {
                root: Root::Local {
                    frame,
                    slot: place.slot,
                },
                fields: Path::default().extended(&self.storage, &place.fields)?,
            }),
        }
    }
    pub fn read(&self, address: &Address) -> Result<Value> {
        let mut value = match address.root {
            Root::Static(id) => self
                .globals
                .get(&id)
                .ok_or(Rule::InvalidPlan)?
                .value
                .clone(),
            Root::Local { frame, slot } => {
                let Some(Cell::Data(value)) = self
                    .frames
                    .get(frame)
                    .and_then(|frame| frame.cells.get(&slot))
                    .map(|entry| &entry.value)
                else {
                    return Err(Rule::InvalidPlan);
                };
                value.clone()
            }
        };
        for &field in address.fields.iter() {
            let Data::Record(values) = value.data() else {
                return Err(Rule::InvalidPlan);
            };
            value = values.get(field).ok_or(Rule::InvalidPlan)?.clone();
        }
        Ok(value)
    }
    pub fn store(&mut self, address: &Address, value: Value) -> Result<()> {
        let (frame_id, slot) = address.local()?;
        if !value.belongs_to(&self.storage) {
            return Err(Rule::InvalidPlan);
        }
        let frame = self.frames.get_mut(frame_id).ok_or(Rule::InvalidPlan)?;
        if address.fields.is_empty() {
            return frame.set(&self.storage, slot, Cell::Data(value));
        }
        let Some(Cell::Data(root)) = frame.cells.get_mut(&slot).map(|entry| &mut entry.value)
        else {
            return Err(Rule::InvalidPlan);
        };
        let mut root = root;
        for &field in &address.fields[..address.fields.len() - 1] {
            let Data::Record(values) = root.make_mut(&self.storage)? else {
                return Err(Rule::InvalidPlan);
            };
            root = values.get_mut(field).ok_or(Rule::InvalidPlan)?;
        }
        let Data::Record(values) = root.make_mut(&self.storage)? else {
            return Err(Rule::InvalidPlan);
        };
        *values
            .get_mut(*address.fields.last().ok_or(Rule::InvalidPlan)?)
            .ok_or(Rule::InvalidPlan)? = value;
        Ok(())
    }
    pub fn construct(&mut self, address: &Address) -> Result<()> {
        let (frame_id, slot) = address.local()?;
        let frame = self.frames.get_mut(frame_id).ok_or(Rule::InvalidPlan)?;
        if address.fields.is_empty() {
            frame.own(&self.storage, slot)
        } else {
            let holes = frame.owned.get_mut(&slot).ok_or(Rule::InvalidPlan)?;
            if holes
                .keys()
                .any(|hole| hole != &address.fields && address.fields.starts_with(hole))
            {
                return Err(Rule::InvalidPlan);
            }
            holes.retain(|hole, _| !hole.starts_with(&address.fields));
            Ok(())
        }
    }
    pub fn retire(&mut self, address: &Address, partial: bool) -> Result<()> {
        let (frame_id, slot) = address.local()?;
        let frame = self.frames.get_mut(frame_id).ok_or(Rule::InvalidPlan)?;
        let holes = frame.owned.get_mut(&slot).ok_or(Rule::InvalidPlan)?;
        if holes
            .keys()
            .any(|h| address.fields.starts_with(h) || (!partial && h.starts_with(&address.fields)))
        {
            return Err(Rule::InvalidPlan);
        }
        if address.fields.is_empty() {
            frame.owned.remove(&slot);
        } else {
            holes.retain(|h, _| !h.starts_with(&address.fields));
            holes.insert(address.fields.clone(), self.storage.entry(())?);
        }
        Ok(())
    }
}
