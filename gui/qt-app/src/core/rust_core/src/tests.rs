//! Small unit tests for shared path helpers and profile validation.

#[test]
fn profile_delete_rejects_traversal_before_touching_disk() {
    assert!(crate::profile::delete_profile("../etc").is_err());
    assert!(crate::profile::delete_profile(".").is_err());
    assert!(crate::profile::delete_profile("a/b").is_err());
}

#[test]
fn profile_functions_reject_empty_components() {
    assert!(crate::profile::apply_profile_impl("glass", Some(vec![])).is_err());
    // Validation happens before the script is executed, so no profile needs
    // to exist for this assertion.
    assert!(crate::profile::save_profile_impl("glass", Some(vec![])).is_err());
}
