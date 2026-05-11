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
// `````````````````````````````` BLOCKCHAIN ACTORS ``````````````````````````````
// ===============================================================================

//! Provides the **core runtime logic for managing blockchain actors**
//! (authors/validators) across their full lifecycle.
//!
//! - **Election orchestration** (via [`ElectAuthors`])
//! - **Affidavit submission and validation** (via [`ElectionAffidavits`])
//! - **Contribution tracking** through session-scoped points (via
//!   [`AuthorPoints`], although swappable via [`Config::PointsAdapter`])
//! - **Reward scheduling** based on participation (via [`RewardAuthors`])
//! - **Penalty scheduling** for misbehavior (via [`PenalizeAuthors`])
//!
//! The module acts as a **bridge layer** between generic trait abstractions and
//! pallet-specific storage, timing, and role-management systems.
//!
//! ## Design Overview
//!
//! - **Session-driven lifecycle**:
//!   All operations (affidavits, elections, points, rewards, penalties)
//!   are scoped to sessions and aligned with deterministic session timing.
//!
//! - **Time-gated execution**:
//!   Affidavit submission and election processing are strictly bounded by
//!   windows derived from session start and configurable percentages.
//!
//! - **Separation of concerns**:
//!   - This module coordinates *when* and *what* to execute.
//!   - External adapters/plugins define *how* logic is executed:
//!     - Election logic: [`Config::ElectionAdapter`]
//!     - Reward logic: [`Config::RewardModel`], [`Config::InflationModel`]
//!     - Penalty logic: [`Config::PenaltyModel`]
//!
//! - **Deterministic and auditable**:
//!   All operations avoid side effects and remain reproducible across nodes.
//!
//! - **Deferred execution model**:
//!   Rewards and penalties are **scheduled**, not immediately finalized,
//!   allowing downstream systems to aggregate, adjust, or revert them.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{
    types::*, AffidavitKeys, AllowAffidavits, AuthorAffidavits,
    BlockPointsStore, Config, CurrentSession, Error, Event, Internals, Pallet,
};

// --- FRAME Suite ---
use frame_suite::{blockchain::*, elections::*, roles::*};

// --- FRAME Support ---
use frame_support::{
    ensure,
    traits::{fungible::Inspect, tokens::Precision},
};

// --- Substrate primitives ---
use sp_core::Get;
use sp_runtime::{
    traits::{One, Saturating},
    DispatchError, DispatchResult, Vec, WeakBoundedVec,
};

// ===============================================================================
// ```````````````````````````````` ELECT AUTHORS ````````````````````````````````
// ===============================================================================

/// Implementation of the [`ElectAuthors`] trait for the pallet internal type
/// (not exposable).
///
/// This implementation bridges the generic [`ElectAuthors`] abstraction
/// with the pallet's internal affidavit and role-management infrastructure,
/// coordinating **candidate selection**, **time-gated election execution**,
/// and **result revelation** for upcoming sessions.
///
/// ## Design Notes
/// - Elections are **session-scoped** and always target the *upcoming* session.
/// - Only authors who have successfully submitted affidavits are eligible.
/// - All election logic is **time-gated** and derived from session timing,
///   affidavit windows, and election offsets.
/// - This layer is **deterministic**; it does not perform probabilistic or
///   stateful election logic.
///
/// ## Implementation Notes
/// - This implementation does **not** execute the election algorithm itself.
/// - All ranking, scoring, and selection logic is delegated to the configured
///   [`ElectionManager`] via the pallet's [`Config::ElectionAdapter`].
///
/// - This layer is responsible only for:
///   - Validating election timing
///   - Preparing candidate inputs
///   - Revealing results from the election manager
impl<T: Config> ElectAuthors<AuthorOf<T>, ElectionVia<T>> for Internals<T> {
    /// Type representing the prepared election candidates.
    ///
    /// Typically a vector of author's ID and their corresponding election weights.
    type Candidates = ElectionParams<T>;

    /// Type representing the final elected author set.
    ///
    /// Typically a vector of author IDs.
    type Elected = ElectionElects<T>;

