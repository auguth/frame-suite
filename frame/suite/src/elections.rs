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
// ``````````````````````````````` ELECTIONS SUITE ```````````````````````````````
// ===============================================================================

//! Provides generic traits for **election management**, **weight computation**,
//! and **influence calculation** using [`plugin`](crate::plugins)-based models.
//!
//! The traits include:
//! - [`ElectionManager`]: Core election management for candidates.
//! - [`InspectWeight`]: Provides candidate weight lookup.
//! - [`Influence`]: Computes influence values from raw input using plugin models.
//!
//! ## Note
//!
//! Elections are **pluggable by design**, meaning:
//! - The **inputs** generally consist of candidates along with their backing
//! weights (or votes).  
//! - The **outputs** can be a single candidate or a collection of candidates
//! (stored via an iterator).  
//!
//! Because of this, we define **generic plugin-based traits**, enabling:
//! 1. Multiple election models to be implemented over the same trait interfaces.  
//! 2. Flexible storage and computation strategies depending on the election logic.  
//! 3. Strong type-safety while maintaining runtime configurability via plugins.  

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{
    base::{Buffer, Delimited, Elastic, Keyed, RuntimeType, Sortable, Storable},
    plugin_output, plugin_types,
};

// --- Substrate primitives ---
use sp_runtime::{DispatchError, DispatchResult};

// ===============================================================================
// `````````````````````````````` ELECTION MANAGER ```````````````````````````````
// ===============================================================================

/// A trait for **managing elections** of candidates with associated weights.
///
/// This trait is designed to be **highly generic** and
/// [`plugin-driven`](crate::plugins), enabling multiple election models
/// to be used without modifying the trait itself.
///
/// Elections are inherently **pluggable and diverse**:
/// - Different elections may use distinct **weighting rules** or
///   **selection algorithms**.
/// - Inputs consist of candidates paired with their backing weights.
/// - Outputs may represent either a single winner or multiple winners.
/// - A plugin-based design allows flexible implementations while
///   preserving type safety and runtime configurability.
///
/// ## Type Parameters
/// - `Candidate`: The type representing a candidate in the election.
pub trait ElectionManager<Candidate>: InspectWeight<Candidate, Self::ElectionWeightOf>
where
    Candidate: Keyed,
{
    /// Represents a single vote or its associated weight.
    ///
    /// This can either:
    /// - Store a numeric weight for a vote, or
    /// - Represent a single vote implicitly.
    ///
    /// Must implement [`Ord`] to allow comparison and sorting within
    /// [`Self::ElectionWeightOf`].
    type ElectionWeight: Sortable;

    /// Collection type for storing candidate weights.
    ///
    /// This allows flexibility in the underlying container, such as:
    /// - `Vec`
    /// - Arrays
    /// - Custom buffer types
    ///
    /// Must support:
    /// - Iteration via [`Buffer`]
    /// - Ordering via [`Ord`]
    /// - Storage via [`Storable`]
    type ElectionWeightOf: Buffer<Self::ElectionWeight> + Ord + Storable;

    /// Input type for election computation.
    ///
    /// Each entry maps a candidate to a collection of associated weights.
    ///
    /// This abstraction ensures that plugin implementations only accept
    /// well-structured and compatible input formats.
    type Params: Buffer<(Candidate, Self::ElectionWeightOf)>;

    /// Output type representing elected candidates.
    ///
    /// This can represent:
    /// - Multiple winners (e.g., committee selection), or
    /// - A single winner (as a single-element collection)
    ///
    /// If the output implies ranking, the elements are expected to be
    /// ordered by priority.
    ///
    /// If truncation occurs (e.g., selecting top-N winners), the ordering
    /// must reflect the final priority of elected candidates.
    type Elected: Buffer<Candidate>;

    plugin_types!(
        input: Self::Params,
        output: Self::Elected,
        /// The plugin responsible for computing election results.
        ///
        /// This model defines the election strategy by:
        /// - Consuming [`Self::Params`] as input, and
        /// - Producing [`Self::Elected`] as output.
        ///
        /// This abstraction allows multiple election strategies to be
        /// plugged in safely and interchangeably.
        ///
        /// If the resulting [`Self::Elected`] is truncated (e.g., selecting top-N),
        /// the candidates must be ordered by priority.
        model: ElectionModel,

        /// Provides runtime configuration for the [`Self::ElectionModel`] computation.
        ///
        /// This context supplies dynamic parameters such as:
        /// - Thresholds
        /// - Weights
        /// - Other runtime-specific settings
        ///
        /// It enables flexible behavior without requiring hardcoded values.
        context: ElectionContext,
    );

    /// Executes the election process and persists the results.
    ///
    /// This function:
    /// 1. Runs the election model using [`Self::run_model`]
    /// 2. Stores the resulting elected candidates via [`Self::store`]
    fn prepare(from: Self::Params) -> DispatchResult {
        // Computes the plugin output
        let out = &Self::run_model(from);
        // stores the output of elected candidates
        if let Err(e) = Self::store(out) {
            Self::on_prepare_fail(e);
            return Ok(());
        };
        Self::on_prepare_success(out);
        Ok(())
    }

    plugin_output! {
        /// [`Self::ElectionModel`] plugin output function.
        ///
        /// Utilizes the plugin model's context [`Self::ElectionContext`]
        fn run_model,
        input: Self::Params,
        output: Self::Elected,
        model: Self::ElectionModel,
        context: Self::ElectionContext
    }

    /// Persist the election results. Must be implemented by the consumer.
    fn store(_elects: &Self::Elected) -> DispatchResult;

    /// Retrieve currently elected candidates.
    fn reveal() -> Option<Self::Elected>;

    /// Remove a candidate from the elected pool.
    fn remove(who: &Candidate);

    /// Check if a candidate exist in the elected pool.
    fn is_candidate(who: &Candidate) -> DispatchResult;

    /// Check if election preparation is possible with the given parameters.
    fn can_prepare(from: &Self::Params) -> DispatchResult;

    /// Hook called after a successful election preparation. Default is no-op.
    fn on_prepare_success(_elects: &Self::Elected) {}

    /// Hook called after an election preparation failure. Default is no-op.
    fn on_prepare_fail(_err: DispatchError) {}
}

