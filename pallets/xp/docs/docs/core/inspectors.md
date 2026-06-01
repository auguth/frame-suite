---
toc_min_heading_level: 2
toc_max_heading_level: 2
---

# 🧪 Inspectors

Inspectors are **development-only extrinsics** provided by `pallet-xp` behind the:

```toml
feature = "dev"
```

feature gate.

They exist for:

* local development
* testnets
* QA environments
* debugging runtime integrations
* rapid validation during pallet development

They are intentionally **not part of production runtime design**.

---

## Why Inspectors Exist

Most XP state should be accessed through:

* runtime APIs
* RPC queries
* indexers
* off-chain clients
* UI integrations

That is the correct production architecture.

However, during development, constantly writing custom RPC calls or querying storage manually slows iteration.

Inspectors provide:

> thin wrapper extrinsics for direct state visibility

so developers can quickly validate behavior using normal transactions and events. 

- They are convenience tools, not protocol logic.
- They exist to help developers debug, not to serve end users.
- The `dev` feature must be disabled in production runtimes.

This is a hard operational rule.

---

## Available Inspector Extrinsics

When the `dev` feature is enabled, these additional dispatchables are compiled:

| Extrinsic              | Origin | Purpose                                    |
| ---------------------- | ------ | ------------------------------------------ |
| `inspect_my_xp()`      | Signed | Emit current liquid XP for an owned `XpId` |
| `inspect_xp_keys_of()` | Signed | Emit all `XpId`s owned by an account       |

These extrinsics are excluded entirely when `dev` is disabled.

They do not exist in production builds.

---

## 1. `inspect_my_xp()` - Quick XP Balance Inspection

```rust
inspect_my_xp(origin, xp_id)
```

This allows the owner of an XP identity to quickly inspect the current **liquid XP balance**.

It is useful during:

* reward testing
* Pulse verification
* progression debugging
* lock / reserve validation
* integration testing

This does not mutate storage. It simply emits the current XP value through an event.

```rust
Event::Xp {
    id,
    xp
}
```

This allows quick inspection using:

* block explorers
* event logs
* polkadot.js
* test scripts
* local runtime debugging

without writing custom RPC endpoints.

---

## 2. `inspect_xp_keys_of()` - Inspect Ownership Mapping

```rust
inspect_xp_keys_of(origin, owner)
```

This emits all `XpId`s currently owned by a given account.

Useful for:

* ownership debugging
* verifying handovers
* testing genesis initialization
* validating multi-XP ownership
* account migration checks

Unlike `inspect_my_xp()`, ownership of the target account is not required.

Any signed caller can inspect ownership mappings in development mode. This is intentional for debugging convenience.

```rust
Event::XpOfOwner {
    owner,
    ids
}
```

This provides a full ownership snapshot at execution time. Very useful during integration testing.

---

## Emitting Events Instead of Returning Values

FRAME extrinsics do not return rich query responses like RPC endpoints.

Instead, inspectors use:

```text
dispatch -> emit event -> read result from chain events
```

This keeps them:

* simple
* compatible with standard tooling
* easy to test
* fast to debug

They are intentionally thin wrappers. No business logic lives here.

---

## Important Security Note

Because inspectors expose direct visibility shortcuts:

> they should never be enabled in production

Even read-only convenience extrinsics increase:

* attack surface
* event noise
* operational complexity
* unnecessary chain weight

The cleanest production runtime is:

```toml
default-features = false
dev = disabled
```

This keeps the public surface intentionally minimal.

---

## Final Insight

> 🧪 Inspectors help developers
> ⚙️ RPC serves production

That is the correct separation.

Use inspectors to move fast. Remove them before mainnet.

Always.

---

## 🚀 Next Steps

Now that the development inspection layer is clear, the next step is understanding the event model and how XP state changes become visible across the runtime.

👉 **Core -> [Events](./events.md)**
