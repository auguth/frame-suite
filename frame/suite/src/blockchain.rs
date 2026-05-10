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
// `````````````````````````````` BLOCKCHAIN SUITE ```````````````````````````````
// ===============================================================================

//! Defines a **comprehensive framework for managing authors** (validators) in a
//! decentralized  network maintaining a distributed, deterministic state machine.
//!
//! The network is considered as a set of **independent nodes operated by authors**,
//! where each author is responsible for appending transactions that drive the state
//! machine through deterministic transitions.
//!
//! To ensure consistency across the network, the system requires a mechanism to
//! determine **which author can propose transactions at any given period**, in
//! a way that is approved by a quorum of the authors themselves. This enables
//! coordinated block production, fair transaction inclusion, and consensus-approved
//! progression of the state machine while preserving author accountability.
//!
//! To ensure smooth operation, the system provides a **consistent and pluggable
//! framework** for:
//! - **Author elections** ([`ElectAuthors`]) determining which authors are
//! responsible for producing the next set of blocks or transactions.
//! - **Contribution tracking** ([`AuthorPoints`]) ephemeral points awarded for
//! good behavior or participation during a session, allowing fair evaluation of
//! author performance.
//! - **Reward distribution** ([`RewardAuthors`]) single-point reward computation
//! using accumulated points, distributing assets to authors based on their
//! contributions.
//! - **Penalty enforcement** ([`PenalizeAuthors`]) immediate sanctions for bad
//! behavior, missed duties, or misbehavior.
//! - **Affidavit handling** ([`ElectionAffidavits`]) voluntary self-reported
//! election weights that authors may submit to participate in elections or help
//! the system determine the next set of block producers.
//!
//! ## Design Goals
//!
//! 1. **End-to-end author lifecycle management**: track contributions, elect,
//! reward, and penalize authors in a consistent way.
//! 2. **Fair and transparent reward system**: ephemeral points track good behavior;
//! rewards are computed at a single point in time for all authors and distributed
//! according to points earned.
//! 3. **Immediate accountability**: penalties are applied as soon as misbehavior
//! is detected.
//! 4. **Flexible and pluggable logic**: supports runtime-configurable
//! [`plugins`](crate::plugins) for rewards, penalties, inflation adjustments, and
//! election algorithms.
//! 5. **Framework-agnostic architecture**: can be integrated in:
//!    - **Standalone chains** with independent consensus and security.
//!    - **Parachains or shared-security environments**, where authors act as
//!    collators or validators.
//!
//! ## Terminology
//!
//! - **Authors**: Independent node operators responsible for producing blocks,
//! validating transactions, or fulfilling consensus-critical roles.
//! - **Points**: Temporary metrics representing author contributions for a session;
//! used for computing rewards fairly at a single evaluation point.
//! - **Rewards**: Asset payouts allocated based on points, configurable via
//! runtime [`plugins`](crate::plugins).
//! - **Penalties**: Immediate deductions or sanctions applied to authors for
//! misbehavior.
//! - **Affidavits**: Voluntary, self-reported election weights submitted by
//! authors, used for lazy election mechanisms where authors influence election
//! outcomes.
//!
//! By implementing these traits, a runtime gains a **robust, modular, and
//! chain-agnostic system** for handling author performance, elections, rewards,
//! penalties, and voluntary affidavits, ensuring predictable, fair, and transparent
//! operation of the blockchain network.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{
    base::{Buffer, Countable, Keyed, Percentage, Storable},
    plugin_output, plugin_types,
};

// --- Substrate primitives ---
use sp_runtime::{DispatchError, DispatchResult};

// ===============================================================================
// `````````````````````````````` AUTHOR-ELECTIONS ```````````````````````````````
// ===============================================================================

