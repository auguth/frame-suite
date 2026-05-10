// SPDX-License-Identifier: MPL-2.0
//
// Part of Auguth Labs open-source softwares.
// Built for the Substrate framework.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 Auguth Labs (OPC) Pvt Ltd, India

// ===============================================================================
// ```````````````````````````````` REWARD PLUGINS ```````````````````````````````
// ===============================================================================

//! Defines **pluggable reward models** for computing and distributing value across
//! participants.
//!
//! Rewards are abstracted into two main models:
//!
//! ## Payout (`payout`)
//!
//! - Computes the **total reward value** to be distributed.
//! - Produces a single payout amount that acts as the **source value**
//!   for downstream distribution.
//!
//! In this model:
//! - Input is a scalar representing a measurable quantity (e.g., stake, era, score).
//! - Output is a **total payout value**.
//!
//! Useful for scenarios where:
//! - The system must determine **how much value is available** for distribution.
//! - Reward generation follows configurable economic or logical rules.
//!
//!
//! ## Payee (`payee`)
//!
//! - Distributes the computed payout among a set of participants.
//! - Consumes the payout value and allocates it across entities.
//!
//! In this model:
//! - Input is `(Payout, [(Id, Share)])`.
//! - Output is `[(Id, Payout)]` allocations.
//!
//! Useful for scenarios where:
//! - The total reward must be **split among multiple participants**.
//! - Allocation depends on contribution, weight, or equal participation.
//!
//!
//! ## Purpose
//!
//! Separating reward computation into `payout` and `payee` provides flexibility:
//!
//! - **Payout** determines *how much total value* is available.
//! - **Payee** determines *how that value is distributed*.
//!
//! This separation enables:
//! - Independent evolution of reward generation and distribution strategies.
//! - Composable reward pipelines.
//! - Extensibility without modifying existing models.

// ===============================================================================
// ``````````````````````````````` PAYOUT PLUGINS ````````````````````````````````
// ===============================================================================

pub use payout::*;

/// Defines **pluggable payout models** for computing the total reward value
/// from an input signal.
///
/// Payouts are abstracted as transformation models that convert an input
/// quantity into a **single distributable value**.
///
/// ## Concept
///
/// - A payout model determines **how much total value** should be generated.
/// - The computed payout acts as the **source value** for downstream distribution.
///
/// ## In this model:
///
/// - Input is a scalar representing a measurable quantity.
/// - Output is a **single payout value**.
///
/// ## Purpose
///
/// Payout models provide flexibility in defining reward generation:
///
/// - Control how total rewards are computed from inputs.
/// - Enable configurable reward policies.
/// - Serve as the first stage in reward distribution pipelines.
pub mod payout {

    // ===============================================================================
    // ``````````````````````````````````` IMPORTS ```````````````````````````````````
    // ===============================================================================

    // --- Core / Std ---
    use core::ops::{Shr};

    // --- FRAME Suite ---
    use frame_suite::{
        fixedpoint::{FixedForInteger, FixedOp, IntegerToFixed, FixedSignedCast},
        plugin_model,
    };

    // --- Substrate primitives ---
    use sp_runtime::{
        traits::{Zero, One, CheckedDiv, Bounded}, 
        Saturating, FixedPointNumber, Vec, FixedI128
    };

    // ===============================================================================
    // ````````````````````````````````` ZERO-PAYOUT `````````````````````````````````
    // ===============================================================================

    plugin_model!(
        /// A payout model that always returns zero.
        ///
        /// ## Use Cases
        ///
        /// - Disabling rewards or payouts
        /// - Testing and benchmarking
        /// - Placeholder model in non-monetary systems
        name: pub ZeroPayout,
        input: Asset,
        bounds : [Asset: Zero],
        compute: |_input, _context| {
            Asset::zero()
        }
    );

    // ===============================================================================
    // ``````````````````````````````` CONSTANT-PAYOUT ```````````````````````````````
    // ===============================================================================

    /// Defines the configuration for the [`ConstantPayout`] model.
    ///
    /// This struct provides a **fixed reward value** that will be returned
    /// for every computation performed by the model.
    ///
    /// **Concept**: **Constant Reward Emission**
    ///
    /// Unlike dynamic payout models that depend on input values or contextual
    /// parameters, this configuration enforces a **static reward policy**.

    pub struct ConstantPayoutConfig<T> {
        /// The constant reward value returned by the model.
        pub payout: T,
    }

    plugin_model!(
        /// The **ConstantPayout** model returns a **fixed reward value**
        /// regardless of the provided input.
        ///
        /// **Concept**: **Static Reward Model**
        ///
        /// This model ignores all input signals and instead produces a
        /// deterministic output defined entirely by its configuration.
        ///
        /// ## Characteristics:
        /// - **Input-agnostic**: The input value has no effect on the output.
        /// - **Deterministic**: Always returns the same reward for a given configuration.
        /// - **Context-driven**: Relies solely on [`ConstantPayoutConfig`] for output.
        /// - **Zero-complexity**: No computation or aggregation involved.
        ///
        ///
        /// ## Applications:
        /// - Fixed payout systems (e.g., base rewards, participation rewards)
        /// - Genesis or bootstrap reward distribution
        /// - Testing pipelines where predictable output is required
        ///
        /// ## Use Cases
        ///
        /// - Bootstrap phases where all participants receive equal rewards.
        /// - Fixed incentive systems with no dependency on performance or input.
        /// - Testing and benchmarking deterministic payout behavior.
        ///
        /// ## Example:
        /// ```ignore
        /// let config = ConstantPayoutConfig { init_reward: 100 };
        /// let output = ConstantPayout::compute((), Some(config));
        /// assert_eq!(output, 100);
        /// ```
        name: pub ConstantPayout,
        input: Asset,
        context: ConstantPayoutConfig<Asset>,
        bounds: [Asset: Copy],
        compute: |_input, context| {
            context.payout
        }
    );

    // ===============================================================================
    // `````````````````````````````` INFLATION-PAYOUT ```````````````````````````````
    // ===============================================================================

    /// Defines the configuration for the [`InflationPayout`] model.
    ///
    /// This struct specifies the **inflation rate** used to compute rewards
    /// as a fraction of the input asset.
    ///
    /// **Concept**: **Proportional Inflation-Based Reward**
    ///
    /// Rewards are derived by applying a fixed fractional rate to the input,
    /// enabling linear scaling based on the magnitude of the asset.
    pub struct InflationPayoutConfig<F>
    where
        F: FixedPointNumber,
    {
        /// A fixed-point fraction representing the reward rate.
        ///
        /// Example: `0.01` represents a 1% reward.
        pub inflation_rate: F, // fraction, e.g. 0.01 for 1%
    }

    plugin_model!(
        /// The **InflationPayout** model computes rewards as a **fixed proportion
        /// of the input asset**, based on a configured inflation rate.
        ///
        /// **Concept**: **Linear Inflation Scaling**
        ///
        /// The model converts the input asset into a fixed-point representation,
        /// applies the inflation rate, and converts the result back into the
        /// original asset type.
        ///
        /// ## Formula
        ///
        /// ```text
        /// reward = input * inflation_rate
        /// ```
        ///
        /// ## Characteristics:
        /// - **Proportional**: Rewards scale linearly with the input value.
        /// - **Deterministic**: Same input and rate always produce the same output.
        /// - **Fixed-point safe**: Uses fixed-point arithmetic for precision.
        /// - **Context-driven**: Controlled via [`InflationPayoutConfig`].
        ///
        /// ## Applications:
        /// - Staking reward systems
        /// - Inflationary token supply models
        /// - Proportional incentive distribution
        ///
        /// ## Use Cases
        ///
        /// - Token inflation mechanisms
        /// - Staking rewards proportional to stake
        /// - Emission schedules with fixed percentage growth
        ///
        /// ## Example:
        /// ```ignore
        /// let config = InflationPayoutConfig {
        ///     inflation_rate: FixedU128::from_rational(1, 100)
        /// }; // 1%
        /// let reward = InflationPayout::compute(1_000u128, Some(config));
        /// assert_eq!(reward, 10);
        /// ```
        name: pub InflationPayout,
        input: Asset,
        others: [FixedPoint],
        context: InflationPayoutConfig<FixedPoint>,
        bounds: [
            Asset: IntegerToFixed + FixedForInteger<FixedPoint = FixedPoint>,
            FixedPoint: FixedPointNumber,
        ],
        compute: |input, context| {
            let x = input.to_fixed();
            let inflation = context.inflation_rate;
            let reward_fixed = inflation.saturating_mul(x);
            Asset::from_fixed(&reward_fixed)
        }
    );

    // ===============================================================================
    // ```````````````````````````````` LINEAR-PAYOUT ````````````````````````````````
    // ===============================================================================

    /// Defines the configuration for the [`LinearPayout`] model.
    ///
    /// This struct specifies the parameters of a **linear reward function**.
    ///
    /// The payout is computed as a linear transformation of the input:
    /// a scaled component plus a constant offset.
    pub struct LinearPayoutConfig<F>
    where
        F: FixedPointNumber,
    {
        /// The scaling factor applied to the input.
        pub slope: F,
        /// The constant offset added to the result.
        pub base_reward: F,
    }

    plugin_model!(
        /// The **LinearPayout** model computes rewards using a
        /// **linear function** of the input asset.
        ///
        /// **Concept**: **Linear Transformation**
        ///
        /// ## Formula
        ///
        /// ```text
        /// reward = (slope * input) + base_reward
        /// ```
        ///
        /// ## Characteristics:
        /// - **Linear scaling**: Reward grows proportionally with input.
        /// - **Base offset**: Ensures a minimum reward via `base_reward`.
        /// - **Deterministic**: Same inputs and parameters yield identical results.
        /// - **Fixed-point safe**: Uses fixed-point arithmetic for precision.
        /// - **Context-driven**: Controlled via [`LinearPayoutConfig`].
        ///
        /// ## Applications:
        /// - Staking systems with base + proportional rewards
        /// - Incentive models with guaranteed minimum payout
        /// - Linear emission schedules
        /// - Reward shaping for participation-based systems
        ///
        /// ## Use Cases
        ///
        /// - Reward systems with a base incentive plus proportional scaling
        /// - Gradual incentive curves
        /// - Configurable emission policies
        ///
        /// ## Example:
        /// ```ignore
        /// let config = LinearPayoutConfig {
        ///     slope: FixedU128::from_rational(1, 10), // 0.1x
        ///     base_reward: FixedU128::from_integer(5),
        /// };
        ///
        /// let reward = LinearPayout::compute(100u128, Some(config));
        /// // reward = (0.1 * 100) + 5 = 15
        /// assert_eq!(reward, 15);
        /// ```
        name: pub LinearPayout,
        input: Asset,
        others: [FixedPoint],
        context: LinearPayoutConfig<FixedPoint>,
        bounds: [
            Asset: IntegerToFixed + FixedForInteger<FixedPoint = FixedPoint>,
            FixedPoint: FixedPointNumber,
        ],
        compute: |input, context| {
            let slope = context.slope;
            let base = context.base_reward;
            let x = input.to_fixed();
            let reward_fixed = x.saturating_mul(slope).saturating_add(base);
            Asset::from_fixed(&reward_fixed)
        }
    );

    // ===============================================================================
    // ``````````````````````````````` QUADRATIC-PAYOUT ``````````````````````````````
    // ===============================================================================

    /// Defines the configuration for the [`QuadraticPayout`] model.
    ///
    /// This struct specifies the coefficients of a **quadratic reward function**,
    /// enabling non-linear reward shaping.
    ///
    /// **Concept**: **Quadratic Reward Curve**
    ///
    /// Rewards are computed using a second-degree polynomial:
    ///
    /// ```text
    /// reward = (a * x^2) + (b * x) + c
    /// ```
    ///
    /// This allows flexible modeling of reward behavior:
    ///
    /// - **Convex curve (a > 0):** Rewards accelerate as input increases  
    ///   -> Encourages high participation or large stake  
    ///
    /// - **Concave curve (a < 0):** Rewards grow sublinearly  
    ///   -> Penalizes concentration, discourages dominance  
    ///
    /// - **Linear case (a = 0):** Reduces to a linear function  
    pub struct QuadraticPayoutConfig<F>
    where
        F: FixedPointNumber,
    {
        /// Controls curvature (growth acceleration/decay)
        pub quadratic_coeff: F,
        /// Controls proportional scaling
        pub linear_coeff: F,
        /// Base reward offset
        pub constant_term: F,
    }

    plugin_model!(
        /// The **QuadraticPayout** model computes rewards using a **quadratic
        /// function** of the input asset.
        ///
        /// **Concept**: **Second-Order Reward Transformation**
        ///
        /// ## Formula
        ///
        /// ```text
        /// reward = (a * x^2) + (b * x) + c
        /// ```
        ///
        /// ## Characteristics:
        /// - **Non-linear scaling**: Captures accelerating or diminishing returns
        /// - **Flexible shaping**: Controlled via three coefficients
        /// - **Deterministic**: Same input and parameters yield identical results
        /// - **Fixed-point safe**: Uses fixed-point arithmetic for precision
        /// - **Context-driven**: Controlled via [`QuadraticPayoutConfig`]
        ///
        /// ## Applications:
        /// - Advanced staking reward curves
        /// - Anti-centralization incentive models
        /// - Economic simulations and experimentation
        /// - Reward shaping in governance systems
        ///
        /// ## Use Cases
        ///
        /// - Anti-whale reward shaping (concave curves)
        /// - Incentivizing large contributions (convex curves)
        /// - Flexible economic modeling beyond linear systems
        /// - Approximation of more complex reward curves
        ///
        /// ## Example
        ///
        /// ```ignore
        /// let config = QuadraticPayoutConfig {
        ///     quadratic_coeff: FixedU128::from_rational(1, 100), // 0.01
        ///     linear_coeff: FixedU128::from_integer(2),          // 2x
        ///     constant_term: FixedU128::from_integer(10),        // base reward
        /// };
        ///
        /// let reward = QuadraticPayout::compute(100u128, Some(config));
        /// // reward = (0.01 * 100^2) + (2 * 100) + 10 = 100 + 200 + 10 = 310
        /// ```
        name: pub QuadraticPayout,
        input: Asset,
        others: [FixedPoint],
        context: QuadraticPayoutConfig<FixedPoint>,
        bounds: [
            Asset: IntegerToFixed + FixedForInteger<FixedPoint = FixedPoint>,
            FixedPoint: FixedPointNumber,
        ],
        compute: |input, context| {
            let a = context.quadratic_coeff;
            let b = context.linear_coeff;
            let c = context.constant_term;

            let x = input.to_fixed();

            // Compute a * x^2
            let x_sq = x.saturating_mul(x);
            let term_quadratic = a.saturating_mul(x_sq);

            // Compute b * x
            let term_linear = b.saturating_mul(x);

            // Constant term
            let term_constant = c;

            // Combine all terms
            let reward_fixed = term_quadratic
                .saturating_add(term_linear)
                .saturating_add(term_constant);

            Asset::from_fixed(&reward_fixed)
        }
    );

    // ===============================================================================
    // ``````````````````````````````` HALVING-PAYOUT ````````````````````````````````
    // ===============================================================================

