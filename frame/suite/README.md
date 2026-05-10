# FRAME Suite

A composable, type-driven foundation for building modular runtime systems on Substrate.

FRAME Suite is a collection of reusable, **fully generic abstractions** for expressing common runtime patterns.

It is built around a simple idea:

> Build runtimes as compositions of semantics, not implementations.

Instead of coupling storage, logic, and structure, FRAME Suite separates them into **traits, types, and pluggable execution**, allowing systems to remain flexible, extensible, and reusable across pallets.

## Core Ideas

* **Generic by design**: all components are abstract and type-driven
* **Plugin-based execution**: is defined via pluggable models
* **Virtual structures**: data is described by schema, not fixed layout
* **Routine-driven workflows**: structured offchain execution with best-effort guarantees

These principles enable building systems that evolve without redesign.

## Usage

FRAME Suite is **not a pallet**, it is meant to be used inside your pallets.

Add dependency:

```toml
[dependencies]
frame-suite = { path = "../frame-suite", default-features = false }
```

Import modules:

```rust
use frame_suite::{assets::*, commitment::*, xp::*};
```

Implement traits:

```rust
pub struct MyBalance;

impl LazyBalance for MyBalance {
    type Asset = Balance;
    type Rational = FixedU128;
    type Time = BlockNumber;
}
```

Use in dispatchables:

```rust
pub fn deposit(origin, amount: Balance) -> DispatchResult {
    let who = ensure_signed(origin)?;
    // integrate FRAME Suite logic
    Ok(())
}
```

## Composition

FRAME Suite modules are designed to interoperate:

* `assets` + `commitment` : **staking, escrow, bonded systems**
* `xp` + `roles` : **reputation-driven roles and governance**
* `blockchain` + `elections` : **validator lifecycle and rewards**
* `accumulators` + `xp` : **progression and leveling systems**
* `virtuals` + `plugins` : **logic and structural extensibility**
* `forks` + `routines` : **best-effork fork-aware offchain workers**

Complex systems emerge by composing small, orthogonal primitives.

## Notes

* No storage is imposed, defined by the implementing pallet
* No genesis configuration, handled by the runtime
* No extrinsics, exposed through your pallet

## License

MPL-2.0 (Mozilla Public License)

An Open-Source initiative by **Auguth Labs (OPC) Pvt Ltd, India**
