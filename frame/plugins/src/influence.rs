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
// `````````````````````````````` INFLUENCE PLUGINS ``````````````````````````````
// ===============================================================================

//! Provides a suite of **pluggable influence models** to transform raw input values
//! (e.g., stake, contribution, vote weight, or score) into computed influence metrics
//! used by election systems, reputation engines, and governance mechanisms.
//!
//! ## Why Influence, Not Raw Values
//!
//! Raw values (e.g., total stake, number of votes, or token balances) alone often do
//! not capture the **relative importance**, **fairness**, or **risk-adjusted weight**
//! of participants. Influence allows the system to:
//! - Normalize inputs so that extreme values do not dominate outcomes.
//! - Apply non-linear scaling to reward incremental contributions more fairly.
//! - Implement thresholds, caps, or decay functions to manage governance risk.
//! - Adjust voting power or rewards dynamically without changing the underlying raw
//!   assets.
//!
//! By computing influence, the system abstracts raw contributions into **comparable
//! metrics** that can be safely and consistently used in elections, scoring systems,
//! and reward distribution.
//!
//! ## Purpose
//!
//! Influence models enable flexible, runtime-configurable strategies for calculating
//! how much "power" or "weight" an input carries. By swapping models or adjusting
//! their parameters, the system can adapt to different fairness, risk, or proportionality
//! requirements.
//!
//! ## Key Concepts
//!
//! - **Input (`x`)**: Typically represents the resource, stake, vote, or contribution
//!   that is being converted to influence.
//! - **Output (`f(x)`)**: The computed influence value used by election or scoring
//!   algorithms.
//! - **Context**: Optional runtime parameters or configurations that guide how the
//!   model behaves.
//!
//! ## Usage
//!
//! Each model is implemented as a [`plugin_model!`] and can be applied dynamically
//! in elections, staking, reputation, or governance systems. Context parameters allow
//! fine-tuning without changing the underlying logic.
//!
//! Example usage scenarios:
//! - Flat election systems: compute influence from author stake or backers.
//! - Reputation systems: convert contributions to normalized influence scores.
//! - Governance voting: implement thresholds, caps, or diminishing returns to improve fairness.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- FRAME Suite ---
use frame_suite::{
    fixedpoint::{FixedForInteger, FixedOp, IntegerToFixed, FixedSignedCast,},
    plugin_model,
};

// --- Substrate primitives ---
use sp_runtime::{
    traits::{Bounded, CheckedDiv, One, Zero},
    FixedI128, FixedPointNumber, Saturating, 
};

// ===============================================================================
// `````````````````````````````` LINEAR-INFLUENCE ```````````````````````````````
// ===============================================================================

plugin_model! {

    /// Provides a **linear influence model** where output equals input.
    ///
    /// `f(x) = x`
    ///
    /// - `x`: input value (e.g., vote weight, token amount)
    ///
    /// ## Characteristics
    /// - Direct proportionality between input and output.
    /// - Simplest and most intuitive model; no transformation applied.
    /// - Useful as a **baseline model** or for **linear scoring systems**.
    ///
    /// ## Reference
    /// - Foundational in statistics and physics.
    /// - Used in **linear regression**, **trend estimation**, and **baseline economic models**.
    /// - https://en.wikipedia.org/wiki/Linear_function_(calculus)
    name: pub LinearModel,
    input: Input,
    bounds: [Input: Clone],
    /// Linear model implementation without needing external context.
    ///
    /// Used when influence is directly proportional to the input.
    /// Always returns the input value unmodified.
    compute: |input, _context| {
        input.clone()
    }
}

// ===============================================================================
// ````````````````````````````` QUADRATIC-INFLUENCE `````````````````````````````
// ===============================================================================

plugin_model! {

    /// Provides a **quadratic (square-root) influence model** that compresses large inputs.
    ///
    /// `f(x) = sqrt(x)`
    ///
    /// - `x`: input value (e.g., score, weight, or stake)
    ///
    /// ## Characteristics
    /// - **Non-linear scaling**: grows rapidly at small values, slows for larger inputs.
    /// - **Compression effect**: prevents large inputs from dominating influence.
    /// - **Handles negative inputs** by clamping to zero (no imaginary numbers).
    ///
    /// ## Applications
    /// - Voting power normalization
    /// - Contribution weighting in participatory systems
    ///
    /// ## References
    /// - https://en.wikipedia.org/wiki/Square_root_voting_system
    /// - Signal scaling and information compression
    name: pub QuadraticModel,
    input: Input,
    bounds: [
        Input: IntegerToFixed + Zero,
        <Input as FixedForInteger>::FixedPoint: FixedOp + PartialOrd
    ],
    /// Quadratic model implementation without external context.
    compute: |input, _context| {
        let x = Input::to_fixed(&input);
        match <<Input as FixedForInteger>::FixedPoint as FixedOp>::fixed_sqrt(&x) {
            Some(sqrt) => Input::from_fixed(&sqrt),
            None       => Input::zero(),
        }
    }
}