    /// Defines the configuration for the [`HalvingPayout`] model.
    ///
    /// This struct specifies the parameters for a **halving-based reward schedule**,
    /// where rewards decrease exponentially over time.
    ///
    /// **Concept**: **Exponential Decay via Halving**
    ///
    /// Rewards follow a discrete exponential decay pattern:
    ///
    /// ```text
    /// reward = R0 / 2^n
    /// ```
    ///
    /// where:
    /// - `R0` = initial reward
    /// - `n`  = number of halving intervals (e.g., era, epoch, or block index)
    ///
    /// This model is widely used in monetary systems to:
    /// - Control long-term inflation
    /// - Gradually reduce issuance
    /// - Introduce scarcity over time
    pub struct HalvingPayoutConfig<T> {
        /// Initial reward (R0): payout when n = 0
        pub initial_reward: T,
    }

    plugin_model!(
        /// The **HalvingPayout** model computes rewards using a **binary
        /// exponential decay** based on the input interval.
        ///
        /// **Concept**: **Discrete Halving Function**
        ///
        /// ## Formula
        ///
        /// ```text
        /// reward = R0 / 2^n
        /// ```
        ///
        /// ## Characteristics:
        /// - **Exponential decay**: Reward halves with each increment of input
        /// - **Deterministic**: Same input and configuration yield identical output
        /// - **Efficient**: Uses bit shifting instead of division
        /// - **Discrete**: Stepwise reduction per interval
        /// - **Context-driven**: Controlled via [`HalvingPayoutConfig`]
        ///
        /// ## Applications:
        /// - Blockchain issuance schedules
        /// - Mining or staking reward decay
        /// - Long-term economic stabilization
        /// - Scarcity-driven incentive design
        ///
        /// ## Behavior
        ///
        /// - At `n = 0`: reward = `R0`
        /// - At `n = 1`: reward = `R0 / 2`
        /// - At `n = 2`: reward = `R0 / 4`
        /// - ...
        ///
        /// ## Use Cases
        ///
        /// - Bitcoin-style emission schedules
        /// - Deflationary tokenomics
        /// - Long-term reward tapering
        /// - Controlled supply issuance
        ///
        /// ## Example
        ///
        /// ```ignore
        /// let config = HalvingPayoutConfig {
        ///     initial_reward: 100,
        /// };
        ///
        /// assert_eq!(HalvingPayout::compute(0, Some(config)), 100); // 100 / 2^0
        /// assert_eq!(HalvingPayout::compute(1, Some(config)), 50);  // 100 / 2^1
        /// assert_eq!(HalvingPayout::compute(2, Some(config)), 25);  // 100 / 2^2
        /// ```
        name: pub HalvingPayout,
        input: Asset,  // input: halving index (n), output: reward
        context: HalvingPayoutConfig<Asset>,
        bounds: [
            Asset: Copy + Shr<Output = Asset> + Zero
        ],
        compute: |n, context| {
            // Special case: n = 0 -> return initial reward
            if n.is_zero() {
                return context.initial_reward
            }

            // Compute: R0 >> n  ==  R0 / 2^n
            context.initial_reward >> n
        }
    );

    // ===============================================================================
    // ``````````````````````````````` EXP-DECAY-PAYOUT ``````````````````````````````
    // ===============================================================================

    /// Defines the configuration for the [`ExpDecayPayout`] model.
    ///
    /// This struct specifies the parameters for an **exponential decay reward function**,
    /// where rewards decrease continuously over time.
    ///
    /// **Concept**: **Continuous Exponential Decay**
    ///
    /// Rewards follow a smooth exponential decay curve:
    ///
    /// ```text
    /// reward = r0 * e^(-a * x)
    /// ```
    ///
    /// where:
    /// - `r0` = initial reward
    /// - `a`  = decay constant (rate of decay, a > 0)
    /// - `x`  = input variable (e.g., time, era, or block index)
    ///
    /// This model enables:
    /// - Smooth reward reduction over time
    /// - More natural decay compared to discrete halving
    /// - Fine-grained control over emission rate
    pub struct ExpDecayPayoutConfig<T, F>
    where
        F: FixedPointNumber,
    {
        /// Initial reward (r0): reward at x = 0
        pub initial_reward: T,

        /// Decay constant (a): controls how fast rewards decrease
        pub decay_constant: F,
    }

    plugin_model!(
        /// The **ExpDecayPayout** model computes rewards using a **continuous
        /// exponential decay** based on the input variable.
        ///
        /// **Concept**: **Smooth Decay Function**
        ///
        /// ## Formula
        ///
        /// ```text
        /// reward = r0 * e^(-a * x)
        /// ```
        ///
        /// ## Signed Arithmetic
        ///
        /// The exponent `-a * x` is always non-positive for `a >= 0` and `x >= 0`.
        /// Unsigned fixed-point types cannot represent negative numbers, so the
        /// negation is performed inside a concrete `FixedI128` workspace via
        /// [`FixedSignedCast`], then projected back. This makes the model correct
        /// for both signed and unsigned `Asset` and `FixedPoint` types.
        ///
        /// ## Characteristics:
        /// - **Continuous decay**: Smooth reduction instead of stepwise halving.
        /// - **Non-linear scaling**: Faster decay as `a` increases.
        /// - **Deterministic**: Same input and parameters yield identical results.
        /// - **Fixed-point safe**: Signed intermediate arithmetic via `FixedSignedCast`.
        /// - **Works with unsigned types**: No `Neg` bound; negation in `FixedI128`.
        /// - **Context-driven**: Controlled via [`ExpDecayPayoutConfig`].
        ///
        /// ## Applications:
        /// - Emission schedules with smooth decay.
        /// - Staking reward tapering.
        /// - Time-based incentive reduction.
        /// - Economic stabilization mechanisms.
        ///
        /// ## Use Cases
        /// - Replacing halving with a smoother continuous decay.
        /// - Gradual reward reduction without abrupt drops.
        /// - Fine-tuned monetary policy control.
        ///
        /// ## Example
        ///
        /// ```ignore
        /// let config = ExpDecayPayoutConfig {
        ///     initial_reward: 1000u128,
        ///     decay_constant: FixedU128::saturating_from_rational(1, 10), // a = 0.1
        /// };
        /// // x = 10: reward = 1000 * e^(-1.0) ~= 367
        /// assert_eq!(ExpDecayPayout::compute(10u128, Some(config)), 367);
        /// ```
        name: pub ExpDecayPayout,
        input: Asset,   // x: time / era / block index
        others: [FixedPoint],
        context: ExpDecayPayoutConfig<Asset, FixedPoint>,
        bounds: [
            Asset: Copy + IntegerToFixed + FixedForInteger<FixedPoint = FixedPoint>,
            FixedPoint: FixedPointNumber + FixedSignedCast<Signed = FixedI128>,
        ],
        compute: |x, context| {
            // Lift input and decay constant into the signed workspace.
            // saturated_into is infallible for FixedU64 (u64 always fits in i128)
            // and clamps at i128::MAX for large FixedU128 values. For signed
            // FixedPoint types it is a zero-cost identity.
            let x_fixed:  FixedPoint = x.to_fixed();
            let x_s:      FixedI128  = FixedSignedCast::saturated_into(x_fixed);
            let a_s:      FixedI128  = FixedSignedCast::saturated_into(context.decay_constant);
    
            // a * x in signed space - always >= 0 when a >= 0 and x >= 0.
            let ax_s: FixedI128 = a_s.saturating_mul(x_s);
    
            // Negate to produce the exponent -a * x.
            // saturating_sub from zero avoids any dependency on Neg being
            // implemented for FixedPoint, which unsigned types do not satisfy.
            let neg_ax: FixedI128 = FixedI128::zero().saturating_sub(ax_s);
    
            // e^(-a * x), result is in (0, 1] for non-negative a and x.
            // Concrete FixedI128::fixed_exp - no generic FixedOp bound needed.
            // unwrap_or(zero) is a safe sentinel, overflow is not reachable here
            // since the exponent is <= 0.
            let exp_s: FixedI128 = FixedI128::fixed_exp(&neg_ax)
                .unwrap_or(FixedI128::zero());
    
            // Project the exp result back to FixedPoint.
            // exp_s is always in (0, 1] so it is non-negative and representable
            // in both signed and unsigned FixedPoint types.
            let exp_fp: FixedPoint = FixedSignedCast::saturated_from(exp_s);
    
            // reward = r0 * e^(-a * x)
            let r0_fixed: FixedPoint = context.initial_reward.to_fixed();
            let reward_fixed: FixedPoint = r0_fixed.saturating_mul(exp_fp);
    
            Asset::from_fixed(&reward_fixed)
        }
    );
 

    // ===============================================================================
    // ``````````````````````````````` SIGMOID-PAYOUT ````````````````````````````````
    // ===============================================================================

    /// Defines the configuration for the [`SigmoidPayout`] model.
    ///
    /// This struct specifies the parameters for a **sigmoid (S-curve) reward function**,
    /// enabling smooth growth and saturation behavior.
    ///
    /// **Concept**: **Logistic Growth Curve**
    ///
    /// Rewards follow a sigmoid function:
    ///
    /// ```text
    /// reward = L / (1 + e^(-k(x - x0)))
    /// ```
    ///
    /// where:
    /// - `L`   = maximum reward (upper bound)
    /// - `k`   = growth rate (steepness of the curve)
    /// - `x0`  = midpoint (inflection point)
    /// - `x`   = input variable (e.g., time, era, or score)
    ///
    /// Instead of directly specifying `k` and `x0`, this model derives them from:
    /// - `growth_start` (a): lower percentile of growth (0 < a < 1)
    /// - `growth_end`   (b): upper percentile of growth (0 < b < 1)
    ///
    /// This allows intuitive configuration of the curve shape.
    pub struct SigmoidPayoutConfig<T, F>
    where
        F: FixedPointNumber,
    {
        /// Maximum reward (L): asymptotic upper bound
        pub max_reward: T,

        /// Lower growth bound (a): fraction where growth begins (0 < a < 1)
        pub growth_start: F,

        /// Upper growth bound (b): fraction where growth saturates (0 < b < 1)
        pub growth_end: F,
    }

    plugin_model!(
        /// The **SigmoidPayout** model computes rewards using a **logistic
        /// (S-shaped) function**, producing slow start, rapid growth, and
        /// eventual saturation.
        ///
        /// **Concept**: **S-Curve Reward Transformation**
        ///
        /// ## Formula
        ///
        /// ```text
        /// f(x) = L / (1 + e^(-k*(x - x0)))
        /// ```
        ///
        /// where `k` and `x0` are derived from `growth_start` (a) and `growth_end` (b):
        ///
        /// ```text
        /// k  = logit(b) - logit(a)    (always > 0 when b > a, both in (0, 1))
        /// x0 = -logit(a) / k          (midpoint; > 0 when a < 0.5)
        /// ```
        ///
        /// ## Signed Arithmetic
        ///
        /// Even when `Asset` and `FixedPoint` are unsigned types, several
        /// intermediate values are inherently signed:
        ///
        /// - `logit(a) < 0` for any `a < 0.5`.
        /// - `x - x0 < 0` for any `x < x0` (the entire left half of the curve).
        /// - `-k*(x - x0) < 0` for any `x > x0` (the entire right half).
        ///
        /// All of these are computed in a concrete `FixedI128` workspace via
        /// [`FixedSignedCast`], then the final result (always >= 0) is projected
        /// back. This makes the model correct for both signed and unsigned types
        /// with no `Neg` bound on `FixedPoint`.
        ///
        /// ## Guard Conditions (returns zero)
        ///
        /// - `growth_start <= 0` or `growth_start >= 1`.
        /// - `growth_end   <= 0` or `growth_end   >= 1`.
        /// - `k == 0` (degenerate; only when `growth_start == growth_end`).
        ///
        /// ## Precision Note
        ///
        /// Fixed-point arithmetic accumulates small rounding errors across the
        /// `ln -> k -> x0 -> exp` chain. In practice the output may be one integer
        /// unit below the analytically exact value at certain inputs. This is
        /// expected and inconsequential for integer rewards.
        ///
        /// ## Characteristics:
        /// - **Bounded**: Reward never exceeds `max_reward`.
        /// - **Smooth growth**: Gradual ramp-up instead of abrupt changes.
        /// - **Works with unsigned types**: No `Neg` bound; negation in `FixedI128`.
        /// - **Deterministic**: Same input and parameters yield identical results.
        /// - **Context-driven**: Controlled via [`SigmoidPayoutConfig`].
        ///
        /// ## Applications:
        /// - Adoption-based reward curves.
        /// - Incentive ramp-up systems.
        /// - Gradual onboarding rewards.
        /// - Supply emission with saturation.
        ///
        /// ## Example
        ///
        /// ```ignore
        /// let config = SigmoidPayoutConfig {
        ///     max_reward:   100u128,
        ///     growth_start: FixedU128::saturating_from_rational(1, 10), // 0.1
        ///     growth_end:   FixedU128::saturating_from_rational(9, 10), // 0.9
        /// };
        /// // k ~= 4.394, x0 ~= 0.5
        /// // f(0) = 10  (= growth_start * L, lower tail)
        /// // f(1) = 89  (= growth_end * L, upper tail, rounded down)
        /// assert_eq!(SigmoidPayout::compute(0u128, Some(config)), 10);
        /// assert_eq!(SigmoidPayout::compute(1u128, Some(config)), 89);
        /// ```
        name: pub SigmoidPayout,
        input: Asset,
        others: [FixedPoint],
        context: SigmoidPayoutConfig<Asset, FixedPoint>,
        bounds: [
            Asset: Copy + IntegerToFixed + FixedForInteger<FixedPoint = FixedPoint> + Zero,
            FixedPoint: FixedPointNumber + FixedSignedCast<Signed = FixedI128>,
        ],
        compute: |x, context| {
            let zero_fp = FixedPoint::zero();
            let one_fp  = FixedPoint::one();
            let zero_s  = FixedI128::zero();
    
            let a = context.growth_start;
            let b = context.growth_end;
    
            // Guard: a and b must be strictly in (0, 1).
            if a <= zero_fp || a >= one_fp || b <= zero_fp || b >= one_fp {
                return Asset::zero();
            }
    
            // logit(p) = ln(p / (1 - p))
            //
            // The ratio p/(1-p) is always > 0 and is computed in FixedPoint space.
            // It is promoted to FixedI128 before calling fixed_ln because the result
            // can be negative when p < 0.5 (ratio < 1, ln < 0).
            // Concrete FixedI128::fixed_ln - no generic FixedOp bound needed.
            let logit = |p: FixedPoint| -> Option<FixedI128> {
                let denom = one_fp.saturating_sub(p);    // 1 - p > 0 since p < 1
                let ratio = p.checked_div(&denom)?;       // p/(1-p) > 0
                let ratio_s: FixedI128 = FixedSignedCast::saturated_into(ratio);
                FixedI128::fixed_ln(&ratio_s)             // may be negative
            };
    
            let logit_a: FixedI128 = match logit(a) {
                Some(v) => v,
                None    => return Asset::zero(),
            };
            let logit_b: FixedI128 = match logit(b) {
                Some(v) => v,
                None    => return Asset::zero(),
            };
    
            // k = logit(b) - logit(a), always > 0 when b > a.
            let k: FixedI128 = logit_b.saturating_sub(logit_a);
            if k == zero_s {
                return Asset::zero();
            }
    
            // x0 = -logit(a) / k
            //
            // The negation is required by the derivation: solving f(0) = a * L gives
            // k * x0 = -logit(a), so x0 = -logit(a) / k. For a < 0.5, logit(a) < 0,
            // so -logit(a) > 0 and x0 > 0. The original code omitted the negation,
            // placing x0 at a negative value and collapsing the curve to its
            // saturation tail at x = 0.
            let neg_logit_a: FixedI128 = zero_s.saturating_sub(logit_a);
            let x0: FixedI128 = match neg_logit_a.checked_div(&k) {
                Some(v) => v,
                None    => return Asset::zero(),
            };
    
            // Evaluate f(x) = L / (1 + e^(-k*(x - x0))).
            // All arithmetic stays in FixedI128 so that x - x0 can be negative
            // when x < x0, and the negated product can be negative when x > x0.
            let x_s:     FixedI128 = FixedSignedCast::saturated_into(x.to_fixed());
            let delta:   FixedI128 = x_s.saturating_sub(x0);
            let k_delta: FixedI128 = k.saturating_mul(delta);
    
            // Negate via subtraction from zero
            let neg_k_delta: FixedI128 = zero_s.saturating_sub(k_delta);
    
            // Concrete FixedI128::fixed_exp - no generic bound needed.
            // On extreme overflow uses max_value() so the denominator is large and
            // the result rounds toward zero rather than producing garbage.
            let exp_val: FixedI128 = FixedI128::fixed_exp(&neg_k_delta)
                .unwrap_or(FixedI128::max_value());
    
            let denom: FixedI128 = FixedI128::one().saturating_add(exp_val);
    
            let l_s: FixedI128 = FixedSignedCast::saturated_into(context.max_reward.to_fixed());
            let output_s: FixedI128 = l_s.checked_div(&denom).unwrap_or(zero_s);
    
            // Project back to FixedPoint.
            // output_s >= 0 because L >= 0 and denom >= 1; saturated_from clamps
            // any accidental negative to zero rather than panicking.
            let output_fp: FixedPoint = FixedSignedCast::saturated_from(output_s);
            Asset::from_fixed(&output_fp)
        }
    );

