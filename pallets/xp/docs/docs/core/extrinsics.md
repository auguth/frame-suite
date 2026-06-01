---
toc_min_heading_level: 2
toc_max_heading_level: 2
---

# 🧭 Extrinsics

Extrinsics are the user-facing dispatchable calls of `pallet-xp`.

They are the official on-chain entry points for interacting with XP identities.

Unlike XP traits (used internally by runtime logic), extrinsics are:

* signed by users
* validated by the runtime
* weight-accounted
* event-emitting
* permission-checked

They form the public execution surface of the pallet. 

## Available Extrinsics

`pallet-xp` exposes only a small set of core production extrinsics:

| Extrinsic                | Origin | Purpose                          |
| ------------------------ | ------ | -------------------------------- |
| `call()`                 | Signed | execute runtime logic via `XpId` |
| `handover()`             | Signed | transfer XP ownership            |
| `dispose()`              | Signed | reap inactive XP                 |
| `force_handover()`       | Root   | force ownership transfer         |
| `force_genesis_config()` | Root   | update XP system parameters      |

> Dev-only inspector extrinsics are excluded here and will be covered separately under [**Inspectors**](./inspectors.md)

---

## 1. `call()` - Execute via XP Identity

```rust
call(origin, xp_id, runtime_call)
```

This is the most important extrinsic in the pallet.

It allows a user to execute runtime logic using an XP identity as the logical execution subject.

```text
origin = AccountId
input  = XpId, runtime-call
ensure owner(origin, XpId)
dispatch runtime-call as Signed(XpId)
```

This means:

* the account signs
* the XP identity executes

This is the foundation of the XP execution model as XP becomes the execution context, not just stored data. 

---

## 2. `handover()` - Transfer XP Ownership

```rust
handover(origin, xp_id, new_owner)
```

This transfers ownership of an XP identity to another account. The `XpId` remains the same. Only the owner changes.

Used for:

* identity migration
* account rotation
* operational handover
* governance transitions

This is the only user-facing "transfer" supported by XP. Not value transfer, ownership transfer.

---

## 3. `dispose()` - Reap Inactive XP

```rust
dispose(origin, owner, xp_id)
```

This permanently removes an inactive XP identity.

Before reaping, the runtime checks:

* XP exists
* XP is below `MinTimeStamp`
* XP has no active locks
* lifecycle rules allow reaping

If valid:

- XP is deleted
- added to `ReapedXp` storage
- can never be initialized again

This preserves lifecycle finality as Reaping is permanent 🚫.

---

## 4. `force_handover()` - Root Ownership Override

```rust
force_handover(origin, current_owner, xp_id, new_owner)
```

Root-only version of [`handover()`](#2-handover---transfer-xp-ownership).

Used by governance or runtime administrators when ownership must be transferred without the current owner.

Useful for:

* recovery operations
* governance intervention
* protocol-level corrections

This bypasses normal signed ownership flow.

---

## 5. `force_genesis_config()` - Update Runtime XP Parameters

```rust
force_genesis_config(origin, field)
```

This is a root-only administrative extrinsic.

It allows updating system-wide XP parameters stored in runtime storage.

Supported fields:

| Field          | Purpose                                     |
| -------------- | ------------------------------------------- |
| `MinPulse`     | minimum pulse required before rewards begin |
| `InitXp`       | initial XP for new identities               |
| `PulseFactor`  | pulse growth speed                          |
| `MinTimeStamp` | inactivity threshold for reaping            |

This allows runtime tuning without requiring a migration. Should be used carefully.

---

## What Is Not an Extrinsic

Some important XP operations are intentionally **not** exposed as public extrinsics.

Examples:

* `begin_xp()`
* `earn_xp()`
* `set_xp()`
* `reserve_xp()`
* `lock_xp()`

These are trait methods used by:

* runtime logic
* other pallets
* internal integrations

not by direct users. This keeps XP secure and prevents reputation abuse.

Users interact through controlled extrinsics, while runtime logic uses traits.

---

## 🚀 Next Steps

Now that the public execution surface is clear, the next step is understanding the developer inspection layer.

👉 **Core -> [Inspectors](./inspectors.md)**