// ===============================================================================
// ```````````````````````````` LOGARITHMIC-INFLUENCE ````````````````````````````
// ===============================================================================

plugin_model! {

    /// Provides a **logarithmic influence model** with diminishing returns.
    ///
    /// `f(x) = log(x)`
    ///
    /// - `x`: input value (e.g., contribution, stake, or score)
    ///
    /// ## Characteristics
    /// - **Diminishing returns**: large gains initially, with decreasing marginal impact.
    /// - Natural inverse of exponential growth.
    /// - Helps **limit influence concentration** in large-scale systems.
    /// - For `x <= 0` (undefined domain) values are clamped to zero.
    ///
    /// ## Applications
    /// - Human perception modeling (e.g., sound, brightness)
    /// - Information theory and utility modeling
    ///
    /// ## Reference
    /// - https://en.wikipedia.org/wiki/Weber%E2%80%93Fechner_law
    /// - https://en.wikipedia.org/wiki/Logarithmic_scale
    name: pub LogarithmicModel,
    input: Input,
    bounds: [
        Input: IntegerToFixed + Zero,
        <Input as FixedForInteger>::FixedPoint: FixedOp
    ],
    /// Logarithmic model implementation without external context.
    ///
    /// Models diminishing influence growth.
    /// If the input is very small or zero, fixed_ln must handle domain restrictions safely.
    compute: |input, _context| {
        let x = Input::to_fixed(&input);
        // fixed_ln returns None for x <= 0 (undefined domain); map to zero,
        // which is the natural sentinel for "no influence" in this system.
        match FixedOp::fixed_ln(&x) {
            Some(ln) => Input::from_fixed(&ln),
            None     => Input::zero(),
        }
    }
}

// ===============================================================================
// ````````````````````````````` THRESHOLD-INFLUENCE `````````````````````````````
// ===============================================================================

/// Configuration: the threshold value to activate influence.
pub struct ThresholdModelConfig<T> {
    pub threshold: T,
}

plugin_model! {

    /// Provides a **threshold-based influence model** that enforces minimum eligibility.
    ///
    /// ```text
    /// f(x) = {
    ///     x,   if x >= threshold
    ///     0,   otherwise
    /// }
    /// ```
    ///
    /// - `x`: input value (e.g., score, stake, weight)
    /// - `threshold`: minimum required input for influence
    ///
    /// ## Characteristics
    /// - Enforces **minimum participation or eligibility**.
    /// - Filters out noise or spam contributions.
    ///
    /// ## Applications
    /// - Eligibility filters in voting or staking
    /// - Activity thresholds in DAOs and moderation systems
    ///
    /// ## Reference
    /// - Widely used in economics, game theory, and governance rule sets
    name: pub ThresholdModel,
    input: Input,
    context: ThresholdModelConfig<Input>,
    bounds: [
        Input: PartialOrd + Zero + Clone,
    ],
    /// If input >= threshold, pass it through; otherwise return zero/default.
    compute: |input, context| {
        match input >= context.threshold {
            true => input.clone(),
            false => Input::zero()
        }
    }
}

// ===============================================================================
// `````````````````````````````` SIGMOID-INFLUENCE ``````````````````````````````
// ===============================================================================