    // ===============================================================================
    // ````````````````````````` INVERSE-PROPORTIONAL-PAYOUT `````````````````````````
    // ===============================================================================

    /// Defines the configuration for the [`InverseProportionalPayout`] model.
    ///
    /// This struct specifies the parameters for an **inverse proportional reward
    /// function**, where rewards decrease as the input increases.
    ///
    /// **Concept**: **Inverse Scaling Function**
    ///
    /// Rewards follow an inverse relationship:
    ///
    /// ```text
    /// reward = k / (x + eps)
    /// ```
    ///
    /// where:
    /// - `k` = proportionality constant (scaling factor)
    /// - `x` = input variable (e.g., stake, time, or score)
    /// - `eps` = small positive constant to prevent division by zero
    ///
    /// This model produces high rewards for small inputs and diminishing rewards
    /// as input increases.
    pub struct InverseProportionalConfig<F>
    where
        F: FixedPointNumber,
    {
        /// Proportionality constant (k): controls overall reward scale
        pub k: F,

        /// Small constant (eps): prevents division by zero and stabilizes
        /// behavior near x = 0
        pub epsilon: F,
    }

    plugin_model!(
        /// The **InverseProportionalPayout** model computes rewards using an
        /// **inverse function** of the input asset.
        ///
        /// **Concept**: **Diminishing Reward Curve**
        ///
        /// ## Formula
        ///
        /// ```text
        /// reward = k / (x + eps)
        /// ```
        ///
        /// ## Characteristics:
        /// - **Inverse scaling**: Reward decreases as input increases
        /// - **High initial rewards**: Small inputs yield larger rewards
        /// - **Diminishing returns**: Larger inputs are progressively penalized
        /// - **Stable near zero**: `eps` prevents singularities at x = 0
        /// - **Deterministic**: Same input and parameters yield identical results
        /// - **Fixed-point safe**: Uses fixed-point arithmetic for precision
        /// - **Context-driven**: Controlled via [`InverseProportionalConfig`]
        ///
        /// ## Applications:
        /// - Anti-whale incentive systems
        /// - Rewarding early or small participants
        /// - Load balancing and fairness mechanisms
        /// - Resource pricing models
        ///
        /// ## Use Cases
        ///
        /// - Favoring smaller stakeholders over large ones
        /// - Preventing concentration of rewards
        /// - Incentivizing early-stage participation
        ///
        /// ## Example
        ///
        /// ```ignore
        /// let config = InverseProportionalConfig {
        ///     k: FixedU128::from_integer(100),
        ///     epsilon: FixedU128::from_integer(1),
        /// };
        ///
        /// let reward = InverseProportionalPayout::compute(9u128, Some(config));
        /// // reward = 100 / (9 + 1) = 10
        /// ```
        name: pub InverseProportionalPayout,
        input: Asset,
        others: [FixedPoint],
        context: InverseProportionalConfig<FixedPoint>,
        bounds: [
            Asset: Copy + IntegerToFixed + FixedForInteger<FixedPoint = FixedPoint>,
            FixedPoint: Copy + FixedPointNumber,
        ],
        compute: |input, context| {
            let x = input.to_fixed();

            // Compute denominator: x + eps
            let denom = x.saturating_add(context.epsilon);

            // non-positive denominator produces nonsensical or undefined
            // results; return zero as a safe fallback.
            if denom <= FixedPoint::zero() {
                return Asset::from_fixed(&FixedPoint::zero());
            }

            // Compute reward: k / (x + eps)
            let reward = context
                .k
                .checked_div(&denom)
                .unwrap_or_else(|| FixedPoint::zero());

            Asset::from_fixed(&reward)
        }
    );

    // ===============================================================================
    // ````````````````````````````` LOGARITHMIC-PAYOUT ``````````````````````````````
    // ===============================================================================

    /// Defines the configuration for the [`LogarithmicPayout`] model.
    ///
    /// This struct specifies the parameters for a **logarithmic reward function**,
    /// enabling diminishing growth as input increases.
    ///
    /// **Concept**: **Logarithmic Growth Curve**
    ///
    /// Rewards follow a logarithmic function:
    ///
    /// ```text
    /// reward = a * ln(b * x + c) + d
    /// ```
    ///
    /// where:
    /// - `a` = vertical scaling (controls steepness)
    /// - `b` = horizontal scaling (input stretch/compression)
    /// - `c` = horizontal shift (ensures argument > 0)
    /// - `d` = vertical shift (baseline reward)
    /// - `x` = input variable (e.g., stake, time, or score)
    ///
    /// This model produces rapid initial growth that slows over time,
    /// resulting in diminishing returns for larger inputs.
    pub struct LogarithmicConfig<F>
    where
        F: FixedPointNumber,
    {
        /// Vertical scaling (a): controls steepness of growth
        pub vertical_scale: F,

        /// Horizontal scaling (b): stretches/compresses input
        pub horizontal_scale: F,

        /// Horizontal shift (c): ensures ln argument remains positive
        pub horizontal_shift: F,

        /// Vertical shift (d): baseline reward offset
        pub vertical_shift: F,
    }

    plugin_model! (
        /// The **LogarithmicPayout** model computes rewards using a **logarithmic function**,
        /// producing fast initial growth followed by diminishing returns.
        ///
        /// **Concept**: **Diminishing Growth Function**
        ///
        /// ## Formula
        ///
        /// ```text
        /// reward = a * ln(b * x + c) + d
        /// ```
        ///
        /// ## Characteristics:
        /// - **Diminishing returns**: Growth slows as input increases
        /// - **High early rewards**: Strong incentives for small inputs
        /// - **Unbounded (slow growth)**: Continues increasing but at decreasing rate
        /// - **Deterministic**: Same input and parameters yield identical results
        /// - **Fixed-point safe**: Uses fixed-point logarithmic operations
        /// - **Context-driven**: Controlled via [`LogarithmicConfig`]
        ///
        /// ## Applications:
        /// - Rewarding early participation more than late participation
        /// - Anti-whale incentive systems
        /// - Pricing curves (e.g., bonding curves)
        /// - Resource allocation with diminishing utility
        ///
        /// ## Use Cases
        ///
        /// - Incentivizing early adopters
        /// - Preventing exponential reward growth
        /// - Modeling diminishing marginal returns
        ///
        /// ## Example
        ///
        /// ```ignore
        /// let config = LogarithmicConfig {
        ///     vertical_scale: FixedU128::from_integer(10), // a
        ///     horizontal_scale: FixedU128::from_integer(1), // b
        ///     horizontal_shift: FixedU128::from_integer(1), // c
        ///     vertical_shift: FixedU128::from_integer(0),   // d
        /// };
        ///
        /// let reward = LogarithmicPayout::compute(9u128, Some(config));
        /// // reward = 10 * ln(9 + 1) ~= 10 * ln(10)
        /// ```
        name: pub LogarithmicPayout,
        input: Asset,
        others: [FixedPoint],
        context: LogarithmicConfig<FixedPoint>,
        bounds: [
            Asset: IntegerToFixed + FixedForInteger<FixedPoint = FixedPoint>,
            FixedPoint: FixedPointNumber + FixedOp
        ],
        compute: |input, context| {
            let x = input.to_fixed();

            // Compute inner term: (b * x + c)
            let inner = context
                .horizontal_scale
                .saturating_mul(x)
                .saturating_add(context.horizontal_shift);

            // Compute ln(bx + c)
            let ln_val = FixedOp::fixed_ln(&inner).unwrap_or(FixedPoint::zero());

            // Compute final reward: a * ln(...) + d
            let reward = context
                .vertical_scale
                .saturating_mul(ln_val)
                .saturating_add(context.vertical_shift);

            Asset::from_fixed(&reward)
        }
    );

    // ===============================================================================
    // `````````````````````````````` FIXED-RATE-PAYOUT ``````````````````````````````
    // ===============================================================================

    /// Defines the configuration for the [`FixedRatePayout`] model.
    ///
    /// This struct specifies the parameters for a **fixed-rate reward function**,
    /// where rewards scale linearly with the input.
    ///
    /// **Concept**: **Proportional Scaling Function**
    ///
    /// Rewards follow a simple proportional relationship:
    ///
    /// ```text
    /// reward = x * r
    /// ```
    ///
    /// where:
    /// - `x` = input variable (e.g., stake, balance, or score)
    /// - `r` = fixed rate (fraction per unit input)
    ///
    /// This model produces rewards directly proportional to the input value.
    pub struct FixedRateConfig<F>
    where
        F: FixedPointNumber,
    {
        /// Fixed rate (r): percentage applied to input
        ///
        /// Example: `0.01` represents 1% of input
        pub rate: F,
    }

    plugin_model!(
        /// The **FixedRatePayout** model computes rewards using a **fixed
        /// proportional rate**, scaling linearly with the input asset.
        ///
        /// **Concept**: **Linear Proportional Function**
        ///
        /// ## Formula
        ///
        /// ```text
        /// reward = x * r
        /// ```
        ///
        /// ## Characteristics:
        /// - **Linear scaling**: Reward increases proportionally with input
        /// - **Simple and efficient**: Minimal computation required
        /// - **Deterministic**: Same input and rate yield identical results
        /// - **Fixed-point safe**: Uses fixed-point arithmetic for precision
        /// - **Context-driven**: Controlled via [`FixedRateConfig`]
        ///
        /// ## Applications:
        /// - Percentage-based rewards (e.g., staking yield)
        /// - Fee or commission calculations
        /// - Proportional incentive distribution
        /// - Basic inflation mechanisms
        ///
        /// ## Use Cases
        ///
        /// - Rewarding participants based on stake size
        /// - Applying consistent percentage returns
        /// - Baseline reward models for systems
        ///
        /// ## Example
        ///
        /// ```ignore
        /// let config = FixedRateConfig {
        ///     rate: FixedU128::from_rational(1, 100), // 1%
        /// };
        ///
        /// let reward = FixedRatePayout::compute(1000u128, Some(config));
        /// // reward = 1000 * 0.01 = 10
        /// ```
        name: pub FixedRatePayout,
        input: Asset,
        others: [FixedPoint],
        context: FixedRateConfig<FixedPoint>,
        bounds: [
            Asset: IntegerToFixed + FixedForInteger<FixedPoint = FixedPoint>,
            FixedPoint: FixedPointNumber
        ],
        compute: |input, context| {
            let x = input.to_fixed();
            let rate = context.rate;

            // reward = x * r  (saturating to avoid overflow)
            let reward = x.saturating_mul(rate);

            Asset::from_fixed(&reward)
        }
    );

    // ===============================================================================
    // ````````````````````````````` FIXED-ANNUAL-PAYOUT `````````````````````````````
    // ===============================================================================

    /// Defines the configuration for the [`FixedAnnualPayout`] model.
    ///
    /// This struct specifies the parameters for converting an **annual percentage
    /// rate (APR)** into a per-period reward.
    ///
    /// **Concept**: **Discrete Compounding Conversion**
    ///
    /// Rewards are derived by converting APR into an **effective per-period
    /// rate (EPR)**:
    ///
    /// ```text
    /// EPR = (1 + APR)^(1 / n) - 1
    /// reward = x * EPR
    /// ```
    ///
    /// where:
    /// - `APR` = annual percentage rate
    /// - `n`   = number of reward intervals per year
    /// - `x`   = input variable (e.g., stake or balance)
    ///
    /// This model ensures that rewards are distributed consistently across
    /// discrete time intervals while preserving the annual rate.
    pub struct FixedAnnualConfig<T, F>
    where
        F: FixedPointNumber,
    {
        /// Annual Percentage Rate (APR)
        pub apr: F,

        /// Number of reward allocations per year (n)
        pub time_count: T,
    }

    plugin_model!(
        /// The **FixedAnnualPayout** model computes rewards using a **per-period
        /// rate derived from an annual percentage rate (APR)**.
        ///
        /// **Concept**: **APR to Periodic Yield Conversion**
        ///
        /// ## Formula
        ///
        /// ```text
        /// EPR = (1 + APR)^(1 / n) - 1
        /// reward = x * EPR
        /// ```
        ///
        /// ## Characteristics:
        /// - **Time-aware scaling**: Converts annual rate into per-period rewards
        /// - **Compounding-consistent**: Preserves APR across discrete intervals
        /// - **Deterministic**: Same input and parameters yield identical results
        /// - **Fixed-point safe**: Uses fixed-point exponentiation
        /// - **Context-driven**: Controlled via [`FixedAnnualConfig`]
        ///
        /// ## Applications:
        /// - Staking reward systems with APR targets
        /// - Financial yield calculations
        /// - Periodic emission schedules
        /// - Interest-based incentive models
        ///
        /// ## Use Cases
        ///
        /// - Converting annual rewards into per-block or per-era payouts
        /// - Maintaining consistent yield across different time granularities
        /// - Financial modeling of compounding returns
        ///
        /// ## Example
        ///
        /// ```ignore
        /// let config = FixedAnnualConfig {
        ///     apr: FixedU128::from_rational(12, 100), // 12% APR
        ///     time_count: 12u128, // monthly payouts
        /// };
        ///
        /// let reward = FixedAnnualPayout::compute(1000u128, Some(config));
        /// // reward ~= 1000 * ((1.12)^(1/12) - 1)
        /// ```
        name: pub FixedAnnualPayout,
        input: Asset,
        others: [FixedPoint],
        context: FixedAnnualConfig<Asset, FixedPoint>,
        bounds: [
            Asset: Copy + IntegerToFixed + FixedForInteger<FixedPoint = FixedPoint>,
            FixedPoint: FixedPointNumber + FixedOp
        ],
        compute: |input, context| {
            let one = FixedPoint::one();
 
            // guard against time_count == 0 (undefined period rate).
            let n = context.time_count.to_fixed();
            if n == FixedPoint::zero() {
                debug_assert!(false, "FixedAnnualPayout: time_count is zero - period rate undefined");
                return Asset::from_fixed(&FixedPoint::zero());
            }
 
            // Compute base: (1 + APR)
            let base = one.saturating_add(context.apr);
 
            // Compute exponent: 1 / n
            let exponent = one
                .checked_div(&n)
                .unwrap_or_else(|| FixedPoint::zero());
 
            // Compute (1 + APR)^(1/n).
            // on overflow (None), saturate toward max rather than zero.
            let power = match FixedOp::fixed_pow(&base, &exponent) {
                Some(v) => v,
                None => {
                    debug_assert!(false, "FixedAnnualPayout: fixed_pow overflowed - saturating to max");
                    FixedPoint::max_value()
                }
            };
 
            // EPR = (1 + APR)^(1/n) - 1
            let epr = power.saturating_sub(one);
 
            // reward = x * EPR
            let x = input.to_fixed();
            let reward = x.saturating_mul(epr);
 
            Asset::from_fixed(&reward)
        }
    );