    /// Prepares election candidates via the configured election manager.
    ///
    /// - Acts as a thin delegation layer to [`ElectionManager::prepare`].
    /// - Any failure here prevents election execution.
    /// - Typically runs the election algorithm and stores the election result.
    /// - Inconsistencies return explicit errors.
    fn prepare_authors(candidates: Self::Candidates) -> DispatchResult {
        T::ElectionAdapter::prepare(candidates)?;
        Ok(())
    }

    /// Checks whether the election can be processed at the current block.
    ///
    /// ## Parameters
    /// - `runner`: Optional executor of the election (runtime or author-driven).
    ///   This is **not validated here**, but is assumed to be the entity
    ///   responsible for executing the election, as permitted by the caller.
    ///
    /// ## Validation
    /// - Ensures the affidavit window is valid (`start < end`).
    /// - Ensures the current block is within the affidavit window.
    /// - Ensures the election window has started.
    /// - Ensures the election has not yet ended (bounded by affidavit end).
    ///
    /// Violations return explicit, user-facing errors.
    fn can_process_election(_runner: &Option<AuthorOf<T>>) -> DispatchResult {
        // Compute affidavit submission window
        let aff_window = Pallet::<T>::compute_affidavit_window()?;
        let start_affidavit = aff_window.start;
        let end_affidavit = aff_window.end;

        // Validate affidavit window configuration
        let invariant = start_affidavit < end_affidavit;
        debug_assert!(
            invariant,
            "Affidavit submission period is invalid, starts at block {:?} and ends at {:?}",
            start_affidavit, end_affidavit
        );
        ensure!(invariant, Error::<T>::InvalidAffidavitPeriod);

        let current_block = frame_system::Pallet::<T>::block_number();

        // Ensure affidavit window has begun
        ensure!(
            start_affidavit <= current_block,
            Error::<T>::NotAffidavitPeriod
        );

        // Compute election start within affidavit window
        let election_window = Pallet::<T>::compute_election_window()?;
        let start_election = election_window.start;

        // Ensure election has started
        ensure!(
            start_election <= current_block,
            Error::<T>::NotElectionPeriod
        );

        // Ensure election has not ended
        ensure!(
            current_block <= end_affidavit,
            Error::<T>::ElectionPeriodEnded
        );

        Ok(())
    }

    /// Prepares the final list of candidates for election.
    ///
    /// ## Overview
    /// - Iterates all affidavits submitted for the upcoming session.
    /// - Extracts and normalizes each author's election weights.
    /// - Produces a deterministic candidate list for the election manager.
    ///
    /// ## Notes
    /// - Only affidavit-submitting authors are included.
    /// - This function performs **no ranking or filtering**.
    /// - Ordering guarantees are provided by downstream election logic.
    fn prepare_candidates() -> Result<Self::Candidates, DispatchError> {
        let for_session = CurrentSession::<T>::get().saturating_add(One::one());

        // Iterate affidavits for the upcoming session
        let iter = AuthorAffidavits::<T>::iter_prefix((for_session,));

        let mut candidates = Self::Candidates::default();

        for (author, (_, weights)) in iter {
            let mut election_weights = ElectionVia::<T>::default();

            for weight in weights.iter().cloned() {
                election_weights.extend(core::iter::once(weight));
            }

            candidates.extend(core::iter::once((author, election_weights)));
        }
        Ok(candidates)
    }

    /// Reveals the elected authors from the underlying election manager.
    ///
    /// Acts as a thin delegation layer to [`ElectionManager::reveal`].
    ///
    /// ## Failure Semantics
    /// This may return `None` if:
    /// - The election was never executed
    /// - Preparation failed
    /// - Minimum candidate constraints were not met
    ///
    /// ## Caller Responsibility
    /// Callers **must** handle the `None` case gracefully,
    /// typically by retaining the previously elected author set.
    #[inline]
    fn reveal() -> Option<Self::Elected> {
        T::ElectionAdapter::reveal()
    }

