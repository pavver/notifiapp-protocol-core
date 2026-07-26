/// Trait for structures that support calculating incremental diffs (patches).
pub trait Diffable {
    /// The patch structure that represents the changes.
    type Patch: serde::Serialize
        + for<'de> serde::Deserialize<'de>
        + std::fmt::Debug
        + Clone
        + PartialEq;

    /// Calculate the difference between `self` (old) and `new`.
    /// Returns `Some(Patch)` if there are differences, or `None` if the objects are identical.
    fn diff(&self, new: &Self) -> Option<Self::Patch>;

    /// Apply a patch to `self` to update its state.
    fn apply_patch(&mut self, patch: &Self::Patch);

    /// Generate a full patch representing the entire current state of `self`.
    /// Used for initial full syncs.
    fn to_full_patch(&self) -> Self::Patch;
}

/// Trait used by the Diffable derive macro to automatically resolve the patch type
/// and diff/apply operations for fields.
pub trait GetPatchType {
    type FieldPatch: serde::Serialize
        + for<'de> serde::Deserialize<'de>
        + std::fmt::Debug
        + Clone
        + PartialEq;

    fn resolve_diff(old: &Self, new: &Self) -> Self::FieldPatch;
    fn resolve_apply(target: &mut Self, patch: &Self::FieldPatch);
    fn resolve_full(val: &Self) -> Self::FieldPatch;
}

// Blanket implementation for any type T that implements Diffable (nested structures).
impl<T> GetPatchType for T
where
    T: Diffable,
{
    type FieldPatch = Option<T::Patch>;

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
}

// Helper macro to implement GetPatchType for value types (non-diffable leaf nodes).
#[macro_export]
macro_rules! impl_value_diff {
    ($ty:ty) => {
        impl $crate::diff::GetPatchType for $ty {
            type FieldPatch = Option<$ty>;

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
    type FieldPatch = Option<Option<T>>;

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
}

/// A patch representing changes in a Vec collection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum VecPatch<T, Patch> {
    /// No changes detected.
    NoChange,
    /// Full reset: replace entire list.
    Full(Vec<T>),
    /// Incremental updates: a sequence of insert, remove, or update operations.
    Incremental(Vec<VecOp<T, Patch>>),
}

/// An operation applied to an element inside a Vec collection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum VecOp<T, Patch> {
    /// Insert a new item at index.
    Insert { index: usize, value: T },
    /// Remove an item at index.
    Remove { index: usize },
    /// Update an item at index with a patch (diff).
    Update { index: usize, patch: Patch },
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
    type FieldPatch = VecPatch<T, T::FieldPatch>;

    #[allow(clippy::needless_range_loop)]
    fn resolve_diff(old: &Self, new: &Self) -> Self::FieldPatch {
        if old == new {
            return VecPatch::NoChange;
        }

        let mut ops = Vec::new();
        let common_len = std::cmp::min(old.len(), new.len());

        for i in 0..common_len {
            if old[i] != new[i] {
                let patch = T::resolve_diff(&old[i], &new[i]);
                ops.push(VecOp::Update { index: i, patch });
            }
        }

        if new.len() > old.len() {
            for i in common_len..new.len() {
                ops.push(VecOp::Insert {
                    index: i,
                    value: new[i].clone(),
                });
            }
        } else if old.len() > new.len() {
            // Remove starting from the end to avoid shifting indices during execution
            for i in (common_len..old.len()).rev() {
                ops.push(VecOp::Remove { index: i });
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
                        VecOp::Insert { index, value } => {
                            if *index <= target.len() {
                                target.insert(*index, value.clone());
                            } else {
                                target.push(value.clone());
                            }
                        }
                        VecOp::Remove { index } => {
                            if *index < target.len() {
                                target.remove(*index);
                            }
                        }
                        VecOp::Update { index, patch } => {
                            if *index < target.len() {
                                T::resolve_apply(&mut target[*index], patch);
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
}
