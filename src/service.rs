use std::future::Future;

use crate::diff::Diffable;

/// Trait for defining a data service for a specific entity type.
pub trait EntityService: Send + Sync + 'static {
    type Item: Diffable + Clone + Send + Sync + 'static;
    type Query: Clone + Send + Sync + 'static;
    type Error: Send + Sync + 'static;
    type Page: Send + Sync + 'static; // Example: DataPage<Item>

    /// Fetch a page of entities based on a query.
    fn fetch_list(
        &self,
        query: &Self::Query,
    ) -> impl Future<Output = Result<Self::Page, Self::Error>> + Send;

    /// Fetch a single entity by its ID.
    fn fetch_one(
        &self,
        id: &<Self::Item as Diffable>::Key,
    ) -> impl Future<Output = Result<Option<Self::Item>, Self::Error>> + Send;
}