    /// Hook invoked after a successful election preparation.
    ///
    /// Emits a [`Event::ElectedInstance`] event if [`Config::EmitEvents`] is `true`.
    fn on_elect_success(runner: &Option<AuthorOf<T>>) {
        let for_session = CurrentSession::<T>::get().saturating_add(One::one());
        let current_block = frame_system::Pallet::<T>::block_number();
        let Some(runner) = runner else {
            debug_assert!(
                false,
                "authors elected for session {:?} at 
                block {:?} but election runner unavailable",
                for_session, current_block
            );
            return;
        };

        #[cfg(not(any(feature = "dev", feature = "runtime-benchmarks")))]
        {
            if T::EmitEvents::get() {
                Pallet::<T>::deposit_event(Event::<T>::ElectedInstance {
                    session: for_session,
                    runner: runner.clone(),
                });
            }
        }

        #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
        {
            if T::EmitEvents::get() {
                let Some(elects) = Self::reveal() else {
                    debug_assert!(
                        false,
                        "authors elected for session {:?} at 
                        block {:?} by election runner {:?}, 
                        but reveal unavailable",
                        runner, for_session, current_block
                    );
                    return;
                };
                Pallet::<T>::deposit_event(Event::<T>::ElectedInstance {
                    session: for_session,
                    runner: runner.clone(),
                    elects,
                });
            }
        }
    }

    /// Hook invoked when an election attempt fails.
    ///
    /// Emits a [`Event::ElectionAttemptFailed`] event if [`Config::EmitEvents`] is `true`.
    fn on_elect_fail(runner: &Option<AuthorOf<T>>, error: DispatchError) {
        let for_session = CurrentSession::<T>::get().saturating_add(One::one());
        let Some(runner) = runner else {
            let current_block = frame_system::Pallet::<T>::block_number();
            debug_assert!(
                false,
                "authors elected for session {:?} at 
                block {:?} but election runner unavailable",
                for_session, current_block
            );
            return;
        };
        if T::EmitEvents::get() {
            Pallet::<T>::deposit_event(Event::<T>::ElectionAttemptFailed {
                session: for_session,
                runner: runner.clone(),
                error,
            });
        }
    }
}

// ===============================================================================
// ```````````````````````````````` AUTHOR POINTS ````````````````````````````````
// ===============================================================================

/// Implementation of the [`AuthorPoints`] trait for the pallet.
///
/// This implementation provides a **session-scoped accounting layer**
/// for tracking and querying abstract points accumulated by authors
/// during active validation.
///
/// Points represent **good behaviour signals**, specifically
/// **block production contributions**, and serve as inputs to downstream
/// reward and incentive mechanisms.
/// They are *not* assets themselves and carry no immediate economic value.
///
/// ## Design Notes
/// - Points are **scoped per session** and never aggregated across sessions.
/// - Accumulation is **monotonic** within a session.
/// - Each point corresponds to a **unit of block production activity**.
/// - Points are intentionally retained after session end for:
///   - Auditability
///   - Historical analysis
///   - Deterministic reward calculation
/// - This layer is **deterministic and side-effect minimal**.
///
/// ## Implementation Notes
/// - This implementation does not perform reward distribution.
/// - Economic interpretation of points is delegated to [`RewardAuthors`].
/// - Clearing of points is intentionally unsupported at this layer.
impl<T: Config> AuthorPoints<AuthorOf<T>, T::Points> for Pallet<T> {
    /// Returns the total accumulated points for an author
    /// in the **current session**.
    ///
    /// ## Semantics
    /// - Points are accumulated incrementally during the session.
    /// - Each point reflects a **block production contribution**.
    /// - Calling this function **mid-session** returns a partial total.
    /// - Calling this function at **session end** yields the final value
    ///   used for reward calculation.
    ///
    /// ## Errors
    /// - Returns `DispatchError` if the author has not accumulated
    ///   any points in the current session.
    fn points_of(author: &AuthorOf<T>) -> Result<T::Points, DispatchError> {
        let current_session = CurrentSession::<T>::get();
        let points = BlockPointsStore::<T>::get((current_session, author))
            .ok_or(Error::<T>::BlockPointsNotFound)?;
        Ok(points)
    }

