use notifiapp_protocol_core::{
    DiffResult, JsonCodec, PostcardCodec, ProtocolCodec, ReactiveTracker, RequestEnvelope,
    ResponseEnvelope, SubscriptionRegistry,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
enum DummyAction {
    Ping,
    GetData(String),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
struct DummyResponse {
    message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
enum DummyError {
    AccessDenied,
    NotFound,
}

#[test]
fn test_postcard_codec() {
    let req = RequestEnvelope {
        id: 42,
        action: DummyAction::GetData("test_query".to_string()),
    };

    let bytes = PostcardCodec::serialize(&req).expect("Failed to serialize with Postcard");
    let decoded: RequestEnvelope<DummyAction> =
        PostcardCodec::deserialize(&bytes).expect("Failed to deserialize with Postcard");

    assert_eq!(decoded.id, 42);
    assert_eq!(
        decoded.action,
        DummyAction::GetData("test_query".to_string())
    );
}

#[test]
fn test_json_codec() {
    let req = RequestEnvelope {
        id: 99,
        action: DummyAction::Ping,
    };

    let bytes = JsonCodec::serialize(&req).expect("Failed to serialize with JSON");
    let decoded: RequestEnvelope<DummyAction> =
        JsonCodec::deserialize(&bytes).expect("Failed to deserialize with JSON");

    assert_eq!(decoded.id, 99);
    assert_eq!(decoded.action, DummyAction::Ping);
}

#[test]
fn test_envelopes() {
    let req = RequestEnvelope {
        id: 1,
        action: DummyAction::Ping,
    };

    let response_data = DummyResponse {
        message: "Pong".to_string(),
    };

    let resp: ResponseEnvelope<DummyResponse, DummyError> = req.reply_ok(response_data.clone());

    assert_eq!(resp.id, 1);
    assert_eq!(resp.payload, Ok(response_data));
}

#[test]
fn test_subscription_registry() {
    let registry = SubscriptionRegistry::<String>::new();
    let sub_id = Uuid::new_v4();

    let triggered = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let triggered_clone = triggered.clone();

    registry.register(sub_id, move |event| {
        assert_eq!(event, "hello_event");
        triggered_clone.store(true, std::sync::atomic::Ordering::SeqCst);
    });

    let dispatched = registry.dispatch(&sub_id, "hello_event".to_string());
    assert!(dispatched);
    assert!(triggered.load(std::sync::atomic::Ordering::SeqCst));

    registry.remove(&sub_id);
    let dispatched_again = registry.dispatch(&sub_id, "hello_event".to_string());
    assert!(!dispatched_again);
}

#[test]
fn test_reactive_tracker() {
    let initial_ids = vec![
        Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
        Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap(),
    ];
    let mut tracker = ReactiveTracker::new(initial_ids.clone());

    #[derive(Clone, Debug, PartialEq)]
    struct Item {
        id: Uuid,
        name: String,
    }

    let id1 = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
    let id3 = Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap();

    let new_collection = vec![
        Item {
            id: id1,
            name: "Item 1".to_string(),
        },
        Item {
            id: id3,
            name: "Item 3".to_string(),
        },
    ];

    let diff: DiffResult<Uuid, Item> = tracker.update(new_collection, |item| item.id);

    // Item 2 should be removed, Item 3 should be added, Item 1 remains
    assert_eq!(
        diff.removed,
        vec![Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap()]
    );
    assert_eq!(
        diff.added,
        vec![Item {
            id: id3,
            name: "Item 3".to_string()
        }]
    );
    assert_eq!(tracker.cached_ids(), &vec![id1, id3]);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DummyPatch {
    id: Uuid,
    field_a: Option<String>,
    field_b: Option<String>,
}

impl notifiapp_protocol_core::conflated_queue::Conflatabled for DummyPatch {
    fn conflation_key(&self) -> Option<notifiapp_protocol_core::conflated_queue::ConflationKey> {
        Some(
            notifiapp_protocol_core::conflated_queue::ConflationKey::Entity(
                "dummy".to_string(),
                self.id,
            ),
        )
    }

    fn merge_with(&self, newer: &Self) -> Option<Self> {
        Some(DummyPatch {
            id: self.id,
            field_a: newer.field_a.clone().or_else(|| self.field_a.clone()),
            field_b: newer.field_b.clone().or_else(|| self.field_b.clone()),
        })
    }
}

#[test]
fn test_conflated_queue_merging() {
    let id = Uuid::new_v4();
    let mut queue = notifiapp_protocol_core::conflated_queue::ConflatedQueue::new();

    // 1. Push patch for field_a
    queue.push(DummyPatch {
        id,
        field_a: Some("value_a".to_string()),
        field_b: None,
    });

    // 2. Push patch for field_b (should merge with the previous patch)
    queue.push(DummyPatch {
        id,
        field_a: None,
        field_b: Some("value_b".to_string()),
    });

    assert_eq!(queue.len(), 1);

    // 3. Push overriding patch for field_a (should update field_a in the merged patch)
    queue.push(DummyPatch {
        id,
        field_a: Some("new_value_a".to_string()),
        field_b: None,
    });

    assert_eq!(queue.len(), 1);

    let final_patch = queue.pop().expect("Queue should have 1 patch");
    assert_eq!(final_patch.field_a, Some("new_value_a".to_string()));
    assert_eq!(final_patch.field_b, Some("value_b".to_string()));
}