    // ===============================================================================
    // ``````````````````````````````` PIECEWISE-PAYOUT ``````````````````````````````
    // ===============================================================================
    
    /// Defines parameters for a **logistic (sigmoid) curve** used in piecewise reward modeling.
    ///
    /// **Concept**: **Logistic Curve Parameterization**
    ///
    /// The curve follows:
    ///
    /// ```text
    /// f(x) = L / (1 + e^(-k*(x - x0)))
    /// ```
    ///
    /// where:
    /// - `L`  = maximum value (upper asymptote)
    /// - `k`  = steepness (growth rate; positive for a growing S-curve)
    /// - `x0` = midpoint / inflection point
    ///
    /// ## Type Requirements
    ///
    /// `FixedPoint` must implement [`FixedSignedCast<Signed = FixedI128>`].
    /// Both unsigned and signed fixed-point types satisfy this bound. Signed
    /// arithmetic (`x - x0` when x < x0, and the negated exponent) is performed
    /// in the concrete `FixedI128` workspace and projected back.
    pub struct CurveParams<F>
    where
        F: FixedPointNumber,
    {
        /// Maximum reward level (L): upper asymptotic bound of the curve
        pub l: F,
    
        /// Steepness factor (k): controls how sharply the curve transitions.
        ///
        /// - k > 0: growing S-curve (output increases with x)
        /// - k < 0: decaying S-curve (output decreases with x)
        /// - k = 0: constant L/2 for all inputs
        pub k: F,
    
        /// Inflection point (x0): input value where the curve is steepest.
        ///
        /// At x = x0: f(x0) = L / 2.
        pub x0: F,
    }
    
    impl<FixedPoint> CurveParams<FixedPoint>
    where
        FixedPoint: FixedPointNumber + FixedSignedCast<Signed = FixedI128>,
    {
        /// Evaluates the logistic curve at input `x`.
        ///
        /// **Formula**
        ///
        /// ```text
        /// f(x) = L / (1 + e^(-k*(x - x0)))
        /// ```
        ///
        /// Signed intermediates (`x - x0`, `k*(x - x0)`, and its negation) are
        /// computed in `FixedI128` via [`FixedSignedCast`] so the method works
        /// correctly for both signed and unsigned `FixedPoint` types..
        pub fn evaluate(&self, x: FixedPoint) -> FixedPoint {
            let zero_s = FixedI128::zero();
    
            // Lift all operands into the signed workspace.
            // saturated_into is infallible for FixedU64 and clamps at i128::MAX
            // for large FixedU128 values; for signed types it is a zero-cost identity.
            let x_s:  FixedI128 = FixedSignedCast::saturated_into(x);
            let x0_s: FixedI128 = FixedSignedCast::saturated_into(self.x0);
            let k_s:  FixedI128 = FixedSignedCast::saturated_into(self.k);
            let l_s:  FixedI128 = FixedSignedCast::saturated_into(self.l);
    
            // Compute x - x0; negative for any x left of the inflection point.
            // The old code used unsigned saturating_sub, which clamped to 0 here
            // and made every point left of x0 behave as if x == x0.
            let diff_s: FixedI128 = x_s.saturating_sub(x0_s);
    
            // Compute -k*(x - x0).
            // k_s * diff_s is negative on the left half (diff_s < 0, k_s > 0)
            // and on the right half with a decaying curve (diff_s > 0, k_s < 0).
            // Negate via subtraction from zero - no .neg() on FixedPoint needed.
            let k_diff_s:     FixedI128 = k_s.saturating_mul(diff_s);
            let neg_k_diff_s: FixedI128 = zero_s.saturating_sub(k_diff_s);
    
            // Compute e^(-k*(x - x0)).
            // Concrete FixedI128::fixed_exp - handles negative arguments correctly.
            // On overflow use max_value() so the denominator is large and the
            // result approaches 0, which is the correct limit for a large exponent.
            let exp_s: FixedI128 = FixedI128::fixed_exp(&neg_k_diff_s)
                .unwrap_or(FixedI128::max_value());
    
            // Compute denominator: 1 + e^(...)
            let denom_s: FixedI128 = FixedI128::one().saturating_add(exp_s);
    
            // Final logistic value: L / denom
            let output_s: FixedI128 = l_s.checked_div(&denom_s).unwrap_or(zero_s);
    

            // Project back to FixedPoint.
            // output_s >= 0 always (L >= 0, denom >= 1); saturated_from clamps
            // any accidental negative to 0 rather than panicking.
            FixedSignedCast::saturated_from(output_s)
        }
    }
    
    /// Defines a **piecewise segment** used in [`PiecewisePayout`].
    ///
    /// **Concept**: **Segmented Function Composition**
    ///
    /// Each segment defines a function over a specific input interval `[start_x, end_x]`.
    /// The overall payout function is constructed by combining multiple segments.
    pub enum Segment<F>
    where
        F: FixedPointNumber,
    {
        /// Linear segment using interpolation between two points.
        ///
        /// **Concept**: **Linear Interpolation**
        ///
        /// ```text
        /// f(x) = y1 + ((x - x1) / (x2 - x1)) * (y2 - y1)
        /// ```
        ///
        /// Both increasing (`y2 > y1`) and decreasing (`y2 < y1`) segments are
        /// supported for signed and unsigned types. The signed delta `y2 - y1`
        /// is computed in `FixedI128` via [`FixedSignedCast`].
        Linear {
            /// Starting input value (x1)
            start_x: F,
            /// Ending input value (x2)
            end_x: F,
            /// Output at start_x (y1)
            start_y: F,
            /// Output at end_x (y2)
            end_y: F,
        },
    
        /// Curve-based segment using a parameterized logistic function.
        ///
        /// **Concept**: **Parameterized Curve Segment**
        ///
        /// Uses [`CurveParams`] to evaluate:
        ///
        /// ```text
        /// f(x) = L / (1 + e^(-k*(x - x0)))
        /// ```
        ///
        /// Works correctly for both signed and unsigned `FixedPoint` types.
        Curve {
            /// Starting input value for the segment
            start_x: F,
            /// Ending input value for the segment
            end_x: F,
            /// Curve parameters defining shape and behavior
            params: CurveParams<F>,
        },
    }
    
    /// Defines the configuration for the [`PiecewisePayout`] model.
    ///
    /// **Concept**: **Piecewise Curve Composition**
    ///
    /// The reward function is:
    ///
    /// ```text
    /// reward(x) = f_i(x),  if x is in [x_i_start, x_i_end]
    ///             0,        if x matches no segment
    /// ```
    pub struct PiecewiseConfig<F>
    where
        F: FixedPointNumber,
    {
        /// Ordered list of segments defining the piecewise function.
        ///
        /// The first matching segment is used; segments may overlap without error.
        pub segments: Vec<Segment<F>>,
    }
    
    plugin_model!(
        /// The **PiecewisePayout** model computes rewards using a **piecewise-defined function**,
        /// selecting different behaviors depending on the input range.
        ///
        /// **Concept**: **Segmented Reward Function**
        ///
        /// ## Formula
        ///
        /// ```text
        /// reward(x) = f_i(x),  for the first segment i where x is in [start_x_i, end_x_i]
        ///             0,        if no segment matches
        /// ```
        ///
        /// ## Segment Types
        ///
        /// - **Linear**: `y = start_y + t * (end_y - start_y)` where `t = (x - x1) / (x2 - x1)`.
        ///   Supports both increasing and decreasing slopes for signed and unsigned types.
        /// - **Curve**: `y = L / (1 + e^(-k*(x - x0)))`.
        ///   Works correctly for unsigned types; signed arithmetic handled via `FixedI128`.
        ///
        /// ## Signed Arithmetic
        ///
        /// Two sub-computations require signed intermediates that unsigned fixed-point
        /// cannot represent:
        ///
        /// - **Curve segments**: `x - x0` is negative for `x < x0` (the left sigmoid tail).
        ///   The old code clamped this to 0 via unsigned `saturating_sub`, flattening the
        ///   entire left half of every curve segment.
        /// - **Decreasing linear segments**: `end_y - start_y` is negative when `end_y < start_y`.
        ///   The old code clamped to 0, turning every ramp-down into a flat line.
        ///
        /// Both are now computed in concrete `FixedI128` via [`FixedSignedCast`].
        ///
        /// ## Characteristics:
        /// - **Composable**: Combine Linear and Curve segments freely.
        /// - **Works with unsigned types**: No `Neg` bound, signed arithmetic in `FixedI128`.
        /// - **Deterministic**: Same input and configuration yield identical output.
        /// - **Context-driven**: Controlled via [`PiecewiseConfig`].
        ///
        /// ## Applications:
        /// - Multi-phase emission schedules.
        /// - Bootstrap, growth, and stabilization curves.
        /// - DeFi incentive design.
        /// - Tiered reward systems.
        ///
        /// ## Example
        ///
        /// ```ignore
        /// // Linear ramp: x in [0, 10] maps reward from 0 to 100.
        /// let config = PiecewiseConfig {
        ///     segments: vec![
        ///         Segment::Linear {
        ///             start_x: FixedU128::zero(),
        ///             end_x:   FixedU128::saturating_from_integer(10),
        ///             start_y: FixedU128::zero(),
        ///             end_y:   FixedU128::saturating_from_integer(100),
        ///         }
        ///     ],
        /// };
        /// assert_eq!(PiecewisePayout::compute(5u128, Some(config)), 50);
        /// ```
        name: pub PiecewisePayout,
        input: Asset,
        others: [FixedPoint],
        context: PiecewiseConfig<FixedPoint>,
        bounds: [
            Asset: Copy + IntegerToFixed + FixedForInteger<FixedPoint = FixedPoint> + Zero,
            FixedPoint: FixedPointNumber + FixedSignedCast<Signed = FixedI128>,
        ],
        compute: |input, context| {
            let zero_fp = FixedPoint::zero();
            let _zero_s  = FixedI128::zero();
            let x: FixedPoint = input.to_fixed();
    
            // Find the first segment whose range contains x.
            // Segment bounds are stored as FixedPoint values; for unsigned types
            // they are always >= 0, and x (from a non-negative Asset) is also >= 0,
            // so the comparisons are correct without any signed-space conversion.
            let seg = context.segments.iter().find(|seg| match seg {
                Segment::Linear { start_x, end_x, .. } => x >= *start_x && x <= *end_x,
                Segment::Curve  { start_x, end_x, .. } => x >= *start_x && x <= *end_x,
            });
    
            let Some(segment) = seg else {
                return Asset::zero();
            };
    
            match segment {
                Segment::Linear { start_x, end_x, start_y, end_y } => {
                    let width = end_x.saturating_sub(*start_x);
    
                    // Degenerate segment (zero width): return start_y unchanged.
                    if width == zero_fp {
                        return Asset::from_fixed(start_y);
                    }
    
                    // Compute t = (x - start_x) / (end_x - start_x), always in [0, 1].
                    // x >= start_x is guaranteed by the segment match above, so
                    // saturating_sub is safe in unsigned space here.
                    let t: FixedPoint = x
                        .saturating_sub(*start_x)
                        .checked_div(&width)
                        .unwrap_or(zero_fp);
    
                    // Compute delta_y = end_y - start_y in signed space.
                    // This value is negative for any decreasing segment (end_y < start_y).
                    // The old code used end_y.saturating_sub(start_y), which clamped
                    // to 0 for unsigned FixedPoint, turning every ramp-down into a
                    // flat line at start_y.
                    let start_y_s: FixedI128 = FixedSignedCast::saturated_into(*start_y);
                    let end_y_s:   FixedI128 = FixedSignedCast::saturated_into(*end_y);
                    let delta_y_s: FixedI128 = end_y_s.saturating_sub(start_y_s); // may be < 0
    
                    // t is in [0, 1] and non-negative; promote for the multiply.
                    let t_s: FixedI128 = FixedSignedCast::saturated_into(t);
                    // Compute y = start_y + t * delta_y
                    let y_s: FixedI128 = start_y_s.saturating_add(t_s.saturating_mul(delta_y_s));
    
                    // Project back to FixedPoint.
                    // For unsigned types, a negative y (possible when delta_y < 0 and
                    // t > 0) clamps to 0 via saturated_from, which is the correct
                    // saturating semantic for rewards.
                    let y_fp: FixedPoint = FixedSignedCast::saturated_from(y_s);
                    Asset::from_fixed(&y_fp)
                }
    
                Segment::Curve { params, .. } => {
                    // Delegate to CurveParams::evaluate, which performs all signed
                    // arithmetic in FixedI128 via FixedSignedCast.
                    let y: FixedPoint = params.evaluate(x);
                    Asset::from_fixed(&y)
                }
            }
        }
    );

}

pub use payee::*;

/// Defines **pluggable payee models** for distributing a total payout
/// across a set of participants.
///
/// Payees are abstracted as allocation models that transform a total payout
/// and a set of participant weights into **individual reward assignments**.
///
/// ## Concept
///
/// - A payee model determines **how a total payout is distributed**.
/// - The distribution is computed over a set of `(Id, Share)` pairs.
/// - The output assigns a payout value to each participant.
///
/// ## Mathematical Form
///
/// Where:
/// - `P` = total payout
/// - `p_i` = payout assigned to participant `i`
/// - `n` = number of participants
///
/// ## In this model:
///
/// - Input is `(Payout, [(Id, Share)])`.
/// - Output is `[(Id, Payout)]`.
///
/// ## Properties
///
/// - **Conservative distribution**: Total assigned payout equals input payout.
/// - **Deterministic mapping**: Same inputs produce identical allocations.
/// - **Context-free or context-driven** depending on model.
/// - **Finite partitioning** of total value across participants.
///
/// ## Purpose
///
/// Payee models provide flexibility in defining reward distribution:
///
/// - Control how total rewards are allocated among participants.
/// - Support different allocation strategies based on weights or structure.
/// - Serve as the final stage in reward pipelines.
pub mod payee {

    // ===============================================================================
    // ``````````````````````````````````` IMPORTS ```````````````````````````````````
    // ===============================================================================

    // --- Core / Std ---
    use core::iter::once;

    // --- FRAME Suite ---
    use frame_suite::{
        fixedpoint::{FixedForInteger, FixedOp, IntegerToFixed},
        plugin_model,
    };

    // --- Substrate primitives ---
    use sp_runtime::{
        traits::{One, Saturating, Zero},
        FixedPointNumber, Vec,
    };

    // ===============================================================================
    // ````````````````````````````````` SHARES PAYEE ````````````````````````````````
    // ===============================================================================