    /// **No-op method** for clearing accumulated points.
    ///
    /// Point data is retained indefinitely to:
    /// - Preserve full historical traceability
    /// - Support deterministic audits
    /// - Avoid accidental data loss before reward finalization
    ///
    /// Any future clearing, pruning, or archival must be performed
    /// via explicit governance or maintenance extrinsics.
    fn clear_points() {}

    /// Sets the points for an author in the current session.
    ///
    /// ## Semantics
    /// - Overwrites the existing points value for the author.
    /// - Acts as the **primitive storage write** for point updates.
    ///
    /// ## Notes
    /// - Typically used internally by higher-level operations such as
    ///   [`Self::add_point`].
    fn set_points(author: &AuthorOf<T>, points: T::Points) -> DispatchResult {
        let current_session = CurrentSession::<T>::get();
        BlockPointsStore::<T>::insert((current_session, author), points);
        Ok(())
    }

    /// Returns an iterator over all authors and their accumulated points
    /// for the **current session**.
    ///
    /// ## Semantics
    /// - Provides a complete view of the session-scoped points state.
    /// - Includes all authors who have accumulated at least one point.
    /// - The iterator reflects the **current state** and may change as
    ///   new points are added during the session.
    ///
    /// ## Usage
    /// - Intended for runtime operations such as:
    ///   - Reward computation
    ///   - Ranking or selection
    ///   - Performance evaluation
    ///
    /// ## Notes
    /// - Any clearing, pruning, or archival is the responsibility of
    ///   external logic (e.g., governance or maintenance extrinsics).
    fn iter_points() -> impl Iterator<Item = (AuthorOf<T>, T::Points)> {
        let current_session = CurrentSession::<T>::get();
        BlockPointsStore::<T>::iter_prefix((current_session,))
    }
}

// ===============================================================================
// ```````````````````````````````` REWARD AUTHORS ```````````````````````````````
// ===============================================================================

/// Implementation of the [`RewardAuthors`] trait for the pallet internal type
/// (not exposable).
///
/// This implementation bridges **abstract author points** with the
/// protocol's **reward and inflation mechanisms**, translating
/// session-scoped behavioural signals into scheduled economic rewards.
///
/// This layer does **not** mint, transfer, or finalize rewards directly.
/// Instead, it provides deterministic inputs to downstream reward logic
/// owned by the configured [`RoleManager`] adapters.
///
/// ## Design Notes
/// - Rewards are derived from **session-scoped point accumulation**.
/// - Points are interpreted as **relative behavioural weights**, not
///   absolute reward amounts.
/// - The payout context is configurable and may be based on:
///   - Total token issuance (inflation-based) or,
///   - Total backing + collateral stake (stake-weighted)
/// - All reward operations must remain **deterministic, auditable,
///   and reversible** until finalization.
///
/// ## Implementation Notes
/// - This implementation does not compute reward shares.
/// - Reward distribution logic is delegated to:
///   - [`Config::InflationModel`]
///   - [`Config::RewardModel`]
///   - [`CompensateRoles`]
/// - This layer only exposes:
///   - The payout context
///   - The eligible payee set
///   - A scheduling hook for rewards
impl<T: Config> RewardAuthors<AuthorOf<T>, AssetOf<T>, T::Points> for Internals<T> {
    /// Adapter used to query accumulated author points.
    type AuthorPointsAdapter = T::PointsAdapter;

    /// Type representing authors eligible for payout and their points.
    ///
    /// Typically a vector of author's ID and their correspoinding points.
    type PayoutFor = PayoutFor<T>;

    /// Context used by the inflation plugin model.
    type PayoutContext = T::InflationContext;

    /// Inflation plugin model used to derive reward budgets.
    type PayoutModel = T::InflationModel;

