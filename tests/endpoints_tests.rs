use notifiapp_protocol_core::{EndpointManager, EndpointPriority};

#[tokio::test]
async fn test_endpoints_sorting_and_priorities() {
    let manager = EndpointManager::new();

    let local_handle = manager
        .add_endpoint("ws://192.168.1.50/ws", EndpointPriority::Local)
        .unwrap();
    let remote_handle = manager
        .add_endpoint("ws://my.server.com", EndpointPriority::Remote)
        .unwrap();
    let dev_handle = manager
        .add_endpoint("ws://localhost:8080", EndpointPriority::Dev)
        .unwrap();

    let urls = manager.ordered_urls();
    assert_eq!(urls.len(), 3);
    // Sort order: Local (0) -> Remote (1) -> Dev (2)
    assert_eq!(urls[0].0, local_handle.id());
    assert_eq!(urls[1].0, remote_handle.id());
    assert_eq!(urls[2].0, dev_handle.id());
}

#[tokio::test]
async fn test_endpoints_change_notifications() {
    let manager = EndpointManager::new();
    let mut rx = manager.subscribe_changes();

    // Spawn a task to listen for changes
    manager
        .add_endpoint("ws://192.168.1.50/ws", EndpointPriority::Local)
        .unwrap();

    // Check we get notified
    assert!(rx.has_changed().unwrap());
    let _ = rx.borrow_and_update();

    // Switch endpoint manually
    let dev_handle = manager
        .add_endpoint("ws://localhost:8080", EndpointPriority::Dev)
        .unwrap();
    assert!(rx.has_changed().unwrap());
    let _ = rx.borrow_and_update();

    manager.switch_to_endpoint(&dev_handle).unwrap();
    assert!(rx.has_changed().unwrap());
    let _ = rx.borrow_and_update();
}

#[tokio::test]
async fn test_endpoints_last_connected_promotion() {
    let manager = EndpointManager::new();

    let local1 = manager
        .add_endpoint("ws://192.168.1.10", EndpointPriority::Local)
        .unwrap();
    let local2 = manager
        .add_endpoint("ws://192.168.1.20", EndpointPriority::Local)
        .unwrap();

    // Default order is insertion-based or whatever if priorities match
    let urls = manager.ordered_urls();
    assert_eq!(urls[0].0, local1.id());

    // Mark local2 connected successfully
    manager.mark_connected(local2.id());

    // local2 should now be promoted to the front of local priorities
    let urls = manager.ordered_urls();
    assert_eq!(urls[0].0, local2.id());
}

#[tokio::test]
async fn test_endpoints_forced_override() {
    let manager = EndpointManager::new();

    let local_handle = manager
        .add_endpoint("ws://192.168.1.50/ws", EndpointPriority::Local)
        .unwrap();
    let dev_handle = manager
        .add_endpoint("ws://localhost:8080", EndpointPriority::Dev)
        .unwrap();

    // Forced override
    manager.switch_to_endpoint(&dev_handle).unwrap();

    // Only the forced one should be returned
    let urls = manager.ordered_urls();
    assert_eq!(urls.len(), 1);
    assert_eq!(urls[0].0, dev_handle.id());

    // Clear override
    manager.clear_forced_endpoint();
    let urls = manager.ordered_urls();
    assert_eq!(urls.len(), 2);
    assert_eq!(urls[0].0, local_handle.id());
}
