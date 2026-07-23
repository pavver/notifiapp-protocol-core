use notifiapp_protocol_core::Diffable;
use uuid::Uuid;

#[derive(Debug, Clone, Diffable, PartialEq)]
pub struct SubConfig {
    #[diff(required)]
    pub sub_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Diffable, PartialEq)]
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
