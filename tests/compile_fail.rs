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

/// STOR-02 store-side structural guard: the checkpoint store's concrete-typed
/// persist methods reject a signing nonce, so a nonce is a non-expressible
/// checkpoint input (T-02-10). The `.stderr` snapshot pins the *reason* (a type
/// mismatch — `EphemeralNonces` is not a dkg round `SecretPackage`).
#[test]
fn checkpoint_rejects_nonce_material() {
    trybuild::TestCases::new().compile_fail("tests/ui/checkpoint_no_nonce.rs");
}

/// STOR-02 store-side structural guard: the checkpoint store exposes no generic
/// `persist<T: Serialize>` sink. The `.stderr` snapshot pins the *reason* (no
/// such method exists), proving there is no generic escape hatch.
#[test]
fn checkpoint_has_no_generic_persist() {
    trybuild::TestCases::new().compile_fail("tests/ui/checkpoint_no_generic_persist.rs");
}
