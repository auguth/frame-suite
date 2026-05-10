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
// ``````````````````````````````` PENALTY PLUGINS ```````````````````````````````
// ===============================================================================

//! Provides a suite of **pluggable penalty models** used to transform,
//! normalize, and constrain penalty values associated with entities.
//!
//! These models are designed to operate as **post-processing layers**
//! in governance, reputation, and scoring systems, ensuring that penalties
//! remain **bounded, fair, and resistant to extreme values**.
//!
//! ## Design Philosophy
//!
//! Raw penalty values (e.g., slashing amounts, negative scores, or risk weights)
//! can often be:
//! - **Unbounded**: leading to disproportionate punishment
//! - **Noisy**: zero or insignificant values
//! - **Inconsistent**: varying widely across participants
//!
//! Penalty models provide a structured way to:
//! - **Cap excessive penalties** to prevent extreme outcomes
//! - **Enforce minimum penalties** to avoid negligible punishments
//! - **Normalize distributions** for fair comparison across entities
//! - **Filter irrelevant values** (e.g., zero penalties)
//!
//! By transforming raw penalties into **controlled and comparable values**,
//! these plugins ensure stable and predictable behavior in downstream systems.
//!
//! ## Core Concepts
//!
//! - **Input (`p`)**: A penalty value associated with an entity.
//! - **Output (`f(p)`)**: The transformed penalty after applying constraints.
//! - **Penalty List**: A collection of `(Id, Penalty)` pairs.
//! - **Context**: Optional configuration defining bounds (e.g., threshold, floor, cap).
//!
//! ## Applications
//!
//! - **Governance systems**: Slashing, penalties, or reputation decay
//! - **Reputation engines**: Penalizing malicious or low-quality behavior
//! - **Scoring systems**: Normalizing negative contributions
//! - **Risk models**: Bounding downside exposure

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Core / Std ---
use core::iter::once;

// --- FRAME Suite ---
use frame_suite::plugin_model;

// --- Substrate primitives ---
use sp_runtime::traits::Zero;

// ===============================================================================
// `````````````````````````````` THRESHOLD-PENALTY ``````````````````````````````
// ===============================================================================

/// Configuration for [`ThresholdPenalty`] plugin.
///
/// Defines the **maximum allowable penalty** per entity.
///
/// - `threshold`: Upper bound for any individual penalty value.
///   - Penalties above this value are **clamped down** to the threshold.
///   - Useful for preventing excessive punishment or outliers.
///
/// ## Example
/// ```ignore
/// let config = ThresholdPenaltyConfig { threshold: 50 };
/// ```
pub struct ThresholdPenaltyConfig<T> {
    pub threshold: T,
}

plugin_model!(
    /// Applies an **upper threshold cap** to penalties.
    ///
    /// **Concept**: **Penalty Clamping (Upper Bound)**
    ///
    /// Each `(Id, Penalty)` pair is processed such that:
    ///
    /// ```text
    /// f(p) = min(p, threshold)
    /// ```
    ///
    /// - Zero penalties are ignored.
    /// - Values above the threshold are reduced to the threshold.
    ///
    /// ## Characteristics:
    /// - **Upper-bounded**: Prevents penalties from exceeding a maximum.
    /// - **Noise filtering**: Removes zero penalties entirely.
    /// - **Deterministic**: Stateless and predictable transformation.
    ///
    /// ## Applications:
    /// - Limiting punishment severity in governance systems
    /// - Anti-abuse mechanisms (prevent extreme slashing)
    /// - Normalizing penalty distributions
    ///
    /// ## Example:
    /// ```ignore
    /// input = [(A, 10), (B, 80)]
    /// threshold = 50
    /// output = [(A, 10), (B, 50)]
    /// ```
    name: pub ThresholdPenalty,
    input: PenaltyList,
    others: [Id, Penalty],
    context: ThresholdPenaltyConfig<Penalty>,
    bounds: [
        PenaltyList: IntoIterator<Item = (Id, Penalty)>
            + FromIterator<(Id, Penalty)>
            + Extend<(Id, Penalty)>
            + Default
            + Clone,
        Penalty: PartialOrd + Copy + Zero,
    ],
    compute: |input, context| {
        let mut result = PenaltyList::default();
        // 1. Iterate through all penalties
        for (id, penalty) in input.clone().into_iter() {
            // 2. Skip zero penalties
            if penalty.is_zero() {
                continue;
            }
            // 3. Clamp penalty to threshold
            let actual = penalty > context.threshold;
            let new_penalty = match actual {
                true => context.threshold,
                false => penalty
            };
            // 4. Insert adjusted value
            result.extend(once((id, new_penalty)));
        }
        result
    }
);

// ===============================================================================
// ```````````````````````````````` CAPPED-PENALTY ```````````````````````````````
// ===============================================================================

/// Configuration for [`CappedPenalty`] plugin.
///
/// Defines both **minimum and maximum bounds** for penalties.
///
/// - `floor`: Minimum penalty value (lower bound)
/// - `cap`: Maximum penalty value (upper bound)
///
/// ## Example
/// ```ignore
/// let config = CappedPenaltyConfig { floor: 10, cap: 50 };
/// ```
pub struct CappedPenaltyConfig<T> {
    pub floor: T,
    pub cap: T,
}

plugin_model!(
    /// Applies a **bounded range constraint** to penalties.
    ///
    /// **Concept**: **Range Clamping (Floor + Cap)**
    ///
    /// Each penalty is transformed as:
    ///
    /// ```text
    /// f(p) = max(floor, min(p, cap))
    /// ```
    ///
    /// - Ensures penalties stay within a defined range.
    /// - Zero penalties are ignored.
    ///
    /// ## Characteristics:
    /// - **Bi-directional bounds**: Enforces both minimum and maximum limits.
    /// - **Stabilizing**: Prevents extremely low or high penalties.
    /// - **Deterministic**: Stateless transformation.
    ///
    /// ## Applications:
    /// - Enforcing minimum punishment levels
    /// - Preventing extreme slashing or negligible penalties
    /// - Maintaining consistent penalty distributions
    ///
    /// ## Example:
    /// ```ignore
    /// input = [(A, 5), (B, 100)]
    /// floor = 10, cap = 50
    /// output = [(A, 10), (B, 50)]
    /// ```
    name: pub CappedPenalty,
    input: PenaltyList,
    others: [Id, Penalty],
    context: CappedPenaltyConfig<Penalty>,
    bounds: [
        PenaltyList: IntoIterator<Item = (Id, Penalty)>
            + FromIterator<(Id, Penalty)>
            + Extend<(Id, Penalty)>
            + Default
            + Clone,
        Penalty: PartialOrd + Copy + Zero,
    ],
    compute: |input, context| {
        let mut result = PenaltyList::default();
        // 1. Iterate through all penalties
        for (id, penalty) in input.clone().into_iter() {
            // 2. Skip zero penalties
            if penalty.is_zero() {
                continue;
            }
            // 3. Apply range clamp: floor <= p <= cap
            let adjusted_penalty = if penalty > context.cap {
                context.cap
            } else if penalty < context.floor {
                context.floor
            } else {
                penalty
            };
            // 4. Insert adjusted value
            result.extend(once((id, adjusted_penalty)));
        }
        result
    }
);