/// Trait for structures that support calculating incremental diffs (patches).
pub trait Diffable {
    /// The key type used to identify elements uniquely (e.g. Uuid).
    type Key: PartialEq
        + Clone
        + std::fmt::Debug
        + serde::Serialize
        + for<'de> serde::Deserialize<'de>;

    /// The patch structure that represents the changes.
    type Patch: serde::Serialize
        + for<'de> serde::Deserialize<'de>
        + std::fmt::Debug
        + Clone
        + PartialEq;

    /// Returns the key for this item.
    fn get_key(&self) -> Self::Key;

    /// Calculate the difference between `self` (old) and `new`.
    /// Returns `Some(Patch)` if there are differences, or `None` if the objects are identical.
    fn diff(&self, new: &Self) -> Option<Self::Patch>;

    /// Apply a patch to `self` to update its state.
    fn apply_patch(&mut self, patch: &Self::Patch);

    /// Generate a full patch representing the entire current state of `self`.
    /// Used for initial full syncs.
    fn to_full_patch(&self) -> Self::Patch;

    /// Merge `new` patch into `old` patch.
    fn merge_patch(old: &mut Self::Patch, new: &Self::Patch);
}

/// Trait used by the Diffable derive macro to automatically resolve the patch type
/// and diff/apply operations for fields.
pub trait GetPatchType {
    type Key: PartialEq
        + Clone
        + std::fmt::Debug
        + serde::Serialize
        + for<'de> serde::Deserialize<'de>;

    type FieldPatch: serde::Serialize
        + for<'de> serde::Deserialize<'de>
        + std::fmt::Debug
        + Clone
        + PartialEq;

    /// Whether this type supports ID-based list diffing.
    const IS_ID_BASED: bool;

    fn resolve_key(&self) -> Self::Key;
    fn index_to_key(index: usize) -> Self::Key;
    fn key_to_index(key: &Self::Key) -> usize;
    fn resolve_diff(old: &Self, new: &Self) -> Self::FieldPatch;
    fn resolve_apply(target: &mut Self, patch: &Self::FieldPatch);
    fn resolve_full(val: &Self) -> Self::FieldPatch;
    fn resolve_merge(old: &mut Self::FieldPatch, new: &Self::FieldPatch);
}

// Blanket implementation for any type T that implements Diffable (nested structures).
impl<T> GetPatchType for T
where
    T: Diffable,
{
    type Key = T::Key;
    type FieldPatch = Option<T::Patch>;

    const IS_ID_BASED: bool = true;

    fn resolve_key(&self) -> Self::Key {
        self.get_key()
    }

    fn index_to_key(_index: usize) -> Self::Key {
        panic!("Cannot convert index to key for ID-based collections")
    }

    fn key_to_index(_key: &Self::Key) -> usize {
        panic!("Cannot convert key to index for ID-based collections")
    }

    fn resolve_diff(old: &Self, new: &Self) -> Self::FieldPatch {
        old.diff(new)
    }

    fn resolve_apply(target: &mut Self, patch: &Self::FieldPatch) {
        if let Some(inner_patch) = patch {
            target.apply_patch(inner_patch);
        }
    }

    fn resolve_full(val: &Self) -> Self::FieldPatch {
        Some(val.to_full_patch())
    }

    fn resolve_merge(old: &mut Self::FieldPatch, new: &Self::FieldPatch) {
        if let Some(new_patch) = new {
            if let Some(old_patch) = old {
                T::merge_patch(old_patch, new_patch);
            } else {
                *old = Some(new_patch.clone());
            }
        }
    }
}

