use notifiapp_protocol_core::Diffable;
use uuid::Uuid;

#[derive(Debug, Clone, Diffable, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SubConfig {
    #[diff(required)]
    pub sub_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Diffable, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UserProfile {
    #[diff(required)]
    pub id: Uuid,

    pub display_name: String,

    #[diff(immutable)]
    pub created_at: u64,

    // Inlined nested Diffable struct (detected automatically!)
    pub config: SubConfig,
}

#[test]
fn test_to_full_patch() {
    let sub_id = Uuid::new_v4();
    let profile_id = Uuid::new_v4();

    let profile = UserProfile {
        id: profile_id,
        display_name: "Pavlo".to_string(),
        created_at: 123456789,
        config: SubConfig {
            sub_id,
            name: "Default".to_string(),
        },
    };

    let full_patch = profile.to_full_patch();

    assert_eq!(full_patch.id, profile_id);
    assert_eq!(full_patch.display_name, Some("Pavlo".to_string()));
    // Immutable field must be None in the patch (or default Option value)
    assert_eq!(full_patch.created_at, None);
    assert_eq!(
        full_patch.config,
        Some(SubConfigPatch {
            sub_id,
            name: Some("Default".to_string())
        })
    );
}

#[test]
fn test_incremental_diff_and_apply() {
    let sub_id = Uuid::new_v4();
    let profile_id = Uuid::new_v4();

    let mut old_profile = UserProfile {
        id: profile_id,
        display_name: "Pavlo".to_string(),
        created_at: 123456789,
        config: SubConfig {
            sub_id,
            name: "Default".to_string(),
        },
    };

    let new_profile = UserProfile {
        id: profile_id,
        display_name: "Pavlo Updated".to_string(), // changed
        created_at: 999999999,                     // changed but immutable (should be ignored)
        config: SubConfig {
            sub_id,
            name: "Default".to_string(), // not changed
        },
    };

    // Calculate diff
    let patch = old_profile.diff(&new_profile).expect("Should have diff");

    // Verify patch details
    assert_eq!(patch.id, profile_id);
    assert_eq!(patch.display_name, Some("Pavlo Updated".to_string()));
    assert_eq!(patch.created_at, None); // immutable field remains None
    assert_eq!(patch.config, None); // nested struct config was not changed

    // Apply patch
    old_profile.apply_patch(&patch);

    // Verify mutated state
    assert_eq!(old_profile.display_name, "Pavlo Updated");
    assert_eq!(old_profile.created_at, 123456789); // immutable field stayed unchanged
    assert_eq!(old_profile.config.name, "Default");
}

#[test]
fn test_nested_incremental_diff() {
    let sub_id = Uuid::new_v4();
    let profile_id = Uuid::new_v4();

    let mut old_profile = UserProfile {
        id: profile_id,
        display_name: "Pavlo".to_string(),
        created_at: 123456789,
        config: SubConfig {
            sub_id,
            name: "Default".to_string(),
        },
    };

    let new_profile = UserProfile {
        id: profile_id,
        display_name: "Pavlo".to_string(), // not changed
        created_at: 123456789,
        config: SubConfig {
            sub_id,
            name: "Custom Settings".to_string(), // nested field changed
        },
    };

    // Calculate diff
    let patch = old_profile.diff(&new_profile).expect("Should have diff");

    assert_eq!(patch.id, profile_id);
    assert_eq!(patch.display_name, None);
    assert_eq!(
        patch.config,
        Some(SubConfigPatch {
            sub_id,
            name: Some("Custom Settings".to_string())
        })
    );

    // Apply patch
    old_profile.apply_patch(&patch);
    assert_eq!(old_profile.config.name, "Custom Settings");
}

use notifiapp_protocol_core::diff::{VecOp, VecPatch};

