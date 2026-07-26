use std::sync::Arc;
use url::Url;
use uuid::Uuid;

/// Priority of an endpoint: lower value = higher priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EndpointPriority {
    /// Local network (highest priority, tried first).
    Local = 0,
    /// Remote / internet access.
    Remote = 1,
    /// Developer / staging server (lowest priority, manual selection only).
    Dev = 2,
}

/// Opaque handle returned when adding an endpoint.
/// Used to reference, switch to, or remove this endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EndpointHandle(Arc<EndpointHandleInner>);

#[derive(Debug, PartialEq, Eq, Hash)]
struct EndpointHandleInner {
    id: Uuid,
}

impl EndpointHandle {
    fn new() -> Self {
        Self(Arc::new(EndpointHandleInner { id: Uuid::new_v4() }))
    }

    /// Returns the unique identifier of the endpoint.
    pub fn id(&self) -> Uuid {
        self.0.id
    }
}

/// Representation of a single registered endpoint.
#[derive(Debug, Clone)]
pub struct EndpointData {
    pub id: Uuid,
    pub url: Url,
    pub priority: EndpointPriority,
}

/// Thread-safe manager for multiple connection endpoints.
/// Supports priorities, last connected promotion, manual overrides,
/// and provides events/notifications when endpoints change.
pub struct EndpointManager {
    endpoints: parking_lot::RwLock<Vec<EndpointData>>,
    forced_endpoint: parking_lot::RwLock<Option<Uuid>>,
    last_connected: parking_lot::RwLock<Option<Uuid>>,
    change_notify: tokio::sync::watch::Sender<()>,
    change_rx: tokio::sync::watch::Receiver<()>,
}

impl EndpointManager {
    /// Create a new endpoint manager.
    pub fn new() -> Self {
        let (change_tx, change_rx) = tokio::sync::watch::channel(());
        Self {
            endpoints: parking_lot::RwLock::new(Vec::new()),
            forced_endpoint: parking_lot::RwLock::new(None),
            last_connected: parking_lot::RwLock::new(None),
            change_notify: change_tx,
            change_rx,
        }
    }

    /// Add a new endpoint.
    pub fn add_endpoint(
        &self,
        url_str: &str,
        priority: EndpointPriority,
    ) -> Result<EndpointHandle, String> {
        let url = Url::parse(url_str).map_err(|e| e.to_string())?;

        let handle = EndpointHandle::new();
        let data = EndpointData {
            id: handle.id(),
            url,
            priority,
        };

        {
            let mut eps = self.endpoints.write();
            eps.push(data);
            eps.sort_by_key(|e| e.priority);
        }

        self.notify_change();
        Ok(handle)
    }

    /// Remove an endpoint by handle.
    pub fn remove_endpoint(&self, handle: &EndpointHandle) {
        let id = handle.id();
        self.endpoints.write().retain(|e| e.id != id);

        // Clear forced override if it pointed to the removed endpoint.
        let mut forced = self.forced_endpoint.write();
        if *forced == Some(id) {
            *forced = None;
        }

        self.notify_change();
    }

    /// Force connection to a specific endpoint, overriding priority selection.
    pub fn switch_to_endpoint(&self, handle: &EndpointHandle) -> Result<(), String> {
        let id = handle.id();
        let exists = self.endpoints.read().iter().any(|e| e.id == id);
        if !exists {
            return Err("Endpoint not found".to_string());
        }
        *self.forced_endpoint.write() = Some(id);
        self.notify_change();
        Ok(())
    }

    /// Clear the forced override and return to priority-based selection.
    pub fn clear_forced_endpoint(&self) {
        *self.forced_endpoint.write() = None;
        self.notify_change();
    }

    /// Record that a connection to this endpoint succeeded.
    /// This promotes it within its priority tier.
    pub fn mark_connected(&self, id: Uuid) {
        *self.last_connected.write() = Some(id);
    }

    /// Subscribe to changes in the endpoints configuration or selection.
    pub fn subscribe_changes(&self) -> tokio::sync::watch::Receiver<()> {
        self.change_rx.clone()
    }

    /// Get all endpoints in the order they should be tried.
    pub fn ordered_urls(&self) -> Vec<(Uuid, String)> {
        let forced = *self.forced_endpoint.read();
        let last = *self.last_connected.read();
        let eps = self.endpoints.read();

        if let Some(forced_id) = forced {
            return eps
                .iter()
                .filter(|e| e.id == forced_id)
                .map(|e| (e.id, e.url.as_str().to_string()))
                .collect();
        }

        let mut result: Vec<(Uuid, String)> = Vec::with_capacity(eps.len());

        // 1. If we have a last_connected endpoint, we might want to check its priority.
        // Let's group endpoints by priority.
        // But since eps is already sorted by priority:
        // We can just iterate over each priority level, and within that level,
        // if there's a last_connected endpoint, put it first.
        let mut sorted_eps = eps.clone();
        if let Some(last_id) = last
            && sorted_eps.iter().any(|e| e.id == last_id)
        {
            // We want to sort sorted_eps such that:
            // - sorted by priority (e.g. Local < Remote < Dev)
            // - if priorities are equal, last_connected_ep comes first.
            sorted_eps.sort_by(|a, b| {
                if a.priority != b.priority {
                    a.priority.cmp(&b.priority)
                } else if a.id == last_id {
                    std::cmp::Ordering::Less
                } else if b.id == last_id {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            });
        }

        for e in sorted_eps.iter() {
            result.push((e.id, e.url.as_str().to_string()));
        }
        result
    }

    fn notify_change(&self) {
        let _ = self.change_notify.send(());
    }
}

impl Default for EndpointManager {
    fn default() -> Self {
        Self::new()
    }
}