/// Configuration for the SigmoidModel.
/// Parameters define the maximum output and the growth phase range for the curve.
pub struct SigmoidModelConfig<F>
where
    F: FixedPointNumber,
{
    /// `L` - Maximum possible output of the sigmoid curve.
    /// This is the upper bound the curve approaches but never exceeds.
    /// Example: If L = 100, the curve will asymptotically approach 100.
    pub max_output: F,

    /// `alpha` - Starting fraction of `max_output` for the growth phase.
    /// Example: `alpha` = 0.10 means growth phase starts when output = 10% of L.
    /// If L = 100, this means growth starts at output = 10.
    pub start_frac: F,

    /// `beta` - Ending fraction of `max_output` for the growth phase.
    /// Example: `beta` = 0.90 means growth phase ends when output = 90% of L.
    /// If L = 100, this means growth ends at output = 90.
    pub end_frac: F,

    /// `x_alpha` - Input value (stake, score, etc.) at which output = alpha * L.
    /// Marks the *start point* of the rapid growth region on the curve.
    /// Example: If x_alpha = 50 and alpha = 0.10, then at stake = 50 the output is 10% of L.
    pub start_x: F,

    /// `x_beta` - Input value at which output = beta * L.
    /// Marks the *end point* of the rapid growth region on the curve.
    /// Example: If x_beta = 80 and beta = 0.90, then at stake = 80 the output is 90% of L.
    pub end_x: F,
}

