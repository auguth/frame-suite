# Pallet Authors

An economically-backed **role system** for runtimes that need active participants such as validators, operators, or contributors.

Authors are actors who commit collateral, can receive backing, and participate in selection processes.

It is designed for managing **author/validator-style roles** (e.g. staking systems) as a **drop-in runtime module**, exposing enrolled participants that other pallets can use for activities and duties.

## Why use this pallet

Use `pallet_authors` when your runtime needs:

* Participants with configured **collateral**
* Support for **external backing / funding**
* A structured **role lifecycle** (`enroll -> active -> resign`)
* **Deterministic accounting** for rewards and penalties
* **Pluggable elections** to select active participants
* A **drop-in role module** for managing validators/authors and their stake using commitment-based primitives

## How it works (at a glance)

```text
account -> collateral + backing -> influence -> election
```

* users enroll with collateral
* others can fund them
* influence is derived from economic backing
* selected authors are exposed for use by other runtime modules

All funds are handled via the **commitment system** (preferrably via [`pallet_commitment`](https://crates.io/crates/pallet-commitment)), ensuring consistent locking and resolution.

### Lifecycle

```text
enroll -> probation -> permanence -> resign
```

* **Enroll**: join with collateral
* **Probation**: initial safety window
* **Permanence/Active**: eligible for participation
* **Resign**: exit and recover collateral

### Funding

Authors can be backed via:

* self-collateral
* direct funders
* index / pool commitments

This allows flexible economic participation while maintaining consistent accounting.

### Elections

Selection is **runtime-configurable** via plugin-based models:

* **Flat**: computes a single influence value per author by aggregating all backing (including self-collateral)
* **Fair**: evaluates authors based on individual backing contributions, preserving each funder’s weight

Election behavior is defined by runtime-selected plugin models.

## Adding to your runtime

```toml
pallet-authors = { path = "../pallets/authors", default-features = false }
```

```rust
impl pallet_authors::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;

    type Asset = pallet_balances::Pallet<Self>;
    type CommitmentAdapter = pallet_commitment::Pallet<Self>;

    type ActivityProvider = pallet_chain_manager::Pallet<Self>;

    type InfluenceModel = frame_plugins::LinearModel;
    type InfluenceContext = frame_plugins::MyInfluenceContext;

    type FlatElectionModel = frame_plugins::TopDownFlatModel;
    type FairElectionModel = frame_plugins::PhragmenModel;

    type ....
}
```

## How you use it in your runtime

You typically:

* allow users to **enroll as authors**
* allow others to **fund authors**
* run **elections** periodically
* use selected authors in your protocol logic
* define and apply rewards / penalties from external modules

This pallet provides the **authorization + economic layer**, while your runtime defines behavior.

## Extrinsics

* `enlist`: enroll with collateral
* `demit`: resign
* `refill`: add collateral
* `back`: fund an author
* `draw`: withdraw backing

## Notes

* Defines **economically secured role primitives**, not execution logic
* Uses **commitment system** for all fund-related operations
* Election and influence are **fully pluggable**
* Designed for **non-trusted systems with accountable participants**

## License

MPL-2.0 (Mozilla Public License)

An Open-Source initiative by **Auguth Labs (OPC) Pvt Ltd, India**