// Helper macro to implement GetPatchType for value types (non-diffable leaf nodes).
#[macro_export]
macro_rules! impl_value_diff {
    ($ty:ty) => {
        impl $crate::diff::GetPatchType for $ty {
            type Key = usize;
            type FieldPatch = Option<$ty>;

            const IS_ID_BASED: bool = false;

            fn resolve_key(&self) -> Self::Key {
                0
            }

            fn index_to_key(index: usize) -> Self::Key {
                index
            }

            fn key_to_index(key: &Self::Key) -> usize {
                *key
            }

            fn resolve_diff(old: &Self, new: &Self) -> Self::FieldPatch {
                if old != new { Some(new.clone()) } else { None }
            }

            fn resolve_apply(target: &mut Self, patch: &Self::FieldPatch) {
                if let Some(val) = patch {
                    *target = val.clone();
                }
            }

            fn resolve_full(val: &Self) -> Self::FieldPatch {
                Some(val.clone())
            }

            fn resolve_merge(old: &mut Self::FieldPatch, new: &Self::FieldPatch) {
                if new.is_some() {
                    *old = new.clone();
                }
            }
        }
    };
}

// Implement GetPatchType for standard library types.
impl_value_diff!(String);
impl_value_diff!(u8);
impl_value_diff!(u16);
impl_value_diff!(u32);
impl_value_diff!(u64);
impl_value_diff!(i8);
impl_value_diff!(i16);
impl_value_diff!(i32);
impl_value_diff!(i64);
impl_value_diff!(f32);
impl_value_diff!(f64);
impl_value_diff!(bool);
impl_value_diff!(uuid::Uuid);

// Implement GetPatchType for Option of standard types.
impl<T> GetPatchType for Option<T>
where
    T: PartialEq + Clone + serde::Serialize + for<'de> serde::Deserialize<'de> + std::fmt::Debug,
{
    type Key = usize;
    type FieldPatch = Option<Option<T>>;

    const IS_ID_BASED: bool = false;

    fn resolve_key(&self) -> Self::Key {
        0
    }
    fn index_to_key(index: usize) -> Self::Key {
        index
    }
    fn key_to_index(key: &Self::Key) -> usize {
        *key
    }

    fn resolve_diff(old: &Self, new: &Self) -> Self::FieldPatch {
        if old != new { Some(new.clone()) } else { None }
    }

    fn resolve_apply(target: &mut Self, patch: &Self::FieldPatch) {
        if let Some(val) = patch {
            *target = val.clone();
        }
    }

    fn resolve_full(val: &Self) -> Self::FieldPatch {
        Some(val.clone())
    }

    fn resolve_merge(old: &mut Self::FieldPatch, new: &Self::FieldPatch) {
        if new.is_some() {
            *old = new.clone();
        }
    }
}

/// A patch representing changes in a Vec collection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum VecPatch<K, T, Patch> {
    /// No changes detected.
    NoChange,
    /// Full reset: replace entire list.
    Full(Vec<T>),
    /// Incremental updates: a sequence of insert, remove, or update operations.
    Incremental(Vec<VecOp<K, T, Patch>>),
}

impl<K, T, Patch> VecPatch<K, T, Patch> {
    /// Returns true if the patch represents actual changes.
    pub fn is_some(&self) -> bool {
        !matches!(self, VecPatch::NoChange)
    }
}

/// An operation applied to an element inside a Vec collection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum VecOp<K, T, Patch> {
    /// Insert a new item.
    Insert { key: K, value: T },
    /// Remove an item.
    Remove { key: K },
    /// Update an item with a patch (diff).
    Update { key: K, patch: Patch },
}

