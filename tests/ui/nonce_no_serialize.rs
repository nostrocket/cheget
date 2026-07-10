// SIGN-05 compile-fail proof: an `EphemeralNonces` value MUST NOT be
// serializable. `serde_json::to_vec` requires its argument to implement
// `serde::Serialize`; `EphemeralNonces` implements no serialization trait, so
// this program must fail to compile (E0277: `EphemeralNonces: Serialize` is not
// satisfied). If this file ever compiles, the highest-severity structural
// control in the project has regressed.

fn main() {
    let nonce: tsig::crypto::EphemeralNonces = unimplemented!();
    let _ = serde_json::to_vec(&nonce);
}
