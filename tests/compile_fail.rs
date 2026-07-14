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

/// D-13 structural guard: no FROST secret type converts into `IdentityKeypair`,
/// so transport↔FROST reuse is non-expressible (T-02-05). The `.stderr` snapshot
/// pins the *reason* (a missing `From` impl), not merely that it fails to build.
#[test]
fn identity_has_no_frost_conversion() {
    trybuild::TestCases::new().compile_fail("tests/ui/identity_no_frost_conversion.rs");
}
