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
// ``````````````````````````````` ELECTION PLUGINS ``````````````````````````````
// ===============================================================================

//! Defines two distinct **pluggable election models** for ranking and selecting
//! candidates based on their stake, backing, or influence metrics.
//!
//! Elections are abstracted into two main models:
//!
//! ## Flat Election (`flat`)
//!
//! - Aggregates all contributions (including the candidates's own stake-if any)
//! into a single scalar metric.
//! - Candidates are compared using their **flattened total weight**.
//! - Useful for scenarios where **every unit of support counts equally**
//!   and simple proportionality is desired.
//!
//! ## Fair Election (`fair`)
//!
//! - Each backer's contribution is **kept unaggregated** (including the
//! candidate's own stake-if any as one of the backing), preserving individual
//! influence granularity.
//! - Useful when the goal is to **prevent candidates from dominating through
//! self-funding** and emphasize external support.
//!
//! ## Purpose
//!
//! Separating election models into `flat` and `fair` provides flexibility:
//! - **Flat** for simple, proportional elections where total stake matters.
//! - **Fair** for more security-conscious or governance-focused elections
//! emphasizing community support.
//!
//! Both models implement a **pluggable algorithm interface**, enabling runtime
//! substitution, testing of different strategies, and easy extension with new
//! election rules.

// ===============================================================================
// ```````````````````````````` FLAT-ELECTION PLUGINS ````````````````````````````
// ===============================================================================

pub use flat::*;

/// Contains **FlatElection plugin models**, which rank entities using a
/// **single aggregated scalar (flat weight)** computed from a list of entities and
/// their corresponding weights.
///
/// In this model:
/// - Each entity is represented as a `(entity, flat_weight)` pair.
/// - Individual contributions are **not tracked separately**; only the total flat
/// weight per entity matters.
/// - The plugin takes the list of entities with their flat weights and **produces
/// a ranked output**.
///
/// ## Characteristics
/// - Requires a **list of entities with their flat weights** as input.
/// - Produces a **sorted list of entities** according to their flat weight.
/// - Ignores the structure or distribution of contributions, focusing purely on the
/// aggregated value.
///
/// ## Example Flow
/// 1. Prepare a list of entities with their flat weights:
///     `[(entity1, w1), (entity2, w2), ...]`.  
/// 2. Pass this list to a FlatElection plugin model.  
/// 3. The model sorts and outputs entities in **descending order of flat weight**.
pub mod flat {

    // ===============================================================================
    // ``````````````````````````````````` IMPORTS ```````````````````````````````````
    // ===============================================================================

    // --- FRAME Suite ---
    use frame_suite::plugin_model;

    // --- Substrate primitives ---
    use sp_runtime::Vec;

    // ===============================================================================
    // ```````````````````````````` TOP-DOWN FLAT-ELECTION ```````````````````````````
    // ===============================================================================