    plugin_model!(
        /// The **SharesPay** model distributes a total payout proportionally
        /// based on participant shares.
        ///
        /// **Concept**: **Proportional Allocation with Remainder Correction**
        ///
        /// ## Mathematical Form
        ///
        /// `p_i = floor(P * s_i / sum(s_j))`
        ///
        /// Subject to:
        ///
        /// `sum(p_i) = P`
        ///
        /// Where:
        /// - `P` = total payout
        /// - `s_i` = share of participant `i`
        /// - `p_i` = payout assigned to participant `i`
        /// - `n` = number of participants
        ///
        /// ## Properties
        ///
        /// - **Proportional allocation**: Each payout is derived from relative share.
        /// - **Discrete rounding**: Values are floored to integer representation.
        /// - **Remainder redistribution**: Residual units are reassigned to preserve
        ///   total sum.
        /// - **Conservative**: Total distributed payout equals input payout.
        /// - **Deterministic**: Same inputs produce identical allocation.
        ///
        /// ## Purpose
        ///
        /// - Enables share-based distribution of rewards.
        /// - Preserves proportional fairness under discrete constraints.
        /// - Ensures exact conservation of total payout.
        name: pub SharesPay,
        input: (Payout,PayoutFor),
        output: Payees,
        others: [Id, Share, FixedPoint],
        bounds: [
            Payout: IntegerToFixed + FixedForInteger<FixedPoint = FixedPoint> + Saturating + Copy + Zero + PartialOrd,
        PayoutFor: IntoIterator<Item = (Id, Share)> + Clone + FromIterator<(Id, Share)>,
        Share: Copy + IntegerToFixed + FixedForInteger<FixedPoint = FixedPoint> + Zero + Saturating,
        Payees: Extend<(Id, Payout)> + Default,
        Id: Clone,
        FixedPoint: FixedPointNumber + FixedOp
    ],
    compute: |input, _context| {
        let (payout, payout_for) = input;
        let payout_fixed = payout.to_fixed();

        let (total_shares, len) = payout_for
            .clone()
            .into_iter()
            .fold((Share::zero(), 0usize), |(acc_share, acc_len), (_, share)| {
                (acc_share.saturating_add(share), acc_len.saturating_add(1usize))
            });

        let mut payees = Payees::default();

        if total_shares.is_zero() || len.is_zero() {
            return payees
        }

        // Calculate each payee's raw payout (fixed-point), floor it, and track the remainder
        let mut payouts: Vec<(Id, Payout, FixedPoint)> = Vec::with_capacity(len);
        let mut total_distributed: Payout = Payout::zero();
        let mut remainders: Vec<(usize, FixedPoint)> = Vec::with_capacity(len);

        for (idx, (id, share)) in payout_for.clone().into_iter().enumerate() {
            let ratio = share.to_fixed().checked_div(&total_shares.to_fixed()).unwrap_or_else(|| Share::zero().to_fixed());
            let pay_fp = payout_fixed.saturating_mul(ratio);
            let pay_int = Payout::from_fixed(&pay_fp);
            let pay_fp_int = pay_int.to_fixed();
            let remainder = pay_fp.saturating_sub(pay_fp_int);
            payouts.push((id.clone(), pay_int, remainder));
            total_distributed = total_distributed.saturating_add(pay_int);
            remainders.push((idx, remainder));
        }

        // Calculate how much is left undistributed due to flooring
        let mut undistributed = payout.saturating_sub(total_distributed);

        // Sort remainders descending, distribute +1 to top N
        remainders.sort_by(|a, b| b.1.cmp(&a.1));
        let mut i = 0;
        while undistributed > Payout::zero() && i < remainders.len() {
            let idx = remainders[i].0;
            payouts[idx].1 = payouts[idx].1.saturating_add(Payout::from_fixed(&FixedPoint::one()));
            undistributed = undistributed.saturating_sub(Payout::from_fixed(&FixedPoint::one()));
            i += 1;
        }

        for (id, pay, _) in payouts {
            payees.extend(core::iter::once((id, pay)));
        }

        payees
    }

    );

    // ===============================================================================
    // ````````````````````````````````` EQUAL PAYEE `````````````````````````````````
    // ===============================================================================

    plugin_model!(
        /// The **EqualPay** model distributes a total payout equally
        /// among all participants.
        ///
        /// **Concept**: **Uniform Allocation**
        ///
        /// ## Mathematical Form
        ///
        /// `p_i = floor( P / n )`
        ///
        /// Subject to:
        ///
        /// `sum(p_i) <= P`
        ///
        /// Where:
        /// - `P` = total payout
        /// - `p_i` = payout assigned to participant `i`
        /// - `n` = number of participants
        ///
        /// ## Properties
        ///
        /// - **Uniform allocation**: Each participant receives the same payout.
        /// - **Discrete division**: Values are derived via integer division.
        /// - **Non-exhaustive**: Total distributed payout may be less than input due to rounding.
        /// - **Deterministic**: Same inputs produce identical allocation.
        ///
        /// ## Purpose
        ///
        /// - Enables equal distribution of rewards.
        /// - Provides simple and uniform allocation strategy.
        /// - Suitable when all participants are treated identically.
        name: pub EqualPay,
        input: (Payout,PayoutFor),
        output: Payees,
        others: [Id, Share, FixedPoint],
        bounds: [
            Payout: IntegerToFixed + FixedForInteger<FixedPoint = FixedPoint> + Saturating + Zero + One,
            PayoutFor: IntoIterator<Item = (Id, Share)> + Clone + FromIterator<(Id, Share)>,
            Share: Copy + IntegerToFixed + FixedForInteger<FixedPoint = FixedPoint>,
            Payees: Extend<(Id, Payout)> + Default,
            Id: Clone,
            FixedPoint: FixedPointNumber + FixedOp
        ],
        compute: |input, _context| {
        let (payout, payout_for) = input;
            let payout_fixed = payout.to_fixed();
            let mut payees = Payees::default();

            // Count the number of payees
            let count: usize = payout_for.clone().into_iter().count();

            if count == 0 {
                return payees
            }

            // Build count as FixedPoint by adding One repeatedly
            let mut count_fixed = FixedPoint::one();
            for _ in 1..count {
                count_fixed = count_fixed.saturating_add(FixedPoint::one());
            }

            // Divide payout equally among all payees
            let equal_pay = payout_fixed.checked_div(&count_fixed)
                .unwrap_or_else(|| FixedPoint::zero());

            for (id, _) in payout_for.clone() {
                payees.extend(once((
                    id.clone(),
                    Payout::from_fixed(&equal_pay)
                )));
            }
            payees
        }
    );
}

// ===============================================================================
// ````````````````````` PAYOUT & PAYEE MODELS PLUGIN TESTS ``````````````````````
// ===============================================================================

#[cfg(test)]
mod tests {

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` IMPORTS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    // --- Local crate imports ---
    use super::*;
    use crate::rewards::payout::{CurveParams, Segment};

    // --- FRAME Suite ---
    use frame_suite::plugin_test;

    // --- Substrate primitives ---
    use sp_runtime::{AccountId32, FixedI128, FixedPointNumber, FixedU128, traits::{One, Zero}};

    // --- Substrate std (no_std helpers) ---
    use sp_std::vec;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````````` CONSTANTS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    const fn account_frm_seed(seed: u8) -> AccountId32 {
        let mut data = [0u8; 32];
        data[31] = seed;
        AccountId32::new(data)
    }