/// Higher-Level Trait for performing election of authors (e.g., block producers,
/// validators, etc) in a FRAME-based runtime.
///
/// This trait abstracts the process of **candidate preparation**, **author
/// selection**, and **election result handling** in a generic,
/// [`pluggable`](crate::plugins) way.  
///
/// The design allows different implementations to define their own election logic -
/// such as weighted randomness, reputation-based scoring, or round-robin selection -
/// while maintaining a consistent interface.
///
/// ## Type Parameters
/// - `Author`: The type representing an author or candidate
/// - `ElectionWeight`: The type representing each candidate's election weight,
///   score, or stake. Should be comparable [`Ord`].
///
/// ## Runner Model
/// The election process can be executed in two modes:
/// - **Global execution (`None`)**: The runtime itself triggers and executes
///   the election (e.g., via inherent or root-driven logic).
/// - **Author-driven execution (`Some(Author)`)**: A specific author acts as a
///   **runner**, voluntarily or by assignment, to execute the election logic.
///
/// This enables flexible designs where:
/// - Elections can be decentralized and triggered by participants.
/// - A designated author (e.g., block producer, coordinator) can perform elections.
/// - The runtime can still retain full control when required.
///
/// ## Usage
///
/// A runtime pallet implementing this trait would typically:
/// 1. Collect candidates via [`Self::prepare_candidates`].
/// 2. Elect authors via [`Self::prepare_authors`].
/// 3. Handle success or failure with [`Self::on_elect_success`] or
///   [`Self::on_elect_fail`], optionally using the `runner`.
/// 4. Reveal the final elected authors with [`Self::reveal`].
pub trait ElectAuthors<Author, ElectionWeight>
where
    Author: Keyed,
    ElectionWeight: Ord + Storable,
{
    /// Type representing all candidates in the election.
    type Candidates: Buffer<(Author, ElectionWeight)>;

    /// Type representing the set of successfully elected authors.
    type Elected: Buffer<Author>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Determines whether the election process can currently be run.
    ///
    /// ## Parameters
    /// - `runner`: Optional executor of the election process.
    ///   - `None`: runtime-driven execution.
    ///   - `Some(author)`: author-driven execution.
    ///
    /// This is a pre-check to ensure that conditions are right for running
    /// an election. For example, a runtime may enforce:
    /// - Minimum number of candidates.
    /// - Required system state (e.g., epoch boundaries).
    /// - Timing constraints or cooldown periods.
    /// - Authorization or eligibility of the `runner`.
    ///
    /// Returns `Ok(())` if the election is allowed to proceed, `DispatchError` otherwise.
    fn can_process_election(_runner: &Option<Author>) -> DispatchResult;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Returns the final set of recently elected authors.
    ///
    /// Implementations may retrieve this from storage or memory.
    /// This is usually called after a successful [`Self::prepare_election`] run.
    fn reveal() -> Option<Self::Elected>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` CONSTRUCTORS ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Prepares the set of candidates for the next election round.
    ///
    /// Typically this would collect eligible authors, validators, or accounts
    /// from storage or an external data source, and attach their election weights.
    ///
    /// Returns either the collection of candidates or an error if preparation fails.
    fn prepare_candidates() -> Result<Self::Candidates, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    
    /// Main entry point for the election process.
    ///
    /// Takes a collection of candidates (with weights) and performs
    /// the author election logic - storing or returning the result as appropriate.
    ///
    /// Returns `Ok(())` if election succeeds, or `Err(DispatchError)` if
    /// election logic fails.
    fn prepare_authors(candidates: Self::Candidates) -> DispatchResult;

    /// High-level orchestration function for running the entire election flow.
    ///
    /// ## Parameters
    /// - `runner`: Optional executor of the election process.
    ///   - `None`: election is executed by the runtime.
    ///   - `Some(author)`: a specific author executes the election.
    ///
    /// ## Workflow
    /// 1. Calls [`Self::can_process_election`] to validate execution conditions.
    /// 2. Calls [`Self::prepare_candidates`] to collect all eligible candidates.
    /// 3. Passes them to [`Self::prepare_authors`] to perform the election.
    /// 4. Triggers [`Self::on_elect_success`] or [`Self::on_elect_fail`] with `runner`.
    ///
    /// This ensures consistent election control flow, centralized error handling,
    /// and proper attribution of execution responsibility.
    ///
    /// Returns `DispatchError` if any stage fails.
    fn prepare_election(runner: &Option<Author>) -> DispatchResult {
        Self::can_process_election(runner)?;
        let candidates = Self::prepare_candidates()?;
        if let Err(e) = Self::prepare_authors(candidates) {
            Self::on_elect_fail(runner, e);
            return Err(e);
        };
        Self::on_elect_success(runner);
        Ok(())
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Hook called when an election completes successfully.
    ///
    /// ## Parameters
    /// - `runner`: Optional executor of the election.
    ///
    /// To perform post-election actions such as:
    /// - Persisting results to storage.
    /// - Emitting pallet events.
    /// - Updating system state or metrics.
    /// - Attributing execution responsibility or rewards to the runner.
    ///
    /// Default implementation is no-op
    fn on_elect_success(_runner: &Option<Author>) {}

    /// Hook called when an election fails.
    ///
    /// ## Parameters
    /// - `runner`: Optional executor of the election.
    /// - `err`: The error that caused the failure.
    ///
    /// Runtime implementations can override this to:
    /// - Emit error events.
    /// - Retry logic.
    /// - Fallback to default authors.
    /// - Attribute failure or responsibility to the runner.
    ///
    /// The passed `DispatchError` provides detailed context on what went wrong.
    /// Default implementation is no-op
    fn on_elect_fail(_runner: &Option<Author>, _err: DispatchError) {}
}

// ===============================================================================
// ```````````````````````````````` AUTHOR-POINTS ````````````````````````````````
// ===============================================================================

/// Trait representing **temporary, abstract points assigned to authors**.
///
/// These points are **not finalized rewards or assets**, but rather a
/// lightweight mechanism to track "good behavior" or participation within a
/// specific, temporary context, such as:
/// - A single author round.
/// - A session of block production.
/// - A single task or contribution that is ephemeral in nature.
///
/// ## Design notes
/// - Points are **incremented one at a time** using [`Self::add_point`], rather
///   than in bulk. This enforces a clear, single-event granularity per point. It also
///   simplifies logic for ephemeral metrics and prevents accidental double-counting.
/// - Different implementations of this trait can represent **distinct points systems**
///   or contexts. For example, one implementation could track "block production points",
///   while another tracks "validation participation points".
///
/// ## Ephemeral behavior
/// - Points do not correspond to finalized payments or balances.
/// - They are **temporary**: cleared via [`Self::clear_points`] at the end of the round
///   or session.
/// - They provide a way to accumulate **good behavior metrics** for temporary evaluation.
///
/// ## Type Parameters
/// - `Author`: The entity receiving points (e.g., `AccountId` or validator identifier).  
/// - `Points`: The numeric type representing points; typically `u32`, `u64`. This type
///   can be used to compute metrics, rankings, or temporary rewards.
pub trait AuthorPoints<Author, Points>
where
    Author: Keyed,
    Points: Countable,
{
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Returns the current points of a given author, if any.
    ///
    /// Points are **temporary** and reflect only the current round or session.
    /// Returns a `DispatchError` if there is an issue reading storage or runtime state.
    ///
    /// ## Parameters
    /// - `author`: The author whose points are queried.
    ///
    /// ## Returns
    /// - `Ok(points)` if the author has points.
    /// - `Err(DispatchError)` if reading fails.
    fn points_of(author: &Author) -> Result<Points, DispatchError>;

    /// Returns an iterator over all authors and their current points.
    ///
    /// This provides a view of the **entire ephemeral points state** for the
    /// current round or session, typically used by runtime operations such as
    /// reward computation, ranking, or evaluation.
    ///
    /// Since points are **ephemeral**, any such operation is expected to
    /// eventually clear them via [`Self::clear_points`], which is left to
    /// the caller's discretion.
    ///
    /// ## Returns
    /// - An iterator yielding `(Author, Points)` pairs.
    fn iter_points() -> impl Iterator<Item = (Author, Points)>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Sets the points for a given author.
    ///
    /// This is the **primitive write operation** for point state.
    /// All mutations to points (including increments) should be expressed
    /// in terms of this method to ensure consistency.
    ///
    /// ## Returns
    /// - `Ok(())` if the update succeeds.
    /// - `Err(DispatchError)` if the operation fails.
    fn set_points(author: &Author, points: Points) -> DispatchResult;

    /// Adds a single point to a given author.
    ///
    /// Each invocation represents **one unit of contribution or good behavior**.
    /// For bulk or weighted increments, implementers could wrap this in higher-level
    /// logic externally, keeping the trait focused on single-point increments.
    ///
    /// This default implementation:
    /// - Reads the current points (or assumes zero if none exist)
    /// - Performs a **saturating addition** of one point
    /// - Writes the updated value via [`Self::set_points`]
    ///
    /// Returns a `DispatchError` if updating fails (e.g., storage error or overflow).
    fn add_point(author: &Author) -> DispatchResult {
        let current = Self::points_of(author).unwrap_or_else(|_| Points::zero());
        let new = current.saturating_add(Points::one());
        Self::set_points(author, new)
    }

    /// Clears all points for all authors.
    ///
    /// - Typically called at the end of a round or session to reset the
    ///   ephemeral scoring.
    /// - Ensures the points system remains **context-specific** and avoids
    ///   carry-over between rounds.
    fn clear_points();
}

// ===============================================================================
// ``````````````````````````````` AUTHOR-REWARDS ````````````````````````````````
// ===============================================================================

/// Provides a **plugin-driven reward system** for authors, connecting
/// ephemeral **points** to actual **asset payouts**.
///
/// This trait is designed for modular, flexible, and runtime-configurable
/// reward logic in Substrate pallets.
///
/// Points should reflect temporary contributions (e.g., block production,
/// validation) and are **cleared after each reward cycle**.
///
/// ## Core Responsibilities
/// 1. Track author points using [`Self::AuthorPointsAdapter`].
/// 2. Compute the total payout using a runtime-configurable [`Self::PayoutModel`].
/// 3. Generate a per-author payout list using [`Self::PayeeModel`].
/// 4. Execute rewards safely, with callbacks for success or failure.
///
/// ## Type Parameters
/// - `Author`: Entity receiving rewards.
/// - `Asset`: Type of reward (e.g., token balance).
/// - `Points`: Type representing ephemeral points .
pub trait RewardAuthors<Author, Asset, Points>
where
    Author: Keyed,
    Asset: crate::Asset,
    Points: Countable,
{
    /// Adapter connecting **author points** with the reward logic.
    ///
    /// This associated type provides the bridge between ephemeral
    /// **points** assigned to authors and the current reward computation system.  
    ///
    /// - Defined as an associated type because points may be **tracked
    ///   externally** or come from non-local sources.
    /// - Responsible for **capturing all point contributions** relevant to rewarding,
    ///   ensuring accurate reward calculations.
    type AuthorPointsAdapter: AuthorPoints<Author, Points>;

    /// Collection of authors and their **ephemeral points** for the current
    /// reward cycle.
    ///
    /// This type represents the **input to the [`Self::PayeeModel`] plugin**,
    /// which determines how the total payout is distributed among authors
    /// based on their points.
    type PayoutFor: Buffer<(Author, Points)>;

    /// Collection of authors and their **final reward amounts** for the current cycle.
    ///
    /// This type represents the **output of the [`Self::PayeeModel`] plugin**,
    /// which maps author points and the total payout into actual rewards (`Asset`).
    type PayeeList: Buffer<(Author, Asset)>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    
    plugin_types!(
        input: Asset,
        output: Asset,
        /// Plugin model for computing the **total reward pool**.
        ///
        /// This model determines how the raw total payout is transformed
        /// into the final total reward.
        ///
        /// This plugin allows runtime-configurable logic to adjust the raw
        /// total payout (`Asset`) before distributing it to authors.
        ///
        /// ## Input
        /// - `Asset`: The raw total reward amount for the current cycle.
        ///
        /// ## Output
        /// - `Asset`: The optimized total reward after applying payout logic
        ///   (e.g., capping, scaling, or applying curves).
        model: PayoutModel,

        /// Provides optional runtime parameters for the payout
        /// plugin [`Self::PayoutModel`].
        ///
        /// Enables dynamic configuration, such as thresholds, multipliers,
        /// or other runtime-adjustable settings.
        context: PayoutContext,
    );

    /// Returns the **raw, unprocessed total payout** for the current reward cycle.
    ///
    /// This is the low-level deterministic total reward amount before any adjustments:
    /// - May reflect a global metric (e.g., total stake, contributions, or available funds).
    /// - Can be modified by the [`Self::PayoutModel`] plugin to produce the final payout
    ///   via [`Self::payout`].
    /// - May be very large or very small depending on the cycle conditions.
    fn payout_via() -> Asset;

    /// Computes the **final total payout** for the current reward cycle.
    ///
    /// - Uses the [`Self::PayoutModel`] plugin via [`Self::payout_process`] to adjust
    /// the raw payout ([`Self::payout_via`]).
    /// - Can apply reward curves, scaling, caps, or other runtime-configurable rules.
    /// - Represents the **high-level payout** that will be distributed to authors.
    fn payout() -> Asset {
        let via = Self::payout_via();
        Self::payout_process(via)
    }

    plugin_output! {
        /// [`Self::PayoutModel`] plugin output function.
        /// Utilizes the plugin model's context [`Self::PayoutContext`]
        fn payout_process,
        input: Asset,
        output: Asset,
        model: Self::PayoutModel,
        context: Self::PayoutContext
    }

    /// Returns the **authors and their accumulated points** for the current reward cycle.
    ///
    /// - Serves as the input for the [`Self::PayeeModel`] plugin to compute per-author payouts.
    /// - Represents ephemeral contributions, which are cleared after reward distribution.
    /// - Can include multiple roles or activities that contribute points for rewards.
    fn payout_for() -> Self::PayoutFor;

    plugin_types!(
        input: (Asset, Self::PayoutFor),
        output: Self::PayeeList,
        /// Plugin model for computing **per-author payouts** from total payout and points.
        ///
        /// This plugin model responsible for distributing the total payout
        /// among authors according to their points.
        ///
        /// - Input: Tuple (`total_payout`, [`Self::PayoutFor`]).
        /// - Output: [`Self::PayeeList`].
        ///
        /// Computes the mapping from `(Author, Points)` to `(Author, Asset)`.
        model: PayeeModel,

        /// Provides optional runtime parameters for the payee plugin
        /// [`Self::PayeeModel`] computation.
        ///
        /// Enables dynamic configuration, such as thresholds, multipliers,
        /// or other runtime-adjustable settings.
        context: PayeeContext,
    );

    plugin_output! {
        /// [`Self::PayeeModel`] plugin output function.
        /// Utilizes the plugin model's context [`Self::PayeeContext`]
        fn payee_process,
        input: (Asset, Self::PayoutFor),
        output: Self::PayeeList,
        model: Self::PayeeModel,
        context: Self::PayeeContext
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Distributes rewards to authors based on their points and available payout.
    ///
    /// ## Workflow
    /// 1. Compute total payout using [`Self::payout`].
    /// 2. If payout is zero, skip distribution.
    /// 3. Generate per-author payout list via the [`Self::PayeeModel`] plugin
    /// function [`Self::payee_process`].
    /// 4. Distribute rewards using [`Self::reward`] for each author.
    /// 5. Call [`Self::on_reward_success`] or [`Self::on_reward_fail`] callbacks
    ///   for each author.
    /// 6. Clear ephemeral points via [`Self::AuthorPointsAdapter`].
    ///
    /// ## Notes
    /// - Ensure that enough points are supplied; otherwise, a few authors may receive
    ///   disproportionately high rewards.
    /// - Individual failures are handled gracefully and do not abort the reward cycle.
    fn reward_authors() {
        let payout = Self::payout();

        if !payout.is_zero() {
            let towards = Self::payout_for();

            let payees = Self::payee_process((payout, towards));
            for (ref id, value) in payees {
                if let Err(err) = Self::reward(id, value) {
                    // gracefully handle failures; cannot re-run entire distribution
                    Self::on_reward_fail(id, err);
                }
                Self::on_reward_success(id, value);
            }
        }

        Self::AuthorPointsAdapter::clear_points()
    }

    /// Distributes a reward `value` to a specific author `who`.
    ///
    /// ## Requirements
    /// - Must be implemented by the pallet using this trait.
    /// - Example implementations: token transfer, minting, or
    ///   crediting balances.
    fn reward(who: &Author, value: Asset) -> DispatchResult;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Callback invoked after a successful individual reward.
    ///
    /// Useful for logging, metrics collection, or triggering side-effects.
    ///
    /// Default is no-op
    fn on_reward_success(_who: &Author, _value: Asset) {}

    /// Callback invoked when an individual reward fails.
    ///
    /// Useful for logging, alerting, or implementing retry logic.
    ///
    /// Default is no-op
    fn on_reward_fail(_who: &Author, _err: DispatchError) {}
}

// ===============================================================================
// `````````````````````````````` AUTHOR-PENALTIES ```````````````````````````````
// ===============================================================================

/// Provides a **plugin-driven penalty system** for authors, allowing
/// permanent penalties to be applied in a modular and runtime-configurable way.
///
/// ## Core Responsibilities
/// 1. Accept a set of authors and penalties ([`Self::PenaltyFor`]).
/// 2. Transform penalties via a runtime-configurable [`Self::PenaltyModel`] plugin.
/// 3. Apply penalties individually with safe callbacks for success or failure.
///
/// ## Type Parameters
/// - `Author`: Entity receiving the penalty.
/// - `Penalty`: Type representing the penalty (e.g., token deduction, score reduction).
pub trait PenalizeAuthors<Author, Penalty>
where
    Author: Keyed,
    Penalty: Percentage,
{
    /// Collection of authors and their associated penalties for normalization.
    ///
    /// - Represents the **input** to the [`Self::PenaltyModel`] plugin.
    /// - Represents the **output** after plugin transformation.
    /// - Supports iteration, extension, and default construction.
    type PenaltyFor: Buffer<(Author, Penalty)>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    plugin_types!(
        input: Self::PenaltyFor,
        output: Self::PenaltyFor,
        /// The plugin model implementing the penalty transformation logic.
        ///
        /// - Input: [`Self::PenaltyFor`] collection.
        /// - Output: Transformed [`Self::PenaltyFor`] collection.
        ///
        /// Transforms the mapping from `(Author, Penalty)` to `(Author, Penalty)`.
        model: PenaltyModel,
        /// Provides optional runtime parameters for the penalty-transformation
        /// plugin [`Self::PenaltyModel`] computation.
        ///
        /// Enables dynamic configuration, such as thresholds, multipliers,
        /// or other runtime-adjustable settings.
        context: PenaltyContext,
    );

    /// Apply penalties to a list of authors.
    ///
    /// Workflow:
    /// 1. Transform input penalties using [`Self::PenaltyModel`].
    /// 2. Apply penalties to each author via [`Self::penalize`].
    /// 3. Call [`Self::on_penalty_success`] or [`Self::on_penalty_fail`] for
    ///   each author.
    ///
    /// Notes:
    /// - Handles multiple penalties or single penalty uniformly.
    /// - Failures for individual authors do not halt the process.
    fn penalize_authors(towards: Self::PenaltyFor) {
        let penalty_for = Self::transform_penalty(towards);
        for (ref who, penalty) in penalty_for {
            if let Err(err) = Self::penalize(who, penalty) {
                Self::on_penalty_fail(who, err);
            }
            Self::on_penalty_success(&who, penalty);
        }
    }

    plugin_output! {
        /// Transforms raw penalties using the [`Self::PenaltyModel`] plugin.
        ///
        /// - Applies runtime-configured rules, multipliers, or caps.
        /// - Utilizes the plugin model's context [`Self::PenaltyContext`]
        fn transform_penalty,
        input: Self::PenaltyFor,
        output: Self::PenaltyFor,
        model: Self::PenaltyModel,
        context: Self::PenaltyContext
    }

    /// Applies a single penalty to an author.
    ///
    /// This is a **direct application** of a penalty and does not involve
    /// transformation via the [`Self::PenaltyModel`] plugin.
    fn penalize(who: &Author, penalty: Penalty) -> DispatchResult;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Callback invoked after a successful individual penalty.
    ///
    /// Can be used for logging, metrics, or side-effects.
    ///
    /// Default is no-op
    fn on_penalty_success(_who: &Author, _penalty: Penalty) {}

    /// Callback invoked when an author penalty fails.
    ///
    /// Can be used for logging, alerting, or retry logic.
    ///
    /// Default is no-op
    fn on_penalty_fail(_who: &Author, _err: DispatchError) {}
}

// ===============================================================================
// `````````````````````````````` AUTHOR-AFFIDAVITS ``````````````````````````````
// ===============================================================================

/// Defines the behavior for **author affidavits** - self-declared affirmations
/// of election weights during a given cycle.
///
/// ## Concept
/// Affidavits represent an **author's own self-declaration** of their election weight
/// (e.g., performance, stake, or participation value) for the current election cycle.
/// They are not requested or enforced by the system; instead, they are **voluntarily
/// submitted by the authors** themselves.
///
/// These affidavits are **ephemeral**:
/// - Stored temporarily for the next (upcoming) election cycle.
/// - Must be cleared once the cycle ends or after all have been processed.
/// - Can later be used by external modules (e.g., `SessionManager`)
///   to inform election outcomes or validations.
///
/// ## Responsibilities
/// This trait defines:
/// - Submission and validation of author affidavits.
/// - Storage and retrieval of **ephemeral** affidavit data.
/// - Lifecycle management (generation, existence check, removal, clearing).
///
/// ## Type Parameters
/// - `Author`: Entity submitting the affidavit (e.g., validator, collator,
///   consortium-roles, etc).
/// - `ElectionWeight`: The numeric or structured weight being affirmed.
///   Requires [`Ord`].
pub trait ElectionAffidavits<Author, ElectionWeight>
where
    Author: Keyed,
    ElectionWeight: Ord + Storable,
{
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Determines whether an author is eligible to submit an affidavit
    /// for the next election cycle.
    ///
    /// Implementations should define conditions such as:
    /// - Whether the author can be part of the next election round.
    /// - Whether the submission window is still open.
    /// - Whether the author already submitted.
    fn can_submit_affidavit(who: &Author) -> DispatchResult;

    /// Checks whether an affidavit currently exists for the given author.
    ///
    /// ## Returns
    /// - `Ok(())` if exists.
    /// - `Err(DispatchError)` if not or on query failure.
    fn affidavit_exists(who: &Author) -> DispatchResult;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Retrieves the **stored affidavit** for a given author.
    ///
    /// Used only for external queries to fetch the most recent
    /// submitted affidavit.
    ///
    /// Should not be used internally during submission or processing of
    /// a single affidavit.
    fn get_affidavit(who: &Author) -> Result<ElectionWeight, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` CONSTRUCTORS ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Generates a new affidavit (i.e., computes or constructs an election weight)
    /// for the given author.
    ///
    /// Typically derived from the author's participation or other context-dependent
    /// metrics in the current cycle.  
    ///
    /// **Note:** This may produce a different value than any previously submitted
    /// affidavit.
    ///
    /// ## Returns
    /// - `Ok(ElectionWeight)` if generation is successful.
    /// - `Err(DispatchError)` if generation fails.
    fn gen_affidavit(who: &Author) -> Result<ElectionWeight, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Submits an affidavit for the author for the next election cycle.
    ///
    /// ## Parameters
    /// - `who`: Author submitting the affidavit.
    /// - `affidavit`: The generated election weight for submission.
    fn submit_affidavit(who: &Author, affidavit: &ElectionWeight) -> DispatchResult;

    /// Processes the full lifecycle of an affidavit submission for a given author
    /// for the next election cycle.
    ///
    /// This is the **entry point** for submitting an affidavit.  
    /// Workflow:
    /// 1. Check if the author **can** submit ([`Self::can_submit_affidavit`]).
    /// 2. Generate the author's affidavit ([`Self::gen_affidavit`]).
    /// 3. **Submit** the affidavit ([`Self::submit_affidavit`]) with the
    ///   generated weight.
    /// 4. Trigger optional post-submission hook ([`Self::on_submit_affidavit`]).
    fn process_affidavit(who: &Author) -> DispatchResult {
        Self::can_submit_affidavit(who)?;
        let affidavit = Self::gen_affidavit(who)?;
        Self::submit_affidavit(who, &affidavit)?;
        Self::on_submit_affidavit(who, &affidavit);
        Ok(())
    }

    /// Removes a stored affidavit for the given author.
    ///
    /// Typically invoked by **external modules** to remove an affidavit
    /// before processing the collected affidavits for a cycle.
    ///
    /// Pre- or post-processing removal is allowed, as appropriate.
    fn remove_affidavit(who: &Author) -> DispatchResult;

    /// Clears **all** affidavits for the current election cycle.
    ///
    /// Should be called once all affidavits have been processed or validated,
    /// ensuring that new cycles start with a clean state.
    fn clear_affidavits();

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Hook triggered **after a successful affidavit submission**.
    ///
    /// Allows external logic, e.g., logging, reward triggers, or
    /// cross-pallet coordination.
    ///
    /// Default is no-op
    fn on_submit_affidavit(_who: &Author, _affidavit: &ElectionWeight) {}
}