#[test]
fn test_vec_scalar_diff() {
    use notifiapp_protocol_core::diff::GetPatchType;

    let old_vec = vec![1u32, 2, 3];
    let new_vec = vec![1u32, 4, 3, 5]; // 2 -> 4, add 5 at the end

    let patch = <Vec<u32> as GetPatchType>::resolve_diff(&old_vec, &new_vec);

    assert_eq!(
        patch,
        VecPatch::Incremental(vec![
            VecOp::Update {
                index: 1,
                patch: Some(4)
            },
            VecOp::Insert { index: 3, value: 5 },
        ])
    );

    let mut target = old_vec;
    <Vec<u32> as GetPatchType>::resolve_apply(&mut target, &patch);
    assert_eq!(target, vec![1, 4, 3, 5]);

    // Test deletion from end
    let shorter_vec = vec![1u32, 4];
    let delete_patch = <Vec<u32> as GetPatchType>::resolve_diff(&target, &shorter_vec);
    assert_eq!(
        delete_patch,
        VecPatch::Incremental(vec![VecOp::Remove { index: 3 }, VecOp::Remove { index: 2 },])
    );

    <Vec<u32> as GetPatchType>::resolve_apply(&mut target, &delete_patch);
    assert_eq!(target, vec![1, 4]);
}

#[test]
fn test_vec_nested_diffable_diff() {
    use notifiapp_protocol_core::diff::GetPatchType;

    let sub_id1 = Uuid::new_v4();
    let sub_id2 = Uuid::new_v4();

    let old_vec = vec![
        SubConfig {
            sub_id: sub_id1,
            name: "Sub1".to_string(),
        },
        SubConfig {
            sub_id: sub_id2,
            name: "Sub2".to_string(),
        },
    ];

    let new_vec = vec![
        SubConfig {
            sub_id: sub_id1,
            name: "Sub1".to_string(),
        },
        SubConfig {
            sub_id: sub_id2,
            name: "Sub2 Updated".to_string(),
        }, // changed name
    ];

    let patch = <Vec<SubConfig> as GetPatchType>::resolve_diff(&old_vec, &new_vec);

    match &patch {
        VecPatch::Incremental(ops) => {
            assert_eq!(ops.len(), 1);
            match &ops[0] {
                VecOp::Update {
                    index,
                    patch: nested_patch,
                } => {
                    assert_eq!(*index, 1);
                    assert_eq!(
                        nested_patch,
                        &Some(SubConfigPatch {
                            sub_id: sub_id2,
                            name: Some("Sub2 Updated".to_string()),
                        })
                    );
                }
                _ => panic!("Expected VecOp::Update"),
            }
        }
        _ => panic!("Expected VecPatch::Incremental"),
    }

    let mut target = old_vec;
    <Vec<SubConfig> as GetPatchType>::resolve_apply(&mut target, &patch);
    assert_eq!(target[1].name, "Sub2 Updated");
}

#[test]
fn test_vec_deep_nesting_diff() {
    use notifiapp_protocol_core::diff::GetPatchType;

    let sub_id = Uuid::new_v4();
    let profile_id = Uuid::new_v4();

    let old_vec = vec![UserProfile {
        id: profile_id,
        display_name: "Pavlo".to_string(),
        created_at: 12345,
        config: SubConfig {
            sub_id,
            name: "Old Config".to_string(),
        },
    }];

    let new_vec = vec![UserProfile {
        id: profile_id,
        display_name: "Pavlo".to_string(),
        created_at: 12345,
        config: SubConfig {
            sub_id,
            name: "Deep Updated Config".to_string(), // nested changed
        },
    }];

    let patch = <Vec<UserProfile> as GetPatchType>::resolve_diff(&old_vec, &new_vec);

    match &patch {
        VecPatch::Incremental(ops) => {
            assert_eq!(ops.len(), 1);
            match &ops[0] {
                VecOp::Update {
                    index,
                    patch: user_patch,
                } => {
                    assert_eq!(*index, 0);
                    let user_patch = user_patch.as_ref().unwrap();
                    assert_eq!(user_patch.id, profile_id);
                    assert_eq!(user_patch.display_name, None);
                    assert_eq!(
                        user_patch.config,
                        Some(SubConfigPatch {
                            sub_id,
                            name: Some("Deep Updated Config".to_string()),
                        })
                    );
                }
                _ => panic!("Expected VecOp::Update"),
            }
        }
        _ => panic!("Expected VecPatch::Incremental"),
    }

    let mut target = old_vec;
    <Vec<UserProfile> as GetPatchType>::resolve_apply(&mut target, &patch);
    assert_eq!(target[0].config.name, "Deep Updated Config");
}
