---
toc_min_heading_level: 2
toc_max_heading_level: 2
---

# 🪢 Staking (Bonding)

This section is **not about `pallet-xp` staking logic**, as there isn't one.

Instead, it explains the recommended way to handle **bonding and staking** when XP should be economically committed rather than simply locked.

The correct path is:

> use `pallet-commitment`

not direct XP locks.

---

## What is `pallet-commitment`?

`pallet-commitment` is a generalized bonding layer pallet for fungible assets.

Instead of simple locks, it creates:

```text
Commitment = bond(asset) -> (reason, digest)
```

This means assets are not just locked, they are bonded with:

* a semantic reason
* a contextual digest
* controlled lifecycle rules

This allows much richer staking behavior than standard locks. 

It supports:

* direct commitments
* digest-level aggregation
* pooled commitments
* index-based commitments
* manager-controlled pools
* lazy resolution
* structured slashing / resolution flows

It is designed as a shared infrastructure pallet for staking and bonding systems. 

---

## Why Not Simple XP Locks?

`pallet-xp` supports:

* locks
* reserves
* lifecycle constraints

but these are intentionally simple.

For real staking systems, especially where XP should represent:

* bonded reputation
* validator commitment
* governance stake
* structured participation guarantees

simple locks are not enough.

You need:

* semantic bonding
* commitment reasons
* digest-based grouping
* pooled commitment models
* controlled resolution flows

This is exactly what `pallet-commitment` provides. 

---

## Why It Works

`pallet-commitment` is built on top of:

```rust
type Fungible: Mutate + Unbalanced + MuatetHold + MuatetFreeze` ; // and other unbalanced fungible traits
```

and `pallet-xp` already provides an **Xp Fungible Adapter** through its fungible implementation layer.

That means XP can behave as a fungible commitment asset for commitment-based bonding.

Also `pallet-balances` can use the same commitment system.

So both:

* token balances
* XP balances

can participate in the same staking architecture using the same bonding primitive. 

### Mental Model

- Balances: financial stake
- XP: reputation stake
- Commitment: semantic bond layer

This gives much better protocol design than raw locks.

---

## Typical Use Cases

Use commitment-backed XP for:

* validator reputation staking
* contributor bond systems
* DAO governance guarantees
* slashing-aware participation
* structured reputation escrow

not simple temporary locks.

---

## Final Recommendation

If your question is:

> should XP be locked for staking?

the answer is usually:

> use `pallet-commitment`

not direct `lock_xp()`.

This is the recommended production path.

Simple locks are for constraints.

Commitments are for protocol staking.

---

## 🚀 Next Steps

Now that staking architecture is clear, the next step is future protocol directions and planned ecosystem expansion.

👉 **Advanced -> [Upcoming](./upcoming.md)**