// ===============================================================================
// ``````````````````````````````` INSPECT WEIGHT ````````````````````````````````
// ===============================================================================

/// Trait for inspecting the **weight of a candidate** for an upcoming
/// election.
///
/// - Different election models may compute or store weights differently.
/// - Providing a trait allows generic election managers or plugins
///   to query a candidate's weight without knowing the underlying structure.
///
/// ## Type Parameters
/// - `Candidate`: The type representing a candidate.
/// - `Weight`: The type representing the candidate's vote weight.
pub trait InspectWeight<Candidate, Weight>
where
    Candidate: Keyed,
    Weight: RuntimeType,
{
    /// Return the weight of a candidate if available.
    ///
    /// Returns an error if the candidate has no associated weight.
    fn weight_of(who: &Candidate) -> Result<Weight, DispatchError>;
}

// ===============================================================================
// `````````````````````````````````` INFLUENCE ``````````````````````````````````
// ===============================================================================

/// A trait for computing **influence**, a normalized and comparable metric
/// representing the relative power or importance of an entity.
///
/// Influence is intended to capture **non-transferable system weight**,
/// derived from various inputs such as votes, stake, participation,
/// or other domain-specific factors.
///
/// ## Key Properties
///
/// - **Non-transferable**:
///   Influence is a derived metric and must not be directly traded or transferred.
/// - **Model-dependent**:
///   Different systems may define influence differently based on their
///   weighting rules or algorithms.
/// - **Deterministic**:
///   Given the same input and context, the computed influence should be consistent.
/// - **Comparable**:
///   Influence values must be bounded and comparable across entities.
///
/// ## Design
///
/// This trait follows a [`plugin`](crate::plugins)-based architecture:
/// - The computation logic is delegated to pluggable models.
/// - Different influence strategies can coexist without changing the trait.
/// - Runtime configuration is supported via context.
///
/// ## Type Parameters
/// - `RawFrom`: The raw input type used to derive influence.
///
///   This allows flexibility in supporting various input formats, such as:
///   - Numeric values (e.g., stake or balance)
///   - Account identifiers
///   - Structured or aggregated data
pub trait Influence<RawFrom>
where
    RawFrom: Elastic,
{
    /// Type representing the computed influence.
    type Influence: Delimited;

    plugin_types!(
        input: RawFrom,
        output: Self::Influence,
        /// The plugin responsible for computing influence.
        ///
        /// This model defines how raw input is transformed into
        /// a bounded influence value.
        ///
        /// Different models may implement:
        /// - Linear scaling
        /// - Weighted aggregation
        /// - Non-linear transformations (e.g., logarithmic influence)
        model: InfluenceModel,

        /// Provides runtime configuration for [`Self::InfluenceModel`].
        ///
        /// This context supplies dynamic parameters such as:
        /// - Scaling factors
        /// - Thresholds
        /// - External weights or modifiers
        ///
        /// This enables flexible and adaptive influence computation
        /// without hardcoding logic.
        context: InfluenceContext,
    );

    plugin_output! {
        /// Computes influence from a raw input using the configured plugin.
        ///
        /// This function:
        /// - Delegates computation to [`Self::InfluenceModel`]
        /// - Uses [`Self::InfluenceContext`] for parameterization
        ///
        /// ## Returns
        /// A bounded influence value derived from the given input.
        fn influence,
        input: RawFrom,
        output: Self::Influence,
        model: Self::InfluenceModel,
        context: Self::InfluenceContext
    }
}
