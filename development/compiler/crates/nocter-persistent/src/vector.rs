use std::sync::Arc;

const BRANCH_BITS: u32 = 5;
const BRANCH_FACTOR: usize = 1 << BRANCH_BITS;
const BRANCH_MASK: usize = BRANCH_FACTOR - 1;

#[derive(Debug)]
enum VectorNode<T> {
    Branch(Box<[Option<Arc<Self>>; BRANCH_FACTOR]>),
    Leaf(Box<[Option<Arc<T>>; BRANCH_FACTOR]>),
}

impl<T> VectorNode<T> {
    fn empty_leaf() -> Self {
        Self::Leaf(Box::new(std::array::from_fn(|_| None)))
    }
}

/// An append-only persistent vector with a 32-way structurally shared tree.
#[derive(Debug)]
pub struct PersistentVector<T> {
    root: Arc<VectorNode<T>>,
    shift: u32,
    len: usize,
}

impl<T> Clone for PersistentVector<T> {
    fn clone(&self) -> Self {
        Self {
            root: Arc::clone(&self.root),
            shift: self.shift,
            len: self.len,
        }
    }
}

impl<T> Default for PersistentVector<T> {
    fn default() -> Self {
        Self {
            root: Arc::new(VectorNode::empty_leaf()),
            shift: 0,
            len: 0,
        }
    }
}

impl<T> PersistentVector<T> {
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }
        let mut node = self.root.as_ref();
        let mut shift = self.shift;
        while shift > 0 {
            let VectorNode::Branch(children) = node else {
                return None;
            };
            let slot = (index >> shift) & BRANCH_MASK;
            node = children[slot].as_deref()?;
            shift -= BRANCH_BITS;
        }
        let VectorNode::Leaf(values) = node else {
            return None;
        };
        values[index & BRANCH_MASK].as_deref()
    }

    pub fn push(&mut self, value: T) {
        self.push_shared(Arc::new(value));
    }

    pub fn push_shared(&mut self, value: Arc<T>) {
        if self.len == capacity(self.shift) {
            let mut children = std::array::from_fn(|_| None);
            children[0] = Some(Arc::clone(&self.root));
            children[1] = Some(new_path(self.shift, self.len, value));
            self.root = Arc::new(VectorNode::Branch(Box::new(children)));
            self.shift += BRANCH_BITS;
        } else {
            self.root = push_at(&self.root, self.shift, self.len, value);
        }
        self.len += 1;
    }

    #[must_use]
    pub fn set(&mut self, index: usize, value: T) -> Option<()> {
        if index >= self.len {
            return None;
        }
        self.root = set_at(&self.root, self.shift, index, Arc::new(value));
        Some(())
    }

    #[must_use]
    pub fn iter(&self) -> PersistentVectorIter<'_, T> {
        PersistentVectorIter {
            vector: self,
            next: 0,
        }
    }
}

fn capacity(shift: u32) -> usize {
    1_usize
        .checked_shl(shift + BRANCH_BITS)
        .unwrap_or(usize::MAX)
}

fn new_path<T>(shift: u32, index: usize, value: Arc<T>) -> Arc<VectorNode<T>> {
    if shift == 0 {
        let mut values = std::array::from_fn(|_| None);
        values[index & BRANCH_MASK] = Some(value);
        return Arc::new(VectorNode::Leaf(Box::new(values)));
    }
    let mut children = std::array::from_fn(|_| None);
    let slot = (index >> shift) & BRANCH_MASK;
    children[slot] = Some(new_path(shift - BRANCH_BITS, index, value));
    Arc::new(VectorNode::Branch(Box::new(children)))
}

