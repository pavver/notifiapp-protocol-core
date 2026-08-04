use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

/// A registry for storing and dispatching client-side subscription callbacks.
pub struct SubscriptionRegistry<V> {
    subscriptions: DashMap<Uuid, Arc<dyn Fn(V) + Send + Sync>>,
}

impl<V: Clone + Send + Sync + 'static> SubscriptionRegistry<V> {
    /// Create a new empty subscription registry.
    pub fn new() -> Self {
        Self {
            subscriptions: DashMap::new(),
        }
    }

    /// Register a callback for a specific subscription ID.
    pub fn register(&self, id: Uuid, callback: impl Fn(V) + Send + Sync + 'static) {
        self.subscriptions.insert(id, Arc::new(callback));
    }

    /// Remove a callback by subscription ID.
    pub fn remove(&self, id: &Uuid) {
        self.subscriptions.remove(id);
    }

    /// Dispatch an event to the registered callback if it exists.
    pub fn dispatch(&self, id: &Uuid, event: V) -> bool {
        if let Some(callback) = self.subscriptions.get(id) {
            (callback.value())(event);
            true
        } else {
            false
        }
    }

    /// Clear all registered callbacks.
    pub fn clear(&self) {
        self.subscriptions.clear();
    }
}

impl<V: Clone + Send + Sync + 'static> Default for SubscriptionRegistry<V> {
    fn default() -> Self {
        Self::new()
    }
}

/// The result of comparing two collection states.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffResult<Id, Item> {
    /// Items that exist in the new state but were missing in the cached state.
    pub added: Vec<Item>,
    /// Identifiers of items that exist in the cached state but are missing in the new state.
    pub removed: Vec<Id>,
}

use crate::diff::Diffable;
/// A tracker that helps the server maintain subscription state for paginated/filtered collections
/// and compute diffs for incremental synchronization.
use std::collections::HashMap;

/// The result of comparing two collection states for a SubscriptionTracker.
#[derive(Debug, Clone, PartialEq)]
pub struct ListDiffResult<T: Diffable> {
    pub added: Vec<T>,
    pub removed: Vec<T::Key>,
    pub updated: Vec<T::Patch>,
}

/// A stateful tracker for subscriptions that maintains a full copy of tracked entities
/// and generates incremental diffs for updates.
#[derive(Debug, Clone)]
pub struct SubscriptionTracker<T: Diffable + Clone> {
    cached_items: HashMap<T::Key, T>,
}

impl<T: Diffable + Clone> SubscriptionTracker<T>
where
    T::Key: std::hash::Hash + Eq,
{
    pub fn new(initial_items: Vec<T>) -> Self {
        let mut cached_items = HashMap::new();
        for item in initial_items {
            cached_items.insert(item.get_key(), item);
        }
        Self { cached_items }
    }

    /// Update tracker with a new full list of items. Returns added items, removed IDs, and patches for updated items.
    pub fn update_list(&mut self, current_items: Vec<T>) -> ListDiffResult<T> {
        let mut new_cache = HashMap::new();
        let mut added = Vec::new();
        let mut updated = Vec::new();

        for item in current_items {
            let key = item.get_key();
            if let Some(old_item) = self.cached_items.remove(&key) {
                if let Some(patch) = old_item.diff(&item) {
                    updated.push(patch);
                }
            } else {
                added.push(item.clone());
            }
            new_cache.insert(key, item);
        }

        let removed = self.cached_items.keys().cloned().collect();
        self.cached_items = new_cache;

        ListDiffResult {
            added,
            removed,
            updated,
        }
    }

    /// Update a single item in the tracker. Returns an incremental patch if it changed.
    pub fn update_single(&mut self, item: &T) -> Option<T::Patch> {
        let key = item.get_key();
        if let Some(old_item) = self.cached_items.get(&key) {
            let patch = old_item.diff(item);
            if patch.is_some() {
                self.cached_items.insert(key, item.clone());
            }
            patch
        } else {
            // Not tracked previously, or we track it now
            self.cached_items.insert(key, item.clone());
            Some(item.to_full_patch())
        }
    }

    pub fn remove(&mut self, key: &T::Key) -> bool {
        self.cached_items.remove(key).is_some()
    }

    pub fn contains(&self, key: &T::Key) -> bool {
        self.cached_items.contains_key(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::Diffable;

    #[derive(Debug, Clone, PartialEq)]
    struct TestItem {
        id: uuid::Uuid,
        val: String,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct TestItemPatch {
        id: uuid::Uuid,
        val: Option<String>,
    }

    impl serde::Serialize for TestItemPatch {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serde::Serialize::serialize(&self.id, serializer)
        }
    }

    impl<'de> serde::Deserialize<'de> for TestItemPatch {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let id = uuid::Uuid::deserialize(deserializer)?;
            Ok(TestItemPatch { id, val: None })
        }
    }

    impl Diffable for TestItem {
        type Key = uuid::Uuid;
        type Patch = TestItemPatch;

        fn get_key(&self) -> Self::Key {
            self.id
        }

        fn to_full_patch(&self) -> Self::Patch {
            TestItemPatch {
                id: self.id,
                val: Some(self.val.clone()),
            }
        }

        fn diff(&self, new_state: &Self) -> Option<Self::Patch> {
            if self.val != new_state.val {
                Some(TestItemPatch {
                    id: self.id,
                    val: Some(new_state.val.clone()),
                })
            } else {
                None
            }
        }

        fn apply_patch(&mut self, patch: &Self::Patch) {
            if let Some(v) = &patch.val {
                self.val = v.clone();
            }
        }

        fn merge_patch(old: &mut Self::Patch, new: &Self::Patch) {
            if let Some(v) = &new.val {
                old.val = Some(v.clone());
            }
        }
    }

    #[test]
    fn test_subscription_tracker() {
        let id1 = uuid::Uuid::new_v4();
        let item1 = TestItem {
            id: id1,
            val: "A".to_string(),
        };
        let mut tracker = SubscriptionTracker::new(vec![item1.clone()]);

        let id2 = uuid::Uuid::new_v4();
        let item2 = TestItem {
            id: id2,
            val: "B".to_string(),
        };
        let mut item1_updated = item1.clone();
        item1_updated.val = "C".to_string();

        let diff = tracker.update_list(vec![item1_updated.clone(), item2.clone()]);

        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].id, id2);

        assert_eq!(diff.removed.len(), 0);

        assert_eq!(diff.updated.len(), 1);
        assert_eq!(diff.updated[0].id, id1);

        // Remove item1
        let diff2 = tracker.update_list(vec![item2.clone()]);
        assert_eq!(diff2.removed.len(), 1);
        assert_eq!(diff2.removed[0], id1);
        assert_eq!(diff2.added.len(), 0);
        assert_eq!(diff2.updated.len(), 0);
    }
}
