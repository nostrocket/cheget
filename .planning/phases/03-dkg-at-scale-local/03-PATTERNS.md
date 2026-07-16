# Phase 3: DKG at Scale — Local - Pattern Map

**Mapped:** 2026-07-16
**Files analyzed:** 3 modified (`cli/keygen.rs`, `cli/sign.rs`, `cli/mod.rs`) + 1 small export edit (`store/mod.rs`)
**Analogs found:** 3 / 3 (all strong — this is pure wiring; every API already exists)

> Phase 3 writes almost no new logic. It rewires two existing CLI handlers to call
> already-proven store APIs. The best analogs are two **tests** that already perform
> the exact loops the handlers need: `store_checkpoint_n100.rs` (the writer loop) and
> `common/mod.rs::run_confirmed_key_spend` (the reader→sign→confirm loop). The planner
> should treat those two tests as copy-from templates.

## File Classification

| Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---------------|------|-----------|----------------|---------------|
| `src/cli/keygen.rs` | controller (CLI handler) | file-I/O (write) | `tests/store_checkpoint_n100.rs` lines 72–98 (put_share loop) + `store/participant.rs::put_share` | exact (loop already written in test) |
| `src/cli/sign.rs` | controller (CLI handler) | file-I/O (read) → request-response | `tests/common/mod.rs::run_confirmed_key_spend` lines 102–174 + `store/participant.rs::load_share` | exact (sign pipeline) + role-match (load loop) |
| `src/cli/mod.rs` | route + entry point | request-response | existing `resolve_root` / `run_share_status` / `run_roster` (same file) + `store/passphrase.rs::InteractivePassphrase` | exact (in-file precedent) |
| `src/store/mod.rs` | config (module re-exports) | n/a | existing `pub use passphrase::{...}` line 39 | trivial (add one name) |

## Pattern Assignments

### `src/cli/keygen.rs` (controller, file-I/O write)

**What changes:** `run()` at line 66 currently does `let (_shares, group) = run_inprocess_dkg(t, n)?;` (line 74) — it drops the secret shares and writes only the public envelope (lines 76–78). Phase 3 keeps the DKG call but **stops discarding `_shares`** and persists every seat through a `ParticipantStore` per D-02/D-03.

**Primary analog — the writer loop already exists** in `tests/store_checkpoint_n100.rs` lines 73–87:
```rust
let (shares, group) = run_inprocess_dkg(t, n).expect("in-process DKG");
assert_eq!(shares.len(), n as usize, "DKG must yield n shares");

let store = ParticipantStore::new(
    root.clone(),
    Box::new(InCodePassphrase::new(passphrase)),   // <- Phase 3: InteractivePassphrase::for_new_store()
);

// Persist every seat's KeyPackage (encrypted) + the group public envelope.
for (&seat, key_package) in &shares {
    let tag = ShareTag::new(KeyId::active(), Epoch::GENESIS, seat);
    store
        .put_share(&tag, key_package, &group, ShareState::Active)
        .expect("put_share");
}
```

**The exact API to call** — `ParticipantStore::put_share` (`src/store/participant.rs` lines 85–117):
```rust
pub fn put_share(
    &self,
    tag: &ShareTag,
    key_package: &KeyPackage,
    group: &PublicKeyPackage,
    state: ShareState,
) -> Result<(), StoreError>
```
It already writes the plaintext public envelope first, then the encrypted share, then the manifest last (D-07 ordering) — the handler does NOT re-implement any of that. `ShareTag::new(key_id, epoch, seat)` is at lines 52–59; `KeyId::active()` / `Epoch::GENESIS` come from `crate::crypto::types` (types.rs lines 27, 104).

**D-03 topology (100 separate roots):** each seat gets its OWN store rooted at `<base>/seat-NNNN/`. So the loop constructs a **new `ParticipantStore` per seat** (one root each), not one store holding 100 tagged shares. The `store_checkpoint_n100` loop above uses a single root — that is the byte-faithfulness template for `put_share`, NOT the topology. Wrap it: `for (&seat, kp) in &shares { let root = base.join(format!("seat-{:04}", i)); let store = ParticipantStore::new(root, passphrase_source_for_this_seat); store.put_share(&ShareTag::new(KeyId::active(), Epoch::GENESIS, seat), kp, &group, ShareState::Active)?; }`