    /// Returns the total asset context used to compute rewards.
    ///
    /// ## Semantics
    /// Depending on configuration, this returns:
    /// - Total token issuance (supply-based inflation), or
    /// - Total backing + collateral stake (stake-weighted inflation)
    ///
    /// ## Notes
    /// - This value represents the **upper bound** for reward calculation.
    /// - It does not imply immediate minting or transfer.
    fn payout_via() -> AssetOf<T> {
        // Use total token issuance if inflation is supply-based.
        if T::InflateViaSupply::get() {
            return T::Asset::total_issuance().into();
        }

        // Otherwise, use total locked stake (backing + collateral).
        let backing_stake = T::RoleAdapter::total_backing();
        let collateral_stake = T::RoleAdapter::total_collateral();
        backing_stake.saturating_add(collateral_stake)
    }

    /// Type representing the set of reward payees.
    ///
    /// Typically a vector of author's ID and their
    /// correspoinding reward asset amount.
    type PayeeList = PayeeList<T>;

    /// Context supplied to the reward plugin model.
    type PayeeContext = T::RewardContext;

    /// Reward plugin model used to translate points into payouts.
    type PayeeModel = T::RewardModel;

    /// Schedules a reward for the given author.
    ///
    /// Acts as a thin delegation layer to [`CompensateRoles::reward`].
    ///
    /// ## Semantics
    /// - This function **does not finalize** the reward.
    /// - Rewards are scheduled with best-effort precision.
    /// - Downstream logic may:
    ///   - Aggregate
    ///   - Adjust
    ///   - Revert
    ///   the scheduled reward before finalization.
    ///
    /// ## Errors
    /// Returns a `DispatchError` if reward scheduling fails.
    fn reward(who: &AuthorOf<T>, value: AssetOf<T>) -> DispatchResult {
        T::RoleAdapter::reward(who, value, Precision::BestEffort)?;
        Ok(())
    }

    /// Returns the set of authors eligible for payout and their
    /// accumulated points for the current session.
    ///
    /// ## Notes
    /// - This function is expected to be called **at session end**.
    /// - Calling it earlier may yield partial or unstable results.
    /// - The returned data is treated as immutable for reward computation.
    fn payout_for() -> Self::PayoutFor {
        let iter = Self::AuthorPointsAdapter::iter_points();
        let mut payout_for = Self::PayoutFor::default();
        for (author, points) in iter {
            payout_for.extend(core::iter::once((author, points)));
        }

        payout_for
    }

    /// Hook invoked after a reward is successfully applied to an author.
    ///
    /// This hook emits the `Rewarded` event, reflecting the
    /// distributed reward amount for the given author.
    fn on_reward_success(who: &AuthorOf<T>, value: AssetOf<T>) {
        if T::EmitEvents::get() {
            Pallet::<T>::deposit_event(Event::RewardInitiated {
                author: who.clone(),
                value,
            });
        }
    }

    /// Hook invoked when applying a reward to an author fails.
    ///
    /// This hook emits the `RewardFailed` event, reflecting the
    /// error that prevented the reward from being applied.
    fn on_reward_fail(who: &AuthorOf<T>, error: DispatchError) {
        if T::EmitEvents::get() {
            Pallet::<T>::deposit_event(Event::RewardFailed {
                author: who.clone(),
                error,
            });
        }
    }
}

// ===============================================================================
// ``````````````````````````````` PENALIZE AUTHORS ``````````````````````````````
// ===============================================================================

