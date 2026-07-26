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

/// A tracker that helps the server maintain subscription state for paginated/filtered collections
/// and compute diffs for incremental synchronization.
#[derive(Debug, Clone)]
pub struct ReactiveTracker<Id> {
    cached_ids: Vec<Id>,
}

impl<Id> ReactiveTracker<Id>
where
    Id: Eq + Clone + std::hash::Hash + Send + Sync + 'static,
{
    /// Create a tracker with initial list of cached identifiers.
    pub fn new(initial_ids: Vec<Id>) -> Self {
        Self {
            cached_ids: initial_ids,
        }
    }

    /// Update the tracker with the current collection and return items added and IDs removed.
    pub fn update<Item, F>(&mut self, current_items: Vec<Item>, get_id: F) -> DiffResult<Id, Item>
    where
        F: Fn(&Item) -> Id,
    {
        use std::collections::HashSet;

        let new_ids: Vec<Id> = current_items.iter().map(&get_id).collect();
        let new_ids_set: HashSet<Id> = new_ids.iter().cloned().collect();
        let cached_ids_set: HashSet<Id> = self.cached_ids.iter().cloned().collect();

        // Calculate removed items: present in cached_ids, absent in new_ids
        let removed: Vec<Id> = self
            .cached_ids
            .iter()
            .filter(|id| !new_ids_set.contains(id))
            .cloned()
            .collect();

        // Calculate added items: present in current_items, absent in cached_ids
        let added: Vec<Item> = current_items
            .into_iter()
            .filter(|item| !cached_ids_set.contains(&get_id(item)))
            .collect();

        self.cached_ids = new_ids;

        DiffResult { added, removed }
    }

    /// Retrieve currently tracked identifiers.
    pub fn cached_ids(&self) -> &[Id] {
        &self.cached_ids
    }
}
