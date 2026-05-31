# Pallet XP

[![License](https://img.shields.io/badge/license-MPL--2.0-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021-orange?logo=rust)](https://www.rust-lang.org/)
[![Substrate](https://img.shields.io/badge/Substrate-Framework-E6007A)](https://docs.polkadot.com/)
[![Docs](https://img.shields.io/badge/Docs-docs.rs-7f5fff?style=flat-square&logo=docsdotrs)](https://docs.rs/pallet-xp)
[![Crates.io](https://img.shields.io/crates/v/pallet-xp?style=flat-square&color=orange)](https://crates.io/crates/pallet-xp)
[![Docs Site](https://img.shields.io/badge/Docs-Read_the_Docs-7f5fff?style=flat-square&logo=readthedocs&color=green)](https://auguth.github.io/frame-suite/pallet-xp/docs/)

A reputation-driven XP system for runtimes that need to measure **contribution, consistency, and participation** in non-trusted environments.

**XP (Experience Points)** - similar to how games measure player progression, this represents activity and reputation accumulated over time.

Unlike balances, XP is **earned through behavior** and evolves over time. It is designed for systems where trust cannot be assumed and must be built through activity.

## Why use this pallet

Use `pallet_xp` when your runtime needs:

* A **reputation layer** instead of a currency
* **Gamified progression** based on user activity
* Resistance to spam, farming, and short-term exploits
* A system where **consistent participation is rewarded more than bursts**
* Compatibility with pallets expecting **fungible trait interfaces**
* A **drop-in alternative to [`pallet_balances`](https://crates.io/crates/pallet-balances)** where reputation-based scoring is preferred over monetary value

XP is especially useful for:

* governance participation
* contribution tracking
* validator / actor reputation
* on-chain quantifiable-gamification

## How it works (at a glance)

XP is **key-based**, not account balance.

```text
Account -> XpId -> { liquid, reserve, lock, reputation }
```

Each XP key represents a unit of participation owned by an account.

### Earning model

XP does not grow linearly:

* early actions build **reputation (pulse)**
* once active, XP starts accumulating
* higher reputation gives higher rewards

```text
activity -> reputation -> scaled XP
```

XP is awarded externally (by runtime logic or other pallets) and internally scaled by the reputation system.

This ensures:

* no same-block spam
* no instant farming
* long-term engagement is rewarded

## Adding to your runtime

```toml
pallet-xp = { path = "../pallets/xp", default-features = false }
```

```rust
impl pallet_xp::Config<Instance> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;

    type ReserveReason = RuntimeReserveReason;
    type LockReason = RuntimeLockReason;

    type ....
}
```

You can instantiate multiple XP systems using pallet instances if needed.

## Configuring behavior

XP is controlled through runtime-configurable parameters:

* initial XP on creation
* reputation growth rate
* minimum reputation before earning XP
* inactivity threshold (for cleanup)

These allow you to tune:

* how fast users progress
* how strict reputation building is
* how the system reacts to inactivity

## How you use it in your runtime

You typically:

* create XP identities for users or entities
* award XP from other pallets (governance, staking, tasks, etc.)
* lock or reserve XP to represent commitment

An account can own **multiple XP keys**, each representing different intents or contexts (e.g., governance, staking, tasks).
These can be independently used by other pallets via trait adapters, making XP a **modular, intent-specific reputation layer**.

XP acts as a **shared reputation primitive** across your runtime.

## Extrinsics

* `handover`: transfer ownership of an XP identity
* `dispose`: remove inactive XP entries
* `call`: execute runtime logic scoped to an XP key

Root calls allow updating XP parameters at runtime.

## Notes

* XP is **not a token** (no transfer, no supply)
* XP must be **earned via runtime logic**
* Designed for **reputation, not value exchange**

## License

MPL-2.0 (Mozilla Public License)

An Open-Source initiative by **Auguth Labs (OPC) Pvt Ltd, India**