plugin_model! {

    /// Provides a sigmoid (logistic) influence model with a configurable growth phase.
    ///
    /// ```text
    /// f(x) = L / (1 + e^(-k * (x - x0)))
    /// ```
    ///
    /// You do not set `k` or `x0` directly. Instead you describe the shape of the
    /// curve using five intuitive parameters, and the model derives `k` and `x0`
    /// from them:
    ///
    /// - `L`       : the maximum output the curve can ever reach
    /// - `alpha`   : what fraction of `L` marks the start of the growth phase (e.g. 0.1 = 10%)
    /// - `beta`    : what fraction of `L` marks the end of the growth phase (e.g. 0.9 = 90%)
    /// - `x_alpha` : the input value where output first reaches `alpha * L`
    /// - `x_beta`  : the input value where output reaches `beta * L`
    ///
    /// From those, the model computes:
    ///
    /// - `w = x_beta - x_alpha` (growth width)
    /// - `k = [ ln(beta / (1 - beta)) - ln(alpha / (1 - alpha)) ] / w` (growth rate)
    /// - `x0 = x_alpha - (1 / k) * ln(alpha / (1 - alpha))` (midpoint)
    ///
    /// ## Signed Arithmetic
    ///
    /// Even when the context `FixedPoint` is unsigned, all intermediate steps
    /// (logit, k, x0, the exponent) are computed in concrete `FixedI128` via
    /// `FixedSignedCast`. This is necessary because `logit(alpha)` is negative
    /// for any `alpha < 0.5`, and the exponent `-k * (x - x0)` is negative for
    /// all `x > x0`. Unsigned arithmetic would silently clamp both to zero,
    /// producing a completely wrong curve.
    ///
    /// ## Precision Note
    ///
    /// The `ln -> k -> x0 -> exp` chain accumulates a small amount of rounding
    /// error across four fixed-point operations. The practical effect is that `x0`
    /// lands fractionally above its exact value, shifting the curve slightly to the
    /// right. At the definition points `x_alpha` and `x_beta` the logit cancels
    /// cleanly and the output is exact. At all other points including the midpoint,
    /// the output may be 1 integer unit below the ideal value -- for example,
    /// `f(x_beta)` may yield `89` instead of `90` when `L = 100`.
    ///
    /// - Exact:       `f(80) = 90.000000000`
    /// - Fixed-point: `f(80) = 89.999999...` -> truncates to `89` 
    /// 
    /// This is expected and inconsequential for integer influence scores.
    ///
    /// ## Guard Conditions (returns zero)
    ///
    /// - `alpha <= 0` or `alpha >= 1`
    /// - `beta <= 0` or `beta >= 1`
    /// - `x_beta <= x_alpha` (degenerate or inverted growth window)
    /// - `k == 0` (flat curve, midpoint is undefined)
    name: pub SigmoidModel,
    input: Input,
    others: [FixedPoint],
    context: SigmoidModelConfig<FixedPoint>,
    bounds: [
        Input: IntegerToFixed + FixedForInteger<FixedPoint = FixedPoint> + Zero,
        FixedPoint: FixedPointNumber + FixedSignedCast<Signed = FixedI128>,
    ],
    compute: |input, context| {
        let zero_fp = FixedPoint::zero();
        let one_fp  = FixedPoint::one();
        let zero_s  = FixedI128::zero();

        let l       = context.max_output;
        let alpha   = context.start_frac;
        let beta    = context.end_frac;
        let x_alpha = context.start_x;
        let x_beta  = context.end_x;

        // Guard: alpha and beta must be strictly in (0, 1).
        if alpha <= zero_fp || alpha >= one_fp || beta <= zero_fp || beta >= one_fp {
            return Input::zero();
        }

        // Growth width w = x_beta - x_alpha; must be > 0.
        // Compute in signed space so subtraction is safe even for unsigned FixedPoint.
        let x_alpha_s: FixedI128 = FixedSignedCast::saturated_into(x_alpha);
        let x_beta_s:  FixedI128 = FixedSignedCast::saturated_into(x_beta);
        let w_s: FixedI128 = x_beta_s.saturating_sub(x_alpha_s);
        if w_s <= zero_s {
            return Input::zero();
        }

        // logit(p) = ln(p / (1-p))
        //
        // ratio = p/(1-p) is computed in FixedPoint space (always > 0 since 0 < p < 1).
        // Then promoted to FixedI128 for fixed_ln, which can return a negative result.
        // FixedI128::fixed_ln is a CONCRETE call - no generic FixedOp bound needed.
        let logit = |p: FixedPoint| -> Option<FixedI128> {
            let denom = one_fp.saturating_sub(p);       // 1 - p  (> 0 since p < 1)
            let ratio = p.checked_div(&denom)?;          // p/(1-p) > 0
            let ratio_s: FixedI128 = FixedSignedCast::saturated_into(ratio);
            FixedI128::fixed_ln(&ratio_s)                // concrete, can be negative
        };

        let logit_alpha: FixedI128 = match logit(alpha) {
            Some(v) => v,
            None    => return Input::zero(),
        };
        let logit_beta: FixedI128 = match logit(beta) {
            Some(v) => v,
            None    => return Input::zero(),
        };

        // k = (logit(beta) - logit(alpha)) / w  - always > 0 when beta > alpha.
        let k_num: FixedI128 = logit_beta.saturating_sub(logit_alpha);
        let k: FixedI128 = match k_num.checked_div(&w_s) {
            Some(v) => v,
            None    => return Input::zero(),
        };
        if k == zero_s {
            return Input::zero();
        }

        // x0 = x_alpha - logit(alpha) / k
        // For alpha < 0.5: logit(alpha) < 0, so -logit(alpha)/k > 0, meaning x0 > x_alpha.
        let logit_alpha_over_k: FixedI128 = match logit_alpha.checked_div(&k) {
            Some(v) => v,
            None    => return Input::zero(),
        };
        let x0: FixedI128 = x_alpha_s.saturating_sub(logit_alpha_over_k);

        // f(x) = L / (1 + e^(-k * (x - x0)))
        // All computation in FixedI128 to handle negative exponent argument correctly.
        let x_s: FixedI128 = FixedSignedCast::saturated_into(Input::to_fixed(&input));
        let delta:       FixedI128 = x_s.saturating_sub(x0);
        let k_delta:     FixedI128 = k.saturating_mul(delta);
        // This negation requires signed arithmetic. In unsigned space this would clamp to 0.
        let neg_k_delta: FixedI128 = zero_s.saturating_sub(k_delta);

        // Concrete FixedI128::fixed_exp - no generic bound needed.
        let exp_val: FixedI128 = FixedI128::fixed_exp(&neg_k_delta)
            .unwrap_or(FixedI128::max_value());

        let denom: FixedI128 = FixedI128::one().saturating_add(exp_val);

        let l_s: FixedI128 = FixedSignedCast::saturated_into(l);
        let output_s: FixedI128 = l_s.checked_div(&denom).unwrap_or(zero_s);

        // Project back to FixedPoint (always >= 0 since L >= 0 and denom >= 1).
        let output_fp: FixedPoint = FixedSignedCast::saturated_from(output_s);
        Input::from_fixed(&output_fp)
    }
}

// ===============================================================================
// ```````````````````````````` EXPONENTIAL-INFLUENCE ````````````````````````````
// ===============================================================================

/// Configuration for Exponential Model
///
/// - `growth_rate`: Determines how steeply the value grows.
/// - A higher value leads to faster exponential increase.
pub struct ExponentialModelConfig<F>
where
    F: FixedPointNumber,
{
    pub growth_rate: F,
}

