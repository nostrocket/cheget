//! Compile-fail harness proving the signing-nonce type is non-serializable
//! (SIGN-05). The `.stderr` snapshot in `tests/ui/` is the reviewable artifact:
//! it pins the *reason* the program is rejected (a missing `Serialize` bound),
//! not merely that it fails to build.
//!
//! Regenerate the snapshot after an intentional change with:
//! `TRYBUILD=overwrite cargo test --test compile_fail`.

#[test]
fn nonce_is_not_serializable() {
    trybuild::TestCases::new().compile_fail("tests/ui/nonce_no_serialize.rs");
}