    const ALICE: AccountId32 = account_frm_seed(1);
    const BOB: AccountId32 = account_frm_seed(2);
    const MIKE: AccountId32 = account_frm_seed(5);

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````` PAYOUT MODELS ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    // ---------------------------------- ZERO-PAYOUT ---------------------------------

    plugin_test! {
        model: payout::ZeroPayout,
        input: u128,
        cases: {
        (zero_payout_returns_zero_for_zero_input, 0, 0),
        (zero_payout_returns_zero_for_positive_input, 1000, 0),
        (zero_payout_returns_zero_for_random_input, 15755, 0),
        (zero_payout_returns_zero_for_max_input, u128::MAX, 0),
        }
    }

    // ---------------------------------- CONSTANT-PAYOUT ---------------------------------

    plugin_test! {
        model: payout::ConstantPayout,
        input: u128,
        output: u128,
        context: payout::ConstantPayoutConfig<u128>,
        value: payout::ConstantPayoutConfig { payout: 500 },
        cases: {
        (constant_payout_always_returns_init_reward_for_zero, 0, 500),
        (constant_payout_always_returns_init_reward_for_positive, 1000, 500),
        (constant_payout_always_returns_init_reward_for_random, 45200, 500),
        (constant_payout_always_returns_init_reward_for_max, u128::MAX, 500),
        }
    }

    // ---------------------------------- INFLATION-PAYOUT ---------------------------------

    plugin_test! {
        model: payout::InflationPayout,
        input: u128,
        output: u128,
        context: payout::InflationPayoutConfig<FixedU128>,
        value: payout::InflationPayoutConfig {
            inflation_rate: FixedU128::saturating_from_rational(1, 10), // 10%
        },
        cases: {
        (inflation_payout_10_percent_of_1000, 1000, 100),
        (inflation_payout_10_percent_of_500, 500, 50),
        (inflation_payout_10_percent_of_10, 10, 1),
        }
    }

    plugin_test! {
        model: payout::InflationPayout,
        input: i128,
        output: i128,
        context: payout::InflationPayoutConfig<FixedI128>,
        value: payout::InflationPayoutConfig {
            inflation_rate: FixedI128::saturating_from_rational(1, 10), // 10%
        },
        cases: {
        (inflation_payout_signed_10_percent_of_1000, -1000, -100),
        (inflation_payout_signed_10_percent_of_500, -500, -50),
        (inflation_payout_signed_10_percent_of_10, -10, -1),
        }
    }

    // ---------------------------------- LINEAR-PAYOUT ---------------------------------

    plugin_test! {
        model: payout::LinearPayout,
        input: u128,
        output: u128,
        context: payout::LinearPayoutConfig<FixedU128>,
        value: payout::LinearPayoutConfig {
            slope: FixedU128::saturating_from_integer(2),
            base_reward: FixedU128::saturating_from_integer(10),
        },
        cases: {
        (linear_payout_2x_plus_10_for_5, 5, 20), // 2*5 + 10
        (linear_payout_2x_plus_10_for_0, 0, 10),
        (linear_payout_2x_plus_10_for_150, 150, 310),
        (linear_payout_2x_plus_10_for_725, 725, 1460),
        }
    }

    plugin_test! {
        model: payout::LinearPayout,
        input: i128,
        output: i128,
        context: payout::LinearPayoutConfig<FixedI128>,
        value: payout::LinearPayoutConfig {
            slope: FixedI128::saturating_from_integer(2),
            base_reward: FixedI128::saturating_from_integer(10),
        },
        cases: {
        (linear_payout_signed_case_1, -5, 0), // 2*-5 + 10
        (linear_payout_signed_case_2, -2, 6),
        (linear_payout_signed_case_3, -150, -290),
        (linear_payout_signed_case_4, -725, -1440),
        }
    }
 
    // ---------------------------------- QUADRATIC-PAYOUT ---------------------------------

    plugin_test! {
        model: payout::QuadraticPayout,
        input: u128,
        output: u128,
        context: payout::QuadraticPayoutConfig<FixedU128>,
        value: payout::QuadraticPayoutConfig {
            quadratic_coeff: FixedU128::saturating_from_integer(1),
            linear_coeff: FixedU128::saturating_from_integer(2),
            constant_term: FixedU128::saturating_from_integer(3),
        },
        cases: {
        (quadratic_payout_x2_plus_2x_plus_3_for_2, 2, 11), // 1*4 + 2*2 + 3
        (quadratic_payout_x2_plus_2x_plus_3_for_0, 0, 3),
        (quadratic_payout_x2_plus_2x_plus_3_for_125, 125, 15878),
        (quadratic_payout_x2_plus_2x_plus_3_for_2675, 2675, 7160978),
        }
    }

    plugin_test! {
        model: payout::QuadraticPayout,
        input: i128,
        output: i128,
        context: payout::QuadraticPayoutConfig<FixedI128>,
        value: payout::QuadraticPayoutConfig {
            quadratic_coeff: FixedI128::saturating_from_integer(1),
            linear_coeff: FixedI128::saturating_from_integer(2),
            constant_term: FixedI128::saturating_from_integer(3),
        },
        cases: {
        (quadratic_payout_signed_case_1, -2, 3), // 1*4 + 2*-2 + 3
        (quadratic_payout_signed_case_2, -24, 531),
        (quadratic_payout_signed_case_3, -125, 15378),
        (quadratic_payout_signed_case_4, -2675, 7150278),
        }
    }

    // ---------------------------------- HALVING-PAYOUT ---------------------------------
    
    plugin_test! {
        model: payout::HalvingPayout,
        input: u128,
        output: u128,
        context: payout::HalvingPayoutConfig<u128>,
        value: payout::HalvingPayoutConfig { initial_reward: 1024 },
        cases: {
        (halving_payout_initial_reward_for_era_0, 0, 1024),
        (halving_payout_halved_for_era_1, 1, 512),
        (halving_payout_halved_for_era_2, 2, 256),
        (halving_payout_halved_for_era_8, 8, 4),
        (halving_payout_halved_for_era_10, 10, 1),
        (halving_payout_zero_for_era_11, 11, 0)
        }
    }
 
    plugin_test! {
        model: payout::HalvingPayout,
        input: i128,
        output: i128,
        context: payout::HalvingPayoutConfig<i128>,
        value: payout::HalvingPayoutConfig { initial_reward: -1024 },
        cases: {
        (halving_payout_negative_initial_for_era_0, 0, -1024),
        (halving_payout_negative_halved_for_era_1, 1, -512),
        (halving_payout_negative_halved_for_era_2, 2, -256),
        (halving_payout_negative_halved_for_era_6, 6, -16),
        }
    }

    // ---------------------------------- EXP-DECAY-PAYOUT ----------------------------------

    // A -> UNSIGNED ASSET

    // --- A1. Standard decay: a=0.1, r0=1000 ---
    plugin_test! {
        model: ExpDecayPayout,
        input: u128,
        output: u128,
        context: ExpDecayPayoutConfig<u128, FixedU128>,
        value: ExpDecayPayoutConfig {
            initial_reward: 1000u128,
            decay_constant: FixedU128::saturating_from_rational(1, 10), // a = 0.1
        },
        cases: {
        // x=0: e^0 = 1.0 -> reward = 1000
        (exp_decay_unsigned_x0, 0, 1000),
        // x=1: e^-0.1 ~= 0.9048 -> 904
        (exp_decay_unsigned_x1, 1, 904),
        // x=5: e^-0.5 ~= 0.6065 -> 606
        (exp_decay_unsigned_x5, 5, 606),
        // x=10: e^-1.0 ~= 0.3679 -> 367
        (exp_decay_unsigned_x10, 10, 367),
        // x=20: e^-2.0 ~= 0.1353 -> 135
        (exp_decay_unsigned_x20, 20, 135),
        // x=50: e^-5.0 ~= 0.0067 -> 6
        (exp_decay_unsigned_x50, 50, 6),
        // x=100: e^-10 ~= 0.000045 -> 0 (truncated)
        (exp_decay_unsigned_x100, 100, 0),
        }
    }

    // --- A2. Zero decay constant (a=0): reward is constant at r0 for all x ---
    //
    // e^(-0 * x) = e^0 = 1, so reward = r0 regardless of x.
    plugin_test! {
        model: ExpDecayPayout,
        input: u128,
        output: u128,
        context: ExpDecayPayoutConfig<u128, FixedU128>,
        value: ExpDecayPayoutConfig {
            initial_reward: 500u128,
            decay_constant: FixedU128::zero(), // a = 0
        },
        cases: {
            (exp_decay_unsigned_zero_decay_x0,   0, 500),
            (exp_decay_unsigned_zero_decay_x10,  10, 500),
            (exp_decay_unsigned_zero_decay_x100, 100, 500),
        }
    } 

    // --- A3. Zero initial reward (r0=0): always 0 regardless of decay ---
    //
    // 0 * anything = 0.
    plugin_test! {
        model: ExpDecayPayout,
        input: u128,
        output: u128,
        context: ExpDecayPayoutConfig<u128, FixedU128>,
        value: ExpDecayPayoutConfig {
            initial_reward: 0u128,
            decay_constant: FixedU128::saturating_from_rational(1, 10),
        },
        cases: {
            (exp_decay_unsigned_zero_r0_x0,  0, 0),
            (exp_decay_unsigned_zero_r0_x10, 10, 0),
        }
    }

    // --- A4. a=1 (fast decay), r0=100 ---
    //
    // x=0 -> 100, x=1 -> ~36, x=2 -> ~13, x=3 -> ~4
    plugin_test! {
        model: ExpDecayPayout,
        input: u128,
        output: u128,
        context: ExpDecayPayoutConfig<u128, FixedU128>,
        value: ExpDecayPayoutConfig {
            initial_reward: 100u128,
            decay_constant: FixedU128::one(), // a = 1
        },
        cases: {
            (exp_decay_unsigned_fast_x0, 0, 100),
            (exp_decay_unsigned_fast_x1, 1, 36),
            (exp_decay_unsigned_fast_x2, 2, 13),
            (exp_decay_unsigned_fast_x3, 3, 4),
            (exp_decay_unsigned_fast_x4, 4, 1),
            // x=5: e^-5 ~= 0.0067 -> 0 (truncated to integer)
            (exp_decay_unsigned_fast_x5, 5, 0),
        }
    }

    // --- A5. a=ln(2)~=0.693, r0=1024: each unit step halves the reward ---
    //
    // This is the continuous analog of HalvingPayout. Because ln(2) is
    // irrational and we use a rational approximation (693/1000), results
    // track 512, 256, 128, ... with small rounding errors.
    plugin_test! {
        model: ExpDecayPayout,
        input: u128,
        output: u128,
        context: ExpDecayPayoutConfig<u128, FixedU128>,
        value: ExpDecayPayoutConfig {
            initial_reward: 1024u128,
            // ln(2) ~= 0.6931471... ~= 693/1000 (good enough for 4 halvings)
            decay_constant: FixedU128::saturating_from_rational(693, 1000),
        },
        cases: {
            // x=0: 1024 * e^0 = 1024
            (exp_decay_unsigned_halving_x0, 0, 1024),
            // x=1: 1024 * e^-ln2 = 512 (tiny rounding -> 512)
            (exp_decay_unsigned_halving_x1, 1, 512),
            // x=2: 1024 * e^-2ln2 = 256
            (exp_decay_unsigned_halving_x2, 2, 256),
            // x=3: 1024 * e^-3ln2 = 128
            (exp_decay_unsigned_halving_x3, 3, 128),
        }
    }
 
    // --- A6. Large x causes exp to approach zero - should not panic ---
    plugin_test! {
        model: ExpDecayPayout,
        input: u128,
        output: u128,
        context: ExpDecayPayoutConfig<u128, FixedU128>,
        value: ExpDecayPayoutConfig {
            initial_reward: 1_000_000u128,
            decay_constant: FixedU128::one(), // a = 1
        },
        cases: {
            // e^-50 is astronomically small - truncates to 0
            (exp_decay_unsigned_large_x, 50, 0),
        }
    }

    // B -> SIGNED ASSET
 
    // --- B1. Standard decay: a=0.1, r0=1000 ---
    plugin_test! {
        model: ExpDecayPayout,
        input: i128,
        output: i128,
        context: ExpDecayPayoutConfig<i128, FixedI128>,
        value: ExpDecayPayoutConfig {
            initial_reward: 1000i128,
            decay_constant: FixedI128::saturating_from_rational(1, 10),
        },
        cases: {
            (exp_decay_signed_x0,   0, 1000),
            (exp_decay_signed_x1,   1, 904),
            (exp_decay_signed_x5,   5, 606),
            (exp_decay_signed_x10,  10, 367),
            (exp_decay_signed_x20,  20, 135),
            (exp_decay_signed_x50,  50, 6),
            (exp_decay_signed_x100, 100, 0),
        }
    }
 
    // --- B2. Negative input (x < 0): -a*x becomes positive -> e^(positive) > 1 ---
    plugin_test! {
        model: ExpDecayPayout,
        input: i128,
        output: i128,
        context: ExpDecayPayoutConfig<i128, FixedI128>,
        value: ExpDecayPayoutConfig {
            initial_reward: 1000i128,
            decay_constant: FixedI128::saturating_from_rational(1, 10),
        },
        cases: {
            // x=-10: e^(+1.0) ~= 2.718 -> reward = 2718
            (exp_decay_signed_neg_x10, -10, 2718),
            // x=-5: e^(+0.5) ~= 1.6487 -> reward = 1648
            (exp_decay_signed_neg_x5, -5, 1648),
            // x=-1: e^(+0.1) ~= 1.1052 -> reward = 1105
            (exp_decay_signed_neg_x1, -1, 1105),
        }
    }
 
    // --- B3. Zero decay, signed ---
    plugin_test! {
        model: ExpDecayPayout,
        input: i128,
        output: i128,
        context: ExpDecayPayoutConfig<i128, FixedI128>,
        value: ExpDecayPayoutConfig {
            initial_reward: 500i128,
            decay_constant: FixedI128::zero(),
        },
        cases: {
            (exp_decay_signed_zero_decay_x0,  0, 500),
            (exp_decay_signed_zero_decay_x10, 10, 500),
        }
    }
 
    // --- B4. Zero initial reward, signed ---
    plugin_test! {
        model: ExpDecayPayout,
        input: i128,
        output: i128,
        context: ExpDecayPayoutConfig<i128, FixedI128>,
        value: ExpDecayPayoutConfig {
            initial_reward: 0i128,
            decay_constant: FixedI128::saturating_from_rational(1, 10),
        },
        cases: {
            (exp_decay_signed_zero_r0_x0,  0, 0),
            (exp_decay_signed_zero_r0_x10, 10, 0),
        }
    }
 
    // --- B5. Fast decay (a=1), signed ---
    plugin_test! {
        model: ExpDecayPayout,
        input: i128,
        output: i128,
        context: ExpDecayPayoutConfig<i128, FixedI128>,
        value: ExpDecayPayoutConfig {
            initial_reward: 100i128,
            decay_constant: FixedI128::one(),
        },
        cases: {
            (exp_decay_signed_fast_x0, 0, 100),
            (exp_decay_signed_fast_x1, 1, 36),
            (exp_decay_signed_fast_x2, 2, 13),
            (exp_decay_signed_fast_x3, 3, 4),
            (exp_decay_signed_fast_x5, 5, 0),
        }
    }

    // ----------------------------------- SIGMOID-PAYOUT -----------------------------------

    // A -> UNSIGNED ASSET

    // --- A1. Standard curve: a=0.1, b=0.9, L=100 ---
    plugin_test! {
        model: SigmoidPayout,
        input: u128,
        output: u128,
        context: SigmoidPayoutConfig<u128, FixedU128>,
        value: SigmoidPayoutConfig {
            max_reward: 100u128,
            growth_start: FixedU128::saturating_from_rational(1, 10), // 0.1
            growth_end: FixedU128::saturating_from_rational(9, 10), // 0.9
        },
        cases: {
            // x=0: lower tail, f(0) = a*L = 10 (exact by construction)
            (sigmoid_unsigned_x0_lower_tail, 0, 10),
            // x=1: upper tail, f(1) = b*L = 90 (rounds to 89 due to fixed-point rounding)
            (sigmoid_unsigned_x1_upper_tail, 1, 89),
            // x=5: deep saturation -> 100
            (sigmoid_unsigned_x5_saturation, 5, 99),
            // x=100: fully saturated -> 100
            (sigmoid_unsigned_x100_full_saturation, 100, 100),
        }
    }    

    // --- A2. Wide curve: a=0.2, b=0.8, L=1000 ---
    plugin_test! {
        model: SigmoidPayout,
        input: u128,
        output: u128,
        context: SigmoidPayoutConfig<u128, FixedU128>,
        value: SigmoidPayoutConfig {
            max_reward: 1000u128,
            growth_start: FixedU128::saturating_from_rational(2, 10), // 0.2
            growth_end: FixedU128::saturating_from_rational(8, 10), // 0.8
        },
        cases: {
            (sigmoid_unsigned_wide_x0, 0, 200),
            (sigmoid_unsigned_wide_x1, 1, 799),
            (sigmoid_unsigned_wide_x10, 10, 999),
        }
    }
 
    // --- A3. Very steep curve: a=0.01, b=0.99, L=100 ---
    plugin_test! {
        model: SigmoidPayout,
        input: u128,
        output: u128,
        context: SigmoidPayoutConfig<u128, FixedU128>,
        value: SigmoidPayoutConfig {
            max_reward: 100u128,
            growth_start: FixedU128::saturating_from_rational(1, 100), // 0.01
            growth_end: FixedU128::saturating_from_rational(99, 100), // 0.99
        },
        cases: {
            // x=0: f(0) = 1*L/100 = 1
            (sigmoid_unsigned_steep_x0, 0, 1),
            // x=1: f(1) ~= 99 (rounding -> 98)
            (sigmoid_unsigned_steep_x1, 1, 98),
            // x=5: saturation
            (sigmoid_unsigned_steep_x5, 5, 100),
        }
    }
 
    // --- A4. a = 0.5 (midpoint at x=0): symmetric about x=0 ---
    plugin_test! {
        model: SigmoidPayout,
        input: u128,
        output: u128,
        context: SigmoidPayoutConfig<u128, FixedU128>,
        value: SigmoidPayoutConfig {
            max_reward: 100u128,
            growth_start: FixedU128::saturating_from_rational(1, 2), // 0.5 -> logit=0 -> x0=0
            growth_end: FixedU128::saturating_from_rational(9, 10), // 0.9
        },
        cases: {
            // f(0) = L/2 = 50 (exact - x0=0 so x=0 is the midpoint)
            (sigmoid_unsigned_midpoint_at_x0, 0,  50),
            // f(1) ~= b*L = 90 -> rounding -> 89
            (sigmoid_unsigned_midpoint_x1, 1, 89),
            // f(10): saturation
            (sigmoid_unsigned_midpoint_x10, 10, 99),
        }
    }
 
    // --- A5. Zero max_reward: always returns 0 ---
    plugin_test! {
        model: SigmoidPayout,
        input: u128,
        output: u128,
        context: SigmoidPayoutConfig<u128, FixedU128>,
        value: SigmoidPayoutConfig {
            max_reward: 0u128,
            growth_start: FixedU128::saturating_from_rational(1, 10),
            growth_end: FixedU128::saturating_from_rational(9, 10),
        },
        cases: {
            (sigmoid_unsigned_zero_max_x0,   0, 0),
            (sigmoid_unsigned_zero_max_x1,   1, 0),
            (sigmoid_unsigned_zero_max_x100, 100, 0),
        }
    }

    // B -> SIGNED ASSET

    // --- B1. Standard curve: a=0.1, b=0.9, L=100 (same config as A1) ---
    plugin_test! {
        model: SigmoidPayout,
        input: i128,
        output: i128,
        context: SigmoidPayoutConfig<i128, FixedI128>,
        value: SigmoidPayoutConfig {
            max_reward: 100i128,
            growth_start: FixedI128::saturating_from_rational(1, 10),
            growth_end: FixedI128::saturating_from_rational(9, 10),
        },
        cases: {
            (sigmoid_signed_x0_lower_tail, 0, 10),
            (sigmoid_signed_x1_upper_tail, 1, 89),
            (sigmoid_signed_x5_saturation, 5, 99),
            (sigmoid_signed_x100_full_saturation, 100, 100),
        }
    }
 
    // --- B2. Negative input (x < 0), further into the lower tail ---
    plugin_test! {
        model: SigmoidPayout,
        input: i128,
        output: i128,
        context: SigmoidPayoutConfig<i128, FixedI128>,
        value: SigmoidPayoutConfig {
            max_reward: 100i128,
            growth_start: FixedI128::saturating_from_rational(1, 10),
            growth_end: FixedI128::saturating_from_rational(9, 10),
        },
        cases: {
            // x=-1: deep in lower tail -> 0
            (sigmoid_signed_neg_x1,  -1, 0),
            // x=-5: even deeper -> 0
            (sigmoid_signed_neg_x5,  -5, 0),
        }
    }
 
    // --- B3. Wide curve, signed: a=0.2, b=0.8, L=1000 ---
    plugin_test! {
        model: SigmoidPayout,
        input: i128,
        output: i128,
        context: SigmoidPayoutConfig<i128, FixedI128>,
        value: SigmoidPayoutConfig {
            max_reward: 1000i128,
            growth_start: FixedI128::saturating_from_rational(2, 10),
            growth_end: FixedI128::saturating_from_rational(8, 10),
        },
        cases: {
            (sigmoid_signed_wide_x0, 0, 200),
            (sigmoid_signed_wide_x1, 1, 799),
            (sigmoid_signed_wide_x10, 10, 999),
            // x=-1: far into lower tail, exponent = k*(1+x0) ~= 2.773*1.5 ~= 4.16
            //        e^4.16 ~= 64 -> f = 1000/65 ~= 15
            (sigmoid_signed_wide_neg_x1, -1, 15),
        }
    }
 
    // --- B4. Midpoint at x=0 (a=0.5): negative input -> below L/2 ---
    plugin_test! {
        model: SigmoidPayout,
        input: i128,
        output: i128,
        context: SigmoidPayoutConfig<i128, FixedI128>,
        value: SigmoidPayoutConfig {
            max_reward: 100i128,
            growth_start: FixedI128::saturating_from_rational(1, 2),
            growth_end: FixedI128::saturating_from_rational(9, 10),
        },
        cases: {
            (sigmoid_signed_mid_x0, 0, 50),
            (sigmoid_signed_mid_x1, 1, 89),
            // x=-1: symmetric to x=1 around x0=0 -> f(-1) = L - f(1) ~= 100-90 = 10
            (sigmoid_signed_mid_neg_x1, -1, 10),
            // x=-2: f(-2) = L - f(2) ~= 100-98 = 2
            (sigmoid_signed_mid_neg_x2, -2, 1),
        }
    }

    // C -> GAURD CONDITIONS

    // --- C1. growth_start = 0 (invalid: not strictly in (0,1)) ---
    plugin_test! {
        model: SigmoidPayout,
        input: u128,
        output: u128,
        context: SigmoidPayoutConfig<u128, FixedU128>,
        value: SigmoidPayoutConfig {
            max_reward: 100u128,
            growth_start: FixedU128::zero(), // INVALID
            growth_end: FixedU128::saturating_from_rational(9, 10),
        },
        cases: {
            (sigmoid_guard_unsigned_start_zero, 50, 0),
        }
    }
 
    // --- C2. growth_start = 1 (invalid: not strictly in (0,1)) ---
    plugin_test! {
        model: SigmoidPayout,
        input: u128,
        output: u128,
        context: SigmoidPayoutConfig<u128, FixedU128>,
        value: SigmoidPayoutConfig {
            max_reward: 100u128,
            growth_start: FixedU128::one(),  // INVALID
            growth_end: FixedU128::saturating_from_rational(9, 10),
        },
        cases: {
            (sigmoid_guard_unsigned_start_one, 50, 0),
        }
    }
 
    // --- C3. growth_end = 0 (invalid) ---
    plugin_test! {
        model: SigmoidPayout,
        input: u128,
        output: u128,
        context: SigmoidPayoutConfig<u128, FixedU128>,
        value: SigmoidPayoutConfig {
            max_reward: 100u128,
            growth_start: FixedU128::saturating_from_rational(1, 10),
            growth_end: FixedU128::zero(), // INVALID
        },
        cases: {
            (sigmoid_guard_unsigned_end_zero, 50, 0),
        }
    }
 
    // --- C4. growth_end = 1 (invalid) ---
    plugin_test! {
        model: SigmoidPayout,
        input: u128,
        output: u128,
        context: SigmoidPayoutConfig<u128, FixedU128>,
        value: SigmoidPayoutConfig {
            max_reward: 100u128,
            growth_start: FixedU128::saturating_from_rational(1, 10),
            growth_end: FixedU128::one(), // INVALID
        },
        cases: {
            (sigmoid_guard_unsigned_end_one, 50, 0),
        }
    }
 
    // --- C5. growth_start = growth_end -> k = 0, degenerate ---
    plugin_test! {
        model: SigmoidPayout,
        input: u128,
        output: u128,
        context: SigmoidPayoutConfig<u128, FixedU128>,
        value: SigmoidPayoutConfig {
            max_reward: 100u128,
            growth_start: FixedU128::saturating_from_rational(5, 10), // 0.5
            growth_end: FixedU128::saturating_from_rational(5, 10), // 0.5 - same -> k=0
        },
        cases: {
            (sigmoid_guard_unsigned_equal_fracs, 50, 0),
        }
    }
 
    // --- C6. Same guards for signed type ---
    plugin_test! {
        model: SigmoidPayout,
        input: i128,
        output: i128,
        context: SigmoidPayoutConfig<i128, FixedI128>,
        value: SigmoidPayoutConfig {
            max_reward: 100i128,
            growth_start: FixedI128::zero(), // INVALID
            growth_end: FixedI128::saturating_from_rational(9, 10),
        },
        cases: {
            (sigmoid_guard_signed_start_zero, 50, 0),
        }
    }
 
    plugin_test! {
        model: SigmoidPayout,
        input: i128,
        output: i128,
        context: SigmoidPayoutConfig<i128, FixedI128>,
        value: SigmoidPayoutConfig {
            max_reward: 100i128,
            growth_start: FixedI128::saturating_from_rational(1, 10),
            growth_end: FixedI128::one(), // INVALID
        },
        cases: {
            (sigmoid_guard_signed_end_one, 50, 0),
        }
    }
 
    // --- C7. Inverted curve (growth_start > growth_end) ---
    //
    // Both parameters are individually valid (in (0,1)), but since
    // growth_start > growth_end, the derived slope k wil be negative.
    //
    // k < 0 inverts the sigmoid:
    //   - The curve decays instead of grows
    //   - x = 0     -> high value  (~ b * L)
    //   - x -> large  -> low value   (~ a * L)
    // This is valid behavior for a decreasing reward schedule.
    //
    // This behavior is not explicitly guarded in the model
    // (only k = 0 is guarded), so we document it here as
    // a valid "decreasing reward schedule".
    plugin_test! {
        model: SigmoidPayout,
        input: u128,
        output: u128,
        context: SigmoidPayoutConfig<u128, FixedU128>,
        value: SigmoidPayoutConfig {
            max_reward:   100u128,
            growth_start: FixedU128::saturating_from_rational(9, 10), // 0.9 - inverted
            growth_end:   FixedU128::saturating_from_rational(1, 10), // 0.1
        },
        cases: {
            // Decreasing curve -> high at x=0, low at x=1
            (sigmoid_inverted_unsigned_x0,  0,  89),
            (sigmoid_inverted_unsigned_x1,  1,  10),
            // Deep past the curve -> very small
            (sigmoid_inverted_unsigned_x10, 10,  0),
        }
    }

    // ----------------------------- INVERSE-PROPORTIONAL-PAYOUT ----------------------------
 
    plugin_test! {
        model: payout::InverseProportionalPayout,
        input: u128,
        output: u128,
        context: payout::InverseProportionalConfig<FixedU128>,
        value: payout::InverseProportionalConfig {
            k: FixedU128::saturating_from_integer(100),
            epsilon: FixedU128::one(),
        },
        cases: {
            (inv_prop_u128_x0,   0,   100),
            (inv_prop_u128_x9,   9,   10),
            (inv_prop_u128_x99,  99,  1),
            (inv_prop_u128_x999, 999, 0),
        }
    }
 
    // Signed: negative x -> denom = x + eps may be <= 0 -> return 0 
    // With k = 100, eps = 1:
    //   x = -1 -> denom = -1 + 1 = 0 -> return 0
    //   x = -5 -> denom = -5 + 1 = -4 -> return 0
    plugin_test! {
        model: payout::InverseProportionalPayout,
        input: i128,
        output: i128,
        context: payout::InverseProportionalConfig<FixedI128>,
        value: payout::InverseProportionalConfig {
            k: FixedI128::saturating_from_integer(100),
            epsilon: FixedI128::one(),
        },
        cases: {
            (inv_prop_i128_positive_x9,   9,   10),
            (inv_prop_i128_positive_x99,  99,  1),
            // FIX 1: non-positive denom -> 0
            (inv_prop_i128_neg_x1,  -1,  0),
            (inv_prop_i128_neg_x5,  -5,  0),
            // x=0 -> denom = 0 + 1 = 1 -> 100 / 1 = 100
            (inv_prop_i128_zero,     0,  100),
        }
    }

    // ---------------------------------- FIXED-RATE-PAYOUT ---------------------------------

    plugin_test! {
        model: payout::FixedRatePayout,
        input: u128,
        output: u128,
        context: payout::FixedRateConfig<FixedU128>,
        value: payout::FixedRateConfig {
            rate: FixedU128::saturating_from_rational(5, 100), // 5%
        },
        cases: {
        (fixed_rate_payout_5_percent_of_1000, 1000, 50),
        (fixed_rate_payout_5_percent_of_200, 200, 10),
        (fixed_rate_payout_5_percent_of_16800, 16800, 840),
        (fixed_rate_payout_5_percent_of_0, 0, 0),
        }
    }

    plugin_test! {
        model: payout::FixedRatePayout,
        input: i128,
        output: i128,
        context: payout::FixedRateConfig<FixedI128>,
        value: payout::FixedRateConfig {
            rate: FixedI128::saturating_from_rational(1, 10), // 10%
        },
        cases: {
            (fixed_rate_payout_signed_10_percent_of_370,  -370, -37),
            (fixed_rate_payout_signed_10_percent_of_1000, -1000, -100),
            (fixed_rate_payout_signed_10_percent_of_max, i128::MIN, -17014118346046923173),
        }
    }

    // --------------------------------- FIXED-ANNUAL-PAYOUT --------------------------------

    plugin_test! {
        model: payout::FixedAnnualPayout,
        input: u128,
        output: u128,
        context: payout::FixedAnnualConfig<u128, FixedU128>,
        value: payout::FixedAnnualConfig {
            apr: FixedU128::saturating_from_rational(12, 100), // 12%
            time_count: 12u128, // monthly
        },
        cases: {
            // 1000 * (1.12^(1/12) - 1) ~= 9
            (fixed_annual_12_percent_monthly_of_1000, 1000, 9),
            // 10000 * same ~= 94
            (fixed_annual_12_percent_monthly_of_10000, 10000, 94),
            (fixed_annual_12_percent_monthly_of_0, 0, 0),
        }
    }
 
    plugin_test! {
        model: payout::FixedAnnualPayout,
        input: u128,
        output: u128,
        context: payout::FixedAnnualConfig<u128, FixedU128>,
        value: payout::FixedAnnualConfig {
            apr: FixedU128::zero(), // 0% APR
            time_count: 12u128,
        },
        cases: {
            // EPR = (1+0)^(1/12) - 1 = 0 -> reward = 0
            (fixed_annual_zero_apr_any_input,  1000, 0),
            (fixed_annual_zero_apr_zero_input, 0,    0),
        }
    }
 
    // APR = 100%, n = 1 -> EPR = 1.0 -> reward = x
    plugin_test! {
        model: payout::FixedAnnualPayout,
        input: u128,
        output: u128,
        context: payout::FixedAnnualConfig<u128, FixedU128>,
        value: payout::FixedAnnualConfig {
            apr: FixedU128::one(), // 100%
            time_count: 1u128,
        },
        cases: {
            (fixed_annual_100_percent_n1_of_500,  500,  500),
            (fixed_annual_100_percent_n1_of_1000, 1000, 1000),
        }
    }
     
    // ---------------------------------- LOGARITHMIC-PAYOUT ---------------------------------

    plugin_test! {
        model: payout::LogarithmicPayout,
        input: u128,
        output: u128,
        context: payout::LogarithmicConfig<FixedU128>,
        value: payout::LogarithmicConfig {
            vertical_scale: FixedU128::one(),
            horizontal_scale: FixedU128::one(),
            horizontal_shift: FixedU128::one(),
            vertical_shift: FixedU128::zero(),
        },
        cases: {
            (logarthmic_payout_unsigned_case_1, 1, 0),
            (logarthmic_payout_unsigned_case_2, 10, 2),
            (logarthmic_payout_unsigned_case_3, 112, 4),
            (logarthmic_payout_unsigned_case_4, 7025, 8)
        }
    }

    plugin_test! {
        model: payout::LogarithmicPayout,
        input: i128,
        output: i128,
        context: payout::LogarithmicConfig<FixedI128>,
        value: payout::LogarithmicConfig {
            vertical_scale: FixedI128::one(),
            horizontal_scale: FixedI128::one(),
            horizontal_shift: FixedI128::one(),
            vertical_shift: FixedI128::zero(),
        },
        cases: {
            (logarthmic_payout_signed_case_1, -1, 0),
            (logarthmic_payout_signed_case_2, -10, 0),
            (logarthmic_payout_signed_case_3, -112, 0),
            (logarthmic_payout_signed_case_4, -7025, 0)
        }
    }

    // ------------------------------- PIECEWISE-PAYOUT ------------------------------
 
    // A -> LINEAR SEGMENT - UNSIGNED TYPES
 
    // --- A1. Increasing linear [0, 10] -> [0, 100] ---
    plugin_test! {
        model: PiecewisePayout,
        input: u128,
        output: u128,
        context: PiecewiseConfig<FixedU128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Linear {
                    start_x: FixedU128::zero(),
                    end_x:   FixedU128::saturating_from_integer(10),
                    start_y: FixedU128::zero(),
                    end_y:   FixedU128::saturating_from_integer(100),
                }
            ],
        },
        cases: {
            (linear_unsigned_increasing_x0, 0, 0),
            (linear_unsigned_increasing_x5, 5, 50),
            (linear_unsigned_increasing_x10, 10, 100),
        }
    }
 
    // --- A2. Decreasing linear [0, 10] -> [100, 0] ---
    plugin_test! {
        model: PiecewisePayout,
        input: u128,
        output: u128,
        context: PiecewiseConfig<FixedU128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Linear {
                    start_x: FixedU128::zero(),
                    end_x:   FixedU128::saturating_from_integer(10),
                    start_y: FixedU128::saturating_from_integer(100),
                    end_y:   FixedU128::zero(),
                }
            ],
        },
        cases: {
            // x=0: t=0 -> y = 100
            (linear_unsigned_decreasing_x0, 0, 100),
            // x=5: t=0.5 -> y = 50
            (linear_unsigned_decreasing_x5, 5, 50),
            // x=10: t=1.0 -> y = 0
            (linear_unsigned_decreasing_x10, 10, 0),
        }
    }
 
    // --- A3. Decreasing linear [0, 100] -> [1000, 0], larger scale ---
    plugin_test! {
        model: PiecewisePayout,
        input: u128,
        output: u128,
        context: PiecewiseConfig<FixedU128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Linear {
                    start_x: FixedU128::zero(),
                    end_x:   FixedU128::saturating_from_integer(100),
                    start_y: FixedU128::saturating_from_integer(1000),
                    end_y:   FixedU128::zero(),
                }
            ],
        },
        cases: {
            (linear_unsigned_large_decreasing_x0, 0, 1000),
            (linear_unsigned_large_decreasing_x25, 25, 750),
            (linear_unsigned_large_decreasing_x50, 50, 500),
            (linear_unsigned_large_decreasing_x75, 75, 250),
            (linear_unsigned_large_decreasing_x100, 100, 0),
        }
    }
 
    // --- A4. Flat segment (start_y == end_y), always returns start_y ---
    plugin_test! {
        model: PiecewisePayout,
        input: u128,
        output: u128,
        context: PiecewiseConfig<FixedU128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Linear {
                    start_x: FixedU128::zero(),
                    end_x:   FixedU128::saturating_from_integer(50),
                    start_y: FixedU128::saturating_from_integer(42),
                    end_y:   FixedU128::saturating_from_integer(42),
                }
            ],
        },
        cases: {
            (linear_unsigned_flat_x0, 0, 42),
            (linear_unsigned_flat_x25, 25, 42),
            (linear_unsigned_flat_x50, 50, 42),
        }
    }
 
    // B -> LINEAR SEGMENT - SIGNED TYPES (regression)
 
    // --- B1. Increasing linear ---
    plugin_test! {
        model: PiecewisePayout,
        input: i128,
        output: i128,
        context: PiecewiseConfig<FixedI128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Linear {
                    start_x: FixedI128::zero(),
                    end_x:   FixedI128::saturating_from_integer(10),
                    start_y: FixedI128::zero(),
                    end_y:   FixedI128::saturating_from_integer(100),
                }
            ],
        },
        cases: {
            (linear_signed_increasing_x0, 0, 0),
            (linear_signed_increasing_x5, 5, 50),
            (linear_signed_increasing_x10, 10, 100),
        }
    }
 
    // --- B2. Decreasing linear ---
    plugin_test! {
        model: PiecewisePayout,
        input: i128,
        output: i128,
        context: PiecewiseConfig<FixedI128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Linear {
                    start_x: FixedI128::zero(),
                    end_x:   FixedI128::saturating_from_integer(10),
                    start_y: FixedI128::saturating_from_integer(100),
                    end_y:   FixedI128::zero(),
                }
            ],
        },
        cases: {
            (linear_signed_decreasing_x0, 0, 100),
            (linear_signed_decreasing_x5, 5, 50),
            (linear_signed_decreasing_x10, 10, 0),
        }
    }
 
    // --- B3. Signed segment with negative y-values, ramp from 0 to -100 ---
    plugin_test! {
        model: PiecewisePayout,
        input: i128,
        output: i128,
        context: PiecewiseConfig<FixedI128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Linear {
                    start_x: FixedI128::zero(),
                    end_x:   FixedI128::saturating_from_integer(10),
                    start_y: FixedI128::zero(),
                    end_y:   FixedI128::saturating_from_integer(-100),
                }
            ],
        },
        cases: {
            (linear_signed_neg_y_x0, 0, 0),
            (linear_signed_neg_y_x5, 5, -50),
            (linear_signed_neg_y_x10, 10, -100),
        }
    }
 
    // C. -> CURVE SEGMENT - UNSIGNED TYPES 

    // --- C1. L=100, k=1, x0=5 ---
    //
    //   f(x) = 100 / (1 + e^(-1*(x - 5)))
    //
    //   x=0 (left tail):  exp_arg = -(0-5) = +5 -> e^5 ~= 148.41 -> 100/149.41 ~= 0
    //   x=2 (left tail):  exp_arg = -(2-5) = +3 -> e^3 ~=  20.09 -> 100/21.09 ~= 4
    //   x=4 (left tail):  exp_arg = -(4-5) = +1 -> e^1 ~=   2.72 -> 100/3.72 ~= 26
    //   x=5 (midpoint):   exp_arg = 0 -> e^0 = 1 -> 100/2 = 50
    //   x=6 (right tail): exp_arg = -(6-5) = -1 -> e^(-1) ~=0.368 -> 100/1.368 ~= 73
    //   x=8 (right tail): exp_arg = -(8-5) = -3 -> e^(-3) ~=0.050 -> 100/1.050 ~= 95
    //   x=100 (saturation): -> 100
    plugin_test! {
        model: PiecewisePayout,
        input: u128,
        output: u128,
        context: PiecewiseConfig<FixedU128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Curve {
                    start_x: FixedU128::zero(),
                    end_x:   FixedU128::saturating_from_integer(100),
                    params:  CurveParams {
                        l:  FixedU128::saturating_from_integer(100),
                        k:  FixedU128::one(),
                        x0: FixedU128::saturating_from_integer(5),
                    },
                }
            ],
        },
        cases: {
            // Left of x0=5 
            (curve_unsigned_left_tail_x0, 0, 0),
            (curve_unsigned_left_tail_x2, 2, 4),
            (curve_unsigned_left_tail_x4, 4, 26),
            // Midpoint
            (curve_unsigned_midpoint_x5, 5, 50),
            // Right of x0=5
            (curve_unsigned_right_tail_x6, 6, 73),
            (curve_unsigned_right_tail_x8, 8, 95),
            // Saturation
            (curve_unsigned_saturation, 100, 100),
        }
    }
 
    // --- C2. L=100, k=2, x0=3, unsigned: steeper curve, x0 not at 5 ---
    //
    //   f(x) = 100 / (1 + e^(-2*(x - 3)))
    //
    //   x=0: exp_arg = -2*(0-3) = +6 -> e^6 ~= 403.4 -> 100/404.4 ~= 0
    //   x=1: exp_arg = -2*(1-3) = +4 -> e^4 ~= 54.6 -> 100/55.6 ~= 1
    //   x=2: exp_arg = -2*(2-3) = +2 -> e^2 ~= 7.4 -> 100/8.4 ~= 11
    //   x=3: exp_arg = 0 -> 100/2 = 50
    //   x=4: exp_arg = -2*(4-3) = -2 -> e^(-2) ~= 0.135-> 100/1.135 ~= 88
    //   x=5: exp_arg = -2*(5-3) = -4 -> e^(-4) ~= 0.018-> 100/1.018 ~= 98
    plugin_test! {
        model: PiecewisePayout,
        input: u128,
        output: u128,
        context: PiecewiseConfig<FixedU128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Curve {
                    start_x: FixedU128::zero(),
                    end_x:   FixedU128::saturating_from_integer(20),
                    params:  CurveParams {
                        l:  FixedU128::saturating_from_integer(100),
                        k:  FixedU128::saturating_from_integer(2),
                        x0: FixedU128::saturating_from_integer(3),
                    },
                }
            ],
        },
        cases: {
            (curve_unsigned_steep_x0, 0, 0),
            (curve_unsigned_steep_x1, 1, 1),
            (curve_unsigned_steep_x2, 2, 11),
            (curve_unsigned_steep_x3, 3, 50),
            (curve_unsigned_steep_x4, 4, 88),
            (curve_unsigned_steep_x5, 5, 98),
        }
    }
 
    // --- C3. x0=0 (midpoint at the boundary), x=0 always gives L/2 ---
    plugin_test! {
        model: PiecewisePayout,
        input: u128,
        output: u128,
        context: PiecewiseConfig<FixedU128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Curve {
                    start_x: FixedU128::zero(),
                    end_x:   FixedU128::saturating_from_integer(20),
                    params:  CurveParams {
                        l:  FixedU128::saturating_from_integer(100),
                        k:  FixedU128::one(),
                        x0: FixedU128::zero(), // midpoint at x=0
                    },
                }
            ],
        },
        cases: {
            (curve_unsigned_x0_at_midpoint, 0, 50),
            (curve_unsigned_x0_right_x1, 1, 73),
            (curve_unsigned_x0_right_x5, 5, 99),
        }
    }
 
    // D. CURVE SEGMENT - SIGNED TYPES (regression)
 
    // --- D1. L=100, k=1, x0=5 ---
    plugin_test! {
        model: PiecewisePayout,
        input: i128,
        output: i128,
        context: PiecewiseConfig<FixedI128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Curve {
                    start_x: FixedI128::zero(),
                    end_x:   FixedI128::saturating_from_integer(100),
                    params:  CurveParams {
                        l:  FixedI128::saturating_from_integer(100),
                        k:  FixedI128::one(),
                        x0: FixedI128::saturating_from_integer(5),
                    },
                }
            ],
        },
        cases: {
            (curve_signed_at_midpoint, 5, 50),
            (curve_signed_saturation, 100, 100),
            (curve_signed_at_zero, 0, 0),
            (curve_signed_left_x2, 2, 4),
            (curve_signed_left_x4, 4, 26),
            (curve_signed_right_x6, 6, 73),
            (curve_signed_right_x8, 8, 95),
        }
    }
 
    // --- D2. Signed input with negative x (x < 0), x0=5 ---
    //
    //   x=-2: diff = -2 - 5 = -7 -> exp_arg = +7 -> e^7 ~= 1096 -> 100/1097 ~= 0
    //   x=-5: diff = -10 -> exp_arg = +10 -> e^10 ~= 22026 -> ~= 0
    plugin_test! {
        model: PiecewisePayout,
        input: i128,
        output: i128,
        context: PiecewiseConfig<FixedI128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Curve {
                    start_x: FixedI128::saturating_from_integer(-10),
                    end_x:   FixedI128::saturating_from_integer(100),
                    params:  CurveParams {
                        l:  FixedI128::saturating_from_integer(100),
                        k:  FixedI128::one(),
                        x0: FixedI128::saturating_from_integer(5),
                    },
                }
            ],
        },
        cases: {
            (curve_signed_neg_x2, -2, 0),
            (curve_signed_neg_x5, -5, 0),
            (curve_signed_midpoint, 5, 50),
        }
    }
 
    // E -> MULTI-SEGMENT COMPOSITIONS
 
    // --- E1. Linear increase -> decreasing linear (triangle wave) ---
    //
    //   Seg 1: [0, 5]  -> [0, 100] (ramp up)
    //   Seg 2: [5, 10] -> [100, 0] (ramp down)
    //
    //   x=0:  Seg 1, t=0 -> 0
    //   x=2:  Seg 1, t=0.4 -> 40
    //   x=5:  Seg 1 matches (x <= end_x=5), t=1.0 -> 100 (first-match wins)
    //   x=7:  Seg 2, t=0.4 -> 60
    //   x=10: Seg 2, t=1.0 -> 0
    plugin_test! {
        model: PiecewisePayout,
        input: u128,
        output: u128,
        context: PiecewiseConfig<FixedU128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Linear {
                    start_x: FixedU128::zero(),
                    end_x:   FixedU128::saturating_from_integer(5),
                    start_y: FixedU128::zero(),
                    end_y:   FixedU128::saturating_from_integer(100),
                },
                Segment::Linear {
                    start_x: FixedU128::saturating_from_integer(5),
                    end_x:   FixedU128::saturating_from_integer(10),
                    start_y: FixedU128::saturating_from_integer(100),
                    end_y:   FixedU128::zero(),
                },
            ],
        },
        cases: {
            (multi_triangle_x0, 0, 0),
            (multi_triangle_x2, 2, 40),
            (multi_triangle_x5, 5, 100), // first segment matches at x=5
            (multi_triangle_x7, 7, 60),
            (multi_triangle_x10, 10, 0),
        }
    }
 
    // --- E2. Linear ramp -> Curve saturation (bootstrapping schedule) ---
    //
    //   Seg 1: Linear [0, 10] -> [0, 50] (ramp up)
    //   Seg 2: Curve  [10, 100], L=100, k=1, x0=10
    //
    //   x=5:   Seg 1, t=0.5 -> 25
    //   x=10:  Seg 1 (first match), t=1.0 -> 50
    //   x=15:  Seg 2, diff=5, exp_arg=-5 -> e^(-5)~=0.0067 -> 100/1.0067~=99
    //   x=100: Seg 2, saturation -> 100
    plugin_test! {
        model: PiecewisePayout,
        input: u128,
        output: u128,
        context: PiecewiseConfig<FixedU128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Linear {
                    start_x: FixedU128::zero(),
                    end_x:   FixedU128::saturating_from_integer(10),
                    start_y: FixedU128::zero(),
                    end_y:   FixedU128::saturating_from_integer(50),
                },
                Segment::Curve {
                    start_x: FixedU128::saturating_from_integer(10),
                    end_x:   FixedU128::saturating_from_integer(100),
                    params:  CurveParams {
                        l:  FixedU128::saturating_from_integer(100),
                        k:  FixedU128::one(),
                        x0: FixedU128::saturating_from_integer(10),
                    },
                },
            ],
        },
        cases: {
            (multi_ramp_curve_x5, 5, 25),
            (multi_ramp_curve_x10, 10, 50), // linear segment matches first
            (multi_ramp_curve_x15, 15, 99),
            (multi_ramp_curve_x100, 100, 100),
        }
    }
 
    // --- E3. Gap between segments: x in the gap -> 0 ---
    //
    //   Seg 1: [0, 5]
    //   Seg 2: [20, 30]
    //   x=10: no segment matches -> 0
    plugin_test! {
        model: PiecewisePayout,
        input: u128,
        output: u128,
        context: PiecewiseConfig<FixedU128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Linear {
                    start_x: FixedU128::zero(),
                    end_x:   FixedU128::saturating_from_integer(5),
                    start_y: FixedU128::zero(),
                    end_y:   FixedU128::saturating_from_integer(50),
                },
                Segment::Linear {
                    start_x: FixedU128::saturating_from_integer(20),
                    end_x:   FixedU128::saturating_from_integer(30),
                    start_y: FixedU128::saturating_from_integer(80),
                    end_y:   FixedU128::saturating_from_integer(100),
                },
            ],
        },
        cases: {
            (multi_gap_in_range_seg1, 5, 50),
            (multi_gap_between, 10, 0),
            (multi_gap_in_range_seg2, 20, 80),
        }
    }
 
    // F -> EDGE / GUARD CASES
 
    // --- F1. Empty segment list -> 0 for any input ---
    plugin_test! {
        model: PiecewisePayout,
        input: u128,
        output: u128,
        context: PiecewiseConfig<FixedU128>,
        value: PiecewiseConfig {
            segments: vec![],
        },
        cases: {
            (empty_segments_x0, 0, 0),
            (empty_segments_x42, 42, 0),
        }
    }
 
    // --- F2. Out-of-range input -> 0 ---
    plugin_test! {
        model: PiecewisePayout,
        input: u128,
        output: u128,
        context: PiecewiseConfig<FixedU128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Linear {
                    start_x: FixedU128::zero(),
                    end_x:   FixedU128::saturating_from_integer(10),
                    start_y: FixedU128::zero(),
                    end_y:   FixedU128::saturating_from_integer(100),
                }
            ],
        },
        cases: {
            (out_of_range_above, 50, 0),
        }
    }
 
    // --- F3. Degenerate linear segment (start_x == end_x) -> always start_y ---
    //
    //   width = 0, so we return start_y unconditionally
    plugin_test! {
        model: PiecewisePayout,
        input: u128,
        output: u128,
        context: PiecewiseConfig<FixedU128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Linear {
                    start_x: FixedU128::saturating_from_integer(5),
                    end_x:   FixedU128::saturating_from_integer(5), // same as start
                    start_y: FixedU128::saturating_from_integer(77),
                    end_y:   FixedU128::saturating_from_integer(99),
                }
            ],
        },
        cases: {
            (degenerate_linear_at_point, 5, 77),
        }
    }
 
    // --- F4. Curve with k=0: e^0 = 1 always -> L/2 everywhere ---
    plugin_test! {
        model: PiecewisePayout,
        input: u128,
        output: u128,
        context: PiecewiseConfig<FixedU128>,
        value: PiecewiseConfig {
            segments: vec![
                Segment::Curve {
                    start_x: FixedU128::zero(),
                    end_x:   FixedU128::saturating_from_integer(100),
                    params:  CurveParams {
                        l:  FixedU128::saturating_from_integer(100),
                        k:  FixedU128::zero(),  // k=0 -> exp_arg=0 -> e^0=1 -> L/2
                        x0: FixedU128::saturating_from_integer(5),
                    },
                }
            ],
        },
        cases: {
            (curve_k_zero_x0,  0,  50),
            (curve_k_zero_x50, 50, 50),
        }
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` PAYEE MODELS ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
        
