// D-13 compile-fail proof: there is NO conversion from any FROST secret type
// into the transport `IdentityKeypair`. Reusing FROST key material as the Nostr
// identity must be non-expressible (T-02-05), exactly as reusing a signing nonce
// is non-expressible (SIGN-05).
//
// `.into()` requires a `From<SigningShare> for IdentityKeypair` impl to exist;
// none does, so this program must fail to compile (E0277: the trait bound
// `IdentityKeypair: From<SigningShare>` is not satisfied). If this file ever
// compiles, the transport↔FROST structural separation has regressed.

fn main() {
    let share: frost_secp256k1_tr::keys::SigningShare = unimplemented!();
    let _id: cheget::store::IdentityKeypair = share.into();
}
