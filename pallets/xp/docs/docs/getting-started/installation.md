---
toc_min_heading_level: 2
toc_max_heading_level: 2
---

# 🛠️ Installation

Before installing `pallet-xp`, it helps to start from a proper Substrate runtime foundation.

The recommended approach is:

> start from a clean Substrate node template
> then integrate `pallet-xp`

This keeps your runtime architecture clean and production-safe ✨

You can use:

* standard Substrate node template
* your existing FRAME runtime
* our XP-ready templates (recommended)

This section focuses on getting `pallet-xp` running safely using the default production-ready setup.

Detailed customization of pallet's `Config` trait and `GenesisConfig` structure will be covered in: [Configuration](./configuration.md)

---

# Recommended Starting Point

## Option A - Standard Substrate Template

If you're starting fresh, the standard Substrate template is a very good base.

It already provides:

* `frame_system`
* `frame` macros
* runtime presets
* chain spec setup
* genesis presets
* runtime call/event wiring

This means you only need to add XP on top.

Typical structure:

```text 
node/
runtime/
pallets/
Cargo.toml
```

This is the cleanest way to begin.

---

## Option B - Existing FRAME Runtime

If you already have a running Substrate runtime, you can integrate XP directly into it.

This is common when:

* adding governance reputation
* contributor scoring
* validator reputation
* protocol participation systems

In this case, XP becomes an additional execution layer, not a new chain.

---

## Option C - XP-Ready Runtime Templates (Recommended)

We also provide templates already prepared for XP integration.

These include:

* preconfigured XP runtime setup
* `RuntimeHoldReason`
* `RuntimeFreezeReason`
* genesis presets
* XP wiring
* proper config defaults
* FRAME Suite compatibility

This is much faster for real projects 🚀

Useful starting points:

* [XP Substrate Template](https://github.com/auguth/xp-substrate-template)
* XP-Commitment Substrate Template (Coming Soon, see [Advanced -> Staking](../advanced/staking.md))

(Use the runtime templates with XP examples there)

---

# After Choosing Your Base Runtime

Whether you start from:

* a standard Substrate template
* an existing FRAME runtime
* an XP-ready runtime template

the next installation steps are mostly the same.

The difference is:

| Starting Point | What You Need To Do  |
| ---------------| ---------------------|
| Standard Substrate Template | complete XP setup manually  |
| Existing FRAME Runtime | integrate XP into existing architecture |
| XP-Ready Template | mostly review + adjust config values |

XP-ready templates already include much of the setup:

* pallet registration
* runtime wiring
* hold/freeze reasons
* genesis presets
* default config implementations

So in many cases:

```text
Steps 2-5 become review steps
instead of setup steps
```

while standard templates require full manual integration.

---

## 1. Rust Requirements

Before installing `pallet-xp`, make sure your Rust environment is ready for Substrate runtime development.

You should have:

* stable Rust installed (`rustc`)
* the `wasm32-unknown-unknown` target enabled
* Protocol Buffers (`protoc`) installed

These are required because:

* Substrate runtimes compile to WebAssembly
* FRAME pallets require Rust toolchains
* some dependencies use protobuf generation during build

### Install Rust

```bash 
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Add WASM Target

```bash 
rustup target add wasm32-unknown-unknown
```

### Install Protocol Buffers

#### Ubuntu / Debian

```bash 
sudo apt install protobuf-compiler
```

#### macOS

```bash 
brew install protobuf
```

#### Arch Linux

```bash
sudo pacman -S protobuf
```

### Verify Installation

```bash
rustc --version
rustup target list --installed
protoc --version
```

Once these are available, your machine is ready for Substrate + XP runtime development.

---

## 2. Add Dependencies (Optional for XP Templates)

Inside your runtime project:

```bash
cargo add pallet-xp
cargo add frame-suite
```

Why both?

| Package       | Purpose                        |
| ------------- | ------------------------------ |
| `pallet-xp`   | actual runtime pallet          |
| `frame-suite` | XP traits, listeners, adapters |

`pallet-xp` is built on top of `frame_suite::xp`, so both are required.

If you use an XP-ready template, this step may already be done.

---

## 3. Register the Pallet in Runtime

Inside:

```text
runtime/src/lib.rs
```

you must register the pallet inside your runtime definition.

This is where Substrate composes all pallets into the final chain runtime.

A typical runtime section looks like this:

```rust 
#[frame_support::runtime]
mod runtime {
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask
    )]
    pub struct Runtime;

    #[runtime::pallet_index(0)]
    pub type System = frame_system::Pallet<Runtime>;

    // Add XP here with sequential pallet index

    #[runtime::pallet_index(1)]
    pub type Xp = pallet_xp::Pallet<Runtime>;
}
```

This registration makes XP part of:

* `RuntimeCall` extrinsincs
* `RuntimeEvent` events
* `RuntimeHoldReason` composite enum
* `RuntimeFreezeReason` composite enum
* runtime metadata generation
* dispatch system
* storage metadata

Without this:

> the pallet does not exist inside the chain

XP must be registered before it can execute.

---

## 4. Implement the Config Trait & Configure Later

Now wire the runtime behavior using the default safe setup:

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

This is the recommended default configuration for installation.

It provides:

* development-safe defaults
* runtime-native compatibility
* hold/freeze support
* lifecycle hooks
* proper dispatch weights

You do not need to customize anything yet.

That will be covered in the next page: [Configuration](./configuration.md)

---

## 5. Attest Genesis Struct & Configure Later

XP behavior is initialized from:

```rust
pallet_xp::GenesisConfig
```

inside:

```text
runtime/src/genesis_config_presets.rs
```

Use the default production-safe setup:

```rust
xp: pallet_xp::GenesisConfig {
    genesis_acc: genesis_accounts,
    ..Default::default()
}
```

or fully explicit:

```rust 
xp: pallet_xp::GenesisConfig {
    min_pulse: 3,
    init_xp: 10,
    pulse_factor: pallet_xp::types::Stepper::new(50, 10).unwrap(),
    genesis_acc: genesis_accounts,
}
```

This creates:

* initial XP identities
* owner mappings
* pulse rules
* starting XP values

from block zero 🧠

Detailed explanation of each field will be covered in: [Configuration](./configuration.md)

---

## Final Installation Checklist

| Step                            | Required |
| ------------------------------- | -------- |
| Choose base runtime             | ✅        |
| Install Rust + WASM target      | ✅        |
| Install protobuf                | ✅        |
| Add `pallet-xp`                 | ✅        |
| Add `frame-suite`               | ✅        |
| Register pallet in runtime      | ✅        |
| Implement default Config        | ✅        |
| Configure default GenesisConfig | ✅        |

After this:

> XP becomes a native execution layer inside your runtime

and is ready for custom configuration + usage 🚀

---

## 🚀 Next Steps

Now that installation is complete, the next step is understanding how to configure XP behavior inside your chain.

👉 **Getting Started -> [Configuration](./configuration.md)**
