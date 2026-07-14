# Phase 2: Persistence & Storage - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-14
**Phase:** 2-persistence-storage
**Areas discussed:** Passphrase / unlock UX, On-disk store layout, Checkpoint lifecycle, Identity keypair scope

---

## Passphrase / unlock UX

### How is the passphrase supplied when a command decrypts a share?
| Option | Description | Selected |
|--------|-------------|----------|
| Interactive prompt each time | No-echo stdin prompt every command; no passphrase in env/memory | ✓ |
| Prompt + env-var override | Prompt by default, honor CHEGET_PASSPHRASE for CI | |
| Short-lived unlock agent | Daemon caches decrypted material for a TTL | |

### One passphrase for the whole store, or per key?
| Option | Description | Selected |
|--------|-------------|----------|
| One store passphrase | Single passphrase unlocks identity + all shares | ✓ |
| Per-key passphrase | ACTIVE vs STANDBY separately encrypted | |

### How do headless test/CI runs decrypt?
| Option | Description | Selected |
|--------|-------------|----------|
| Passphrase provider at API layer | PassphraseSource trait; prod=interactive-only, tests inject in-code; no env/flag in binary | ✓ |
| Test-gated env var | CHEGET_PASSPHRASE behind a testing cargo feature | |
| Documented env var (all builds) | Ship CHEGET_PASSPHRASE everywhere | |

### How is the passphrase first established?
| Option | Description | Selected |
|--------|-------------|----------|
| Confirm-twice, no strength policy | Prompt twice, require match; document no-recovery | ✓ |
| Confirm-twice + strength check | Also reject weak passphrases | |
| Single prompt, no confirmation | One prompt, typo locks share forever | |

**Notes:** Central tension is security vs convenience; user consistently chose the
minimal-attack-surface option and pushed testability into an API abstraction rather than a shipped
flag/env var.

---

## On-disk store layout

### How is the encrypted secret store organized?
| Option | Description | Selected |
|--------|-------------|----------|
| File-per-share tree + manifest | age file per (key_id, epoch, seat) + plaintext index | ✓ |
| Single encrypted store blob | One age file with a serialized map | |
| SQLite for participant too | Encrypted blobs as rows in a participant DB | |

### Manifest visibility + public package in-store?
| Option | Description | Selected |
|--------|-------------|----------|
| Plaintext manifest + public pkg in-store | Metadata + public PublicKeyPackage plaintext; address/status need no unlock | ✓ |
| Plaintext manifest, secrets only | Public artifact stays a separate --pubkey file | |
| Encrypted manifest | Even listing requires unlock | |

### Decrypted-material lifetime?
| Option | Description | Selected |
|--------|-------------|----------|
| Decrypt-use-drop per operation | Zeroizing, drop immediately after the op | ✓ |
| Held for one command | Zeroizing session struct across a command | |

### Write discipline?
| Option | Description | Selected |
|--------|-------------|----------|
| Atomic temp+rename, restrictive perms | temp+fsync+rename, 0700/0600, manifest last | ✓ |
| Direct write, restrictive perms | In-place write, no temp+rename | |

**Notes:** File-per-share maps directly onto the existing (key_id, epoch, seat) tagging and onto
Phase 4's new-epoch-dir-alongside-old rotation model.

---

## Checkpoint lifecycle

### When should between-round checkpointing engage?
| Option | Description | Selected |
|--------|-------------|----------|
| Build the capability, wire it at the seam | Build encrypted checkpoint store + persist/reload test; don't touch hot in-process DKG | ✓ |
| Refactor DKG to checkpoint each round now | Persist/reload each round even in-process | |
| Defer to Phase 7 | Only build the stores now | |

### What key encrypts the checkpoints?
| Option | Description | Selected |
|--------|-------------|----------|
| Same store passphrase | Passphrase-derived, survives restart | ✓ |
| Separate ceremony passphrase | Distinct passphrase per ceremony | |

### When are checkpoints deleted?
| Option | Description | Selected |
|--------|-------------|----------|
| Wipe on success, keep on abort | Resume per (ceremony_id, round, seat) | ✓ |
| Wipe on both success and abort | No resume | |
| Keep until explicitly cleared | Manual clean command | |

### Where do checkpoints live?
| Option | Description | Selected |
|--------|-------------|----------|
| Participant store, ceremony subdir | Per-participant secrets stay with participant persona | ✓ |
| Coordinator SQLite store | Centralize ceremony state in coordinator DB | |

**Notes:** Discussed the tension that Phase 1's DKG runs all seats in one call with no pause —
resolved by building the durable seam + a test now, real between-round use in Phase 7. STOR-02's
nonce-exclusion half is already satisfied by EphemeralNonces.

---

## Identity keypair scope

### Generate + store the transport-only identity keypair now?
| Option | Description | Selected |
|--------|-------------|----------|
| Generate + store now | secp256k1 key, independent of FROST, encrypted in identity slot | ✓ |
| Reserve the slot, defer generation | Slot only, generate in Phase 7 | |

### How is identity↔FROST separation enforced?
| Option | Description | Selected |
|--------|-------------|----------|
| Distinct type + independent draw | Own newtype, independent OsRng, no cross-conversion API | ✓ |
| Distinct type + runtime assertion | Also assert pubkey ≠ any FROST key | |

### Which crate for the identity keypair?
| Option | Description | Selected |
|--------|-------------|----------|
| secp256k1 (already in graph) | C-lib crate via rust-bitcoin; same family as Phase 7 Nostr keys | ✓ |
| Raw 32 secret bytes, defer crate | Store bytes, pick type in Phase 7 | |
| Pull in nostr-sdk Keys now | Exact Phase 7 alignment, drags in transport dep | |

### Does the coordinator roster store real npubs now?
| Option | Description | Selected |
|--------|-------------|----------|
| Populate npubs now | Derived from generated identities; roster real & testable | ✓ |
| Nullable npub, fill in Phase 7 | Truer to out-of-band flow, leaves roster partial | |

**Notes:** User treats the identity↔FROST separation as effectively a fourth structural control,
in the spirit of Phase 1's four.

---

## Claude's Discretion

- `age::scrypt` work-factor / recipient wiring; exact `Zeroizing` boundaries.
- `manifest.json` schema + versioning; secure-delete mechanism for wiped checkpoints.
- **Coordinator SQLite specifics (STOR-03):** full table schema, migration strategy, DB location,
  single-writer discipline, transcript/session-log record shapes — in scope for Phase 2 but left
  to planning, bounded by the persona rule (no secret material in coordinator DB).
- Old-epoch retention beyond "hold ≥2" (active pruning is Phase 4).

## Deferred Ideas

- Coordinator SQLite schema depth (in Phase 2 scope, not deep-dived here).
- Old-epoch pruning policy (Phase 4).
- Unlock agent / passphrase caching (rejected for Phase 2; revisit only if re-prompt friction
  proves painful).