    plugin_model! {
        /// The **TopDownFlat** model ranks candidates based solely on their **aggregated
        /// flat weight**. Each candidate's score represents the total support or stake
        /// they possess.
        ///
        /// **Concept**: **Total-Weight Dominance**
        ///
        /// The resulting order is strictly determined by these scalar totals - higher
        /// weights dominate, ensuring proportional fairness where every unit of influence
        /// counts equally.
        ///
        /// ## Characteristics:
        /// - **Simple aggregation**: Treats all contributions as additive and equivalent.
        /// - **Self-inclusive**: Candidate's own weight contributes to their score.
        /// - **Deterministic ordering**: Sorted descending by total flat weight.
        /// - **Context-free**: Requires no external context or normalization.
        ///
        /// ## Applications:
        /// - **Stake-based elections**
        /// - **Token-weighted voting**
        /// - **Simple ranking systems** where total influence determines outcome
        ///
        /// ## Example:
        /// ```ignore
        /// let input = vec![
        ///     ("Alice", 90),
        ///     ("Bob", 50),
        ///     ("Charlie", 80),
        /// ];
        /// let output = TopDownFlat::compute(input, None);
        /// assert_eq!(output, vec!["Alice", "Charlie", "Bob"]);
        /// ```
        ///
        /// ## References
        /// - [Sorting - Wikipedia](https://en.wikipedia.org/wiki/Sorting)
        name: pub TopDownFlatModel,
        input: Input,
        output: Output,
        others: [Candidate, FlatWeight],
        bounds: [
            Input: FromIterator<(Candidate, FlatWeight)>
                + IntoIterator<Item = (Candidate, FlatWeight)> + Clone,
            Output: FromIterator<Candidate>,
            FlatWeight: Ord + Clone,
            Candidate: Clone,
        ],
        /// Computes the **descending rank** of candidates by total flat weight.
        ///
        /// 1. Collects all `(Candidate, FlatWeight)` pairs.
        /// 2. Sorts them in **descending order** of their weight.
        /// 3. Returns the ordered list of candidates.
        compute: |input, _context| {
            // Step 1: Collect input into a mutable vector
            let mut items: Vec<(Candidate, FlatWeight)> = Vec::new();
            for pair in input.clone() {
                items.push(pair);
            }

            // Step 2: Sort items in descending order by FlatWeight
            items.sort_by(|a, b| b.1.cmp(&a.1));

            // Step 3: Extract only the candidates in order
            let mut output: Vec<Candidate> = Vec::new();
            for (candidate, _) in items {
                output.push(candidate);
            }

            // Step 4: Return candidates as iterator
            output.into_iter().collect()
        }
    }

    // ===============================================================================
    // ``````````````````````````` THRESHOLD FLAT-ELECTION ```````````````````````````
    // ===============================================================================

    /// Context Config for Plugin Model [`ThresholdFlatModel`]
    ///
    /// This struct provides the configuration for threshold-based elections, where
    /// only candidates whose weight meets or exceeds a specified threshold are
    /// considered.
    pub struct ThresholdFlatModelConfig<T> {
        /// The minimum weight required for a candidate to be included in the output.
        ///
        /// - Candidates with a weight **greater than or equal to** this value will be
        /// retained.
        /// - Candidates with a weight **below** this value will be filtered out.
        ///
        /// ## Example
        /// ```ignore
        /// let config = ThresholdModelConfig { threshold: 50 };
        /// // Only candidates with weight >= 50 will be returned by the model
        /// ```
        pub threshold: T,
    }

    plugin_model! {
        /// The **ThresholdFlat** model filters candidates based on a **minimum flat
        /// weight threshold**. Only candidates whose weight meets or exceeds the threshold
        /// are included in the output.
        ///
        /// **Concept**: **Minimum Weight Filtering**
        ///
        /// This is useful in elections or ranking systems where very low-contributing
        /// candidates should be excluded from consideration.
        ///
        /// ## Characteristics:
        /// - **Threshold-based**: Filters out candidates below a specified weight.
        /// - **Self-inclusive**: Candidate's own weight counts toward the threshold.
        /// - **Deterministic ordering**: Relative ordering of remaining candidates is
        /// preserved from the input (or can be further sorted in downstream models).
        /// - **Context-driven**: Uses an external [`ThresholdModelConfig`] struct to
        /// provide the threshold.
        ///
        /// ## Applications:
        /// - Removing candidates with insufficient stake or support.
        /// - Pre-filtering before proportional or ranked elections.
        /// - Reward distribution where a minimum contribution is required.
        ///
        /// ## Example:
        /// ```ignore
        /// let input = vec![
        ///     ("Alice", 90),
        ///     ("Bob", 50),
        ///     ("Charlie", 30),
        /// ];
        /// let config = ThresholdModelConfig { threshold: 50 };
        /// let output = ThresholdModel::compute(input, Some(config));
        /// assert_eq!(output, vec!["Alice", "Bob"]);
        /// ```
        ///
        /// ## References:
        /// - [Thresholding and Filtering - Wikipedia](https://en.wikipedia.org/wiki/Thresholding)
        /// - Standard minimum weight / eligibility filtering in proportional systems.
        name: pub ThresholdFlatModel,
        input: Input,
        output: Output,
        others: [Candidate, FlatWeight],
        context: ThresholdFlatModelConfig<FlatWeight>,  // external struct
        bounds: [
            Input: IntoIterator<Item = (Candidate, FlatWeight)> + Clone,
            Output: FromIterator<Candidate>,
            FlatWeight: Ord + Clone,
            Candidate: Clone,
        ],
        /// Filters candidates below a configurable flat weight threshold.
        ///
        /// - Iterates over all `(Candidate, FlatWeight)` pairs.
        /// - Keeps only candidates where weight >= `ctx.threshold`.
        /// - Returns an iterator over the remaining candidates.
        compute: |input, ctx| {
            // Step 1: Collect input into a mutable vector (optional, ensures
            // we can iterate multiple times if needed)
            let items: Vec<(Candidate, FlatWeight)> = input.clone().into_iter().collect();

            // Step 2: Prepare an output vector to store candidates that meet the threshold
            let mut output: Vec<Candidate> = Vec::new();

            // Step 3: Iterate over each candidate and weight
            for (candidate, weight) in items {
                // Step 4: Check if the weight meets or exceeds the threshold
                if weight >= ctx.threshold.clone() {
                    // Step 5: If yes, add the candidate to the output
                    output.push(candidate);
                }
            }

            // Step 6: Return the output as an iterator (FromIterator)
            output.into_iter().collect()
        }
    }
}