/// Implementation of the [`PenalizeAuthors`] trait for the pallet internal type
/// (not-exposable).
///
/// This implementation bridges **author offence signals** with the
/// protocol's **penalty and slashing mechanisms**, enabling penalties
/// to be **scheduled and processed** according to runtime-defined rules.
///
/// Penalties, like rewards, follow a **deferred enforcement model**.
/// They are recorded and transformed first, then enforced later by
/// downstream role and penalty management logic.
///
/// ## Design Notes
/// - Penalties are **author-scoped** and apply to active roles.
/// - Enforcement is **scheduled**, not immediate.
/// - Multiple penalties may be:
///   - Aggregated
///   - Scaled
///   - Capped
///   - Reverted
///   prior to final enforcement.
/// - Penalty values are interpreted as **inputs**, not final amounts.
/// - Transformation and enforcement are governed by runtime-configured
///   penalty models for flexibility and governance control.
///
/// ## Implementation Notes
/// - This layer does **not** detect offences or compute severity.
/// - It does **not** finalize or immediately apply penalties.
/// - All penalty logic is delegated to:
///   - [`Config::PenaltyModel`]
///   - [`CompensateRoles::penalize`]
/// - This implementation guarantees deterministic, auditable scheduling
///   of penalties without side effects.
impl<T: Config> PenalizeAuthors<AuthorOf<T>, PenaltyOf<T>> for Internals<T> {
    /// Mapping of authors to their applied penalties (percentage typically).
    type PenaltyFor = PenaltyFor<T>;

    /// Context provided to the penalty plugin model for transformation.
    type PenaltyContext = T::PenaltyContext;

    /// Plugin Model responsible for transforming raw penalties according to
    /// runtime-defined rules (e.g. caps, scaling, thresholds).
    type PenaltyModel = T::PenaltyModel;

    /// Applies a penalty to the given author.
    ///
    /// Acts as a thin delegation layer to [`CompensateRoles::penalize`].
    ///
    /// ## Semantics
    /// - Penalties are **scheduled**, not applied immediately.
    /// - Downstream logic may:
    ///   - Aggregate multiple penalties
    ///   - Scale or cap penalties
    ///   - Delay or revert enforcement prior to finalization
    ///
    /// ## Notes
    /// - This function does not persist offence metadata.
    /// - Offence detection and validation are the responsibility
    ///   of the caller.
    ///
    /// ## Errors
    /// Returns a `DispatchError` if penalty scheduling fails.
    fn penalize(who: &AuthorOf<T>, penalty: PenaltyOf<T>) -> DispatchResult {
        <T::RoleAdapter as CompensateRoles<AuthorOf<T>>>::penalize(who, penalty)?;
        Ok(())
    }

    /// Hook invoked after a penalty is successfully applied to an author.
    ///
    /// This hook emits the `Penalized` event, reflecting the
    /// penalty enforced against the author.
    fn on_penalty_success(who: &AuthorOf<T>, penalty: PenaltyOf<T>) {
        if T::EmitEvents::get() {
            Pallet::<T>::deposit_event(Event::<T>::PenaltyInitiated {
                author: who.clone(),
                penalty,
            });
        }
    }

    /// Hook invoked when applying a penalty to an author fails.
    ///
    /// This hook emits the `PenaltyFailed` event, reflecting the
    /// error that prevented the penalty from being applied.
    fn on_penalty_fail(who: &AuthorOf<T>, error: DispatchError) {
        if T::EmitEvents::get() {
            Pallet::<T>::deposit_event(Event::<T>::PenaltyFailed {
                author: who.clone(),
                error,
            });
        }
    }
}

// ===============================================================================
// ````````````````````````````` ELECTION AFFIDAVITS `````````````````````````````
// ===============================================================================

