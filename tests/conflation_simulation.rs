use notifiapp_protocol_core::conflated_queue::{Conflatabled, ConflatedQueue, ConflationKey};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};
use uuid::Uuid;

// 1. Define the 20-field state structure
#[derive(Debug, Clone, PartialEq, Eq)]
struct BigState {
    id: Uuid,
    fields: Vec<String>,
}

impl BigState {
    fn new(id: Uuid) -> Self {
        Self {
            id,
            fields: vec!["initial".to_string(); 20],
        }
    }

    fn apply_patch(&mut self, patch: &BigStatePatch) {
        for (idx, val) in &patch.updates {
            if let Some(v) = val.as_ref().filter(|_| *idx < self.fields.len()) {
                self.fields[*idx] = v.clone();
            }
        }
    }
}

// 2. Define the incremental patch structure
#[derive(Debug, Clone, PartialEq, Eq)]
struct BigStatePatch {
    entity_id: Uuid,
    updates: HashMap<usize, Option<String>>,
}

impl Conflatabled for BigStatePatch {
    fn conflation_key(&self) -> Option<ConflationKey> {
        Some(ConflationKey::Entity(
            "big_state".to_string(),
            self.entity_id,
        ))
    }

    fn merge_with(&self, newer: &Self) -> Option<Self> {
        let mut merged_updates = self.updates.clone();
        for (idx, val) in &newer.updates {
            merged_updates.insert(*idx, val.clone());
        }
        Some(BigStatePatch {
            entity_id: self.entity_id,
            updates: merged_updates,
        })
    }
}

// 3. Define client configuration for the simulation
struct ClientSim {
    name: String,
    state: BigState,
    msg_received: usize,
    // Network channel simulation: delay per message
    delay: Duration,
    // Connection stability simulation
    is_online: bool,
}

impl ClientSim {
    fn new(name: &str, id: Uuid, delay: Duration) -> Self {
        Self {
            name: name.to_string(),
            state: BigState::new(id),
            msg_received: 0,
            delay,
            is_online: true,
        }
    }
}

