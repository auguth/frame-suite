---
toc_min_heading_level: 2
toc_max_heading_level: 2
---

# 🌐 RPC + UI

`pallet-xp` exposes a set of **public read-only pallet functions** designed specifically for:

* RPC providers
* frontend applications
* indexers
* dashboards
* explorers
* API layers
* off-chain services

These functions are accessed directly through:

```rust
pallet_xp::Pallet::<Runtime>::function(...)
```

where `Runtime` is the type which implements all pallets `Config` traits.

They are the correct production inspection surface.

### Why Public APIs Instead of Inspectors

Development runtimes may use [inspectors](./inspectors.md):

```text
inspect_my_xp()
inspect_xp_keys_of()
```

for fast debugging.

But these are:

* `dev`-gated extrinsics
* excluded from production builds
* event-based wrappers
* not intended for real applications

Production systems should use:

```text
RPC
-> Pallet public functions
-> direct state responses
```

not:

```text
Extrinsic
-> Event parsing
-> inferred state
```

This is cleaner, cheaper (offchain-calls), and deterministic. 

### Why RPC Providers Should Use These

These APIs are:

* read-only
* stable
* non-mutating
* lightweight
* directly aligned with UI needs

They avoid:

* unnecessary extrinsic submission
* chain event parsing
* verbose event replay
* inspector-only dev dependencies

This is exactly why they exist.

Even inspector extrinsics are only thin wrappers around these same functions.

For production:

> RPC should query pallet functions directly

not inspector events.

---

## Available Public APIs

These functions are implemented on:

```rust
impl Pallet<T> { ... }
```

and are intended for direct external usage.

## 1. `xp_state()` - Full XP Snapshot

```rust
Pallet::xp_state(&xp_id)
```

Returns the complete current XP state for an identity.

Includes:

* liquid XP
* reserved XP
* locked XP
* eligibility state
* effective multiplier

Best for:

* profile pages
* dashboards
* explorer views
* wallet summaries

This is usually the primary UI endpoint. 

---

## 2. `xp()` - Current Liquid XP

```rust
Pallet::xp(&xp_id)
```

Returns only the current **liquid (free, spendable)** XP.

Excludes:

* reserved XP
* locked XP

Best for:

* balance widgets
* quick status displays
* minimal UI reads

Simple and fast. 

---

## 3. `xp_keys()` - Owner -> XpId Mapping

```rust
Pallet::xp_keys(&owner)
```

Returns all `XpId`s owned by an account.

Best for:

* account profile pages
* ownership lookup
* multi-identity systems
* explorer account views

This replaces inspector ownership queries in production. 

---

## 4. `is_disposable()` - Reap Eligibility Check

```rust
Pallet::is_disposable(&xp_id)
```

Checks whether an XP identity can be safely reaped.

Validates:

* inactive timestamp
* no active locks
* lifecycle safety rules

Best for:

* admin panels
* governance tooling
* maintenance systems
* cleanup automation

Useful before calling `dispose()`. 

---

## 5. `xp_eligibility()` - Can XP Start Earning?

```rust
Pallet::xp_eligibility(&xp_id)
```

Returns whether XP is currently:

* already earning (`Earning`)
* still progressing toward activation (`Progressing(n)`)

This shows how many additional valid actions are required before rewards begin.

Best for:

* onboarding UX
* contributor progress
* gamification displays
* "how close am I?" views

Very important for user understanding. 

---

## 6. `xp_multiplier()` - Current Reward Multiplier

```rust
Pallet::xp_multiplier(&xp_id)
```

Returns the active Pulse multiplier if available.

Returns:

* `Some(multiplier)`: reward boost available
* `None`: no multiplier applies

Useful for:

* reward previews
* action estimators
* contributor incentive UIs

Helps explain why users earn more over time. 

---

## 7. `xp_progress()` - Pulse Progress Details

```rust
Pallet::xp_progress(&xp_id)
```

Returns:

* current Pulse level
* progress toward next level
* threshold
* per-action increment

Best for:

* progress bars
* level displays
* gamified contribution systems

Perfect for UI visualization. 

---

## 8. `earn_preview()` - Simulate XP Gain

```rust
Pallet::earn_preview(&xp_id, raw_points)
```

Simulates an `earn_xp()` action without changing storage.

Returns the resulting future XP state as if the action happened.

Best for:

* "preview reward" UI
* contribution estimators
* action planning
* governance previews

Extremely useful for frontend UX. 

---

## 9. `xp_last_earn()` - Last Activity Block

```rust
Pallet::xp_last_earn(&xp_id)
```

Returns the block number of the last successful `earn_xp()`.

Used for:

* same-block protection visibility
* activity monitoring
* cooldown displays
* debugging multiplier availability

Helpful for advanced interfaces. 

---

## Full API Table

| Function           | Returns              | Best For          |
| ------------------ | -------------------- | ----------------- |
| `xp_state()`       | Full XP snapshot     | Main dashboards   |
| `xp()`             | Liquid XP only       | Balance display   |
| `xp_keys()`        | Owner -> XpIds        | Account lookup    |
| `is_disposable()`  | Reap eligibility     | Admin checks      |
| `xp_eligibility()` | Activation status    | Progress tracking |
| `xp_multiplier()`  | Current multiplier   | Reward preview    |
| `xp_progress()`    | Pulse progression    | Gamified UI       |
| `earn_preview()`   | Simulated next state | UX previews       |
| `xp_last_earn()`   | Last earn block      | Activity tracking |

This is the full production inspection surface. 

---

## Final Insight

> 🧪 [Inspectors](./inspectors.md) are for development
> 🌐 Public APIs are for production

Inspector extrinsics help developers move fast.

Public pallet functions power real systems.

That separation keeps the runtime efficient, predictable, and maintainable.

---

## 🚀 Next Steps

Now that the full production read surface is clear, the next step is exploring advanced integrations like multi-instance XP systems and runtime listeners.

👉 **Advanced -> [Instances](../advanced/instances.md)**
