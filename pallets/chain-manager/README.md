# Pallet Chain Manager

A runtime module for coordinating **validator (author) selection and session participation**.

This pallet connects **roles (author abstractions)**, elections, and sessions into a working validator system, handling **validator selection, activation, and settlement** across sessions.

> *Roles* refer to author/validator abstractions provided by a role-management pallet (typically [`pallet_authors`](https://crates.io/crates/pallet-authors) or any pallet implementing the [`frame_suite::roles`](https://crates.io/crates/frame-suite) traits).

## Why use this pallet

Use `pallet_chain_manager` when your runtime needs:

* A ready-to-use **validator selection + session coordination layer**
* **Session-based elections** for selecting active validators
* A **lazy participation model** where only interested candidates enter elections
* Automatic progression from **candidate -> validator -> settlement**
* Integration with **offchain workers for continuous operation**
* A system that connects **roles (stake-backed authors) + elections + rewards/penalties**

## How it works (at a glance)

```text 
authors -> affidavit -> election -> session validators -> rewards / penalties
```

* authors (from a role-management pallet like [`pallet_authors`](https://crates.io/crates/pallet-authors)) signal intent to participate
* offchain workers submit **affidavits** declaring current election weight (stake/backing)
* only authors with affidavits are considered in elections
* elections select validators for the next session
* selected validators become active via [`pallet_session`](https://crates.io/crates/pallet-session)
* rewards and penalties are applied over time

All transitions occur **at the correct point in the session lifecycle**.

## Affidavits (Lazy Election Model)

Instead of including all authors in elections, this pallet uses an **affidavit-based model**:

* authors **signal intent** (via extrinsic)
* offchain workers declare **current election weight** (self-collateral + backing)
* only declared candidates are included in the election

```text
intent -> affidavit -> eligible set -> election
```
## What this pallet does

* coordinates **election timing**
* builds the **eligible candidate set via affidavits**
* prepares and exposes the **next validator set**
* settles **rewards and penalties per session**
* integrates with offence reporting
* bridges runtime modules into a working validator flow

It acts as an **orchestration layer**, not a logic provider.

## What this pallet does NOT do

* does **not manage stake or roles**, uses a role pallet (e.g. [`pallet_authors`](https://crates.io/crates/pallet-authors))
* does **not define election/reward/penalty logic**, configure via plugins (e.g., [`frame_plugins`](https://crates.io/crates/frame-plugins))

It only **connects these pieces together**.

## Adding to your runtime

```toml
pallet-chain-manager = { path = "../pallets/chain-manager", default-features = false }
```

```rust
impl pallet_chain_manager::Config for Runtime {
    type RoleAdapter = pallet_authors::Pallet<Self>;
    type ElectionAdapter = pallet_authors::FairElection<Self>;
    type Asset = pallet_balances::Pallet<Self>;

    type RewardModel = frame_plugins::SharesPay;
    type InflationModel = frame_plugins::ConstantPayout;
    type PenaltyModel = frame_plugins::ThresholdPenalty;

    type ....
}
```

## Extrinsics

* `validate`: signal intent to participate as a session validator
* `chill`: opt out of validation participation

These are the **only user-driven actions**. All other operations are handled automatically by offchain workers and unsigned extrinsics internally.

## Notes

* **OCW-heavy pallet**, as most logic is executed via offchain workers
* Uses a **lazy election model via affidavits**
* Expects only **intent signaling from valid authors**
* **Tightly coupled with**:
  * [`pallet_session`](https://crates.io/crates/pallet-session): validator rotation and session lifecycle
  * [`pallet_authorship`](https://crates.io/crates/pallet-authorship): block production tracking
  * [`pallet_offences`](https://crates.io/crates/pallet-offences): offence reporting and penalty triggering
* Requires a role provider (e.g. [`pallet_authors`](https://crates.io/crates/pallet-authors))
* Fully **session-driven and deterministic**
* Works with **pluggable election, reward, and penalty models**

## License

MPL-2.0 (Mozilla Public License)

An Open-Source initiative by **Auguth Labs (OPC) Pvt Ltd, India**