#[tokio::test]
async fn test_network_and_conflation_simulation() {
    let entity_id = Uuid::new_v4();

    // Server's ground truth state
    let server_state = Arc::new(Mutex::new(BigState::new(entity_id)));

    // Conflated queues for three clients
    let client_fast_queue = Arc::new(Mutex::new(ConflatedQueue::<BigStatePatch>::new()));
    let client_slow_queue = Arc::new(Mutex::new(ConflatedQueue::<BigStatePatch>::new()));
    let client_unstable_queue = Arc::new(Mutex::new(ConflatedQueue::<BigStatePatch>::new()));

    // Instantiate clients
    let client_fast = Arc::new(Mutex::new(ClientSim::new(
        "Fast Client (Unlimited)",
        entity_id,
        Duration::from_millis(0),
    )));
    let client_slow = Arc::new(Mutex::new(ClientSim::new(
        "Slow Client (Narrow Bandwidth)",
        entity_id,
        Duration::from_millis(500),
    )));
    let client_unstable = Arc::new(Mutex::new(ClientSim::new(
        "Unstable Client (Slow + Dropouts)",
        entity_id,
        Duration::from_millis(500),
    )));

    println!("\n=== STARTING CONFLATION SIMULATION ===");
    println!("Server initializes BigState with 20 fields ('initial').");

    // Spawn server updates task: updates random fields incrementally every 200 ms for 4 seconds
    let server_state_clone = Arc::clone(&server_state);
    let q_fast = Arc::clone(&client_fast_queue);
    let q_slow = Arc::clone(&client_slow_queue);
    let q_unstable = Arc::clone(&client_unstable_queue);

    let server_task = tokio::spawn(async move {
        let mut field_update_counter = 0;
        for step in 1..=20 {
            sleep(Duration::from_millis(200)).await;

            // Incrementally update two fields per step
            let f1 = (step * 2 - 2) % 20;
            let f2 = (step * 2 - 1) % 20;
            field_update_counter += 1;

            let val1 = format!("val_{}_{}", f1, field_update_counter);
            let val2 = format!("val_{}_{}", f2, field_update_counter);

            let mut updates = HashMap::new();
            updates.insert(f1, Some(val1));
            updates.insert(f2, Some(val2));

            let patch = BigStatePatch { entity_id, updates };

            // Update server ground truth
            {
                let mut s_state = server_state_clone.lock().await;
                s_state.apply_patch(&patch);
            }

            // Push the patch into all client queues
            q_fast.lock().await.push(patch.clone());
            q_slow.lock().await.push(patch.clone());
            q_unstable.lock().await.push(patch);
        }
    });

    // Spawn fast client worker task (immediately processes queue)
    let c_fast = Arc::clone(&client_fast);
    let q_fast_clone = Arc::clone(&client_fast_queue);
    let fast_task = tokio::spawn(async move {
        loop {
            let patch_opt = q_fast_clone.lock().await.pop();
            if let Some(patch) = patch_opt {
                let mut client = c_fast.lock().await;
                client.state.apply_patch(&patch);
                client.msg_received += 1;
                println!(
                    "[{}] Received patch updating fields: {:?}",
                    client.name,
                    patch.updates.keys().collect::<Vec<_>>()
                );
            }
            sleep(Duration::from_millis(50)).await; // Fast polling
        }
    });

    // Spawn slow client worker task (processes queue with 500ms delay per msg)
    let c_slow = Arc::clone(&client_slow);
    let q_slow_clone = Arc::clone(&client_slow_queue);
    let slow_task = tokio::spawn(async move {
        loop {
            let patch_opt = q_slow_clone.lock().await.pop();
            if let Some(patch) = patch_opt {
                let delay = { c_slow.lock().await.delay };
                sleep(delay).await; // Simulate slow network transmission delay

                let mut client = c_slow.lock().await;
                client.state.apply_patch(&patch);
                client.msg_received += 1;
                println!(
                    "[{}] Received merged/conflated patch updating fields: {:?}",
                    client.name,
                    patch.updates.keys().collect::<Vec<_>>()
                );
            }
            sleep(Duration::from_millis(50)).await;
        }
    });

    // Spawn unstable client worker task (disconnects every 1.5 seconds)
    let c_unstable = Arc::clone(&client_unstable);
    let q_unstable_clone = Arc::clone(&client_unstable_queue);
    let unstable_task = tokio::spawn(async move {
        let start_time = tokio::time::Instant::now();
        loop {
            let elapsed = start_time.elapsed();

            // Simulate periodic disconnections
            // Every 1.6 seconds, disconnect for 800 ms
            let is_offline = (elapsed.as_millis() % 1600) < 800;

            {
                let mut client = c_unstable.lock().await;
                if is_offline && client.is_online {
                    client.is_online = false;
                    println!("[{}] !!! CONNECTION LOST !!!", client.name);
                } else if !is_offline && !client.is_online {
                    client.is_online = true;
                    println!("[{}] *** CONNECTION RESTORED ***", client.name);
                }
            }

            let is_currently_online = { c_unstable.lock().await.is_online };

            if is_currently_online {
                let patch_opt = q_unstable_clone.lock().await.pop();
                if let Some(patch) = patch_opt {
                    let delay = { c_unstable.lock().await.delay };
                    sleep(delay).await; // Slow network delay

                    let mut client = c_unstable.lock().await;
                    // Double check online state after delay
                    if client.is_online {
                        client.state.apply_patch(&patch);
                        client.msg_received += 1;
                        println!(
                            "[{}] Received conflated patch (recovered): {:?}",
                            client.name,
                            patch.updates.keys().collect::<Vec<_>>()
                        );
                    } else {
                        // Put it back to queue if we got offline during delay
                        q_unstable_clone.lock().await.push(patch);
                    }
                }
            }
            sleep(Duration::from_millis(50)).await;
        }
    });

    // Wait for the server to finish updating fields (4 seconds)
    let _ = server_task.await;

    // Give workers an extra 1.5 seconds to drain remaining conflated queues
    sleep(Duration::from_millis(1500)).await;

    // Terminate worker tasks
    fast_task.abort();
    slow_task.abort();
    unstable_task.abort();

    // 4. Print final summary and audit results
    let s_state = server_state.lock().await;
    let fast = client_fast.lock().await;
    let slow = client_slow.lock().await;
    let unstable = client_unstable.lock().await;

    println!("\n=== SIMULATION RESULTS & AUDIT ===");
    println!("Server Final State Fields: {:?}", s_state.fields);
    println!("--------------------------------------------------");

    println!("{}:", fast.name);
    println!(" - Messages Received: {}", fast.msg_received);
    println!(" - Final State Fields: {:?}", fast.state.fields);
    println!(" - State Matches Server? {}", fast.state == *s_state);

    println!("{}:", slow.name);
    println!(" - Messages Received: {}", slow.msg_received);
    println!(" - Final State Fields: {:?}", slow.state.fields);
    println!(" - State Matches Server? {}", slow.state == *s_state);

    println!("{}:", unstable.name);
    println!(" - Messages Received: {}", unstable.msg_received);
    println!(" - Final State Fields: {:?}", unstable.state.fields);
    println!(" - State Matches Server? {}", unstable.state == *s_state);

    // Verify all clients eventually reached the identical final state as the server
    assert!(
        fast.state == *s_state,
        "Fast client failed to match server state"
    );
    assert!(
        slow.state == *s_state,
        "Slow client failed to match server state via conflation"
    );
    assert!(
        unstable.state == *s_state,
        "Unstable client failed to match server state after reconnects"
    );

    println!("\n=== SUCCESS: All clients successfully reached the server's final state ===");
}