fn push_at<T>(
    node: &Arc<VectorNode<T>>,
    shift: u32,
    index: usize,
    value: Arc<T>,
) -> Arc<VectorNode<T>> {
    if shift == 0 {
        let VectorNode::Leaf(current) = node.as_ref() else {
            unreachable!("persistent vector depth must end in a leaf")
        };
        let mut values = std::array::from_fn(|slot| current[slot].clone());
        values[index & BRANCH_MASK] = Some(value);
        return Arc::new(VectorNode::Leaf(Box::new(values)));
    }
    let VectorNode::Branch(current) = node.as_ref() else {
        unreachable!("persistent vector internal depth must contain a branch")
    };
    let mut children = std::array::from_fn(|slot| current[slot].clone());
    let slot = (index >> shift) & BRANCH_MASK;
    children[slot] = Some(match &children[slot] {
        Some(child) => push_at(child, shift - BRANCH_BITS, index, value),
        None => new_path(shift - BRANCH_BITS, index, value),
    });
    Arc::new(VectorNode::Branch(Box::new(children)))
}

fn set_at<T>(
    node: &Arc<VectorNode<T>>,
    shift: u32,
    index: usize,
    value: Arc<T>,
) -> Arc<VectorNode<T>> {
    if shift == 0 {
        let VectorNode::Leaf(current) = node.as_ref() else {
            unreachable!("persistent vector depth must end in a leaf")
        };
        let mut values = std::array::from_fn(|slot| current[slot].clone());
        values[index & BRANCH_MASK] = Some(value);
        return Arc::new(VectorNode::Leaf(Box::new(values)));
    }
    let VectorNode::Branch(current) = node.as_ref() else {
        unreachable!("persistent vector internal depth must contain a branch")
    };
    let mut children = std::array::from_fn(|slot| current[slot].clone());
    let slot = (index >> shift) & BRANCH_MASK;
    let child = children[slot]
        .as_ref()
        .expect("in-range persistent vector index must have a path");
    children[slot] = Some(set_at(child, shift - BRANCH_BITS, index, value));
    Arc::new(VectorNode::Branch(Box::new(children)))
}

pub struct PersistentVectorIter<'a, T> {
    vector: &'a PersistentVector<T>,
    next: usize,
}

impl<'a, T> Iterator for PersistentVectorIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let value = self.vector.get(self.next)?;
        self.next += 1;
        Some(value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.vector.len - self.next;
        (remaining, Some(remaining))
    }
}

impl<T> ExactSizeIterator for PersistentVectorIter<'_, T> {}

impl<'a, T> IntoIterator for &'a PersistentVector<T> {
    type Item = &'a T;
    type IntoIter = PersistentVectorIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::PersistentVector;

    #[test]
    fn descendants_preserve_ancestor_values_across_tree_growth() {
        let mut ancestor = PersistentVector::default();
        for value in 0..1_100 {
            ancestor.push_shared(Arc::new(value));
        }
        let mut descendant = ancestor.clone();
        for value in 1_100..2_200 {
            descendant.push_shared(Arc::new(value));
        }

        assert_eq!(ancestor.len(), 1_100);
        assert_eq!(descendant.len(), 2_200);
        assert_eq!(
            ancestor.iter().copied().collect::<Vec<_>>(),
            (0..1_100).collect::<Vec<_>>()
        );
        assert_eq!(descendant.get(2_199), Some(&2_199));
        assert_eq!(ancestor.get(1_100), None);
    }

    #[test]
    fn sibling_appends_do_not_change_each_other() {
        let mut base = PersistentVector::default();
        for value in 0..40 {
            base.push_shared(Arc::new(value));
        }
        let mut first = base.clone();
        let mut second = base.clone();
        first.push_shared(Arc::new(100));
        second.push_shared(Arc::new(200));

        assert_eq!(first.get(40), Some(&100));
        assert_eq!(second.get(40), Some(&200));
        assert_eq!(base.get(40), None);
    }

    #[test]
    fn sibling_updates_replace_only_the_selected_path() {
        let mut base = PersistentVector::default();
        for value in 0..100 {
            base.push(value);
        }
        let mut first = base.clone();
        let mut second = base.clone();

        assert_eq!(first.set(50, 500), Some(()));
        assert_eq!(second.set(50, 600), Some(()));

        assert_eq!(base.get(50), Some(&50));
        assert_eq!(first.get(50), Some(&500));
        assert_eq!(second.get(50), Some(&600));
        assert_eq!(base.set(100, 0), None);
    }
}
