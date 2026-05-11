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
// ````````````````````````````` SESSION MANAGEMENT ``````````````````````````````
// ===============================================================================

//! Implements [`SessionManager`] for [`Pallet`].
//!
//! Session management logic for author rotation, reward settlement,
//! and session boundary coordination.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{
    types::*, Config, CurrentSession, ElectionRunnerPoints, ElectionRunnerPointsUpgrade,
    ElectsPreparedBy, Internals, Pallet, SessionStartAt,
};

// --- FRAME Suite ---
use frame_suite::{blockchain::*, roles::RoleManager};

// --- External pallets ---
use pallet_session::SessionManager;

// --- Substrate primitives ---
use sp_runtime::{
    traits::{Convert, Saturating, Zero},
    Vec,
};

// ===============================================================================
// ```````````````````````````` SESSION-ID CONVERSION ````````````````````````````
// ===============================================================================

impl<T: Config> Convert<AuthorOf<T>, Option<SessionId<T>>> for Pallet<T> {
    /// Converts a valid `Author` to an `Result<SessionId, DispatchError>`
    ///
    /// None if no SessionId found
    fn convert(a: AuthorOf<T>) -> Option<SessionId<T>> {
        // Verify that the author has an existing role
        let Ok(_) = <T::RoleAdapter as RoleManager<AuthorOf<T>>>::role_exists(&a) else {
            return None;
        };
        let Ok(id) = a.try_into() else { return None };
        Some(id)
    }
}

// ===============================================================================
// ``````````````````````````````` SESSION MANAGER ```````````````````````````````
// ===============================================================================

/// Implementation of [`SessionManager`] for the pallet.
///
/// This implementation integrates **author election**, **reward settlement**,
/// and **session boundary tracking** into Substrate's session lifecycle.
///
/// It acts as the coordination layer between:
/// - Election resolution ([`ElectAuthors`])
/// - Reward scheduling ([`RewardAuthors`])
/// - Session metadata management ([`CurrentSession`], [`SessionStartAt`])
///
/// ## Design Notes
/// - Elections always target the *next* session.
/// - Rewards are settled for the *ending* session.
/// - Session state is updated deterministically at boundaries.
/// - No election logic or reward computation is performed here.
///
/// ## Implementation Notes
/// - This implementation assumes election results are already finalized
///   before `new_session` is invoked.
/// - All side effects are **session-boundary safe** and audit-friendly.
impl<T: Config> SessionManager<AuthorOf<T>> for Pallet<T> {
    /// Prepares the author set for the upcoming session.
    ///
    /// ## Workflow
    /// - Reveal elected authors via [`ElectAuthors::reveal`].
    /// - Reward the election runner with additional block points
    ///   for the *previous* session.
    ///
    /// ## Semantics
    /// - Returns `None` if:
    ///   - No election was executed, or
    ///   - The elected author set is empty.
    /// - Election runner rewards are credited to the session
    ///   that is ending (`new_index - 1`).
    fn new_session(new_index: SessionIndex) -> Option<Vec<AuthorOf<T>>> {
        // Reveal the elected authors for the upcoming session.
        let Some(authors) = <Internals<T> as ElectAuthors<AuthorOf<T>, ElectionVia<T>>>::reveal()
        else {
            return None;
        };

        // Materialize the elected set into a concrete vector.
        let mut elected: Vec<AuthorOf<T>> = Vec::new();
        for author in authors.into_iter() {
            elected.push(author);
        }

        // Abort if the election yielded no authors.
        if elected.is_empty() {
            return None;
        }

        // Reward the election runner in the session that is going to end.
        if let Some((ref runner, _block)) = ElectsPreparedBy::<T>::get(new_index) {
            let runner_points = ElectionRunnerPoints::<T>::get();
            let points = T::PointsAdapter::points_of(runner).unwrap_or(Zero::zero());
            let _ = T::PointsAdapter::set_points(runner, points.saturating_add(runner_points));
        }

        Some(elected)
    }

    /// Finalizes the ending session.
    ///
    /// ## Workflow
    /// - Schedule rewards for authors based on accumulated points.
    /// - Apply any pending configuration upgrades related to
    ///   election runner incentives.
    ///
    /// ## Notes
    /// - Reward scheduling is deferred; no immediate transfers occur.
    /// - Configuration upgrades take effect atomically at session end.
    fn end_session(_end_index: SessionIndex) {
        // Schedule rewards for authors of the ending session.
        <Internals<T> as RewardAuthors<AuthorOf<T>, AssetOf<T>, T::Points>>::reward_authors();

        // Apply pending election runner point upgrades, if any.
        if let Some(update) = ElectionRunnerPointsUpgrade::<T>::get() {
            ElectionRunnerPoints::<T>::set(update);
            ElectionRunnerPointsUpgrade::<T>::set(None);
        }
    }

    /// Initializes metadata for the newly started session.
    ///
    /// ## Workflow
    /// - Record the new session index.
    /// - Capture the block number at which the session begins.
    ///
    /// ## Invariants
    /// - Must be called exactly once per session start.
    /// - Session metadata is monotonic and never rewritten.
    fn start_session(start_index: SessionIndex) {
        CurrentSession::<T>::set(start_index);
        SessionStartAt::<T>::set(frame_system::Pallet::<T>::block_number());
    }
}