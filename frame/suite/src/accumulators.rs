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
// `````````````````````````````` ACCUMULATORS SUITE `````````````````````````````
// ===============================================================================

//! Defines a generic interface for step-based progression systems.
//!
//! This abstraction models systems where a value is derived from
//! internal state that is updated through discrete steps.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

use crate::base::{Countable, Delimited};

// ===============================================================================
// ````````````````````````````` DISCRETE ACCUMULATOR ````````````````````````````
// ===============================================================================

/// A trait for discrete accumulation with configurable step rules.
///
/// This trait models systems where progress happens in small, discrete steps,
/// and those steps gradually accumulate into a larger meaningful value.
///
/// Instead of directly changing the final value every time, progress is first
/// stored internally and only converted into a visible result when certain
/// conditions (such as thresholds) are met.
///
/// ## Intuition
///
/// Think of this as a two-layer progression system:
///
/// 1. Small repeated actions add *internal progress*
/// 2. Once enough progress is collected, the visible value increases
///
/// The internal progress is hidden, and `reveal` exposes only the final result.
///
/// ## Generic Roles of the Associated Types
///
/// * `Value`
///   The final meaningful result exposed to consumers.
///   This is what users care about (e.g., level, score, reputation).
///
/// * `Step`
///   Represents one discrete unit of progress applied during each operation.
///   Each increment or decrement applies one such step.
///
/// * `Accumulator`
///   Internal state that tracks progress and any additional data needed
///   to determine how the final value evolves over time.
///
/// * `Stepper`
///   Defines the rules for how steps affect the accumulator.
///   This may include configuration such as thresholds, scaling factors,
///   or other logic governing accumulation behavior.
///
/// ## How Accumulation Works (Conceptually)
///
/// Forward progression:
///
/// ```text
/// progress += step
/// if progress reaches some condition (e.g., threshold):
///     value increases
///     progress is adjusted/reset accordingly
/// ```
///
/// Reverse progression:
///
/// ```text
/// progress -= step
/// if progress would go below zero:
///     value decreases
///     progress is restored based on the rules
/// ```
///
/// The exact logic is fully defined by the implementation.
///
/// ## Example Scenario (Conceptual)
///
/// Imagine a system where:
/// - Each action adds a fixed amount of progress
/// - A visible value increases only after enough progress is accumulated
///
/// Progress might evolve like this:
///
/// ```text
/// Start: value = 0, internal progress = 0
/// Step 1 -> internal progress increases
/// Step 2 -> internal progress increases
/// Step 3 -> internal progress reaches condition -> value becomes 1
/// ```
///
/// Decrementing would reverse this process, potentially reducing the value
/// and restoring some internal progress.
///
/// ## Design Flexibility
///
/// Implementors are free to define:
/// - How internal progress is stored
/// - What condition converts progress into value changes
/// - How increments and decrements interact with that state
/// - Any custom or domain-specific accumulation logic
///
/// This makes the trait suitable for a wide range of stepped progression systems
/// such as reward meters, scoring engines, leveling mechanics, or quota trackers.
pub trait DiscreteAccumulator {
    /// The final value type that represents the accumulated result.
    ///
    /// This is the user-facing result derived from the internal accumulator state.
    /// Implementations decide how internal progress maps to this value.
    type Value: Countable;

    /// The discrete unit of progress applied during accumulation operations.
    ///
    /// Each increment or decrement uses this unit to modify internal state.
    type Step: Countable;

    /// The internal state used to track accumulation progress.
    ///
    /// This may contain any data required to determine how the final value evolves,
    /// including partial progress toward future value changes.
    type Accumulator: Delimited;

    /// Configuration describing how steps affect the accumulator.
    ///
    /// This governs the rules of accumulation, such as when progress should
    /// convert into value changes or how reverse progression behaves.
    type Stepper: Delimited;

    /// Applies forward progression to the accumulator.
    ///
    /// This operation increases internal progress according to the rules defined
    /// by the `Stepper`. Depending on the implementation, this may cause the
    /// revealed value to increase once certain conditions are met.
    fn increment(accum: &mut Self::Accumulator, stepper: &Self::Stepper);

    /// Applies reverse progression to the accumulator.
    ///
    /// This operation removes internal progress. If reversing progress crosses
    /// important boundaries (such as previously completed thresholds), the
    /// revealed value may decrease and internal progress may be restored
    /// accordingly.
    fn decrement(accum: &mut Self::Accumulator, stepper: &Self::Stepper);

    /// Reveals the current accumulated value derived from the internal state.
    ///
    /// This provides read-only access to the meaningful result while hiding
    /// the internal progress details used to compute it.
    fn reveal(accum: &Self::Accumulator) -> Self::Value;
}