// ===============================================================================
// ```````````````````````````` FAIR-ELECTION PLUGINS ````````````````````````````
// ===============================================================================

pub use fair::*;

/// Contains **FairElection plugin models**, which rank entities using a
/// **fair weight**, where each entity's weight is derived from a **list of
/// contributors** and their individual contributions.
///
/// In this model:
/// - Each entity is represented as `(entity, fair_weight)`.
/// - The `fair_weight` itself is a **collection of `(contributor, contribution)`
/// pairs**.
/// - The plugin aggregates or evaluates these individual contributions according
/// to its algorithm to produce a ranked output.
///
/// ## Characteristics
/// - Requires a **list of entities with their fair weights** as input.
/// - Each fair weight preserves the **granularity of individual contributions**,
/// unlike flat weights.
/// - Produces a **sorted list of entities**, taking into account the distribution and
/// magnitude of contributions.
/// - Ideal for scenarios where **fairness or proportional representation** matters,
/// rather than just total aggregated value.
///
/// ## Example Flow
/// 1. Prepare a list of entities with their fair weights:
///     `[(entity1, [(c1, v1), (c2, v2)]), ...]`.  
/// 2. Pass this list to a FairElection plugin model.  
/// 3. The model computes a ranking based on the **individual contributions within
/// each fair weight** and outputs entities in order.
pub mod fair {

    // ===============================================================================
    // ````````````````````````````````` IMPORTS `````````````````````````````````````
    // ===============================================================================

    // --- FRAME Suite ---
    use frame_suite::plugin_model;

    // --- Substrate primitives ---
    use sp_runtime::{
        traits::{Saturating, Zero},
        Vec,
    };

    // --- Substrate std (no_std helpers) ---
    use sp_std::ops::{Add, Div, Mul};

    // ===============================================================================
    // ``````````````````````````` TOP-DOWN FAIR-ELECTION ````````````````````````````
    // ===============================================================================

