//! In-process FROST DKG generic over `(t, n)` + even-Y normalization +
//! client-side group-key confirmation (KEY-01, KEY-02, KEY-05, KEY-06).
//!
//! Runs `keys::dkg::part1/2/3` across `n` simulated seats **entirely in one
//! process** — no transport, no I/O (KEY-02). The n=100 acceptance target
//! (D-02) is proven correct by the `#[ignore]` `dkg_1000_correctness` test
//! (KEY-06); this module is generic so the same code paths run at 3-of-5 for
//! fast tests (D-01) and 51-of-100 for the real run.
//!
//! Both output packages are normalized to even-Y via `into_even_y(None)` (D-11)
//! so the group key is the canonical BIP340/341 Taproot **internal** key `P`
//! that `bridge::address_from_group_key` accepts.
//!
//! **Purity:** this module imports no chain / transport / filesystem code. The
//! secret shares it returns live only in the caller's process; only the public
//! `PublicKeyPackage` is ever written to disk (D-09), by the CLI layer.

use std::collections::BTreeMap;

use frost_secp256k1_tr as frost;
use frost::keys::dkg;
use frost::keys::{EvenY, KeyPackage, PublicKeyPackage};
use frost::rand_core::{CryptoRng, RngCore};
use frost::{Error as FrostError, Identifier};
use rayon::prelude::*;

/// Errors from the in-process DKG and client-side confirmation.
#[derive(Debug)]
pub enum KeygenError {
    /// A `frost` DKG primitive returned an error (invalid params, bad package).
    Frost(FrostError),
    /// A seat identifier could not be built from its index (identifiers are
    /// nonzero and bounded by the curve order).
    Identifier(u16),
    /// Client-side confirmation found a seat whose verifying key disagrees with
    /// the group verifying key — the ceremony MUST abort (KEY-05).
    GroupKeyMismatch {
        /// The seat whose `KeyPackage` verifying key did not match the group.
        seat: Identifier,
    },
    /// The DKG produced no seats (`max_signers == 0`).
    Empty,
}

impl std::fmt::Display for KeygenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeygenError::Frost(e) => write!(f, "frost DKG error: {e}"),
            KeygenError::Identifier(i) => {
                write!(f, "could not build a FROST identifier from seat index {i}")
            }
            KeygenError::GroupKeyMismatch { seat } => write!(
                f,
                "group-key confirmation failed: seat {seat:?} disagrees with the \
                 group verifying key (KEY-05 abort)"
            ),
            KeygenError::Empty => write!(f, "DKG produced no seats (max_signers == 0)"),
        }
    }
}

impl std::error::Error for KeygenError {}

/// Run the full in-process FROST DKG across `max_signers` simulated seats using
/// the OS CSPRNG, returning every seat's (even-Y) [`KeyPackage`] and the single
/// group [`PublicKeyPackage`].
///
/// See [`run_inprocess_dkg_with_rng`] for a deterministic-RNG variant used by
/// tests. `min_signers` is the threshold `t`; `max_signers` is the membership
/// `n` (the real target is `t = 51`, `n = 100` — D-02).
pub fn run_inprocess_dkg(
    min_signers: u16,
    max_signers: u16,
) -> Result<(BTreeMap<Identifier, KeyPackage>, PublicKeyPackage), KeygenError> {
    let mut rng = frost::rand_core::OsRng;
    run_inprocess_dkg_with_rng(min_signers, max_signers, &mut rng)
}

