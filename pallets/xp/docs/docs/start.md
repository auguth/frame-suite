---
title: 🧭 Start Here
toc_min_heading_level: 2
toc_max_heading_level: 2
---

Different readers come to `pallet-xp` for different reasons.

- Some want to learn the model 🧠
- Some want to integrate it into their runtime ⚙️
- Some want to extend it 🔌
- Some want to modify the pallet itself 🛠️

This page helps you choose the right path quickly.

---

## 🧠 1. I Want to Learn How XP Works

Start here if you want to understand:

* what XP is
* why it exists
* how Pulse works
* how lifecycle + ownership work
* why XP is not a token

### 🔗 Direct Links

* 📘 [Intro](./intro)
* 🧩 [Concepts](./concepts/xp)
* 🏗️ [Architecture Overview](./architecture/overview)

Understand the model before writing code.

Most mistakes happen when XP is treated like balances. It is not.

---

## ⚙️ 2. I Want to Use XP in My Project

Start here if you want to:

* add XP to your runtime
* build governance reputation
* create contributor scoring
* use staking / commitment flows
* integrate XP into another pallet

### 🔗 Direct Links

* 📦 [Installation](./getting-started/installation)
* ⚙️ [Configuration](./getting-started/configuration)
* 🌱 [First XP](./getting-started/first-xp)
* 🌈 [Extrinsics](./core/extrinsics)
* 🧪 [Inspectors](./core/inspectors)
* 🌐 [RPC + UI](./core/rpc-ui)
* 🧬 [Traits](./architecture/traits)
* 📞 [Call Surface](./architecture/call-surface)
* 🧩 [Instances](./advanced/instances)
* 🔒 [Staking](./advanced/staking)

This is the real integration path.

Use this for production runtime development.

---

## 🛠️ 3. I Want to Modify `pallet-xp` Itself

Start here if you want to:

* add features
* change lifecycle rules
* improve architecture
* contribute to the pallet

### 🔗 Direct Links

* 🧩 [Concepts](./concepts/xp)
* 🏗️ [Architecture Overview](./architecture/overview)
* 🗄️ [Storage](./architecture/storage)
* 🧬 [Traits](./architecture/traits)
* 🌈 [Core / Extrinsics](./core/extrinsics)
* 📡 [Core / Events](./core/events)
* 🧪 [Core / Inspectors](./core/inspectors)
* 🔌 [Advanced / Listeners](./advanced/listeners)
* 🚀 [Advanced / Upcoming](./advanced/upcoming)

Understand architecture first.

Never modify lifecycle logic before understanding invariants.

---

## 🌐 4. I Am Building RPC / API Providers

Start here if you want to:

* build RPC endpoints
* power frontend apps
* support indexers
* create dashboards
* build explorer integrations

### 🔗 Direct Links

* 🌐 [RPC + UI](./core/rpc-ui)
* 📡 [Events](./core/events)
* 🧪 [Inspectors](./core/inspectors)
* 🧩 [Concepts](./concepts/xp)
* 🏗️ [Architecture Overview](./architecture/overview)

### Important Rule

```text
Production -> public pallet APIs (RPC + UI)
Development -> inspector extrinsics
```

Use pallet public functions directly and do not build production APIs around inspector events.

---

## 🔍 5. I Want to Critic / Audit the Design

### 📚 Read everything first

If you want to audit or challenge the design:

read everything first.

We always welcome criticism, we genuinely value it.

Good criticism helps make the system stronger by bringing new perspectives, uncovering blind spots, and challenging assumptions.

Most incorrect criticism comes from:

> treating XP like balances
or
> ignoring lifecycle assumptions

XP is a lifecycle-bound execution + reputation model.

Not a token system.

---

# 💡 Final Advice

Start with:

```text
Why does XP exist?
```

Everything else depends on that answer.

> XP is identity-bound programmable reputation

Understanding that first makes everything easier.

---

## 🚀 Next Steps

To understand how XP works in detail:

👉 **Concepts -> [XP Model](./concepts/xp.md)**