    plugin_model! {
        /// The **TopDownFair** model evaluates candidates based on the **aggregate
        /// support** they receive from **external backers**.
        ///
        /// **Concept**: **Aggregated External Fairness**
        ///
        /// Each candidate has a list of `(Backer, Backed)` pairs representing
        /// individual endorsements. The model sums these contributions using
        /// **saturating addition** to prevent overflow and sorts candidates
        /// in **descending order** by their total received backing.
        ///
        /// ## Characteristics:
        /// - **Fair aggregation**: Each candidate's score is the sum of their
        /// backers' support.
        /// - **No self-weighting**: Excludes self-stake to prevent dominance
        /// by self-funding.
        /// - **Overflow-safe**: Uses `Saturating` arithmetic to avoid overflow
        /// errors.
        /// - **Context-free**: Deterministic and stateless ranking.
        ///
        /// ## Applications:
        /// - **Governance elections** prioritizing external community support
        /// - **Fair staking models** with anti-self-dealing constraints
        /// - **Collaborative credit or delegation-based systems**
        ///
        /// ## Example:
        /// ```ignore
        /// let input = vec![
        ///     ("Alice", vec![("Bob", 10), ("Carol", 20)]),
        ///     ("Dave", vec![("Eve", 15), ("Frank", 15)]),
        ///     ("Grace", vec![("Heidi", 5), ("Ivan", 10)]),
        /// ];
        /// let output = TopDownFair::compute(input, None);
        /// // Aggregated totals:
        /// // Alice: 30, Dave: 30, Grace: 15
        /// assert_eq!(output, vec!["Alice", "Dave", "Grace"]);
        /// ```
        ///
        /// ## Reference:
        /// - [Weighted voting - Wikipedia](https://en.wikipedia.org/wiki/Weighted_voting)
        /// - [Delegated voting / Liquid democracy](https://en.wikipedia.org/wiki/Liquid_democracy)
        /// - Osborne, M. J., *An Introduction to Game Theory*, Oxford University Press, 2004 (proportional influence)
        name: pub TopDownFairModel,
        input: Input,
        output: Output,
        others: [Candidate, FairWeight, Backer, Backed],
        bounds: [
            Input: FromIterator<(Candidate, FairWeight)>
                + IntoIterator<Item = (Candidate, FairWeight)> + Clone,
            Output: FromIterator<Candidate>,
            FairWeight: FromIterator<(Backer, Backed)>
                + IntoIterator<Item = (Backer, Backed)> + Clone,
            Backed: Clone + Ord + Zero + Saturating,
            Candidate: Clone,
        ],
        /// Computes the **fair aggregated rank** of candidates imperatively.
        ///
        /// - Iterates over each candidate and sums their backers' contributions
        /// using saturating addition.
        /// - Sorts candidates by descending total backing.
        /// - Returns only the ordered list of candidates.
        compute: |input, _context| {
            // Step 1: Collect input into a vector for processing
            let items_input: Vec<(Candidate, FairWeight)> = input.clone().into_iter().collect();

            // Step 2: Prepare vector to store total backing for each candidate
            let mut items: Vec<(Candidate, Backed)> = Vec::new();

            // Step 3: Sum external backing for each candidate
            for (candidate, fairweight) in items_input {
                let mut total = Backed::zero();
                for (_, backed) in fairweight {
                    total = total.saturating_add(backed);
                }
                items.push((candidate, total));
            }

            // Step 4: Sort candidates by descending total backing
            items.sort_by(|a, b| b.1.cmp(&a.1));

            // Step 5: Collect only the candidates in order
            let mut output: Vec<Candidate> = Vec::new();
            for (candidate, _) in items {
                output.push(candidate);
            }

            output.into_iter().collect()
        }
    }

    // ===============================================================================
    // ``````````````````````````` BALANCED FAIR-ELECTION ````````````````````````````
    // ===============================================================================