/// As [`run_inprocess_dkg`], but driven by a caller-supplied RNG (deterministic
/// seeds make DKG failures reproducible in tests).
pub fn run_inprocess_dkg_with_rng<R: RngCore + CryptoRng>(
    min_signers: u16,
    max_signers: u16,
    rng: &mut R,
) -> Result<(BTreeMap<Identifier, KeyPackage>, PublicKeyPackage), KeygenError> {
    // ---- Determinism decision (why round 1 stays sequential and rounds 2/3 are
    // parallel) ----
    //
    // `dkg::part1` is the ONLY round that consumes the RNG, so round 1 stays a
    // sequential `for` loop that threads `&mut *rng` through seats in exactly the
    // current order — the caller-supplied RNG is consumed identically on every
    // run, preserving bit-for-bit reproducible failures for both the `_with_rng`
    // and `OsRng` paths.
    //
    // `dkg::part2` / `dkg::part3` consume NO randomness — they are pure
    // deterministic functions of the round-1 output — so rounds 2 & 3 are
    // parallelized across seats with rayon without touching determinism. Error
    // attribution is made deterministic independently of thread scheduling by
    // collecting each parallel round into an ORDER-PRESERVING `Vec<Result<..>>`
    // (indexed Vec par-iter preserves input order) and then resolving failures
    // SEQUENTIALLY in ascending-id order: the first `?` yields the smallest-id
    // Frost error, and `GroupKeyMismatch { seat }` reports the smallest-id
    // mismatching seat.
    //
    // The O(n^2) per-seat clone of the round-1 package map is eliminated by
    // rayon's `map_with`: each worker clones the round-1 pool ONCE, then for each
    // seat removes its own entry (yielding "all others"), calls the primitive,
    // and re-inserts its entry — mirroring the pattern proven in the ignored
    // `dkg_1000_correctness` test.

    // ---- Round 1 (sequential — the only RNG-consuming round) ----
    let mut r1_secret: Vec<(Identifier, dkg::round1::SecretPackage)> =
        Vec::with_capacity(max_signers as usize);
    let mut r1_pkgs: BTreeMap<Identifier, dkg::round1::Package> = BTreeMap::new();
    for i in 1..=max_signers {
        let id: Identifier = i.try_into().map_err(|_| KeygenError::Identifier(i))?;
        // part1 takes the RNG by value; hand it a mutable reborrow so the same
        // RNG threads through every seat.
        let (secret, pkg) =
            dkg::part1(id, max_signers, min_signers, &mut *rng).map_err(KeygenError::Frost)?;
        r1_secret.push((id, secret));
        r1_pkgs.insert(id, pkg);
    }

    // ---- Round 2 (parallel across seats; no RNG) ----
    // Each worker clones the round-1 pool once (`map_with`), then per seat
    // removes/re-inserts its own entry to present "all others" to `part2`.
    #[allow(clippy::type_complexity)]
    let r2_computed: Vec<
        Result<
            (
                Identifier,
                dkg::round2::SecretPackage,
                BTreeMap<Identifier, dkg::round2::Package>,
            ),
            KeygenError,
        >,
    > = r1_secret
        .into_par_iter()
        .map_with(r1_pkgs.clone(), |pool, (id, secret)| {
            let self_pkg = pool.remove(&id).expect("seat present in round-1 pool");
            let (secret2, sent) = dkg::part2(secret, pool).map_err(KeygenError::Frost)?;
            pool.insert(id, self_pkg);
            Ok((id, secret2, sent))
        })
        .collect();

    // Resolve round-2 results SEQUENTIALLY in ascending-id order (order-preserving
    // Vec): the first `?` returns the smallest-id Frost error deterministically.
    // Fan the round-2 packages out by recipient (cheap) and keep the secrets.
    let mut r2_secret: Vec<(Identifier, dkg::round2::SecretPackage)> =
        Vec::with_capacity(r2_computed.len());
    let mut r2_by_recipient: BTreeMap<Identifier, BTreeMap<Identifier, dkg::round2::Package>> =
        BTreeMap::new();
    for res in r2_computed {
        let (id, secret2, sent) = res?;
        for (recipient, pkg) in sent {
            r2_by_recipient.entry(recipient).or_default().insert(id, pkg);
        }
        r2_secret.push((id, secret2));
    }

    // ---- Round 3 (parallel across seats; no RNG) ----
    // Each seat derives its KeyPackage + the group PublicKeyPackage; all must
    // agree on the group verifying key (KEY-06 correctness invariant). Same
    // clone-once-per-worker + remove/re-insert pool trick as round 2.
    let empty: BTreeMap<Identifier, dkg::round2::Package> = BTreeMap::new();
    let r2_by_recipient_ref = &r2_by_recipient;
    let empty_ref = &empty;
    #[allow(clippy::type_complexity)]
    let r3_computed: Vec<Result<(Identifier, KeyPackage, PublicKeyPackage), KeygenError>> =
        r2_secret
            .into_par_iter()
            .map_with(r1_pkgs.clone(), |pool, (id, secret2)| {
                let self_pkg = pool.remove(&id).expect("seat present in round-1 pool");
                let r2_for_me = r2_by_recipient_ref.get(&id).unwrap_or(empty_ref);
                let (kp, pubkeys) =
                    dkg::part3(&secret2, pool, r2_for_me).map_err(KeygenError::Frost)?;
                pool.insert(id, self_pkg);

                // D-11: normalize both packages to even-Y before anything bridges/signs.
                let kp = kp.into_even_y(None);
                let pubkeys = pubkeys.into_even_y(None);
                Ok((id, kp, pubkeys))
            })
            .collect();

    // ---- Group-key confirmation (sequential, deterministic) ----
    // Walk the ordered round-3 results smallest-id first: the first result sets
    // the reference group key; any later seat whose verifying key differs is the
    // smallest-id mismatch and aborts (KEY-05/KEY-06). The first `?` still yields
    // the smallest-id Frost error from round 3.
    let mut key_packages: BTreeMap<Identifier, KeyPackage> = BTreeMap::new();
    let mut group: Option<PublicKeyPackage> = None;
    for res in r3_computed {
        let (id, kp, pubkeys) = res?;
        match &group {
            Some(g) => {
                if g.verifying_key() != pubkeys.verifying_key() {
                    return Err(KeygenError::GroupKeyMismatch { seat: id });
                }
            }
            None => group = Some(pubkeys),
        }
        key_packages.insert(id, kp);
    }

    let group = group.ok_or(KeygenError::Empty)?;
    Ok((key_packages, group))
}

/// Client-side group-key confirmation (KEY-05): assert that **every** seat's
/// `KeyPackage` verifying key equals the group verifying key. Any mismatch is a
/// hard abort — the coordinator is untrusted, so this check is client-side and
/// mandatory, not advisory.
pub fn confirm_group_key(
    packages: &BTreeMap<Identifier, KeyPackage>,
    group: &PublicKeyPackage,
) -> Result<(), KeygenError> {
    for (id, kp) in packages {
        if kp.verifying_key() != group.verifying_key() {
            return Err(KeygenError::GroupKeyMismatch { seat: *id });
        }
    }
    Ok(())
}