**Passphrase (D-04) — prompt ONCE, reuse for all 100 roots:** `ParticipantStore::new` takes `Box<dyn PassphraseSource>` (participant.rs line 73). To reuse one entered passphrase across 100 stores WITHOUT re-prompting, do not hand each store a fresh `InteractivePassphrase` (that would prompt 100×). Resolve the `SecretString` once and wrap it in an `InCodePassphrase`-style source, OR make a small `PassphraseSource` that caches the first `passphrase()` result. See Shared Patterns → Passphrase.

**Existing imports/arg pattern to preserve** (keygen.rs lines 9–48): keep `KeygenArgs`, `resolve_tn()` (lines 51–62), the `--full`/`--seats`/`--threshold` surface, and the `(t,n)` validation guard (lines 68–70). The public-envelope write (lines 76–78) MAY remain — but `put_share` already writes the same envelope via `PubkeyEnvelope::from_package` (participant.rs lines 137–150), so the standalone `--out` write is now redundant for the store path (Claude's Discretion per CONTEXT D-02).

**CLI handler error idiom** (keygen.rs lines 68–69): errors bubble as `Box<dyn std::error::Error>` via `CliResult`; `StoreError` implements `std::error::Error` (store/mod.rs line 105) so `store.put_share(...)?` composes with `?` directly.

---

### `src/cli/sign.rs` (controller, file-I/O read → request-response)

**What changes:** `run()` at line 95 currently calls `run_inprocess_dkg(t, n)?` (line 109) to fabricate a fresh key set every invocation. Phase 3 replaces the persisted-share path so it **loads 51 KeyPackages from stores** (D-05) instead of re-running the DKG, then drives the same `SigningSession`.

**Primary analog — the full sign→confirm pipeline** in `tests/common/mod.rs::run_confirmed_key_spend` lines 102–174. The load step replaces its line 107 `run_inprocess_dkg` with a load loop; everything from the address bridge (line 111) through `session.run(true)` (line 162) and the regtest confirm assertions (lines 164–173) is the D-05 acceptance template. The session construction (lines 153–161):
```rust
let transport = InMemoryTransport::new();
let mut session = SigningSession::new(
    "e2e-key-spend",
    &transport,
    key_packages,          // <- Phase 3: assembled from load_share, not fresh DKG
    group,                 // <- Phase 3: from a loaded public envelope
    psbt,
    t as usize,
    Network::Regtest,
);
let signed = session.run(true).expect(...);
```

**The exact load API** — `ParticipantStore::load_share` (`src/store/participant.rs` lines 124–130):
```rust
pub fn load_share(&self, tag: &ShareTag) -> Result<KeyPackage, StoreError> {
    let passphrase = self.passphrase.passphrase()?;
    let ciphertext = std::fs::read(self.share_path(tag)).map_err(StoreError::Io)?;
    let plaintext = decrypt_secret(&passphrase, &ciphertext)?;   // Zeroizing, wiped at fn end (D-06)
    KeyPackage::deserialize(&plaintext).map_err(StoreError::Frost)
}
```
The load loop the reader assembles (adapted from `store_checkpoint_n100.rs` lines 90–98, which loads-and-verifies the full set):
```rust
let mut key_packages: BTreeMap<Identifier, KeyPackage> = BTreeMap::new();
for seat in selected_51 {                         // D-05: 51 of 100 roots
    let store = ParticipantStore::new(base.join(seat_dir), passphrase_source);
    let tag = ShareTag::new(KeyId::active(), Epoch::GENESIS, seat_id);
    key_packages.insert(seat_id, store.load_share(&tag)?);
}
```

**The group public package — load WITHOUT unlock:** `SigningSession::new` needs a `PublicKeyPackage` (session/mod.rs line 154). Get it from the plaintext envelope, no passphrase, via `ParticipantStore::load_public_envelope` (participant.rs lines 157–165) → `envelope.decode_package()` (used in participant.rs test line 298). This is the "one canonical address-derivation path" (participant.rs lines 132–136).

**Which 51 to load (Claude's Discretion, CONTEXT lines 104–105):** first-51 / liveness-sim / configurable — the `SigningSession` only needs a `t`-sized (or larger, it selects `t` via `liveness_select`, session/mod.rs lines 234–259) `BTreeMap`. Loading exactly 51 satisfies D-05.

**Preserve the display-before-sign gate** (sign.rs lines 85–92, 130–140): `prompt_ack()` + `session.preview()` + `session.run(true)`. SIGN-07 recompute happens inside `round2` (session/mod.rs lines 343–353) — the handler must NOT bypass it. `--yes` stays automation/regtest-only (sign.rs lines 59–61).

**Preserve** the PSBT read (`read_psbt`, sign.rs lines 79–83), the `--psbt` requirement error (lines 96–99), and the address print via `address_from_group_key` (line 111, `crate::bridge`).

---

### `src/cli/mod.rs` (route + passphrase entry point)

**What changes:** add the passphrase-prompting entry point for the writer/reader, and (per D-04) resolve the store base for the 100 simulated roots. The `Persona`/`ParticipantCmd`/`CoordinatorCmd` tree (lines 32–66) and dispatch `match` (lines 148–164) already route `Keygen`/`Sign` to `keygen::run` / `sign::run` — no tree change needed unless new args are added.

**Analog — CLI handler + root resolution already in this file.** `resolve_root` (lines 89–94):
```rust
fn resolve_root(home: Option<std::path::PathBuf>) -> Result<std::path::PathBuf, StoreError> {
    match home {
        Some(path) => Ok(path),
        None => Ok(StoreRoot::resolve()?.path().to_path_buf()),
    }
}
```
`run_share_status` (lines 97–112) and `run_roster` (lines 115–137) are the in-file templates for a handler that resolves a root then calls the store layer. `StoreRoot::resolve()` honors `CHEGET_HOME` (store/mod.rs lines 140–152); the 100-sim-roots base vs. real single-root path is Claude's Discretion (CONTEXT line 109).

**Passphrase construction — production path** (`src/store/passphrase.rs`):
- New store (writer, D-04): `InteractivePassphrase::for_new_store()` (lines 75–77) → prompts twice, prints the unrecoverability warning, requires a match (impl lines 88–98). **This is the `for_new_store` path CONTEXT says unblocks Phase 2 UAT Test 1.**
- Existing store (reader): `InteractivePassphrase::for_unlock()` (lines 69–71) → single no-echo prompt (impl lines 83–86).

Both are `#[cfg(not(test))]` (passphrase.rs lines 60, 80). The CLI compiles in non-test builds, so this is fine at the binary; but see "No Analog / Gotchas" for the export + test-injection caveat.

**Entry-point error mapping** already exists (`src/main.rs` lines 11–19): `Cli::run() -> CliResult` maps `Err` to `ExitCode::FAILURE` with `eprintln!("cheget: error: {err}")`. No change needed — `StoreError` and `KeygenError`/`SessionError` all impl `std::error::Error`.

---

### `src/store/mod.rs` (re-export edit — do not miss)

Line 39 currently re-exports only the test/trait passphrase types:
```rust
pub use passphrase::{InCodePassphrase, PassphraseSource};
```
`InteractivePassphrase` is defined (`#[cfg(not(test))]`, passphrase.rs lines 60–100) but **NOT re-exported**. The CLI cannot name `InteractivePassphrase::for_new_store()` until it is added here (or referenced via the full `crate::store::passphrase::` path). Smallest change: add it under the same cfg, e.g. `#[cfg(not(test))] pub use passphrase::InteractivePassphrase;`.

## Shared Patterns

### Passphrase acquisition (prompt-once, D-04)
**Source:** `src/store/passphrase.rs` (trait lines 28–35; `InteractivePassphrase` lines 60–100; `InCodePassphrase` lines 39–52)
**Apply to:** both `keygen` and `sign` handlers
`ParticipantStore::new(root, Box<dyn PassphraseSource>)` (participant.rs line 73) is the injection seam. The trait method is `fn passphrase(&self) -> Result<SecretString, StoreError>` — every `put_share`/`load_share` calls it (participant.rs lines 92, 125). To satisfy D-04 (**one prompt, 100 roots**) without prompting 100×, resolve the `SecretString` once and reuse it. Two clean options for the planner:
1. Prompt once via `InteractivePassphrase::for_new_store().passphrase()?`, then hand each per-seat store a cached source (an `InCodePassphrase`-shaped wrapper around the already-resolved `SecretString`). `InCodePassphrase::new(impl Into<String>)` (passphrase.rs lines 43–45) shows the wrapper shape.
2. Introduce a tiny caching `PassphraseSource` that prompts on first `passphrase()` and memoizes — then share one `Box`-equivalent across seats.
Do NOT add an env/flag passphrase source — it is explicitly forbidden (passphrase.rs lines 5–9; CONTEXT D-04).

### Store construction + tagging
**Source:** `src/store/participant.rs` (`ParticipantStore::new` line 73; `ShareTag::new` lines 52–59); `src/crypto/types.rs` (`KeyId::active()` line 27, `Epoch::GENESIS` line 104, `SeatId = frost::Identifier` line 119)
**Apply to:** both handlers. Genesis/`active` are the Phase-3 constants (single DKG, epoch 0 — types.rs line 97 note). Seats are the `Identifier` keys of the `BTreeMap` returned by `run_inprocess_dkg` (keygen.rs uses this map; types.rs line 119).

### DKG source (unchanged — do not re-implement)
**Source:** `src/crypto/keygen.rs::run_inprocess_dkg(min_signers, max_signers)` lines 71–77, returns `(BTreeMap<Identifier, KeyPackage>, PublicKeyPackage)`, both even-Y normalized (lines 182–183). The crypto core imports no fs/transport (lines 14–16) — keep persistence OUT of `crypto/` (CONTEXT "CLI routes, never computes", lines 173–180).

### Error idiom (manual, no thiserror)
**Source:** `src/store/mod.rs` `StoreError` lines 56–105 (`#[derive(Debug)]` + hand-written `Display` + empty `Error` impl); mirrored by `KeygenError` (crypto/keygen.rs lines 28–62) and `SessionError` (session/mod.rs lines 52–97). New handler errors, if any, follow this idiom; but handlers mostly just `?`-propagate into `CliResult = Result<(), Box<dyn std::error::Error>>` (cli/mod.rs line 20).

### Signing pipeline (unchanged mechanics)
**Source:** `src/session/mod.rs` `SigningSession::new` lines 150–183, `run` lines 404–422; `src/transport::InMemoryTransport`. The ONLY change for `sign` is the *source* of `key_packages`/`group` (store vs. fresh DKG) — the two-round flow, nonce discipline (session/mod.rs lines 19–23), and verify-against-Q are untouched.

## No Analog Found / Gotchas

No file lacks an analog — Phase 3 is pure wiring over proven APIs. But three planning hazards have no in-code precedent and must be called out:

| Concern | Role | Why it needs attention |
|---------|------|------------------------|
| `InteractivePassphrase` re-export | config | Defined but not exported from `store/mod.rs` (line 39). Must be added before the CLI can name it. |
| Prompt-once across 100 roots | utility | No existing caching `PassphraseSource`; per-seat `InteractivePassphrase` would prompt 100×, violating D-04. Planner must add a cache/reuse seam (see Shared Patterns → Passphrase). |
| Test injection under `#[cfg(test)]` | test | `InteractivePassphrase` is `#[cfg(not(test))]`. The D-06 small-n correctness test CANNOT construct it. Wiring must accept a `Box<dyn PassphraseSource>` (or `SecretString`) so tests inject `InCodePassphrase` (the pattern in `store_checkpoint_n100.rs` line 78 and `participant.rs` test line 266). Keep the interactive prompt at the thin CLI edge; make the persist/load loops passphrase-source-generic. |

**D-07 disposition note (CONTEXT lines 95–98, 108):** `tests/store_checkpoint_n100.rs::persist_reload_100` is `#[ignore]`d (line 58) and superseded as the criterion-3 vehicle by the real command. Its keep/retire status is a planning decision — but its lines 72–98 remain the authoritative copy-from template for the writer/reader loops regardless.

## Metadata

**Analog search scope:** `src/cli/`, `src/store/`, `src/crypto/`, `src/session/`, `tests/`, `tests/common/`
**Files scanned (read in full or targeted):** `cli/keygen.rs`, `cli/sign.rs`, `cli/mod.rs`, `store/participant.rs`, `store/passphrase.rs`, `store/mod.rs`, `crypto/keygen.rs`, `crypto/types.rs`, `session/mod.rs`, `main.rs`, `tests/inproc_sign_100.rs`, `tests/store_checkpoint_n100.rs`, `tests/common/mod.rs`
**Pattern extraction date:** 2026-07-16
