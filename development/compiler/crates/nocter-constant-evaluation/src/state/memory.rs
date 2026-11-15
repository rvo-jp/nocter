mod ownership;
use super::slot_types::SlotTypes;
use super::{
    Missing, Place, Projection, ProjectionPath, Rule, Slot, Storage as SlotStorage, Value,
    budget::{Lease, Resident, Storage},
};
pub(super) use ownership::Hole;
use ownership::Ownership;
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
    pub(super) fn extended(&self, storage: &Storage, tail: &[usize]) -> Result<Self> {
        self.extended_with(storage, tail.iter().copied().map(Ok))
    }
    fn extended_with(
        &self,
        storage: &Storage,
        tail: impl ExactSizeIterator<Item = Result<usize>>,
    ) -> Result<Self> {
        if tail.len() == 0 {
            return Ok(self.clone());
        }
        let length = self
            .len()
            .checked_add(tail.len())
            .ok_or(Rule::StorageLimit)?;
        let allocation = storage.items(length)?;
        let fields = self
            .iter()
            .copied()
            .map(Ok)
            .chain(tail)
            .collect::<Result<Box<[_]>>>()?;
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
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Root {
    Local { frame: usize, slot: usize },
    Static(nocter_model::ValueStatic),
}
#[derive(Clone, Eq, PartialEq)]
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
    contexts: BTreeMap<usize, Resident<usize>>,
    pub cells: BTreeMap<usize, Resident<Cell>>,
    pub owned: BTreeMap<usize, Resident<Ownership>>,
}
impl Frame {
    pub fn set_context(&mut self, storage: &Storage, slot: usize, data: usize) -> Result<()> {
        let initial = self.slots.get(slot).ok_or(Rule::InvalidPlan)?;
        if initial.storage == SlotStorage::Auxiliary {
            return Err(Rule::InvalidPlan);
        }
        if initial.data == data {
            self.contexts.remove(&slot);
        } else {
            match self.contexts.entry(slot) {
                Entry::Occupied(mut entry) => entry.get_mut().value = data,
                Entry::Vacant(entry) => {
                    entry.insert(storage.entry(data)?);
                }
            }
        }
        Ok(())
    }
    pub fn new(storage: &Storage, slots: Arc<[Slot]>) -> Result<Resident<Self>> {
        storage.entry(Self {
            slots,
            contexts: BTreeMap::new(),
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
        entry.insert(storage.entry(Ownership::default())?);
        Ok(())
    }
}
impl super::slot_types::SlotTypes for Frame {
    fn get(&self, slot: usize) -> Option<Slot> {
        let mut definition = *self.slots.get(slot)?;
        if let Some(data) = self.contexts.get(&slot) {
            definition.data = data.value;
        }
        Some(definition)
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
    /// Read one actual retained owner, without following a user alias or re-evaluating an index.
    pub fn snapshot_index(&self, frame: usize, slot: usize, bound: u64) -> Result<u64> {
        let frame = self.frames.get(frame).ok_or(Rule::InvalidPlan)?;
        if frame
            .slots
            .get(slot)
            .is_none_or(|slot| slot.storage != SlotStorage::Owned)
            || !frame.owned.get(&slot).is_some_and(|owner| owner.is_empty())
        {
            return Err(Rule::InvalidPlan);
        }
        let Some(Cell::Data(value)) = frame.cells.get(&slot).map(|entry| &entry.value) else {
            return Err(Rule::InvalidPlan);
        };
        let super::Data::Integer(value) = value.data() else {
            return Err(Rule::InvalidPlan);
        };
        let index = u64::try_from(*value).map_err(|_| Rule::InvalidPlan)?;
        if index >= bound {
            return Err(Rule::InvalidPlan);
        }
        Ok(index)
    }
    fn projected(&self, frame: usize, base: &Path, fields: &ProjectionPath) -> Result<Path> {
        base.extended_with(
            &self.storage,
            fields.iter().map(|projection| match *projection {
                Projection::Field(field) => Ok(field),
                Projection::Index { slot, bound } => {
                    usize::try_from(self.snapshot_index(frame, slot, bound)?)
                        .map_err(|_| Rule::StorageLimit)
                }
            }),
        )
    }
    pub fn resolve_path(&self, frame: usize, fields: &ProjectionPath) -> Result<Path> {
        self.projected(frame, &Path::default(), fields)
    }
    pub fn resolve_missing(&self, frame: usize, missing: &Missing) -> Result<Hole> {
        Ok(match missing {
            Missing::Value(path) => Hole::Value(self.resolve_path(frame, path)?),
            Missing::ArrayPrefix {
                base,
                endpoint,
                bound,
            } => {
                let index = self.snapshot_index(frame, *endpoint, *bound)?;
                let range = nocter_model::ArrayRange::Through(())
                    .resolve(*bound, |()| Some(index))
                    .ok_or(Rule::InvalidPlan)?;
                Hole::Array {
                    base: self.resolve_path(frame, base)?,
                    start: range.start,
                    end: range.end,
                }
            }
        })
    }
    /// Alias cells store canonical roots at definition time; no chain or cycle can be created.
    pub fn address(
        &self,
        program: &super::Program,
        frame: usize,
        place: &Place,
    ) -> Result<Address> {
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
                let fields = self.projected(frame, &base.fields, &place.fields)?;
                let expected = current.get(place.slot).ok_or(Rule::InvalidPlan)?.data;
                let actual = match base.root {
                    Root::Local { frame, slot } => {
                        self.frames
                            .get(frame)
                            .and_then(|frame| frame.get(slot))
                            .ok_or(Rule::InvalidPlan)?
                            .data
                    }
                    Root::Static(id) => program.statics.get(&id).ok_or(Rule::InvalidPlan)?.data,
                };
                if type_path(program, expected, &fields[base.fields.len()..])?
                    != type_path(program, actual, &fields)?
                {
                    return Err(Rule::InvalidPlan);
                }
                Ok(Address {
                    root: base.root,
                    fields,
                })
            }
            Some(Cell::Alias(_)) => Err(Rule::InvalidPlan),
            _ if role == SlotStorage::Alias => Err(Rule::InvalidPlan),
            _ => Ok(Address {
                root: Root::Local {
                    frame,
                    slot: place.slot,
                },
                fields: self.resolve_path(frame, &place.fields)?,
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
            value = value
                .data()
                .component(field)
                .ok_or(Rule::InvalidPlan)?
                .clone();
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
            root = root.component_mut(&self.storage, field)?;
        }
        *root.component_mut(
            &self.storage,
            *address.fields.last().ok_or(Rule::InvalidPlan)?,
        )? = value;
        Ok(())
    }
    pub fn construct(&mut self, address: &Address) -> Result<()> {
        let (frame_id, slot) = address.local()?;
        let frame = self.frames.get_mut(frame_id).ok_or(Rule::InvalidPlan)?;
        if address.fields.is_empty() {
            frame.own(&self.storage, slot)
        } else {
            let holes = frame.owned.get_mut(&slot).ok_or(Rule::InvalidPlan)?;
            holes.restore(&address.fields)?;
            Ok(())
        }
    }
    pub fn retire_array(&mut self, address: &Address, start: u64, end: u64) -> Result<()> {
        let (frame_id, slot) = address.local()?;
        self.frames
            .get_mut(frame_id)
            .and_then(|frame| frame.owned.get_mut(&slot))
            .ok_or(Rule::InvalidPlan)?
            .retire_array(&self.storage, &address.fields, start, end)
    }
    pub fn consolidate_prefix(&mut self, address: &Address, end: u64) -> Result<()> {
        let (frame_id, slot) = address.local()?;
        let index = usize::try_from(end.checked_sub(1).ok_or(Rule::InvalidPlan)?)
            .map_err(|_| Rule::StorageLimit)?;
        let selected = address.fields.extended(&self.storage, &[index])?;
        self.frames
            .get_mut(frame_id)
            .and_then(|frame| frame.owned.get_mut(&slot))
            .ok_or(Rule::InvalidPlan)?
            .consolidate_prefix(&self.storage, &address.fields, &selected, end)
    }
    pub fn require_array_element(
        &self,
        array: &Address,
        selected: &Address,
        length: u64,
    ) -> Result<()> {
        let (frame, slot) = array.local()?;
        if selected.local()? != (frame, slot)
            || selected.fields.len()
                != array.fields.len().checked_add(1).ok_or(Rule::InvalidPlan)?
            || !selected.fields.starts_with(&array.fields)
            || selected
                .fields
                .last()
                .and_then(|&index| u64::try_from(index).ok())
                .is_none_or(|index| index >= length)
            || self
                .frames
                .get(frame)
                .and_then(|frame| frame.owned.get(&slot))
                .is_none_or(|owner| !owner.can_release(&array.fields, true))
        {
            return Err(Rule::InvalidPlan);
        }
        Ok(())
    }
    pub fn retire(&mut self, address: &Address, partial: bool) -> Result<()> {
        let (frame_id, slot) = address.local()?;
        let frame = self.frames.get_mut(frame_id).ok_or(Rule::InvalidPlan)?;
        let holes = frame.owned.get_mut(&slot).ok_or(Rule::InvalidPlan)?;
        if address.fields.is_empty() {
            if !holes.can_release(&address.fields, partial) {
                return Err(Rule::InvalidPlan);
            }
            frame.owned.remove(&slot);
        } else {
            holes.retire(&self.storage, &address.fields, partial)?;
        }
        Ok(())
    }
}
fn type_path(program: &super::Program, mut data: usize, fields: &[usize]) -> Result<usize> {
    for &field in fields {
        data = program
            .types
            .get(data)
            .and_then(|ty| ty.component(field))
            .ok_or(Rule::InvalidPlan)?;
    }
    Ok(data)
}
