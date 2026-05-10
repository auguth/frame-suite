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

// ===============================================================================
// ```````````````````````` ELECTION MODELS PLUGIN TESTS `````````````````````````
// ===============================================================================

#[cfg(test)]
mod tests {

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` IMPORTS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    // --- Local crate imports ---
    use crate::elections::{
        fair,
        flat::{self},
    };

    // --- FRAME Suite ---
    use frame_suite::plugin_test;

    // --- Substrate primitives ---
    use sp_runtime::AccountId32;

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
    const CHARLIE: AccountId32 = account_frm_seed(3);
    const ALAN: AccountId32 = account_frm_seed(4);
    const MIKE: AccountId32 = account_frm_seed(5);
    const CAROL: AccountId32 = account_frm_seed(6);
    const DAVE: AccountId32 = account_frm_seed(7);
    const FRANK: AccountId32 = account_frm_seed(8);
    const GRACE: AccountId32 = account_frm_seed(9);
    const IVAN: AccountId32 = account_frm_seed(10);
    const EVE: AccountId32 = account_frm_seed(11);
    const WADE: AccountId32 = account_frm_seed(12);
    const NIX: AccountId32 = account_frm_seed(13);
    const LAYA: AccountId32 = account_frm_seed(14);

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````` FLAT-ELECTION MODELS ````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
       
    // ----------------------------------- TOP-DOWN ----------------------------------

    plugin_test! {
        model: flat::TopDownFlatModel,
        input: Vec<(AccountId32, u64)>,
        output: Vec<AccountId32>,
        cases : {
            (top_down_model_basic_flat_election,
            vec![(ALICE, 180), (BOB, 170), (CHARLIE, 200), (ALAN, 210), (MIKE, 90)],
            vec![ALAN, CHARLIE, ALICE, BOB, MIKE]
            ),
            (top_down_model_equal_weights,
            // All candidates have equal weight - order preserved from input
            vec![(ALICE, 100), (BOB, 100), (CHARLIE, 100), (ALAN, 100)],
            vec![ALICE, BOB, CHARLIE, ALAN]
            ),
            (top_down_model_multiple_ties,
            // Multiple groups of tied weights
            vec![(ALICE, 200), (BOB, 100), (CHARLIE, 200), (ALAN, 100), (MIKE, 300)],
            vec![MIKE, ALICE, CHARLIE, BOB, ALAN]
            ),
        }
    }

    // ---------------------------------- THRESHOLD ----------------------------------

    plugin_test! {
        model: flat::ThresholdFlatModel,
        input: Vec<(AccountId32, u64)>,
        output: Vec<AccountId32>,
        context: flat::ThresholdFlatModelConfig<u64>,
        value: flat::ThresholdFlatModelConfig {
            threshold: 200
        },
        cases : {
            (threshold_model_basic_flat_election,
            vec![(ALICE, 180), (BOB, 230), (CHARLIE, 200), (ALAN, 270), (MIKE, 90)],
            vec![BOB, CHARLIE, ALAN]
            ),
            (threshold_model_exact_threshold,
            // Candidate exactly at threshold should be included
            vec![(ALICE, 200), (BOB, 200), (CHARLIE, 170)],
            vec![ALICE, BOB]
            ),
            (threshold_model_preserves_order,
            // Verify input order is preserved for qualifying candidates
            vec![(MIKE, 250), (ALICE, 300), (BOB, 280), (CHARLIE, 190), (ALAN, 260)],
            vec![MIKE, ALICE, BOB, ALAN]
            ),
        }
    }

