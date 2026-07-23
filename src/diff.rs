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
