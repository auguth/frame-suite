---
toc_min_heading_level: 2
toc_max_heading_level: 2
---

# 🪞 Instances

`pallet-xp` supports **multiple independent instances** inside the same runtime.

This is done using [`Config` trait](../getting-started/configuration.md):

```rust
pallet_xp::Config<I>
```

where `I` is the pallet instance type.

Each instance gets:

* separate storage
* separate XP identities
* separate genesis configuration
* separate Pulse rules
* separate listeners
* separate runtime behavior

This allows multiple XP domains to exist in the same chain.

### Why Use Multiple Instances?

Sometimes one runtime needs different XP systems for different domains.

Example:

| Domain         | Purpose                 |
| -------------- | ----------------------- |
| Governance XP  | DAO voting reputation   |
| Gaming XP      | Player progression      |
| Contributor XP | Developer reputation    |
| Validator XP   | Validator trust scoring |

These should not share the same rules.

Each domain can use its own XP pallet instance.

---

## How It Works

Instead of:

```rust
impl pallet_xp::Config for Runtime { ... }
```

you define:

```rust 
pub struct Governance;
pub struct Gaming;
pub struct Contributors;

impl pallet_xp::Config<Governance> for Runtime { ... }
impl pallet_xp::Config<Gaming> for Runtime { ... }
impl pallet_xp::Config<Contributors> for Runtime { ... }
```

Each one becomes a fully separate XP system. 

Now:

```text
Governance XP != Gaming XP
```

They do not share storage.

They are completely isolated.

---

## Separate Storage

Each instance gets its own storage namespace.

Example:

```rust
XpOf<Runtime, Governance>
XpOf<Runtime, Gaming>
```

These are different storage maps.

Same for:

* `XpOwners`
* `LockedXpOf`
* `ReservedXpOf`
* `ReapedXp`
* `MinPulse`
* `InitXp`
* `PulseFactor`
* `MinTimeStamp`

Everything is isolated per instance. 

---

## Separate Genesis Config

Each instance also gets independent genesis values.

Example:

```rust
Governance:
  MinPulse = 10
  InitXp = 1

Gaming:
  MinPulse = 3
  InitXp = 50
```

This allows very different progression models in one runtime.

Perfect for domain-specific behavior.

---

## Separate Runtime Logic

Each instance can also use different:

* listeners
* reserve reasons
* lock reasons
* event behavior
* root tuning rules

This makes instances fully programmable.

Not just duplicated storage.

---

## When NOT to Use Instances

Use instances when the question is:

> Should this XP obey different rules?

Do not create multiple instances if:

- same rules
- same logic
- same lifecycle

In that case:

use a single XP system.

Instances are for:

- different domains
- different behavior
- different governance

not simple categorization.

---

## 🚀 Next Steps

Now that multiple XP systems are possible, the next step is understanding how listeners allow external pallets to react to XP lifecycle changes.

👉 **Advanced -> [Listeners](./listeners.md)**
