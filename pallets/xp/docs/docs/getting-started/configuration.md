---
toc_min_heading_level: 2
toc_max_heading_level: 2
---

# ⚙️ Configuration

After installation, `pallet-xp` exists inside your runtime.

But installation only makes the pallet available.

Configuration defines:

> how XP actually behaves

This is where you decide:

* how much XP exists initially
* how reputation grows
* why XP can be locked or reserved
* how runtime calls execute through `XpId`
* whether listeners and events are enabled

This section explains the full runtime configuration layer.

## What Gets Configured

There are two major configuration areas:

| Area                       | Purpose                    |
| -------------------------- | -------------------------- |
| `impl pallet_xp::Config`   | runtime execution behavior |
| `pallet_xp::GenesisConfig` | initial XP system state    |

Think of it like this:

```text 
Config Trait -> how XP works

GenesisConfig -> how XP starts
```

Both are required.

---

## 1. Config Trait

Inside:

```rust
impl pallet_xp::Config for Runtime
```

you define how XP integrates with your chain.

Default installation usually starts with:

```rust
impl pallet_xp::Config for Runtime {
    type Xp = u64;
    type Pulse = u32;

    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;

    type ReserveReason = RuntimeHoldReason;
    type LockReason = RuntimeFreezeReason;

    type Extensions = frame_suite::Ignore<Xp>;

    type EmitEvents = ConstBool<true>;
    type WeightInfo = pallet_xp::weights::SubstrateWeight<Runtime>;
}
```

Now let's explain what each field actually controls.

## 1.A Scalar Types

These define the fundamental XP accounting model.

| Type    | Meaning                        | Common Choice        |
| ------- | ------------------------------ | -------------------- |
| `Xp`    | actual XP value stored         | `Balance`, `u128`    |
| `Pulse` | reputation progression counter | `BlockNumber`, `u32` |

### `type Xp`

```rust
type Xp = u64;
```

This is the actual value stored inside XP identities.

It controls:

* free XP
* reserved XP
* locked XP
* slashing
* rewards

Recommended:

```rust
u128
```

or your runtime:

```rust
Balance
```

Why?

Because XP may integrate with fungible adapters and larger values are safer.

Avoid small types like:

```rust
u32
u64
```

for production systems.

### `type Pulse`

```rust
type Pulse = u32;
```

This controls long-term reputation progression.

It is NOT XP value.

It controls:

* reputation thresholds
* growth multipliers
* earning progression

Recommended:

```rust
u32
```

or your runtime:

```rust
BlockNumber
```

because pulse behaves like progression over time.

This makes pulse stable and predictable 📈

---

## 1.B Runtime Anchors

These connect XP to the runtime itself.

| Type           | Purpose                    |
| -------------- | -------------------------- |
| `RuntimeEvent` | event emission             |
| `RuntimeCall`  | XP-scoped runtime dispatch |

### `type RuntimeEvent`

```rust 
type RuntimeEvent = RuntimeEvent;
```

This allows XP to emit standard runtime events.

Examples:

* XP earned
* XP locked
* ownership transferred
* XP reaped

Always use the runtime-generated global enum.

Never create a custom type here.

### `type RuntimeCall`

```rust
type RuntimeCall = RuntimeCall;
```

This is one of the most important configuration fields.

It powers the extrinsic:

```rust
call(origin, xp_id, RuntimeCall)
```

which enables:

```text
Account signs
-> ownership verified
-> XpId executes
-> RuntimeCall dispatched
```

Without this:

XP cannot act as an execution identity 🧠

---

## 1.C Composite Reasons

These define why XP can be constrained.

| Type            | Purpose         |
| --------------- | --------------- |
| `ReserveReason` | soft constraint |
| `LockReason`    | hard constraint |

### Recommended Setup

```rust 
type ReserveReason = RuntimeHoldReason;
type LockReason = RuntimeFreezeReason;
```

Do NOT manually create local enums unless absolutely necessary.

Why?

Because Substrate already composes all pallets reasons into:

* `RuntimeHoldReason`
* `RuntimeFreezeReason`

This gives:

- ✅ native compatibility
- ✅ fungible adapter support
- ✅ staking integration
- ✅ governance integration
- ✅ bounded execution

This is the production-safe setup 🔒

## 1.D Extensions

This is often misunderstood.

```rust 
type Extensions
```

means:

> lifecycle listeners

These are NOT events.

They are runtime hooks triggered during:

* XP creation
* XP earning
* reserve
* lock
* slash
* reap
* ownership transfer

### Default Setup

```rust
type Extensions = frame_suite::Ignore<Xp>;
```

This means:

```text
hooks exist
-> intentionally do nothing
```

This is the safest starting point.

It prevents accidental business logic coupling.

### Later You Can Replace It With

* governance listeners
* reward hooks
* contributor rewards
* analytics integrations
* protocol reactions

Example:

```text
on XP earned
-> reward contributor
```

This is where XP becomes programmable ⚙️. And will be analyzed in advanced sections of `pallet-xp`.

---

## 1.E EmitEvents

```rust
type EmitEvents
```

Controls whether the pallet emits standard internal XP events.

These are events related to individual XP operations such as:

* earning XP
* locking XP
* reserving XP
* pulse progression
* internal lifecycle updates

### Recommended Values

