---
toc_min_heading_level: 2
toc_max_heading_level: 2
---

# 🎧 Listeners

Listeners are the extension layer of `pallet-xp`.

They allow external logic to react to XP lifecycle changes without modifying the core pallet itself.

The long-term intention is:

> one unified extension model for both runtime and off-chain integrations

This means listeners are intended to support:

* runtime-to-runtime integrations
* off-chain services
* indexers
* automation layers
* protocol adapters
* external execution systems

However, today, only the **runtime listener layer** is implemented.

So currently:

```text
Listeners = runtime code only
```

not off-chain extensions.

This system should still be treated as **experimental**.

---

## Why Listeners Exist

XP should remain a core primitive, not a monolithic business-logic pallet.

That means:

* XP handles identity
* XP handles Pulse
* XP handles reserve / lock rules
* XP handles lifecycle safety

but external systems may still need to react to XP changes.

Examples:

* staking logic reacting to XP progression
* governance modules tracking contributor reputation
* reward systems extending XP earning
* protocol hooks triggered by lifecycle changes

Listeners provide this integration layer.

They are:

> extensions, not replacements.

---

## How Listeners Work

Internally, XP trait methods trigger hooks like:

```rust
on_xp_earn(...)
on_xp_transfer(...)
on_reserve_update(...)
on_lock_update(...)
```

which forward into:

```rust
Self::Extension::...
```

This allows external runtime logic to react without changing pallet internals.

Example:

```text
earn_xp() -> on_xp_earn() -> Extension::xp_earned() -> your runtime logic
```

This keeps:

```text
core protocol logic != extension behavior
```

cleanly separated.

---

## What You Must Implement

To build a custom listener, your extension type must implement:

```rust
frame_suite::xp::XpSystemExtensions
```

and then the specific listener traits required by your runtime.

Common listener traits include:

| Trait               | Purpose                        |
| ------------------- | ------------------------------ |
| `XpOwnerListener`   | ownership changes              |
| `XpMutateListener`  | create / earn / slash / update |
| `XpReserveListener` | reserve updates / slashes      |
| `XpLockListener`    | lock updates / slashes         |
| `XpReapListener`    | lifecycle finalization         |
| `BeginXpListener`   | initialization flow            |

You only implement what your protocol actually needs.

---

## Example Runtime Extension

```rust 
pub struct MyXpExtension;

impl XpSystemExtensions for MyXpExtension {
    type Via = pallet_xp::Pallet<Runtime>;
}

impl XpMutateListener for MyXpExtension {
    fn xp_earned(key: &XpId, earned: Points) {
        // custom runtime reaction
    }
}
```

Then wire it into runtime config:

```rust
impl pallet_xp::Config for Runtime {
    type Extension = MyXpExtension;
}
```

Now XP lifecycle events trigger your runtime logic automatically.

---

## Current Status

The intended future design is:

```text
Runtime code + Off-chain integrations + External extension providers
```
through a unified extension architecture.

But today: **only runtime listeners exist**.

This means:

* runtime listeners are available now
* off-chain compatibility is still under design

There is not yet a finalized shared extension interface.

So the current system should be treated as:

> experimental runtime extensions

not the final public extension model.

---

## Future Direction

A unified extension model is planned.

This will provide cleaner support for:

* runtime listeners
* off-chain integrations
* async automation layers
* indexer compatibility
* external execution adapters
* shared extension interfaces

with better consistency across both runtime and external systems.

More formal documentation for this unified model will be provided soon.

Until then:

> listeners should be treated as runtime-only and experimental.

---

## Production Recommendation

Because the extension model is still evolving:

production systems should prefer:

```rust
type Extension = frame_suite::Ignore<pallet_xp::Pallet>;
```

unless runtime extensions are absolutely required.

This avoids:

* unstable extension contracts
* migration complexity later
* unexpected runtime coupling
* evolving API surface risks

Until the unified extension architecture is finalized:

> production should default to `Ignore`

and treat custom listeners as advanced experimental integrations.

This is the safest production strategy.

---

## 🚀 Next Steps

Now that extensions are clear, the next step is understanding advanced protocol integrations like staking models and XP-backed execution flows.

👉 **Advanced -> [Staking](./staking.md)**
