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

// ===============================================================================
// ```````````````````````````````` SESSION TESTS ````````````````````````````````
// ===============================================================================

#[cfg(test)]
mod tests {

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` IMPORTS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    // --- Local crate imports ---
    use crate::mock::*;

    // --- FRAME Suite ---
    use frame_suite::{blockchain::*, roles::*};

    // --- FRAME Support ---
    use frame_support::traits::tokens::{Fortitude, Precision};

    // --- External pallets ---
    use pallet_session::SessionManager;

    // --- Substrate primitives ---
    use sp_runtime::traits::Convert;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` CONVERT ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn convert_returns_some() {
        chain_manager_test_ext().execute_with(|| {
            set_user_balance_and_hold(ALICE, 250, 200).unwrap();
            RoleAdapter::enroll(&ALICE, 150, Fortitude::Force).unwrap();
            let session_id = Pallet::convert(ALICE);
            assert!(session_id.is_some());
        })
    }

    #[test]
    fn convert_returns_none() {
        chain_manager_test_ext().execute_with(|| {
            set_user_balance_and_hold(ALICE, 250, 200).unwrap();
            RoleAdapter::enroll(&ALICE, 150, Fortitude::Force).unwrap();
            let session_id = Pallet::convert(BOB);
            assert!(session_id.is_none());
            dbg!(session_id);
        })
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````` SESSION-MANAGER ```````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn new_session_returns_authors_and_awards_election_runner_points() {
        chain_manager_test_ext().execute_with(|| {
            CurrentSession::put(0);
            set_user_balance_and_hold(ALICE, 250, 250).unwrap();
            set_user_balance_and_hold(CHARLIE, 250, 250).unwrap();
            set_user_balance_and_hold(ALAN, 250, 250).unwrap();
            set_user_balance_and_hold(MIKE, 250, 250).unwrap();
            set_user_balance_and_hold(BOB, 250, 250).unwrap();
            set_user_balance_and_hold(NIX, 250, 250).unwrap();

            RoleAdapter::enroll(&ALICE, 200, Fortitude::Force).unwrap();
            RoleAdapter::enroll(&BOB, 200, Fortitude::Force).unwrap();
            RoleAdapter::enroll(&MIKE, 200, Fortitude::Force).unwrap();

            RoleAdapter::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                100,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();
            RoleAdapter::fund(
                &BOB,
                &Funder::Direct(ALAN),
                150,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();
            RoleAdapter::fund(
                &MIKE,
                &Funder::Direct(NIX),
                125,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            AffidavitKeys::insert((1, AFFIDAVIT_KEY_A), ALICE);
            AffidavitKeys::insert((1, AFFIDAVIT_KEY_B), BOB);
            AffidavitKeys::insert((1, AFFIDAVIT_KEY_C), MIKE);

            let affidavit_alice = Pallet::gen_affidavit(&AFFIDAVIT_KEY_A).unwrap();
            Pallet::submit_affidavit(&AFFIDAVIT_KEY_A, &affidavit_alice).unwrap();
            let affidavit_bob = Pallet::gen_affidavit(&AFFIDAVIT_KEY_B).unwrap();
            Pallet::submit_affidavit(&AFFIDAVIT_KEY_B, &affidavit_bob).unwrap();
            let affidavit_mike = Pallet::gen_affidavit(&AFFIDAVIT_KEY_C).unwrap();
            Pallet::submit_affidavit(&AFFIDAVIT_KEY_C, &affidavit_mike).unwrap();

            let candidates = Internals::prepare_candidates().unwrap();
            Internals::prepare_authors(candidates).unwrap();

            // Set up election runner for session 1
            ElectsPreparedBy::insert(1, (ALICE, 100));
            ElectionRunnerPoints::set(50);

            // Simulate new_session call
            let result = Pallet::new_session(1);

            // Verify that authors were returned
            assert!(result.is_some());

            // Election runner received bonus points for session 0
            let alice_points = PointsAdapter::points_of(&ALICE).unwrap();
            assert_eq!(alice_points, 50);
        })
    }

    #[test]
    fn new_session_returns_none_when_reveal_fails() {
        chain_manager_test_ext().execute_with(|| {
            // No election setup, so reveal will fail
            let result = Pallet::new_session(1);
            assert!(result.is_none());
        })
    }

    #[test]
    fn end_session_rewards_authors() {
        chain_manager_test_ext().execute_with(|| {
            CurrentSession::put(1);
            set_user_balance_and_hold(ALICE, 250, 250).unwrap();
            set_user_balance_and_hold(BOB, 250, 250).unwrap();
            set_user_balance_and_hold(ALAN, 250, 250).unwrap();
            set_user_balance_and_hold(MIKE, 250, 250).unwrap();

            System::set_block_number(5);
            RoleAdapter::enroll(&ALICE, 200, Fortitude::Force).unwrap();
            RoleAdapter::enroll(&BOB, 150, Fortitude::Force).unwrap();
            RoleAdapter::fund(
                &ALICE,
                &Funder::Direct(ALAN),
                150,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();
            RoleAdapter::fund(
                &BOB,
                &Funder::Direct(MIKE),
                125,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            Pallet::add_point(&ALICE).unwrap();
            Pallet::add_point(&ALICE).unwrap();
            Pallet::add_point(&BOB).unwrap();
            Pallet::add_point(&BOB).unwrap();
            Pallet::add_point(&BOB).unwrap();
            Pallet::add_point(&ALICE).unwrap();
            Pallet::add_point(&ALICE).unwrap();
            Pallet::add_point(&ALICE).unwrap();

            ElectionRunnerPointsUpgrade::put(Some(50));
            let election_runner_points = ElectionRunnerPoints::get();
            assert_eq!(election_runner_points, 10);

            System::set_block_number(590);
            Pallet::end_session(1);

            let election_runner_points = ElectionRunnerPoints::get();
            assert_eq!(election_runner_points, 50);
            assert!(ElectionRunnerPointsUpgrade::get().is_none());

            let rewards_of_alice = RoleAdapter::get_rewards_of(&ALICE).unwrap();
            let rewards_of_bob = RoleAdapter::get_rewards_of(&BOB).unwrap();

            let expected_alice_rewards = vec![(592, 62)];
            let expected_bob_rewards = vec![(592, 38)];

            assert_eq!(rewards_of_alice, expected_alice_rewards);
            assert_eq!(rewards_of_bob, expected_bob_rewards);
        })
    }

    #[test]
    fn end_session_does_not_upgrade_when_none() {
        chain_manager_test_ext().execute_with(|| {
            ElectionRunnerPoints::set(50);
            ElectionRunnerPointsUpgrade::set(None);

            Pallet::end_session(1);

            assert_eq!(ElectionRunnerPoints::get(), 50);
            assert_eq!(ElectionRunnerPointsUpgrade::get(), None);
        })
    }

    #[test]
    fn start_session_updates_current_session_and_block_number() {
        chain_manager_test_ext().execute_with(|| {
            System::set_block_number(500);
            CurrentSession::put(0);
            SessionStartsAt::put(0);

            Pallet::start_session(5);

            assert_eq!(CurrentSession::get(), 5);
            assert_eq!(SessionStartsAt::get(), 500);
        })
    }
}