| Environment     | Value              |
| --------------- | ------------------ |
| Testing / Local | `ConstBool<true>`  |
| Production      | `ConstBool<false>` |

Why?

Because events cost weight.

For production systems, emitting events for every individual XP operation is usually avoided to reduce unnecessary execution overhead.

Even if these internal events are disabled:

> listeners still execute

This is important.

Also:

> extrinsics will still emit events

because extrinsic execution is user-controlled and should remain visible at the runtime level.

What is typically disabled in production are the fine-grained internal events for each individual XP option, not the standard extrinsic-level runtime visibility.

Production systems often prefer listeners over internal events.

---

## 1.F WeightInfo

```rust
type WeightInfo
```

Used for:

* benchmarking
* dispatch weight calculation
* fee correctness

Recommended:

```rust
type WeightInfo =
    pallet_xp::weights::SubstrateWeight<Runtime>;
```

This uses the default Substrate benchmark weights provided by the pallet.

However:

> never use these benchmark weights for production blindly

Your runtime configuration, hardware environment, execution paths, and optimization choices may produce different benchmark results.

Production runtimes should always generate their own benchmarks using their actual runtime configuration and target hardware assumptions.

If your runtime requires different extrinsic weight benchmarks, you should provide your own custom `WeightInfo` implementation instead.

This ensures runtimes with specialized execution costs or benchmarking requirements can hook into XP safely.

See [Advanced -> Weights](../advanced/weights.md) for custom benchmarking instructions.

---

## 2. GenesisConfig

Now we configure how XP starts from block zero.

Inside:

```text
runtime/src/genesis_config_presets.rs
```

you define:

```rust
pallet_xp::GenesisConfig
```

This controls the initial XP economy.

Not runtime behavior.

Initial state.

---

### GenesisConfig Structure

```rust
pub struct GenesisConfig {
    pub min_pulse,
    pub init_xp,
    pub pulse_factor,
    pub genesis_acc,
}
```

Each field matters.

| Field          | Meaning                                |
| -------------- | -------------------------------------- |
| `init_xp`      | starting XP for new identities         |
| `min_pulse`    | pulse threshold before real XP earning |
| `pulse_factor` | how pulse grows                        |
| `genesis_acc`  | pre-created XP identities              |

These define the XP economy from block zero.

## 2.A `pub init_xp`

```rust
init_xp: 10
```

Meaning:

```text
new XpId -> starts with 10 XP
```

This affects every new identity created through:

```rust
XpMutate::create_xp()
```

Higher values = easier early participation.

Lower values = stricter onboarding.

---

## 2.B `pub min_pulse`

```rust
min_pulse: 3
```

`min_pulse` defines the minimum pulse required before XP rewards are actually credited.

It creates the boundary between reputation building and reward earning

```text
if pulse < min_pulse:
    earn_xp() -> pulse increases only

if pulse >= min_pulse:
    earn_xp() -> pulse increases + XP rewards applied
```

This means a new XP identity does not immediately receive valuable XP.

Instead:

1. pulse must grow first
2. trust must be established
3. only then XP rewards begin

You can think of it as:

```text
pulse = trust score
xp = earned value
```

This is important because XP is designed as reputation, not instant value.

Without this threshold:

```text
new accounts could farm XP immediately
```

which breaks the entire reputation model.

If too low:

```text
XP farming becomes cheap
```

If too high:

```text
new users struggle to participate
```

Hence,

> `min_pulse` defines the minimum trust cost before value creation

This improves:

* spam prevention
* sybil resistance
* governance safety
* contributor reputation integrity

---

## 2.C `pub pulse_factor`

```rust
pulse_factor: Stepper::new(50, 10).unwrap()
```

While `min_pulse` defines when rewards begin `pulse_factor` defines how fast pulse grows.

It controls the speed of trust formation.

Pulse does not increase directly by +1 per action.

Instead, the runtime uses a `DiscreteAccumulator`:

```text
step += per_count

if step >= threshold:
    pulse += 1
    step resets
```

with:

```rust
Stepper::new(threshold, per_count)
```

So for:

```rust
Stepper::new(50, 10)
```

the logic becomes:

```text
each valid earn_xp() call
-> step += 10

when step reaches 50
-> pulse += 1
```

which means:

```text
5 valid actions -> +1 pulse
```

because:

```text
10 + 10 + 10 + 10 + 10 = 50
```

This matches the accumulator implementation and pallet behavior.

* easy onboarding
* strong abuse resistance
* healthy contributor progression
* good governance safety

---

## 2.D `pub genesis_acc`

These are pre-created XP identities.

Each entry creates:

```text 
AccountId--owns-> XpId
```

from block zero.

Used for:

* validators
* governance actors
* treasury operators
* protocol maintainers

No manual initialization needed.

### Example Development Setup

```rust
xp: pallet_xp::GenesisConfig {
    min_pulse: 3,
    init_xp: 10,
    pulse_factor: Stepper::new(50, 10).unwrap(),
    genesis_acc: genesis_accounts,
}
```

This means:

* start with 10 XP
* need 3 pulses before rewards accumulation
* predefined identities exist

Ready for hacking 🚀

After this:

> XP is not just installed
> it is fully configured as a runtime-native execution system

---

## 🚀 Next Steps

Now that XP is configured, the next step is creating and managing your first XP identity **SAFELY**.

👉 **Getting Started -> [First XP](./first-xp.md)**