    plugin_test! {
        model: flat::ThresholdFlatModel,
        input: Vec<(AccountId32, u64)>,
        output: Vec<AccountId32>,
        context: flat::ThresholdFlatModelConfig<u64>,
        value: flat::ThresholdFlatModelConfig {
            threshold: 1000
        },
        cases : {
            (threshold_model_high_threshold,
            // high threshold, only top candidates qualify
            vec![(ALICE, 500), (BOB, 1200), (CHARLIE, 900), (ALAN, 1500)],
            vec![BOB, ALAN]
            ),
            (threshold_model_none_qualify_high_threshold,
            // no candidates qualify
            vec![(ALICE, 500), (BOB, 800), (CHARLIE, 900)],
            vec![]
            )
        }
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````` FAIR-ELECTION MODELS ````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    // ----------------------------------- TOP-DOWN ----------------------------------

    plugin_test! {
        model: fair::TopDownFairModel,
        input: Vec<(AccountId32, Vec<(AccountId32, u64)>)>,
        output: Vec<AccountId32>,
        cases : {
            (top_down_model_basic_fair_election,
            // ALICE : 110, BOB : 170, CHARLIE: 180, ALAN: 60
            vec![(ALICE, vec![(CAROL, 50), (DAVE, 60)]), (BOB, vec![(FRANK, 50), (GRACE, 120)]), (CHARLIE, vec![(MIKE, 80), (IVAN, 100)]), (ALAN, vec![(EVE, 60)])],
            vec![CHARLIE, BOB, ALICE, ALAN]
            ),
            (top_down_model_equal_total_backing,
            // All candidates have equal total backing - preserves input order
            // ALICE: 100, BOB: 100, CHARLIE: 100
            vec![
                (ALICE, vec![(CAROL, 50), (DAVE, 50)]),
                (BOB, vec![(FRANK, 60), (GRACE, 40)]),
                (CHARLIE, vec![(MIKE, 30), (IVAN, 70)])
            ],
            vec![ALICE, BOB, CHARLIE]
            ),
            (top_down_model_no_backers,
            // candidates with no backers (total = 0)
            vec![
                (ALICE, vec![]),
                (BOB, vec![]),
                (CHARLIE, vec![])
            ],
            vec![ALICE, BOB, CHARLIE]
            ),
            (top_down_model_mixed_fair_election,
            vec![
                (ALICE, vec![(IVAN, 200)]), (NIX, vec![(FRANK, 300)]), (MIKE, vec![(GRACE, 200)]), (BOB, vec![]), (CHARLIE, vec![]),
                (DAVE, vec![]), (LAYA, vec![]), (ALAN, vec![(CAROL, 150)])
            ],
            vec![NIX, ALICE, MIKE, ALAN, BOB, CHARLIE, DAVE, LAYA]
            )
        }
    }

    // ----------------------------------- BALANCED ----------------------------------

    plugin_test! {
        model: fair::BalancedModel,
        input: Vec<(AccountId32, Vec<(AccountId32, usize)>)>,
        output: Vec<AccountId32>,
        cases : {
            (balanced_model_basic_fair_election,
            // ALICE : total=120 (3 backer), avg=40, score=140
            // BOB : total=170 (2 backer), avg=85, score=255
            // CHARLIE: total=180 (2 backer), avg=90, score=270
            // ALAN: total=60 (1 backer), avg=60, score=120
            vec![(ALICE, vec![(CAROL, 50), (DAVE, 50), (EVE, 20)]), (BOB, vec![(FRANK, 50), (GRACE, 120)]), (CHARLIE, vec![(MIKE, 80), (IVAN, 100)]), (ALAN, vec![(EVE, 60)])],
            vec![CHARLIE, BOB, ALICE, ALAN]
            ),
            (balanced_model_single_backer_advantage,
            // Single large backer vs many small backers
            // ALICE: total=100 (1 backer), avg=100, score=200
            // BOB: total=100 (5 backers), avg=20, score=120
            // ALICE wins due to higher average
            vec![
                (ALICE, vec![(CAROL, 100)]),
                (BOB, vec![(DAVE, 20), (FRANK, 20), (GRACE, 20), (MIKE, 20), (IVAN, 20)])
            ],
            vec![ALICE, BOB]
            ),
            (balanced_model_many_small_backers,
            // Candidate with many small backers vs few large backers
            // ALICE: total=500 (10 backers), avg=50, score=550
            // BOB: total=500 (2 backers), avg=250, score=750
            // BOB wins due to higher average
            vec![
                (ALICE, vec![
                    (CAROL, 50), (DAVE, 50), (FRANK, 50), (GRACE, 50), (MIKE, 50),
                    (IVAN, 50), (EVE, 50), (WADE, 50), (ALAN, 50), (CHARLIE, 50)
                ]),
                (BOB, vec![(CAROL, 250), (DAVE, 250)])
            ],
            vec![BOB, ALICE]
            ),
            (balanced_model_equal_combined_score,
            // Equal combined scores - preserves input order
            // ALICE: total=100, avg=50, score=150
            // BOB: total=120, avg=30, score=150
            // CHARLIE: total=90, avg=60, score=150
            vec![
                (ALICE, vec![(CAROL, 50), (DAVE, 50)]),
                (BOB, vec![(FRANK, 30), (GRACE, 30), (MIKE, 30), (IVAN, 30)]),
                (CHARLIE, vec![(EVE, 30), (WADE, 60)])
            ],
            vec![ALICE, BOB, CHARLIE]
            ),
            (balanced_model_no_backers,
            // Edge case: candidates with no backers
            // score = 0 + 0 = 0 for all
            vec![
                (ALICE, vec![]),
                (BOB, vec![]),
                (CHARLIE, vec![])
            ],
            vec![ALICE, BOB, CHARLIE]
            ),
        }
    }

    // ----------------------------------- PHRAGMEN ----------------------------------

    plugin_test! {
        model: fair::PhragmenModel,
        input: Vec<(AccountId32, Vec<(AccountId32, usize)>)>,
        output: Vec<AccountId32>,
        context: fair::PhragmenModelConfig<usize>,
        value: fair::PhragmenModelConfig {
            weighted: false,
            scale: None
        },
        cases: {
            (phragmen_sequential_true_fair_election,
            // ALICE: 110 (CAROL:50, DAVE:60) - max_load=60
            // BOB: 170 (FRANK:50, GRACE:120) - max_load=120
            // CHARLIE: 180 (MIKE:80, IVAN:100) - max_load=100
            // ALAN: 60 (EVE:60) - max_load=60
            // Round 1: ALICE wins (60, first encountered)
            // Round 2: ALAN wins (60, loads updated: CAROL=50, DAVE=60)
            // Round 3: CHARLIE wins (100 < 120)
            // Round 4: BOB remains
            vec![
                (ALICE, vec![(CAROL, 50), (DAVE, 60)]),
                (BOB, vec![(FRANK, 50), (GRACE, 120)]),
                (CHARLIE, vec![(MIKE, 80), (IVAN, 100)]),
                (ALAN, vec![(EVE, 60)])
            ],
            vec![ALICE, ALAN, CHARLIE, BOB]
            ),
            (phragmen_whale_monopoly_prevention,
            // Real-world: Company board election with 3 seats
            // BigCorp (10,000 shares) tries to control all 3 seats
            // Small shareholders (100 shares each) band together for 1 candidate
            //
            // Candidates:
            // - ALICE, BOB, CHARLIE: backed by BigCorp (10,000 each)
            // - DAVE: backed by 10 small shareholders (100 each, total 1,000)
            //
            // TopDown would give: [ALICE, BOB, CHARLIE] - BigCorp controls 100%
            // Phragmen gives: [DAVE, ALICE, BOB] - Small shareholders get 33% representation
            //
            // Round 1: DAVE wins (max_load = 100 vs 10,000)
            // Round 2: ALICE wins (BigCorp load still 0, max_load = 10,000)
            // Round 3: BOB wins (BigCorp now at 10,000, max_load = 20,000)
            vec![
                (ALICE, vec![(WADE, 10000)]),
                (BOB, vec![(WADE, 10000)]),
                (CHARLIE, vec![(WADE, 10000)]),
                (DAVE, vec![
                    (CAROL, 100), (DAVE, 100), (FRANK, 100), (GRACE, 100),
                    (IVAN, 100), (EVE, 100), (MIKE, 100), (ALAN, 100)
                ])
            ],
            vec![DAVE, ALICE, BOB, CHARLIE]
            ),
            (phragmen_single_backer_multiple_candidates,
            // CAROL backs all three candidates
            // ALICE: CAROL:100 - max_load=100
            // BOB: CAROL:50 - max_load=50
            // CHARLIE: CAROL:75 - max_load=75
            // Round 1: BOB wins (50)
            // Round 2: CHARLIE wins (50+75=125 vs 50+100=150)
            // Round 3: ALICE (150)
            vec![
                (ALICE, vec![(CAROL, 100)]),
                (BOB, vec![(CAROL, 50)]),
                (CHARLIE, vec![(CAROL, 75)])
            ],
            vec![BOB, CHARLIE, ALICE]
            ),
            (phragmen_equal_contributions_fair_election,
            // All candidates have same total backing and max_load
            // Tie-breaking by input order
            vec![
                (ALICE, vec![(CAROL, 50), (DAVE, 50)]),
                (BOB, vec![(FRANK, 50), (GRACE, 50)]),
                (CHARLIE, vec![(MIKE, 50), (IVAN, 50)])
            ],
            vec![ALICE, BOB, CHARLIE]
            ),
            (phragmen_no_backers_fair_election,
            // All candidates have zero backing
            vec![
                (ALICE, vec![]),
                (BOB, vec![]),
                (CHARLIE, vec![])
            ],
            vec![ALICE, BOB, CHARLIE]
            ),
            (phragmen_overlapping_backers_fair_election,
            // CAROL backs ALICE(60) and BOB(40)
            // DAVE backs only ALICE(80)
            // ALICE: max(CAROL:60, DAVE:80) = 80
            // BOB: CAROL:40 = 40
            // Round 1: BOB wins (40)
            // Round 2: ALICE (CAROL now at 40, so max(40+60=100, 0+80=80) = 100)
            vec![
                (ALICE, vec![(CAROL, 60), (DAVE, 80)]),
                (BOB, vec![(CAROL, 40)])
            ],
            vec![BOB, ALICE]
            ),
        }
    }

    plugin_test! {
        model: fair::PhragmenModel,
        input: Vec<(AccountId32, Vec<(AccountId32, usize)>)>,
        output: Vec<AccountId32>,
        context: fair::PhragmenModelConfig<usize>,
        value: fair::PhragmenModelConfig {
            weighted: true,
            scale: Some(2)  // Double all contributions
        },
        cases: {
            (phragmen_weighted_scaled_fair_election,
            // All contributions doubled:
            // ALICE: CAROL:100, DAVE:120 - max_load=120
            // BOB: FRANK:100, GRACE:240 - max_load=240
            // CHARLIE: MIKE:160, IVAN:200 - max_load=200
            // ALAN: EVE:120 - max_load=120
            // Round 1: ALICE wins (120, first encountered)
            // Round 2: ALAN wins (120, loads: CAROL=100, DAVE=120)
            // Round 3: CHARLIE wins (200 < 240)
            // Round 4: BOB
            vec![
                (ALICE, vec![(CAROL, 50), (DAVE, 60)]),
                (BOB, vec![(FRANK, 50), (GRACE, 120)]),
                (CHARLIE, vec![(MIKE, 80), (IVAN, 100)]),
                (ALAN, vec![(EVE, 60)])
            ],
            vec![ALICE, ALAN, CHARLIE, BOB]
            )
        }
    }
    plugin_test! {
        model: fair::PhragmenModel,
        input: Vec<(AccountId32, Vec<(AccountId32, usize)>)>,
        output: Vec<AccountId32>,
        context: fair::PhragmenModelConfig<usize>,
        value: fair::PhragmenModelConfig {
            weighted: true,
            scale: Some(10)  // 10x multiplier
        },
        cases: {
            (phragmen_large_scale_fair_election,
            // All contributions multiplied by 10
            // ALICE: max(CAROL:500, DAVE:600) = 600
            // BOB: max(FRANK:500, GRACE:1200) = 1200
            // CHARLIE: max(MIKE:800, IVAN:1000) = 1000
            vec![
                (ALICE, vec![(CAROL, 50), (DAVE, 60)]),
                (BOB, vec![(FRANK, 50), (GRACE, 120)]),
                (CHARLIE, vec![(MIKE, 80), (IVAN, 100)])
            ],
            vec![ALICE, CHARLIE, BOB]
            )
        }
    }

    // --------------------------------- MAX-MIN LOAD --------------------------------

    plugin_test! {
        model: fair::MaxMinLoadModel,
        input: Vec<(AccountId32, Vec<(AccountId32, u64)>)>,
        output: Vec<AccountId32>,
        cases: {
            (max_min_load_basic,
            // ALICE: max_load=60, BOB: max_load=120, CHARLIE: max_load=100, ALAN: max_load=60
            vec![
                (ALICE, vec![(CAROL, 50), (DAVE, 60)]),
                (BOB, vec![(FRANK, 50), (GRACE, 120)]),
                (CHARLIE, vec![(MIKE, 80), (IVAN, 100)]),
                (ALAN, vec![(EVE, 60)])
            ],
            vec![ALICE, ALAN, CHARLIE, BOB]
            ),
            (max_min_load_single_backer,
            // CAROL backs all - load accumulates
            vec![
                (ALICE, vec![(CAROL, 100)]),
                (BOB, vec![(CAROL, 50)]),
                (CHARLIE, vec![(CAROL, 75)])
            ],
            vec![BOB, CHARLIE, ALICE]
            ),
            (max_min_load_overlapping_backers,
            // Demonstrates load balancing across shared backers
            vec![
                (ALICE, vec![(CAROL, 60), (DAVE, 80)]),
                (BOB, vec![(CAROL, 40)])
            ],
            vec![BOB, ALICE]
            ),
            (max_min_load_no_backers,
            vec![
                (ALICE, vec![]),
                (BOB, vec![]),
                (CHARLIE, vec![])
            ],
            vec![ALICE, BOB, CHARLIE]
            )
        }
    }

    // ---------------------------------- THRESHOLD ----------------------------------

    plugin_test! {
        model: fair::ThresholdFairModel,
        input: Vec<(AccountId32, Vec<(AccountId32, u64)>)>,
        output: Vec<AccountId32>,
        context: fair::ThresholdFairModelConfig<u64>,
        value: fair::ThresholdFairModelConfig {
            threshold: 100
        },
        cases: {
            (threshold_model_basic_fair_election,
            // Basic case: filter candidates by total backing >= threshold
            // ALICE: 80 (below), BOB: 110 (above), CHARLIE: 130 (above), ALAN: 40 (below)
            vec![
                (ALICE, vec![(CAROL, 50), (DAVE, 30)]),
                (BOB, vec![(FRANK, 50), (GRACE, 60)]),
                (CHARLIE, vec![(MIKE, 80), (IVAN, 50)]),
                (ALAN, vec![(EVE, 40)])
            ],
            vec![BOB, CHARLIE]
            ),
            (threshold_model_all_qualify,
            // All candidates meet threshold
            // ALICE: 150, BOB: 200, CHARLIE: 180
            vec![
                (ALICE, vec![(CAROL, 80), (DAVE, 70)]),
                (BOB, vec![(FRANK, 100), (GRACE, 100)]),
                (CHARLIE, vec![(MIKE, 90), (IVAN, 90)])
            ],
            vec![ALICE, BOB, CHARLIE]
            ),
            (threshold_model_none_qualify,
            // No candidates meet threshold
            // ALICE: 90, BOB: 80, CHARLIE: 70
            vec![
                (ALICE, vec![(CAROL, 50), (DAVE, 40)]),
                (BOB, vec![(FRANK, 40), (GRACE, 40)]),
                (CHARLIE, vec![(MIKE, 30), (IVAN, 40)])
            ],
            vec![]
            ),
            (threshold_model_no_backers,
            // Edge case: candidates with no backers (total = 0)
            vec![
                (ALICE, vec![]),
                (BOB, vec![]),
                (CHARLIE, vec![])
            ],
            vec![]
            ),
        }
    }

    plugin_test! {
        model: fair::ThresholdFairModel,
        input: Vec<(AccountId32, Vec<(AccountId32, u64)>)>,
        output: Vec<AccountId32>,
        context: fair::ThresholdFairModelConfig<u64>,
        value: fair::ThresholdFairModelConfig {
            threshold: 0
        },
        cases: {
            (threshold_model_zero_threshold_all_qualify,
            // Zero threshold - all candidates qualify, even with zero backing
            vec![
                (ALICE, vec![(CAROL, 50), (DAVE, 30)]),
                (BOB, vec![]),
                (CHARLIE, vec![(MIKE, 80)]),
                (ALAN, vec![])
            ],
            vec![ALICE, BOB, CHARLIE, ALAN]
            ),

            (threshold_model_zero_threshold_preserves_order,
            // Verify all candidates preserved in input order
            vec![
                (CHARLIE, vec![(MIKE, 100)]),
                (ALICE, vec![(CAROL, 50)]),
                (BOB, vec![(DAVE, 75)]),
                (ALAN, vec![])
            ],
            vec![CHARLIE, ALICE, BOB, ALAN]
            ),
        }
    }
}