/// Implementation of the [`ElectionAffidavits`] trait for the pallet.
///
/// This implementation bridges the generic [`ElectionAffidavits`] abstraction
/// with the pallet's internal affidavit registry ([`AuthorAffidavits`] &
/// [`AffidavitKeys`]), enabling authors to **self-report their election weights**
/// for upcoming sessions.
///
/// ## Design Notes
/// - **Affidavit submission** is only allowed when [`AllowAffidavits`] is enabled.
/// - Affidavits are stored *per session*, not globally, ensuring clean rotation.
/// - **Time gating** is enforced through [`AffidavitBeginsAt`](crate::AffidavitBeginsAt) and 
/// [`AffidavitEndsAt`](crate::AffidavitEndsAt), relative to average session length.
/// - Affidavit data is immutable within its session once the submission period ends.
/// - All operations must remain audit-safe and deterministic.
///
/// ## Implementation Notes
/// This bridge layer does not perform any ranking, scoring, or weighting logic.
/// Those responsibilities remain with the [`ElectAuthors`] and [`ElectionManager`]
/// implementations. The affidavit simply represents a **candidate's declaration**
/// of intent and associated metrics for the next election round.
impl<T: Config> ElectionAffidavits<AffidavitId<T>, ElectionVia<T>> for Pallet<T> {
    /// Checks whether an author can submit an affidavit for the upcoming session-election.
    ///
    /// - The global [`AllowAffidavits`] flag is enabled.
    /// - The current block is within the configured affidavit submission window.
    ///
    /// DispatchError otherwise
    fn can_submit_affidavit(who: &AffidavitId<T>) -> DispatchResult {
        // Check if Affidavit model is initiated
        ensure!(
            AllowAffidavits::<T>::get(),
            Error::<T>::AffidavitsNotAllowed
        );

        // Check if the author exists for the affidavit key ID
        let for_session = CurrentSession::<T>::get().saturating_add(One::one());
        let Some(author) = AffidavitKeys::<T>::get((for_session, who)) else {
            let try_next_session =
                AffidavitKeys::<T>::contains_key((for_session.saturating_add(One::one()), who));
            ensure!(
                !try_next_session,
                Error::<T>::DeclareDuringNextAffidavitSession
            );
            return Err(Error::<T>::AffidavitAuthorNotFound.into());
        };

        <T::RoleAdapter as RoleManager<AuthorOf<T>>>::is_available(&author)?;

        // Compute allowed submission window relative to session timing.
        let aff_window = Pallet::<T>::compute_affidavit_window()?;
        let start_block = aff_window.start;
        let end_block = aff_window.end;

        let current_block = frame_system::Pallet::<T>::block_number();

        // Ensure affidavit period has started
        ensure!(start_block <= current_block, Error::<T>::NotAffidavitPeriod);

        // Ensure affidavit period has not ended
        ensure!(current_block <= end_block, Error::<T>::AffidavitPeriodEnded);

        Ok(())
    }

    /// Submits a new affidavit for the next session.
    ///
    /// Directly inserts the affidavit into storage for the upcoming session.
    ///
    /// ## Details
    /// - Persists the affidavit under the next session's affidavits mapping.
    /// - Overwrites any previously submitted affidavit for the same session.
    /// - Each author can maintain **only one recent affidavit per future session**.
    fn submit_affidavit(who: &AffidavitId<T>, affidavit: &ElectionVia<T>) -> DispatchResult {
        let for_session = CurrentSession::<T>::get().saturating_add(One::one());
        let author = AffidavitKeys::<T>::get((for_session, who))
            .ok_or(Error::<T>::AffidavitAuthorNotFound)?;
        let current_block = frame_system::Pallet::<T>::block_number();
        let mut try_affidavit: Vec<ElectionWeight<T>> = affidavit.clone().into_iter().collect();
        let result = WeakBoundedVec::<ElectionWeight<T>, T::MaxAffidavitWeights>::try_from(
            try_affidavit.clone(),
        );
        let actual_affidavit = match result {
            Ok(v) => v,
            Err(_) => {
                // Sort in descending order
                try_affidavit.sort_by(|a, b| b.cmp(a));
                WeakBoundedVec::<ElectionWeight<T>, T::MaxAffidavitWeights>::force_from(
                    try_affidavit,
                    None,
                )
            }
        };
        AuthorAffidavits::<T>::insert((for_session, author), (current_block, actual_affidavit));
        Ok(())
    }

    /// Generates an affidavit dynamically for the given author's affidavit ID.
    ///
    /// ## Overview
    /// - Inspects abstract weight via [`InspectWeight`] from [`Config::ElectionAdapter`].
    /// - Produces an [`ElectionVia`] structure that represents the
    ///   author's self-declared election weights.
    ///
    /// ## Returns
    /// - `Ok(ElectionVia)` on success.
    /// - DispatchError otherwise
    fn gen_affidavit(who: &AffidavitId<T>) -> Result<ElectionVia<T>, DispatchError> {
        let for_session = CurrentSession::<T>::get().saturating_add(One::one());
        let author = AffidavitKeys::<T>::get((for_session, who))
            .ok_or(Error::<T>::AffidavitAuthorNotFound)?;
        let weights =
            <T::ElectionAdapter as InspectWeight<AuthorOf<T>, ElectionVia<T>>>::weight_of(&author)?;
        Ok(weights.into())
    }