    plugin_model! {
        /// The **BalancedFair** model evaluates candidates by combining both the
        /// **total backing** and the **average backing per backer**. This approach
        /// balances candidates who have a few very large backers versus those with
        /// many smaller backers.
        ///
        /// **Concept**: **Total + Average External Fairness**
        ///
        /// Each candidate has a list of `(Backer, Backed)` pairs.
        /// The model computes:
        /// 1. `total = sum of all Backed values`
        /// 2. `average = total / number of backers`
        /// 3. `score = total + average`
        ///
        /// Candidates are then sorted in descending order by this combined score.
        ///
        /// ## Characteristics:
        /// - **Balanced aggregation**: Rewards both high total support and broad
        ///   distribution.
        /// - **No self-weighting**: Excludes candidate self-contributions.
        /// - **Overflow-safe**: Uses saturating arithmetic.
        /// - **Deterministic ordering**: Sorted descending by combined score.
        /// - **Context-free**: Stateless and reproducible ranking.
        ///
        /// ## Applications:
        /// - Governance systems balancing "big backers" vs "broad support".
        /// - Collaborative projects or token-based delegation systems.
        /// - Reward allocation where both total and average contributions matter.
        ///
        /// ## References:
        /// - [Weighted voting - Wikipedia](https://en.wikipedia.org/wiki/Weighted_voting)
        /// - [Delegated voting / Liquid democracy](https://en.wikipedia.org/wiki/Liquid_democracy)
        /// - Osborne, M. J., *An Introduction to Game Theory*, Oxford University Press, 2004
        name: pub BalancedModel,
        input: Input,
        output: Output,
        others: [Candidate, FairWeight, Backer, Backed],
        bounds: [
            Input: IntoIterator<Item = (Candidate, FairWeight)> + Clone,
            Output: FromIterator<Candidate>,
            FairWeight: IntoIterator<Item = (Backer, Backed)> + Clone,
            Backed: Clone + Ord + Zero + Saturating + Add<Output = Backed> + Div<usize, Output = Backed>,
            Candidate: Clone,
        ],
        /// Computes the balanced fair rank of candidates imperatively.
        ///
        /// Steps:
        /// 1. Iterate over each candidate and collect all `(Backer, Backed)` pairs.
        /// 2. Compute total backing as the saturating sum of `Backed`.
        /// 3. Compute average backing per backer.
        /// 4. Compute score = total + average.
        /// 5. Sort candidates by descending score.
        /// 6. Return only the ordered candidates.
        compute: |input, _context| {
            // Step 1: Collect input into a vector for processing
            let items_input: Vec<(Candidate, FairWeight)> = input.clone().into_iter().collect();

            // Step 2: Prepare vector to store scores for each candidate
            let mut scores: Vec<(Candidate, Backed)> = Vec::new();

            // Step 3: Compute total, average, and combined score for each candidate
            for (candidate, fairweight) in items_input {
                let entries: Vec<(Backer, Backed)> = fairweight.into_iter().collect();
                // avoid division by zero
                let count = if entries.is_empty() { 1 } else { entries.len() };

                let mut total = Backed::zero();
                for (_, value) in &entries {
                    total = total.saturating_add(value.clone());
                }

                let average = total.clone() / count;
                let score = total.saturating_add(average);

                scores.push((candidate, score));
            }

            // Step 4: Sort candidates by descending score
            scores.sort_by(|a, b| b.1.cmp(&a.1));

            // Step 5: Collect only candidates in order
            let mut output: Vec<Candidate> = Vec::new();
            for (candidate, _) in scores {
                output.push(candidate);
            }

            output.into_iter().collect()
        }
    }

    // ===============================================================================
    // ``````````````````````````` PHRAGMEN FAIR-ELECTION ````````````````````````````
    // ===============================================================================

    /// Configuration for the [`PhragmenModel`] plugin model
    ///
    /// This struct allows fine-tuning of the Phragmen election algorithm
    /// to produce different variants of fair elections, including weighted,
    /// sequential, and scaled variants.
    ///
    /// Each field modifies how candidates are evaluated and how backer loads
    /// are computed.
    ///
    /// ## References:
    /// - [Phragmen voting - Wikipedia](https://en.wikipedia.org/wiki/Phragmen%27s_voting_rules)
    /// - Enestrom, E. "Mathematical Theory of Proportional Representation", 1896
    /// - Thiele, T. N., "Proportional Representation in Elections", 1895
    pub struct PhragmenModelConfig<Backed> {
        /// `weighted`: If true, the algorithm **scales each backer's contribution** by their
        /// influence or stake.
        ///
        /// - **Behavior when true:** Candidates supported by high-influence backers contribute more
        ///   to the backers' load. The algorithm prioritizes minimizing load while respecting
        ///   weighted influence, meaning candidates with concentrated high-weight backers may be
        ///   elected later to balance maximum loads.
        /// - **Behavior when false:** All backers' contributions are treated equally. Only the sum
        ///   of contributions matters, ignoring stake or influence differences.
        pub weighted: bool,

        /// `scale`: Optional multiplier applied to backer contributions when `weighted` is true.
        ///
        /// - **Behavior when Some(value):** Each backer's contribution is multiplied by `scale`.
        ///   This allows amplifying or reducing the effective weight of all backers, controlling
        ///   how heavily influence/stake impacts candidate ranking.
        /// - **Behavior when None:** Contributions are used as-is, without additional scaling.
        pub scale: Option<Backed>,
    }

