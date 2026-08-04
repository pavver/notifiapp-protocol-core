use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use uuid::Uuid;

/// Key used to identify and group messages that can be conflated (merged).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConflationKey {
    /// A unique key for messages that must never be conflated.
    Unique(Uuid),
    /// Key for conflating events related to a specific entity type and ID.
    Entity(String, Uuid),
    /// Key for conflating custom events matching a string identifier.
    Custom(String),
}

/// A trait for messages that support state conflation (merging/overwriting).
pub trait Conflatabled: Sized {
    /// Return the conflation key for this message.
    ///
    /// Return `None` if the message is unique and must not be conflated.
    fn conflation_key(&self) -> Option<ConflationKey>;

    /// Merge this message with a newer message of the same type.
    ///
    /// If the merge is successful, return `Some(merged_message)`.
    /// If the messages cannot be merged (e.g. incompatible formats), return `None`,
    /// and the newer message will overwrite the older one in the queue.
    fn merge_with(&self, _newer: &Self) -> Option<Self> {
        None
    }
}

/// A thread-safe, order-preserving queue that conflates and merges message states.
///
/// If a message with the same `ConflationKey` is pushed while a previous one is still
/// waiting in the queue, the queue will attempt to merge them using `Conflatabled::merge_with`.
/// If merging is not supported, the new message will overwrite the old one in place,
/// preserving its original position in the sending queue.
#[derive(Debug, Clone)]
pub struct ConflatedQueue<M> {
    order: VecDeque<ConflationKey>,
    messages: HashMap<ConflationKey, M>,
}

impl<M: Conflatabled> ConflatedQueue<M> {
    /// Create a new empty conflated queue.
    pub fn new() -> Self {
        Self {
            order: VecDeque::new(),
            messages: HashMap::new(),
        }
    }

    /// Push a message into the queue, merging or overwriting existing duplicates in place.
    pub fn push(&mut self, message: M) {
        let key = message
            .conflation_key()
            .unwrap_or_else(|| ConflationKey::Unique(Uuid::new_v4()));

        if let Some(old_msg) = self.messages.get(&key) {
            // Attempt to merge the old message with the new one
            if let Some(merged) = old_msg.merge_with(&message) {
                self.messages.insert(key, merged);
            } else {
                // If merge is not possible, overwrite the old message
                self.messages.insert(key, message);
            }
        } else {
            // No duplicate key: insert fresh message and keep order
            self.order.push_back(key.clone());
            self.messages.insert(key, message);
        }
    }

    /// Pop the next message from the queue in FIFO order.
    pub fn pop(&mut self) -> Option<M> {
        while let Some(key) = self.order.pop_front() {
            if let Some(msg) = self.messages.remove(&key) {
                return Some(msg);
            }
        }
        None
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Return the number of messages currently waiting in the queue.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Clear all messages from the queue.
    pub fn clear(&mut self) {
        self.order.clear();
        self.messages.clear();
    }
}

impl<M: Conflatabled> Default for ConflatedQueue<M> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestMsg {
        id: Uuid,
        val: i32,
    }

    impl Conflatabled for TestMsg {
        fn conflation_key(&self) -> Option<ConflationKey> {
            Some(ConflationKey::Entity("test".to_string(), self.id))
        }

        fn merge_with(&self, newer: &Self) -> Option<Self> {
            Some(TestMsg {
                id: self.id,
                val: self.val + newer.val,
            })
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct UniqueMsg(Uuid);

    impl Conflatabled for UniqueMsg {
        fn conflation_key(&self) -> Option<ConflationKey> {
            None
        }
    }

    #[test]
    fn test_queue_push_pop() {
        let mut q = ConflatedQueue::new();
        let id1 = Uuid::new_v4();

        q.push(TestMsg { id: id1, val: 1 });
        q.push(TestMsg { id: id1, val: 2 });

        assert_eq!(q.len(), 1);
        assert_eq!(q.pop(), Some(TestMsg { id: id1, val: 3 }));
        assert_eq!(q.pop(), None);
    }

    #[test]
    fn test_queue_unique() {
        let mut q = ConflatedQueue::new();
        let id1 = Uuid::new_v4();

        q.push(UniqueMsg(id1));
        q.push(UniqueMsg(id1));

        assert_eq!(q.len(), 2);
        assert_eq!(q.pop(), Some(UniqueMsg(id1)));
        assert_eq!(q.pop(), Some(UniqueMsg(id1)));
        assert_eq!(q.pop(), None);
    }
}