plugin_model! {

    /// Provides an exponential influence model with rapid growth.
    ///
    /// `f(x) = e^(k * x)`
    ///
    /// - `x`: input value (e.g., vote weight, reputation)
    /// - `k`: growth rate (positive for exponential growth)
    /// - `e`: Euler's number (~2.718)
    ///
    /// ## Characteristics
    /// - Growth rate is proportional to the current value.
    /// - Models **compound growth**, **population increase**, and **epidemics**.
    /// - Overflows are saturated to max_value.
    ///
    /// ## Applications
    /// - Incentive amplification systems
    /// - Growth modeling in economics and networks
    ///
    /// ## References
    /// - https://en.wikipedia.org/wiki/Exponential_growth
    name: pub ExponentialModel,
    input: Input,
    others: [FixedPoint],
    context: ExponentialModelConfig<FixedPoint>,
    bounds: [
        Input: IntegerToFixed + FixedForInteger<FixedPoint = FixedPoint> + PartialOrd,
        FixedPoint: FixedPointNumber + FixedOp,
    ],
    compute: |input, context| {
        // Convert the generic input to the context type (e.g., FixedU128)
        let x = input.to_fixed();
        // Apply growth rate: k * x
        let kx = context.growth_rate.saturating_mul(x);
        // Compute e^(k * x)
        let result = FixedOp::fixed_exp(&kx).unwrap_or(FixedPoint::max_value());
        Input::from_fixed(&result)
    }
}

// ===============================================================================
// ``````````````````````````````` BINARY-INFLUENCE ``````````````````````````````
// ===============================================================================

/// Binary model configuration
///
/// - `pass_threshold`: Minimum input required to be considered a pass
/// - `pass_value`: Output when input passes threshold
/// - `fail_value`: Output when input is below threshold
pub struct BinaryModelConfig<T> {
    pub pass_threshold: T,
    pub pass_value: T,
    pub fail_value: T,
}

plugin_model! {

    /// Provides a **binary influence model** that maps input to one of two fixed outputs.
    ///
    /// ```text
    /// f(x) = pass_value   if x >= threshold
    ///        fail_value   otherwise
    /// ```
    ///
    /// - `x`: input value (e.g., vote weight, signal score, approval rating)
    /// - `threshold`: the boundary that separates pass from fail
    /// - `pass_value`: output when input meets or exceeds the threshold
    /// - `fail_value`: output when input falls below the threshold
    ///
    /// ## Characteristics
    /// - All-or-nothing output; no partial or proportional influence.
    /// - The threshold is inclusive - `x == threshold` is a pass.
    ///
    /// ## Applications
    /// - Quorum checks in voting systems
    /// - On/off feature activation in governance
    /// - Eligibility gates in staking or reputation systems
    ///
    /// ## References:
    /// - [Binary decision rule](https://en.wikipedia.org/wiki/Decision_rule)
    /// - [Threshold logic](https://en.wikipedia.org/wiki/Threshold_logic)
    name: pub BinaryModel,
    input: Input,
    context: BinaryModelConfig<Input>,
    bounds: [
        Input: Copy + PartialOrd,
    ],
    compute: |input, context| {
        let outcome = input >= context.pass_threshold;
        match outcome  {
            true => context.pass_value,
            false => context.fail_value
        }
    }
}

// ===============================================================================
// ````````````````````````````` CAPPED-LINEAR-INFLUENCE `````````````````````````
// ===============================================================================

/// Configuration for the `CappedLinearModel`
///
/// - `max_influence`: the upper bound of the influence, no matter how large the input is
pub struct CappedLinearModelConfig<T> {
    pub max_influence: T,
}

plugin_model! {

    /// Provides a capped linear influence model with an upper bound.
    ///
    /// `f(x) = min(x, max_influence)`
    ///
    /// - `x`: input value (e.g., stake, score)
    /// - `max_influence`: maximum allowed influence
    ///
    /// ## Characteristics
    /// - Grows linearly until reaching a fixed cap.
    /// - Prevents outliers or large inputs from dominating.
    ///
    /// ## Applications
    /// - Capped voting power
    /// - Anti-sybil systems
    /// - Influence throttling in distributed systems
    ///
    /// ## References
    /// - https://en.wikipedia.org/wiki/Quadratic_voting
    /// - https://en.wikipedia.org/wiki/Reputation_system
    name: pub CappedLinearModel,
    input: Input,
    context: CappedLinearModelConfig<Input>,
    bounds: [
        Input: Copy + PartialOrd,
    ],
    compute: |input, context| {
        let result = input > context.max_influence;
        match result {
            true => context.max_influence,
            false => input
        }
    }
}