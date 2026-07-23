use serde::{Deserialize, Serialize};

/// Sorting order direction.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Sort in ascending order (e.g. A to Z, smallest to largest).
    Ascending,
    /// Sort in descending order (e.g. Z to A, largest to smallest).
    Descending,
}

/// Generic pagination query parameters.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub struct Pagination {
    /// 1-based page number.
    pub page: u32,
    /// Number of items per page.
    pub limit: u32,
}

/// Container for a single page of items returned by paginated listings.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct DataPage<T> {
    /// List of items on the current page.
    pub items: Vec<T>,
    /// Total count of items matching the query criteria across all pages.
    pub total_count: u32,
}