    plugin_model! {
        /// The **Phragmen** model ranks candidates based on fair distribution of
        /// voter/backer load. Each candidate has `(Backer, Backed)` pairs representing
        /// support, and the algorithm selects candidates sequentially to **minimize
        /// the maximum load** among all backers.
        ///
        /// **Concept**: **Proportional Load Balancing**
        ///
        /// This plugin supports multiple Phragmen variants through the plugin
        /// context [`PhragmenModelConfig`]:
        /// - Weighted vs unweighted contributions
        /// - Deterministic tie-breaking
        /// - Optional scaling of backer contributions
        ///
        /// ## Applications:
        /// - Governance elections with proportional representation
        /// - Token or stake weighted systems
        /// - Delegation or liquid democracy systems
        ///
        /// ## References:
        /// - [Phragmen voting - Wikipedia](https://en.wikipedia.org/wiki/Phragmen%27s_voting_rules)
        /// - Enestrom, E. "Mathematical Theory of Proportional Representation", 1896
        /// - Thiele, T. N., "Proportional Representation in Elections", 1895
        name: pub PhragmenModel,
        input: Input,
        output: Output,
        others: [Candidate, FairWeight, Backer, Backed],
        context: PhragmenModelConfig<Backed>,
        bounds: [
            Input: IntoIterator<Item = (Candidate, FairWeight)> + Clone,
            Output: FromIterator<Candidate>,
            FairWeight: IntoIterator<Item = (Backer, Backed)> + Clone,
            Backed: Clone + Zero + Add<Output = Backed> + Ord + Div<usize, Output = Backed> + Mul<Backed, Output = Backed>,
            Candidate: Clone + Eq,
            Backer: Clone + Eq + sp_std::hash::Hash + Ord,
        ],

        /// Computes the **Phragmen ranking** of candidates imperatively.
        ///
        /// Steps:
        /// 1. Initialize all backer loads to zero.
        /// 2. While candidates remain:
        ///    - Compute maximum load increase for each candidate if elected.
        ///    - Apply weighting/scaling if configured.
        ///    - Deterministically select candidate with minimal max load (tie = first in list).
        ///    - Update ranking and backer loads.
        /// 3. Return all candidates in ranked order.
        compute: |input, ctx| {
            use sp_std::collections::btree_map::BTreeMap;
            use sp_std::vec::Vec;

            // Step 1: collect candidates and initialize ranking
            let mut remaining: Vec<(Candidate, FairWeight)> = input.clone().into_iter().collect();
            let mut ranking: Vec<Candidate> = Vec::new();

            // Step 1b: initialize all backer loads to zero
            let mut load: BTreeMap<Backer, Backed> = BTreeMap::new();
            for (_, fairweight) in &remaining {
                for (backer, _) in fairweight.clone().into_iter() {
                    load.entry(backer).or_insert(Backed::zero());
                }
            }

            // Step 2: select candidates until all are ranked
            while !remaining.is_empty() {
                let mut best_candidate = None;
                let mut best_max_load = None;

                for (candidate, fairweight) in &remaining {
                    // Compute maximum load this candidate would impose on their backers
                    let mut max_load = Backed::zero();

                    for (backer, backed) in fairweight.clone().into_iter() {
                        let mut contribution = backed;

                        // Apply weighting if configured
                        if ctx.weighted {
                            if let Some(scale) = &ctx.scale {
                                contribution = contribution * scale.clone();
                            }
                        }

                        let new_load = load.get(&backer).unwrap().clone() + contribution;
                        if new_load > max_load {
                            max_load = new_load;
                        }
                    }

                    // Determine if this candidate is currently the best choice
                    if best_max_load.is_none() || max_load < best_max_load.clone().unwrap() {
                        best_candidate = Some(candidate.clone());
                        best_max_load = Some(max_load);
                    }
                    // Tie is resolved deterministically by keeping first found
                }

                // Remove chosen candidate from remaining and add to ranking
                let chosen_index = remaining
                    .iter()
                    .position(|(c, _)| *c == best_candidate.clone().unwrap())
                    .unwrap();
                let (_candidate, fairweight) = remaining.remove(chosen_index);
                ranking.push(best_candidate.unwrap());

                // Step 2c: update backer loads
                for (backer, backed) in fairweight.into_iter() {
                    let mut contribution = backed;
                    if ctx.weighted {
                        if let Some(scale) = &ctx.scale {
                            contribution = contribution * scale.clone();
                        }
                    }
                    let entry = load.entry(backer).or_insert(Backed::zero());
                    *entry = entry.clone() + contribution;
                }
            }

            // Step 3: return ranked candidates
            ranking.into_iter().collect()
        }
    }

