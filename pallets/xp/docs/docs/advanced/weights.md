---
toc_min_heading_level: 2
toc_max_heading_level: 2
---

# ⚖️ Weights

`pallet-xp` does not hardcode production weights.

Instead, it follows the standard FRAME pattern:

```rust
pub trait pallet_xp::Config {
    // Via Runtime impl provide benchmarked weights
    type WeightInfo: pallet_xp::weights::WeightInfo;
}
```
This allows each runtime to provide pre-dispatch weights generated from its own benchmarking environment.

Different chains have different:

* hardware
* storage backends
* runtime composition
* optimization settings
* benchmarking results

so a single weight implementation is never ideal for every deployment.

The recommended production approach is:

> benchmark locally and provide your own type implementing `pallet_xp::weights::WeightInfo`

---

## Default Weights

Most runtimes begin with generated default weights as explained in [Configuration](../getting-started/configuration.md).

```rust
impl pallet_xp::Config for Runtime {
    type WeightInfo = pallet_xp::weights::SubstrateWeight<Runtime>;
}
```
---

## Generating Your Own Weights

Production runtimes should generate weights using FRAME benchmarking.

### 1. Enable `pallet-xp` Benchmarking in Runtime `Cargo.toml`

Open your runtime (or workspace) `Cargo.toml` and ensure `pallet-xp` is included in the runtime benchmarking feature set.

Example:

```toml
[features]
runtime-benchmarks = [
    # other pallets
    "pallet-xp/runtime-benchmarks",
]
```

### 2: Enable Runtime Benchmarking

Before generating weights, build the runtime with benchmarking enabled.

Example:

```bash
cargo build --release --features runtime-benchmarks
```

This enables the benchmarking infrastructure required by FRAME and allows benchmark execution against your runtime.

After a successful build, a benchmarkable runtime WASM should be available inside:

```text
target/release/wbuild/
```

### 3: Install `frame-omni-bencher`

`frame-omni-bencher` is the recommended benchmarking tool for generating pallet weights.

First verify whether it is already installed:

```bash
frame-omni-bencher --version
```

If the command is not available, install it:

```bash
cargo install frame-omni-bencher
```

After installation, verify:

```bash
frame-omni-bencher --version
```

### 4: Generate `pallet-xp` Weights

Use the standard FRAME benchmark command:

Example:

```bash
frame-omni-bencher v1 benchmark pallet \
  --runtime target/release/wbuild/your-runtime/your_runtime.wasm \
  --pallet pallet_xp \
  --extrinsic "*" \
  --output your-path/xp_weights.rs
```
---

### 5. Wiring Generated Weights Into Runtime

After generating the file, expose the generated module through your runtime and then configure the pallet:

```rust
impl pallet_xp::Config for Runtime {
    type WeightInfo = your_mod::xp_weights::SubstrateWeight<Runtime>;
    // other associated types...
}
```

Now all XP extrinsics use your benchmark-generated pre-dispatch weights automatically.