impl<T> GetPatchType for Vec<T>
where
    T: GetPatchType
        + PartialEq
        + Clone
        + serde::Serialize
        + for<'de> serde::Deserialize<'de>
        + std::fmt::Debug,
{
    type Key = usize;
    type FieldPatch = VecPatch<T::Key, T, T::FieldPatch>;

    const IS_ID_BASED: bool = false;

    fn resolve_key(&self) -> Self::Key {
        0
    }
    fn index_to_key(index: usize) -> Self::Key {
        index
    }
    fn key_to_index(key: &Self::Key) -> usize {
        *key
    }

    #[allow(clippy::needless_range_loop)]
    fn resolve_diff(old: &Self, new: &Self) -> Self::FieldPatch {
        if old == new {
            return VecPatch::NoChange;
        }

        let mut ops = Vec::new();

        if T::IS_ID_BASED {
            // ID-based diff algorithm
            for new_item in new.iter() {
                let new_key = new_item.resolve_key();
                if let Some(old_item) = old.iter().find(|o| o.resolve_key() == new_key) {
                    if old_item != new_item {
                        let patch = T::resolve_diff(old_item, new_item);
                        ops.push(VecOp::Update {
                            key: new_key,
                            patch,
                        });
                    }
                } else {
                    ops.push(VecOp::Insert {
                        key: new_key,
                        value: new_item.clone(),
                    });
                }
            }

            for old_item in old.iter() {
                let old_key = old_item.resolve_key();
                if !new.iter().any(|n| n.resolve_key() == old_key) {
                    ops.push(VecOp::Remove { key: old_key });
                }
            }
        } else {
            // Index-based fallback algorithm
            let common_len = std::cmp::min(old.len(), new.len());

            for i in 0..common_len {
                if old[i] != new[i] {
                    let patch = T::resolve_diff(&old[i], &new[i]);
                    ops.push(VecOp::Update {
                        key: T::index_to_key(i),
                        patch,
                    });
                }
            }

            if new.len() > old.len() {
                for i in common_len..new.len() {
                    ops.push(VecOp::Insert {
                        key: T::index_to_key(i),
                        value: new[i].clone(),
                    });
                }
            } else if old.len() > new.len() {
                // Remove starting from the end to avoid shifting indices during execution
                for i in (common_len..old.len()).rev() {
                    ops.push(VecOp::Remove {
                        key: T::index_to_key(i),
                    });
                }
            }
        }

        VecPatch::Incremental(ops)
    }

    fn resolve_apply(target: &mut Self, patch: &Self::FieldPatch) {
        match patch {
            VecPatch::NoChange => {}
            VecPatch::Full(full) => {
                *target = full.clone();
            }
            VecPatch::Incremental(ops) => {
                for op in ops {
                    match op {
                        VecOp::Insert { key, value } => {
                            if T::IS_ID_BASED {
                                if let Some(idx) =
                                    target.iter().position(|o| o.resolve_key() == *key)
                                {
                                    target[idx] = value.clone();
                                } else {
                                    target.push(value.clone());
                                }
                            } else {
                                let idx = T::key_to_index(key);
                                if idx <= target.len() {
                                    target.insert(idx, value.clone());
                                } else {
                                    target.push(value.clone());
                                }
                            }
                        }
                        VecOp::Remove { key } => {
                            if T::IS_ID_BASED {
                                if let Some(idx) =
                                    target.iter().position(|o| o.resolve_key() == *key)
                                {
                                    target.remove(idx);
                                }
                            } else {
                                let idx = T::key_to_index(key);
                                if idx < target.len() {
                                    target.remove(idx);
                                }
                            }
                        }
                        VecOp::Update { key, patch } => {
                            if T::IS_ID_BASED {
                                if let Some(idx) =
                                    target.iter().position(|o| o.resolve_key() == *key)
                                {
                                    T::resolve_apply(&mut target[idx], patch);
                                }
                            } else {
                                let idx = T::key_to_index(key);
                                if idx < target.len() {
                                    T::resolve_apply(&mut target[idx], patch);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn resolve_full(val: &Self) -> Self::FieldPatch {
        VecPatch::Full(val.clone())
    }

    fn resolve_merge(old: &mut Self::FieldPatch, new: &Self::FieldPatch) {
        match new {
            VecPatch::NoChange => {} // Nothing to merge
            VecPatch::Full(full) => {
                *old = VecPatch::Full(full.clone());
            }
            VecPatch::Incremental(new_ops) => {
                match old {
                    VecPatch::NoChange => {
                        *old = new.clone();
                    }
                    VecPatch::Full(old_full) => {
                        // Apply incremental to the full patch directly
                        Self::resolve_apply(old_full, new);
                    }
                    VecPatch::Incremental(old_ops) => {
                        // Merge incremental ops
                        // For simplicity, just append new ops. A true merge would compact them.
                        old_ops.extend(new_ops.clone());
                    }
                }
            }
        }
    }
}
