//! Guards the platform naming contract: every `Target` has a canonical
//! target_os name that round-trips, and the canonical list matches the enum.

use rubyc::native::target::Target;

#[test]
fn names_round_trip() {
    for name in Target::ALL_TARGET_OS {
        let t = Target::from_target_os_name(name)
            .unwrap_or_else(|| panic!("'{name}' does not round-trip"));
        assert_eq!(t.target_os_name(), *name);
    }
    assert_eq!(Target::from_target_os_name("nope"), None);
}

#[test]
fn canonical_list_matches_enum_count() {
    assert_eq!(Target::ALL.len(), Target::ALL_TARGET_OS.len());
    for t in Target::ALL {
        assert!(Target::ALL_TARGET_OS.contains(&t.target_os_name()));
    }
}