    // ===============================================================================
    // ````````````````````````` MAX-MIN LOAD FAIR-ELECTION ``````````````````````````
    // ===============================================================================

    plugin_model! {
        /// The **Max-Min Load Fair** model ranks candidates to achieve a **fair
        /// distribution of influence among all backers**. It selects candidates
        /// sequentially to **minimize the maximum load any backer bears**, ensuring
        /// no single backer is disproportionately overrepresented.
        ///
        /// **Concept**: **Load-Balanced Candidate Selection**
        ///
        /// Each candidate is associated with a list of `(Backer, Backed)` pairs
        /// representing the support they receive. The algorithm repeatedly:
        /// 1. Computes, for each remaining candidate, the maximum load their
        /// selection would impose on their backers.
        /// 2. Selects the candidate whose election results in the **smallest
        /// maximum load increase**.
        /// 3. Updates backer loads accordingly.
        ///
        /// ## Characteristics:
        /// - **Fairness-focused**: Prioritizes balancing backer loads rather
        /// than simply maximizing totals.
        /// - **Deterministic**: Tie-breaking is deterministic (first candidate
        /// in iteration).
        /// - **Context-free**: Does not require any external configuration or
        /// parameters.
        ///
        /// ## Applications:
        /// - Governance or board elections where proportional influence is desired.
        /// - Delegation-based or collaborative voting systems.
        /// - Any scenario where minimizing backer overload improves fairness.
        name: pub MaxMinLoadModel,
        input: Input,
        output: Output,
        others: [Candidate, Backer, Backed, FairWeight],
        bounds: [
            Input: IntoIterator<Item = (Candidate, FairWeight)> + Clone,
            Output: FromIterator<Candidate>,
            FairWeight: IntoIterator<Item = (Backer, Backed)> + Clone,
            Backed: Clone + Zero + Ord + Add<Output = Backed>,
            Candidate: Clone + Eq,
            Backer: Clone + Eq + sp_std::hash::Hash + Ord,
        ],
        /// Computes a fair ranking of candidates by minimizing maximum load
        /// per backer.
        ///
        /// Steps :
        /// 1. **Collect input** into a vector for sequential processing.
        /// 2. **Initialize backer loads** to zero.
        /// 3. While there are unranked candidates:
        ///    a. For each candidate, compute the maximum load increase across
        ///       their backers.
        ///    b. Select the candidate with the smallest maximum load.
        ///    c. Remove the selected candidate from remaining candidates and
        ///       append to ranking.
        ///    d. Update backer loads by adding this candidate's contributions.
        /// 4. Return the ranked candidates as an iterator.
        compute: |input, _ctx| {
            use sp_std::collections::btree_map::BTreeMap;
            use sp_std::vec::Vec;

            // Step 1: Collect candidates and their backing
            let mut remaining: Vec<(Candidate, FairWeight)> = input.clone().into_iter().collect();
            let mut ranking: Vec<Candidate> = Vec::new();

            // Step 2: Initialize backer loads to zero
            let mut load: BTreeMap<Backer, Backed> = BTreeMap::new();
            for (_, fairweight) in &remaining {
                for (backer, _) in fairweight.clone().into_iter() {
                    load.entry(backer).or_insert(Backed::zero());
                }
            }

            // Step 3: Iteratively select candidates
            while !remaining.is_empty() {
                let mut best_candidate = None;
                let mut min_max_load = None;

                // Evaluate each remaining candidate
                for (candidate, fairweight) in &remaining {
                    let mut max_load = Backed::zero();

                    for (backer, backed) in fairweight.clone().into_iter() {
                        let new_load = load.get(&backer).unwrap().clone() + backed;
                        if new_load > max_load {
                            max_load = new_load;
                        }
                    }

                    // Candidate minimizes maximum load
                    if min_max_load.is_none() || max_load < min_max_load.clone().unwrap() {
                        best_candidate = Some(candidate.clone());
                        min_max_load = Some(max_load);
                    }
                }

                // Remove selected candidate from remaining
                let idx = remaining
                    .iter()
                    .position(|(c, _)| *c == best_candidate.clone().unwrap())
                    .unwrap();
                let (_c, fairweight) = remaining.remove(idx);
                ranking.push(best_candidate.unwrap());

                // Update backer loads for selected candidate
                for (backer, backed) in fairweight.into_iter() {
                    let entry = load.entry(backer).or_insert(Backed::zero());
                    *entry = entry.clone() + backed;
                }
            }

            // Step 4: Return final ranking
            ranking.into_iter().collect()
        }
    }

