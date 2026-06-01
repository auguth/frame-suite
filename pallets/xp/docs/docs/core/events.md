---

toc_min_heading_level: 2
toc_max_heading_level: 2
title: 📡 Events
---

# 📡 Events

Events are how `pallet-xp` makes state changes visible across the runtime.

They are used by:

* wallets
* block explorers
* indexers
* frontend apps
* test environments
* debugging tools

They are emitted through:

```rust
deposit_event(...)
```

and become part of normal Substrate runtime visibility.

But in `pallet-xp`, not all events are treated equally.

That behavior is controlled by:

```rust
type EmitEvents: Get<bool>
```

inside the [`Config` trait](../getting-started/configuration.md). 

---

## Event Strategy

There are two event layers:

| Layer                     | Purpose                                     |
| ------------------------- | ------------------------------------------- |
| Extrinsic Events          | Required protocol-visible state transitions |
| Internal Lifecycle Events | Verbose XP mutation visibility              |

This distinction is important.

---

## `EmitEvents = false` (Production)

```rust
type EmitEvents = ConstBool<false>;
```

When disabled:

> only extrinsic events are emitted

This is the recommended production setup. 

Why?

Because emitting every internal XP mutation:

* increases execution weight
* adds unnecessary event noise
* makes indexing expensive
* creates operational overhead

Production systems should prefer:

```text
RPC queries + minimal critical events
```

not verbose mutation logs.

---

## `EmitEvents = true` (Development)

```rust
type EmitEvents = ConstBool<true>;
```

When enabled:

> all lifecycle mutation events are emitted

including:

* XP earned
* XP locked
* XP reserved
* XP slashed
* ownership updates
* reaping
* internal state changes

This is ideal for:

* local development
* testnets
* mock runtimes
* QA
* benchmarking validation
* quick debugging

It provides fast introspection without needing custom RPC inspection.

---

## Extrinsic Events

These are protocol-level events caused by dispatchable calls.

These should remain visible.

| Event                  | Triggered By                        | Meaning                           |
| ---------------------- | ----------------------------------- | --------------------------------- |
| `XpOwner`              | `handover()` / `force_handover()`   | Ownership assigned or transferred |
| `XpReap`               | `dispose()`                         | XP permanently reaped             |
| `GenesisConfigUpdated` | `force_genesis_config()`            | Runtime XP parameters changed     |
| `Xp`                   | `inspect_my_xp()` *(dev only)*      | Inspector read result             |
| `XpOfOwner`            | `inspect_xp_keys_of()` *(dev only)* | Inspector ownership snapshot      |

Important:

When `EmitEvents = false`, these still remain visible where explicitly emitted by extrinsics like:

* `handover()`
* `dispose()`
* root administrative calls

because protocol visibility must remain intact. 

---

## Internal Lifecycle Events

These are emitted by trait methods during internal XP mutation flows.

They are useful for development, but usually too noisy for production.


| Event            | Triggered By             | Meaning               |
| ---------------- | ------------------------ | --------------------- |
| `Xp`             | create / direct mutation | XP created or updated |
| `XpEarn`         | `earn_xp()`              | XP earned             |
| `XpSlash`        | slash operations         | XP reduced            |
| `XpLock`         | `set_lock()`             | XP locked             |
| `XpLockBurn`     | lock removal             | Lock removed          |
| `XpLockSlash`    | lock slash               | Locked XP slashed     |
| `XpReserve`      | `set_reserve()`          | XP reserved           |
| `XpReserveSlash` | reserve slash            | Reserved XP slashed   |
| `XpOwner`        | internal owner changes   | Ownership event       |
| `XpReap`         | internal reap logic      | Reap completion       |

These are excellent for debugging:

```text
Why didn't XP increase?
What triggered this lock?
Did reserve happen correctly?
```

but should not be the primary production read model.

---
## Listeners vs Events

A common misunderstanding:

```text
EmitEvents.false != Hooks disabled
```

Even when events are disabled:

* [listeners](../advanced/listeners.md) still execute
* `Config::Extensions` still run
* lifecycle hooks still fire

Only event emission changes. This is explicitly guaranteed by the pallet design. 

So:

```text
Events = visibility

Listeners = runtime reactions
```

Events make state changes observable. [Listeners](../advanced/listeners.md) allow runtime logic to react to those changes.

They are separate systems.

A common anti-pattern is building runtime logic by inspecting emitted events and adding conditional behavior around them.

That should be avoided.

Instead of:

```text
Emit event -> inspect event -> trigger logic
```

the correct design is:

```text
State change -> listener hook -> runtime reaction
```

This is exactly why listeners are provided. They allow pallets and integrations to react directly to XP lifecycle changes without relying on event parsing.

This is:

* cleaner
* cheaper
* deterministic
* safer for production
* independent of `Config::EmitEvents`

> Use events for visibility and Use listeners for behavior.

---

## All Events Table

| Event                  | Fields       | Category             | Represents                                 |
| ---------------------- | ------------ | -------------------- | ------------------------------------------ |
| `Xp`                   | `id, xp`     | Inspector / Internal | Current XP value for an identity           |
| `XpOwner`              | `id, owner`  | Extrinsic + Internal | Ownership assignment or transfer           |
| `XpOfOwner`            | `owner, ids` | Inspector            | All XP identities owned by an account      |
| `XpEarn`               | `id, amount` | Internal             | XP earned through progression              |
| `XpReap`               | `id`         | Extrinsic + Internal | Permanent deletion of an XP identity       |
| `XpSlash`              | `id, amount` | Internal             | XP reduced due to slash logic              |
| `XpLock`               | `id, amount` | Internal             | XP locked and made temporarily unavailable |
| `XpLockBurn`           | `id, amount` | Internal             | Locked XP removed or released              |
| `XpLockSlash`          | `id, amount` | Internal             | Locked XP reduced through slashing         |
| `XpReserve`            | `id, amount` | Internal             | XP moved into reserved state               |
| `XpReserveSlash`       | `id, amount` | Internal             | Reserved XP reduced through slashing       |
| `GenesisConfigUpdated` | `config`     | Root Extrinsic       | Global XP configuration updated by root    |

This is the complete pallet event surface. 

---

## 🚀 Next Steps

Now that event visibility is clear, the next step is understanding how production clients should query XP using runtime APIs and UI integrations.

👉 **Core -> [RPC + UI](./rpc-ui.md)**