    /// Removes an existing upcoming-election affidavit for the given author.
    ///
    /// ## Workflow
    /// 1. Ensures the affidavit exists.
    /// 2. Removes it from storage for the next session.
    ///
    /// ## Notes
    /// - Used primarily when an author wishes to withdraw from election participation.
    fn remove_affidavit(who: &AffidavitId<T>) -> DispatchResult {
        Self::affidavit_exists(who)?;
        let for_session = CurrentSession::<T>::get().saturating_add(One::one());
        let author = AffidavitKeys::<T>::get((for_session, who))
            .ok_or(Error::<T>::AffidavitAuthorNotFound)?;
        AuthorAffidavits::<T>::remove((for_session, author));
        Ok(())
    }

    /// Retrieves an affidavit for the given author for the next session's election.
    ///
    /// ## Returns
    /// - The [`ElectionVia`] structure associated with the author.
    /// - DispatchError if no affidavit is stored for the next session election.
    fn get_affidavit(who: &AffidavitId<T>) -> Result<ElectionVia<T>, DispatchError> {
        let for_session = CurrentSession::<T>::get().saturating_add(One::one());
        let author = AffidavitKeys::<T>::get((for_session, who))
            .ok_or(Error::<T>::AffidavitAuthorNotFound)?;
        let (_, affidavit) = AuthorAffidavits::<T>::get((for_session, author))
            .ok_or(Error::<T>::AffidavitNotFound)?;
        Ok(affidavit.into_iter().collect())
    }

    /// Checks if an affidavit exists for the given author for the upcoming election.
    ///
    /// ## Returns
    /// - `Ok(())` if the affidavit exists.
    /// - DispatchError otherwise.
    fn affidavit_exists(who: &AffidavitId<T>) -> DispatchResult {
        let for_session = CurrentSession::<T>::get().saturating_add(One::one());
        let author = AffidavitKeys::<T>::get((for_session, who))
            .ok_or(Error::<T>::AffidavitAuthorNotFound)?;
        ensure!(
            AuthorAffidavits::<T>::contains_key((for_session, author)),
            Error::<T>::AffidavitNotFound
        );
        Ok(())
    }

    /// No-op method.
    ///
    /// This low-level implementation is intentionally left empty.
    ///
    /// Affidavit clearing is deferred to higher-level logic to:
    /// - Preserve full **historical traceability**.
    /// - Prevent accidental data loss before election finalization.
    ///
    /// ## Notes
    /// - The pallet should query affidavits per session **only once**.
    /// - Re-querying beyond this point can cause election inconsistencies.
    /// - Reserved for potential audit or archival extensions.
    fn clear_affidavits() {}

    /// Hook invoked after a successful affidavit submission.
    ///
    /// This hook emits the `AffidavitSubmitted` event, reflecting
    /// the submitted election weight for the author.
    fn on_submit_affidavit(who: &AffidavitId<T>, _affidavit: &ElectionVia<T>) {
        if T::EmitEvents::get() {
            let for_session = CurrentSession::<T>::get().saturating_add(One::one());
            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            {
                let Some(author) = AffidavitKeys::<T>::get((for_session, who)) else {
                    return;
                };
                let affidavit = _affidavit;
                Self::deposit_event(Event::<T>::AffidavitSubmitted {
                    afdt_id: who.clone(),
                    session: for_session,
                    author,
                    affidavit: affidavit.clone(),
                });
            }
            #[cfg(not(any(feature = "dev", feature = "runtime-benchmarks")))]
            {
                Self::deposit_event(Event::<T>::AffidavitSubmitted {
                    afdt_id: who.clone(),
                    session: for_session,
                });
            }
        }
    }
}