    // ===============================================================================
    // ``````````````````````````` THRESHOLD FAIR-ELECTION ```````````````````````````
    // ===============================================================================

    /// Defines the configuration for [`ThresholdFairModel`] election plugin.
    ///
    /// Candidates are filtered based on a minimum total backing before being
    /// considered for ranking or selection.
    pub struct ThresholdFairModelConfig<T> {
        /// Minimum total backing required for a candidate to be considered.
        /// - `threshold`: The minimum backing value a candidate must have
        /// to be eligible.
        /// - Candidates with total backing **less than `threshold` are
        /// excluded** from the election.
        /// - Useful for preventing candidates with negligible support
        /// from affecting the outcome.
        pub threshold: T,
    }

    plugin_model! {
        /// Filters candidates whose **total backing** is below a configurable
        /// threshold.
        ///
        /// Each candidate is represented as `(Candidate, FairWeight)` where
        /// `FairWeight` is a list of `(Backer, Backed)` pairs. The algorithm
        /// sums all backings for a candidate and includes only those whose
        /// total meets or exceeds `ctx.threshold`.
        ///
        /// ## Characteristics:
        /// - Context-driven via [`ThresholdModelConfig`].
        /// - Deterministic ordering of candidates passing the threshold.
        /// - Prevents candidates with insufficient support from participating.
        ///
        /// ## Use cases:
        /// - Governance systems requiring minimum backing for eligibility.
        /// - Stake-weighted elections where very low-supported candidates
        /// should be ignored.
        name: pub ThresholdFairModel,
        input: Input,
        output: Output,
        others: [Candidate, Backer, Backed, FairWeight],
        context: ThresholdFairModelConfig<Backed>,
        bounds: [
            Input: IntoIterator<Item = (Candidate, FairWeight)> + Clone,
            Output: FromIterator<Candidate>,
            FairWeight: IntoIterator<Item = (Backer, Backed)> + Clone,
            Backed: Clone + Zero + Add<Output = Backed> + Ord,
            Candidate: Clone,
            Backer: Clone,
        ],

        /// Computes the filtered candidates imperatively.
        ///
        /// Steps:
        /// 1. Initialize an empty `output` vector.
        /// 2. Iterate over all candidates and their fair weights.
        /// 3. Sum the `Backed` values for each candidate to get `total`.
        /// 4. Include candidate in `output` if `total >= ctx.threshold`.
        /// 5. Return an iterator over the filtered candidates.
        compute: |input, ctx| {
            // Step 1: Prepare output vector
            let mut output = Vec::new();

            // Step 2: Iterate over candidates
            for (candidate, fairweight) in input.clone() {
                // Step 3: Compute total backing
                let total: Backed = fairweight
                    .into_iter()
                    .map(|(_, b)| b)
                    .fold(Backed::zero(), |acc, x| acc + x);

                // Step 4: Include only candidates meeting threshold
                if total >= ctx.threshold {
                    output.push(candidate);
                }
            }

            // Step 5: Return as iterator
            output.into_iter().collect()
        }
    }
}