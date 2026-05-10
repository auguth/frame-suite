# FRAME Plugins

Concrete, reusable plugin models for [`frame_suite`](https://crates.io/crates/frame-suite).

FRAME Plugins provides ready-to-use **plugin types and families** that plug into the abstractions defined in `frame_suite`.

> Where `frame_suite` defines *what is possible*, this crate provides the **types used to configure and realize that pluggable-behaviors**.

## Overview

This crate acts as the **behavior configuration layer** of the FRAME Suite ecosystem.

* `frame_suite`: **traits, semantics, abstractions**
* `frame_plugins`: **plugin models (types) used within those traits**

It does **not implement the traits directly**.
Instead, it provides the **associated types and models** that are plugged into trait implementations.

These models are **optional and extensible**, where runtimes are not constrained to this crate.
Custom plugin models can be defined independently and plugged in the same way.

All **plugins** are:

* **Generic**: work across different runtimes and types
* **Composable**: can be combined into larger systems
* **Plugin-driven**: selected via associated types
* **Context-aware**: configurable without changing logic

Plugins are defined using macros `plugin_model!` and grouped into **families** via `define_family!`.

## Usage

Add dependency:

```toml
[dependencies]
frame-plugins = { path = "../frame-plugins", default-features = false }
```

Use alongside `frame_suite`:

```rust
use frame_suite::assets::*;
use frame_plugins::*;
```

## Plugging into Runtime

Plugins are **wired at the type level** inside pallet/runtime configuration.

`frame_suite` traits expose **associated types (models)**,
this crate provides concrete implementations for those types.

Example:

```rust
impl pallet_asset::Config for Runtime {
    type BalanceFamily<'a> = ShareBalanceFamily<'a>;
    type BalanceContext = MyBalanceContext<Commitment>;
}
```

```rust
impl pallet_validation::Config for Runtime {
    type InfluenceModel = LinearModel;
    type RewardModel = SharesPay;
    type InflationModel = ConstantPayout;
    type PenaltyModel = ThresholdPenalty;
}
```

Each plugin may optionally use a **context** (environment):

```rust
plugin_context!(
    name: MyConstantPayoutContext,
    context: ConstantPayoutConfig<u64>,
    value: ConstantPayoutConfig { payout: 100 }
);

impl pallet_rewards::Config for Runtime {
    type PayoutModel = ConstantPayout;
    type PayoutContext = MyConstantPayoutContext;
}
```

Here, the runtime **selects behavior by choosing types**, which are then used internally by trait implementations.

## Composition

Plugins are designed to be combined into pipelines:

* `balances`: manages deposit vaults
* `influence`: transform input weight
* `elections`: select participants
* `rewards`: compute and distribute value
* `penalty`: normalize penalties

Example flow:

```text
balances -> influence -> election -> reward -> penalty
```

Each stage is independently replaceable.

## Design Principles

* **Behavior via types**: logic is selected through associated types
* **Families as execution surfaces**: group related models
* **Well-defined contracts**: all models conform to `frame_suite` expectations
* **No hidden coupling**: models remain fully independent

## Notes

* Requires `frame_suite`
* Does not define storage or extrinsics, used inside pallets
* Provides **plug-in models**, not trait implementations
* Not restrictive - custom models can be implemented and plugged without using this crate

## License

MPL-2.0 (Mozilla Public License)

An Open-Source initiative by **Auguth Labs (OPC) Pvt Ltd, India**