    // ---------------------------------- SHARES-PAY ---------------------------------

    plugin_test! {
        model: payee::SharesPay,
        input: (u128, Vec<(AccountId32, u128)>),
        output: Vec<(AccountId32, u128)>,
        cases: {
            // 100 payout, shares: [50, 50] => each gets 50
            (shares_pay_equal_split, (100, vec![(ALICE, 50), (BOB, 50)]), vec![(ALICE, 50), (BOB, 50)]),
            // 90 payout, shares: [30, 60] => 30/90=1/3*90=30, 60/90=2/3*90=60
            (shares_pay_proportional_split, (90, vec![(ALICE, 30), (BOB, 60)]), vec![(ALICE, 30), (BOB, 60)]),
            // 0 payout, shares: [10, 20] => all get 0
            (shares_pay_zero_payout, (0, vec![(ALICE, 10), (BOB, 20)]), vec![(ALICE, 0), (BOB, 0)]),
            // payout with zero shares
            (shares_pay_all_zero_shares, (100, vec![(ALICE, 0), (BOB, 0)]), vec![]),
            // payout with empty payees
            (shares_pay_no_payees, (100, vec![]), vec![]),
        }
    }

    // ---------------------------------- EQUAL-PAY ----------------------------------

    plugin_test! {
        model: payee::EqualPay,
        input: (u128, Vec<(AccountId32, u128)>),
        output: Vec<(AccountId32, u128)>,
        cases: {
        // 100 payout, 2 payees: each gets 50
        (equal_pay_two_payees_even_split, (100, vec![(ALICE, 10), (BOB, 20)]), vec![(ALICE, 50), (BOB, 50)]),
        // 90 payout, 3 payees: each gets 30
        (equal_pay_three_payees_even_split, (90, vec![(ALICE, 1u128), (BOB, 1u128), (MIKE, 1u128)]), vec![(ALICE, 30), (BOB, 30), (MIKE, 30)]),
        (equal_pay_three_payees_with_remainder, (100, vec![(ALICE, 1u128), (BOB, 1u128), (MIKE, 1u128)]), vec![(ALICE, 33u128), (BOB, 33u128), (MIKE, 33u128)]),
        // 0 payout, 2 payees: each gets 0
        (equal_pay_zero_payout, (0, vec![(ALICE, 10), (BOB, 20)]), vec![(ALICE, 0), (BOB, 0)]),
        // payout with empty payees
        (equal_pay_no_payees, (100, vec![]), vec![]),
        }
    }
}
