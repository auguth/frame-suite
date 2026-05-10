# Pallet Commitment

A **reusable fungible bonding primitive** for runtimes that need to express **structured commitments over assets**.

This pallet provides a **generic commitment layer** where assets are bonded under a `(reason, digest)` pair, enabling multiple pallets to share and reuse the same bonding infrastructure.

It not only locks value, but also maintains a **dynamic bond state at the digest level**, allowing value to be adjusted (e.g. rewards or penalties) and accurately reflected on resolution.

## Why use this pallet

Use `pallet_commitment` when your runtime needs:

* A **reusable bonding/locking primitive** across multiple pallets
* Structured commitments instead of raw balance locks
* Shared infrastructure for staking, escrow, governance, or pooling systems
* Support for **unmanaged (index-based)** and **managed (pool-based)** commitments out-of-the-box
* Fine-grained control over how value is grouped, distributed, and resolved

## What it provides

This pallet generalizes bonding into a composable model:

```text
bond(asset) -> (reason, digest)
```

* **Reason**: why the asset is bonded (defined by consuming pallet)
* **Digest**: context identifier (staking position, pool, etc.)

Together, they form a **commitment**.

Each digest also maintains a **live aggregate value**, which can be adjusted over time:

```text
digest value -> adjusted (reward / penalty) -> reflected on resolve
```

## Core Model

```text
Proprietor -> Commitment(reason) -> Digest -> Value
```

Each account (proprietor):

* holds commitments per **reason**
* binds value to **digests**
* interacts through structured operations (place, raise, resolve)

### Types & Variants

The pallet supports three commitment models:

* **Direct**: single digest commitment
* **Index (Unmanaged)**: grouped digests with share-based distribution
* **Pool (Managed)**: grouped allocation managed by a pool owner (non-custodial), who can rebalance commitments

Commitments can optionally include a **position (variant)**:

```text
(reason, digest, position) -> value
```

Examples: long/short, affirmative/contrary, positive/negative.

Variants enable multiple positions on the same digest with independent value tracking, supporting use cases like directional staking, prediction markets, and multi-sided governance.

## How it works (at a glance)

```text
deposit -> commit -> (index | pool | direct) -> adjust -> resolve
```

* Assets are reserved for commitment
* Value is assigned to a digest (and optionally a position)
* Digest value can be adjusted over time (reward / penalty)
* Resolution reflects the final adjusted value

These adjustments are powered by **lazy balance abstractions** and **plugin-based balance models**, allowing flexible balance behavior without changing commitment logic.

## Key Features

* **Reusable across pallets**: shared bonding infrastructure
* **Fungible-agnostic**: works with any asset implementing fungible traits
* **Lazy evaluation**: values reflect live state and are resolved on demand
* **Structured grouping**: via indexes and pools
* **Positional commitments**: support multiple semantic positions per digest
* **Dynamic value adjustment**: digest-level rewards and penalties
* **Explicit imbalance handling**: rewards and penalties are accounted safely

## Adding to your runtime

```toml
pallet-commitment = { path = "../pallets/commitment", default-features = false }
```

```rust
impl pallet_commitment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;

    type AssetHold = RuntimeHoldReason;
    type AssetFreeze = RuntimeFreezeReason;

    type Asset = pallet_balance::Pallet<Self>;

    type BalanceFamily = frame_plugins::ShareBalanceFamily;
    type BalanceContext = frame_plugins::ShareBalanceContext;

    type ....
}
```

Multiple instances can be used for different configurations if required.

## How you use it in your runtime

Typically, other pallets:

* define a **reason** (e.g., staking, escrow)
* generate a **digest** (context identifier)
* optionally define **positions (variants)**
* place commitments on behalf of users
* adjust digest values (reward / penalty logic)
* resolve commitments based on logic

This pallet acts as a **shared bonding backend**, while domain logic lives in consumer pallets.

## Reserve Model

This pallet uses a **native reserve (hold)** mechanism:

* users deposit into a commitment reserve
* commitments consume from this reserve
* can fallback to liquid balance if configured

This allows:

* efficient reuse of funds across commitments
* controlled execution via directive policies

## Extrinsics

This pallet exposes minimal direct extrinsics:

* deposit to commitment reserve
* withdraw from reserve
* basic read / helper calls

All **commitment logic is accessed via traits**, not direct user calls,
since consumer pallets define their own semantics and control how commitments are created, adjusted, and resolved.

## Notes

* Acts as a **bonding abstraction over fungible assets**
* Does not define business logic, whereas consumer pallets define meaning
* Supports **multiple instances and multiple reasons per instance**
* Designed to be **shared across runtime modules**

## License

MPL-2.0 (Mozilla Public License)

An Open-Source initiative by **Auguth Labs (OPC) Pvt Ltd, India**
