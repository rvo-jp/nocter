use std::cmp::Ordering;
use std::sync::Arc;

#[derive(Debug)]
struct MapNode<K, V> {
    key: Arc<K>,
    value: V,
    left: Option<Arc<Self>>,
    right: Option<Arc<Self>>,
    height: u32,
}

impl<K, V> MapNode<K, V> {
    fn new(key: Arc<K>, value: V, left: Option<Arc<Self>>, right: Option<Arc<Self>>) -> Self {
        let height = 1 + node_height(left.as_ref()).max(node_height(right.as_ref()));
        Self {
            key,
            value,
            left,
            right,
            height,
        }
    }
}

fn node_height<K, V>(node: Option<&Arc<MapNode<K, V>>>) -> u32 {
    node.map_or(0, |node| node.height)
}

/// A persistent ordered map implemented as a path-copying AVL tree.
#[derive(Debug)]
pub(crate) struct PersistentMap<K, V> {
    root: Option<Arc<MapNode<K, V>>>,
    len: usize,
}

impl<K, V> Clone for PersistentMap<K, V> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            len: self.len,
        }
    }
}

impl<K, V> Default for PersistentMap<K, V> {
    fn default() -> Self {
        Self { root: None, len: 0 }
    }
}

impl<K: Ord, V> PersistentMap<K, V> {
    pub(crate) const fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        let mut node = self.root.as_deref();
        while let Some(current) = node {
            match key.cmp(current.key.as_ref()) {
                Ordering::Less => node = current.left.as_deref(),
                Ordering::Greater => node = current.right.as_deref(),
                Ordering::Equal => return Some(&current.value),
            }
        }
        None
    }
}

impl<K: Ord, V: Clone> PersistentMap<K, V> {
    /// Inserts an absent key while preserving every existing snapshot.
    ///
    /// Returns `false` without replacing the existing value when `key` is already present.
    pub(crate) fn insert_absent(&mut self, key: K, value: V) -> bool {
        let (root, inserted) = insert(self.root.as_ref(), Arc::new(key), value);
        if inserted {
            self.root = Some(root);
            self.len += 1;
        }
        inserted
    }
}

fn insert<K: Ord, V: Clone>(
    node: Option<&Arc<MapNode<K, V>>>,
    key: Arc<K>,
    value: V,
) -> (Arc<MapNode<K, V>>, bool) {
    let Some(node) = node else {
        return (Arc::new(MapNode::new(key, value, None, None)), true);
    };
    match key.cmp(&node.key) {
        Ordering::Equal => (Arc::clone(node), false),
        Ordering::Less => {
            let (left, inserted) = insert(node.left.as_ref(), key, value);
            if !inserted {
                return (Arc::clone(node), false);
            }
            (
                rebalance(Arc::new(MapNode::new(
                    Arc::clone(&node.key),
                    node.value.clone(),
                    Some(left),
                    node.right.clone(),
                ))),
                true,
            )
        }
        Ordering::Greater => {
            let (right, inserted) = insert(node.right.as_ref(), key, value);
            if !inserted {
                return (Arc::clone(node), false);
            }
            (
                rebalance(Arc::new(MapNode::new(
                    Arc::clone(&node.key),
                    node.value.clone(),
                    node.left.clone(),
                    Some(right),
                ))),
                true,
            )
        }
    }
}

fn rebalance<K, V: Clone>(node: Arc<MapNode<K, V>>) -> Arc<MapNode<K, V>> {
    let balance =
        i64::from(node_height(node.left.as_ref())) - i64::from(node_height(node.right.as_ref()));
    if balance > 1 {
        let left = node
            .left
            .as_ref()
            .expect("left-heavy AVL node has a left child");
        if node_height(left.left.as_ref()) < node_height(left.right.as_ref()) {
            let rebuilt = Arc::new(MapNode::new(
                Arc::clone(&node.key),
                node.value.clone(),
                Some(rotate_left(left)),
                node.right.clone(),
            ));
            return rotate_right(&rebuilt);
        }
        return rotate_right(&node);
    }
    if balance < -1 {
        let right = node
            .right
            .as_ref()
            .expect("right-heavy AVL node has a right child");
        if node_height(right.right.as_ref()) < node_height(right.left.as_ref()) {
            let rebuilt = Arc::new(MapNode::new(
                Arc::clone(&node.key),
                node.value.clone(),
                node.left.clone(),
                Some(rotate_right(right)),
            ));
            return rotate_left(&rebuilt);
        }
        return rotate_left(&node);
    }
    node
}

fn rotate_left<K, V: Clone>(root: &Arc<MapNode<K, V>>) -> Arc<MapNode<K, V>> {
    let pivot = root
        .right
        .as_ref()
        .expect("left rotation requires a right child");
    let left = Arc::new(MapNode::new(
        Arc::clone(&root.key),
        root.value.clone(),
        root.left.clone(),
        pivot.left.clone(),
    ));
    Arc::new(MapNode::new(
        Arc::clone(&pivot.key),
        pivot.value.clone(),
        Some(left),
        pivot.right.clone(),
    ))
}

fn rotate_right<K, V: Clone>(root: &Arc<MapNode<K, V>>) -> Arc<MapNode<K, V>> {
    let pivot = root
        .left
        .as_ref()
        .expect("right rotation requires a left child");
    let right = Arc::new(MapNode::new(
        Arc::clone(&root.key),
        root.value.clone(),
        pivot.right.clone(),
        root.right.clone(),
    ));
    Arc::new(MapNode::new(
        Arc::clone(&pivot.key),
        pivot.value.clone(),
        pivot.left.clone(),
        Some(right),
    ))
}

#[cfg(test)]
mod tests {
    use super::PersistentMap;

    #[test]
    fn descendants_share_unchanged_entries_and_isolate_insertions() {
        let mut base = PersistentMap::default();
        for key in 0..1_000 {
            assert!(base.insert_absent(key, key * 2));
        }
        let mut first = base.clone();
        let mut second = base.clone();
        assert!(first.insert_absent(1_000, 1));
        assert!(second.insert_absent(1_000, 2));

        assert_eq!(base.len(), 1_000);
        assert_eq!(base.get(&999), Some(&1_998));
        assert_eq!(base.get(&1_000), None);
        assert_eq!(first.get(&1_000), Some(&1));
        assert_eq!(second.get(&1_000), Some(&2));
    }

    #[test]
    fn duplicate_insert_does_not_replace_the_canonical_value() {
        let mut map = PersistentMap::default();
        assert!(map.insert_absent("name", 1));
        assert!(!map.insert_absent("name", 2));
        assert_eq!(map.get(&"name"), Some(&1));
        assert_eq!(map.len(), 1);
    }
}
