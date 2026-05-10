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
// `````````````````````````` AUTHORS INTEGRATION TESTS ``````````````````````````
// ===============================================================================

//! **Integration tests for the Authors pallet.**
//!
//! Integration tests covering author lifecycle, funding, rewards,
//! penalties, and role state transitions.

#[cfg(test)]
mod tests {

    // ===============================================================================
    // ```````````````````````````````````` IMPORTS ``````````````````````````````````
    // ===============================================================================

    // --- Local crate imports ---
    use crate::{
        mock::*,
        types::{AuthorStatus, Funder},
    };

    // --- FRAME Suite ---
    use frame_suite::roles::*;

    // --- FRAME Support ---
    use frame_support::{
        assert_err, assert_ok,
        traits::{
            tokens::{Fortitude, Precision},
            Hooks,
        },
    };

    // --- Substrate primitives ---
    use sp_runtime::Perbill;

    // ===============================================================================
    // `````````````````````````````````` LIFECYCLE ``````````````````````````````````
    // ===============================================================================

    #[test]
    fn author_lifecycle_with_proportional_reward_distribution() {
        authors_test_ext().execute_with(|| {
            // --------------------------------------------------------------------
            // 1. Initialize accounts with balances and holds
            // --------------------------------------------------------------------

            // ALICE will act as the Author
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            // Backers funding the author
            initiate_key_and_set_balance_and_hold(&CHARLIE, INITIAL_BALANCE, STANDARD_HOLD)
                .unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            // Move chain to a non-zero block
            System::set_block_number(8);

            // Author does not exist before enrollment
            assert_err!(Pallet::role_exists(&ALICE), Error::AuthorNotFound);
            // No author collateral exists globally
            assert_eq!(
                Pallet::total_collateral(),
                0 // no collateral yet
            );

            // --------------------------------------------------------------------
            // 2. Author enrollment with initial collateral
            // --------------------------------------------------------------------

            assert_ok!(Pallet::enroll(&ALICE, 100, Fortitude::Force));
            // Author now exists
            assert_ok!(Pallet::role_exists(&ALICE));
            // Collateral is locked correctly
            let actual_collateral = Pallet::get_collateral(&ALICE).unwrap();
            assert_eq!(actual_collateral, 100);
            // Enrollment timestamp recorded
            let enroll_since = Pallet::enroll_since(&ALICE).unwrap();
            assert_eq!(enroll_since, 8);
            // Author starts in Probation
            let current_status = Pallet::get_status(&ALICE).unwrap();
            assert_eq!(current_status, AuthorStatus::Probation);
            // Global collateral reflects author's collateral
            let current_total_collateral = Pallet::total_collateral();
            assert_eq!(current_total_collateral, 100);

            // --------------------------------------------------------------------
            // 3. Probation -> Active transition after probation period
            // --------------------------------------------------------------------

            // probation period not elapsed yet
            System::set_block_number(17); // status_since + probation_period > current_block
            assert_err!(
                Pallet::set_status(&ALICE, AuthorStatus::Active,),
                Error::AuthorInProbation
            );

            // Probation period elapsed
            System::set_block_number(18); // status_since + probation_period <= current_block
            assert_ok!(Pallet::set_status(&ALICE, AuthorStatus::Active,));
            let current_status = Pallet::get_status(&ALICE).unwrap();
            assert_eq!(current_status, AuthorStatus::Active);

            // --------------------------------------------------------------------
            // 4. Backers fund the author
            // --------------------------------------------------------------------

            // No funds yet
            assert_err!(Pallet::has_funds(&ALICE), Error::FundDoesNotExist);
            assert_eq!(Pallet::total_backing(), 0);

            System::set_block_number(20);
            // CHARLIE backs ALICE with 50
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                50,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            // ALAN backs ALICE with 100
            Pallet::fund(
                &ALICE,
                &Funder::Direct(ALAN),
                100,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();
            // // Backers' hold balances updated
            assert_eq!(get_user_hold_balance(&CHARLIE), 200);
            assert_eq!(get_user_hold_balance(&ALAN), 150);

            // Funding reflected on author
            assert_ok!(Pallet::has_funds(&ALICE));
            let alice_backing = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(alice_backing, 150);
            // Backer list matches expectations
            let mut alice_backers = Pallet::backers_of(&ALICE).unwrap();
            let mut expected_backers =
                vec![(Funder::Direct(ALAN), 100), (Funder::Direct(CHARLIE), 50)];
            alice_backers.sort();
            expected_backers.sort();
            assert_eq!(alice_backers, expected_backers);
            // Total backing equals sum of all funders
            let current_total_backing = Pallet::total_backing();
            assert_eq!(current_total_backing, 150);
            // Author hold = collateral + funding
            let alice_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_hold, 250);

            // --------------------------------------------------------------------
            // 5. Partial withdrawal: CHARLIE exits
            // --------------------------------------------------------------------

            // CHARLIE withdraws
            System::set_block_number(24);
            Pallet::draw(&ALICE, &Funder::Direct(CHARLIE)).unwrap();
            // CHARLIE gets his funds back
            assert_eq!(get_user_balance(&CHARLIE), 150);
            // Only ALAN remains as backer
            let alice_backers = Pallet::backers_of(&ALICE).unwrap();
            let expected_backers = vec![(Funder::Direct(ALAN), 100)];
            assert_eq!(alice_backers, expected_backers);
            let alice_backing = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(alice_backing, 100);
            // Total backing updated
            let current_total_backing = Pallet::total_backing();
            assert_eq!(current_total_backing, 100);
            // Author hold updated
            let alice_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_hold, 200);

            // --------------------------------------------------------------------
            // 6. Reward scheduling and enforcement
            // --------------------------------------------------------------------

            System::set_block_number(25);
            // Reward allocated
            let reward_timestamp = Pallet::reward(&ALICE, 10, Precision::BestEffort).unwrap();
            assert_eq!(reward_timestamp, 27); // current_block + reward_buffer -> 25 + 2

            // Reward exists but is not yet applied
            assert_ok!(Pallet::has_reward(&ALICE));
            let rewards_of_alice = Pallet::get_rewards_of(&ALICE).unwrap();
            assert_eq!(rewards_of_alice, vec![(27, 10)]);

            let rewards_on = Pallet::get_rewards_on(27).unwrap();
            assert_eq!(rewards_on, vec![(ALICE, 10)]);

            // Reward applied at start of block 27
            System::set_block_number(27);
            Pallet::on_initialize(27);
            // Hold increases by reward
            let alice_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_hold, 210); // existing_hold + reward -> 200 + 10

            // Reward split proportionally:
            // - Funding: 100 / 200 => 50% -> 5
            // - Collateral: 100 / 200 => 50% -> 5
            assert_eq!(Pallet::total_collateral(), 105);
            // Reward no longer pending
            assert_err!(Pallet::has_reward(&ALICE), Error::RewardNotFound);
            assert_ok!(Pallet::get_rewards_of(&ALICE), vec![]);

            // --------------------------------------------------------------------
            // 7. Penalty scheduling and forgiveness
            // --------------------------------------------------------------------

            System::set_block_number(30);
            // Penalty allocated
            let penalty_timestamp = Pallet::penalize(&ALICE, Perbill::from_percent(2)).unwrap();
            assert_eq!(penalty_timestamp, 34); // current_block + penalties_buffer -> 25 + 4

            // Penalty exists but is not yet applied
            assert_ok!(Pallet::has_penalty(&ALICE));
            let penalties_of_alice = Pallet::get_penalties_of(&ALICE).unwrap();
            assert_eq!(penalties_of_alice, vec![(34, Perbill::from_percent(2))]);

            let rewards_on = Pallet::get_penalties_on(penalty_timestamp).unwrap();
            assert_eq!(rewards_on, vec![(ALICE, Perbill::from_percent(2))]);
            // Penalty forgiven before finalization
            System::set_block_number(32);
            assert_ok!(Pallet::forgive(&ALICE, penalty_timestamp));
            assert_err!(Pallet::has_penalty(&ALICE), Error::PenaltyNotFound);
            assert_ok!(Pallet::get_penalties_of(&ALICE), vec![]);

            // --------------------------------------------------------------------
            // 8. Final backer exits
            // --------------------------------------------------------------------

            Pallet::draw(&ALICE, &Funder::Direct(ALAN)).unwrap();
            // ALAN receives:
            // - backing (100)
            // - his share of reward (5)
            assert_eq!(get_user_balance(&ALAN), 205);

            let alice_backing = Pallet::backed_value(&ALICE).unwrap();
            assert_eq!(alice_backing, 0);

            let alice_backers = Pallet::backers_of(&ALICE).unwrap();
            let expected_backers = vec![];
            assert_eq!(alice_backers, expected_backers);

            let current_total_backing = Pallet::total_backing();
            assert_eq!(current_total_backing, 0);

            let alice_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_hold, 105);

            // --------------------------------------------------------------------
            // 9. Author resigns and receives remaining collateral + rewards
            // --------------------------------------------------------------------

            Pallet::resign(&ALICE).unwrap();
            let current_status = Pallet::get_status(&ALICE).unwrap();
            assert_eq!(current_status, AuthorStatus::Resigned);
            // Author receives remaining collateral + reward share
            assert_eq!(get_user_balance(&ALICE), 205);
            // Resigned authors are unavailable
            assert_err!(Pallet::is_available(&ALICE), Error::AuthorResigned);
        })
    }

    // ===============================================================================
    // ```````````````````````````````` COMPENSATIONS ````````````````````````````````
    // ===============================================================================

    #[test]
    fn penalty_and_reward_resolved_before_resigning() {
        authors_test_ext().execute_with(|| {
            // --------------------------------------------------------------------
            // 1. Initialize author and a single funder
            // --------------------------------------------------------------------

            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, INITIAL_BALANCE, STANDARD_HOLD)
                .unwrap();

            // --------------------------------------------------------------------
            // 2. Author enrollment and activation
            // --------------------------------------------------------------------

            System::set_block_number(10);
            // Enroll ALICE with 100 units of collateral
            assert_ok!(Pallet::enroll(&ALICE, 100, Fortitude::Force));

            System::set_block_number(20);
            // Advance past probation and activate the author
            assert_ok!(Pallet::set_status(&ALICE, AuthorStatus::Active));

            // --------------------------------------------------------------------
            // 3. Backers fund the author
            // --------------------------------------------------------------------

            // CHARLIE backs ALICE with 100 units
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                100,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            // At this point:
            // - ALICE collateral = 100
            // - Funding = 100
            // - Total hold = 200
            let alice_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_hold, 200);

            // --------------------------------------------------------------------
            // 4. Penalty Schedule (deferred enforcement)
            // --------------------------------------------------------------------
            System::set_block_number(22);

            // Scheduled a 10% penalty at block 22
            // Penalty buffer causes it to be applied at block 26
            let penalty_timestamp = Pallet::penalize(&ALICE, Perbill::from_percent(10)).unwrap();
            assert_eq!(penalty_timestamp, 26);

            // --------------------------------------------------------------------
            // 5. Reward scheduled after the penalty
            // --------------------------------------------------------------------

            // Scheduled a reward of 20 at block 25
            // Reward buffer causes it to be applied at block 27
            System::set_block_number(25);
            let reward_timestamp_a = Pallet::reward(&ALICE, 20, Precision::BestEffort).unwrap();
            assert_eq!(reward_timestamp_a, 27);

            // --------------------------------------------------------------------
            // 6. Attempt resignation while a penalty is still pending
            // --------------------------------------------------------------------

            // Author attempts to resign before penalty application
            // This must fail to prevent escaping punishment
            assert_err!(Pallet::resign(&ALICE), Error::AuthorHasPenalties);

            // --------------------------------------------------------------------
            // 7. Penalty applied
            // --------------------------------------------------------------------

            System::set_block_number(26);
            // Penalty is enforced at the start of block 26
            Pallet::on_initialize(26);

            // --------------------------------------------------------------------
            // 8. Resignation is now allowed even though reward is still pending
            // --------------------------------------------------------------------

            // After penalties are resolved, resignation is permitted
            // Pending rewards do not block resignation by design
            assert_ok!(Pallet::resign(&ALICE));
            // ALICE receives:
            // - remaining collateral
            // - 100 (existing) + 100 (collateral) - 10 (penalty) -> 190
            assert_eq!(get_user_balance(&ALICE), 190);

            // --------------------------------------------------------------------
            // 9. Reward applied after resignation
            // --------------------------------------------------------------------

            System::set_block_number(27);
            // Reward is applied at the start of block 27
            Pallet::on_initialize(27);

            // --------------------------------------------------------------------
            // 10. Funder exits and receives complete reward
            // --------------------------------------------------------------------
            System::set_block_number(30);
            // CHARLIE withdraws backing after reward finalization
            assert_ok!(Pallet::draw(&ALICE, &Funder::Direct(CHARLIE)));
            // - penalty applied while ALICE was active (-10 share)
            // - reward applied after ALICE resigned (100% goes to funders)
            //
            // 100 (existing) + 100 (backing) - 10 (penalty) + 20 (reward) -> 210.
            // Also Catering to one unit rounding loss
            let b = get_user_balance(&CHARLIE);
            assert!(b == 210 || b == 209);
        })
    }

    #[test]
    fn funding_droped_when_author_collateral_drops_minimum() {
        authors_test_ext().execute_with(|| {
            // --------------------------------------------------------------------
            // 1. Initialize author and backers
            // --------------------------------------------------------------------

            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, INITIAL_BALANCE, STANDARD_HOLD)
                .unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            // --------------------------------------------------------------------
            // 2. Enroll author with minimum collateral
            // --------------------------------------------------------------------

            // ALICE enrolls with exactly 50 units of collateral.
            // This establishes the minimum acceptable collateral threshold
            System::set_block_number(2);
            Pallet::enroll(&ALICE, 50, Fortitude::Force).unwrap();

            // ALAN backs ALICE with 100 units.
            // Without backing, penalties would be meaningless in practice.
            Pallet::fund(
                &ALICE,
                &Funder::Direct(ALAN),
                100,
                Precision::BestEffort,
                Fortitude::Force,
            )
            .unwrap();

            // Collateral remains untouched by funding
            let alice_collateral = Pallet::get_collateral(&ALICE).unwrap();
            assert_eq!(alice_collateral, 50);
            // Total hold = collateral (50) + backing (100)
            let alice_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_hold, 150);

            // Advance beyond probation and activate ALICE.
            System::set_block_number(12);
            Pallet::set_status(&ALICE, AuthorStatus::Active).unwrap();

            // --------------------------------------------------------------------
            // 5. Penalty scheduled
            // --------------------------------------------------------------------
            // A 10% penalty is scheduled.
            // This penalty should reduce the author's collateral proportionally.
            Pallet::penalize(&ALICE, Perbill::from_percent(10)).unwrap();

            // Penalty is applied at the beginning of the execution block.
            System::set_block_number(16);
            Pallet::on_initialize(16);

            // Collateral: 50 - 10% = 45
            let alice_collateral = Pallet::get_collateral(&ALICE).unwrap();
            assert_eq!(alice_collateral, 45);
            // Hold: collateral (45) + (backing (100) - 10%) = 135
            let alice_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_hold, 135);

            // --------------------------------------------------------------------
            // 8. Attempt new funding while collateral < min_fund
            // --------------------------------------------------------------------

            // CHARLIE attempts to back ALICE.
            // This must fail because ALICE's collateral is now BELOW the minimum (50)
            assert_err!(
                Pallet::fund(
                    &ALICE,
                    &Funder::Direct(CHARLIE),
                    100,
                    Precision::BestEffort,
                    Fortitude::Force
                ),
                Error::AuthorNeedsMoreCollateral
            );

            Pallet::add_collateral(&ALICE, 50, Fortitude::Force).unwrap();

            // Collateral: 45 + 50 = 95
            let alice_collateral = Pallet::get_collateral(&ALICE).unwrap();

            // Catering to one unit rounding loss
            assert!(alice_collateral == 95 || alice_collateral == 94);
            // Hold: collateral (95) + backing (90) = 185
            let alice_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_hold, 185);

            assert_ok!(Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                150,
                Precision::BestEffort,
                Fortitude::Force
            ));

            // Hold: collateral (95) + backing (90 + 150) = 335
            let alice_hold = Pallet::get_hold(&ALICE).unwrap();
            assert_eq!(alice_hold, 335);
        })
    }

    // ===============================================================================
    // `````````````````````````````````` PROBATION ``````````````````````````````````
    // ===============================================================================

    #[test]
    fn author_active_status_revoked_after_exceeding_risk_threshold() {
        authors_test_ext().execute_with(|| {
            // --------------------------------------------------------------------
            // 1. Initialize author
            // --------------------------------------------------------------------
            // ALICE is initialized with sufficient balance and hold capacity.
            // No backers are involved in this scenario - the focus is purely on
            // risk accumulation and status revocation.
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            // --------------------------------------------------------------------
            // 2. Enroll author with minimum collateral
            // --------------------------------------------------------------------
            // ALICE enrolls with the minimum required collateral and is promoted
            // to Active after the probation period elapses.
            System::set_block_number(2);
            Pallet::enroll(&ALICE, 50, Fortitude::Force).unwrap();

            System::set_block_number(12);
            Pallet::set_status(&ALICE, AuthorStatus::Active).unwrap();

            // --------------------------------------------------------------------
            // 3. Continuous penalties scheduled over multiple blocks
            // --------------------------------------------------------------------
            // A series of penalties are scheduled across consecutive blocks.
            // Each penalty contributes to the author's accumulated risk.
            // risk_permanence is explicitly invoked to evaluate whether the
            // accumulated risk exceeds the allowed threshold.
            //
            // Once the threshold is exceeded, the author must be demoted
            // back to Probation automatically.
            System::set_block_number(13);
            Pallet::penalize(&ALICE, Perbill::from_percent(2)).unwrap();

            Pallet::penalize(&ALICE, Perbill::from_percent(2)).unwrap();

            System::set_block_number(14);
            Pallet::risk_permanence(&ALICE).unwrap();

            Pallet::penalize(&ALICE, Perbill::from_percent(2)).unwrap();
            Pallet::risk_permanence(&ALICE).unwrap();

            System::set_block_number(15);
            Pallet::risk_permanence(&ALICE).unwrap();

            Pallet::penalize(&ALICE, Perbill::from_percent(2)).unwrap();

            System::set_block_number(16);
            Pallet::risk_permanence(&ALICE).unwrap();

            Pallet::penalize(&ALICE, Perbill::from_percent(2)).unwrap();

            System::set_block_number(17);
            Pallet::penalize(&ALICE, Perbill::from_percent(2)).unwrap();
            Pallet::risk_permanence(&ALICE).unwrap();
            Pallet::risk_permanence(&ALICE).unwrap();
            Pallet::penalize(&ALICE, Perbill::from_percent(2)).unwrap();
            Pallet::risk_permanence(&ALICE).unwrap();
            Pallet::risk_permanence(&ALICE).unwrap();

            let meta = Pallet::get_meta(&ALICE).unwrap();
            assert_eq!(meta.risk_until, 28);
            // since, risk_until (28) > current_block (17) + probation period (10)
            Pallet::revoke_permanence(&ALICE).unwrap();

            // --------------------------------------------------------------------
            // 4. Final assertion
            // --------------------------------------------------------------------
            // The author is expected to be in Probation.
            let current_status = Pallet::get_status(&ALICE).unwrap();
            assert_eq!(current_status, AuthorStatus::Probation);
        })
    }

    // ===============================================================================
    // ```````````````````````````````` INDEX & POOLS ````````````````````````````````
    // ===============================================================================

    #[test]
    fn index_funding_to_authors() {
        // --------------------------------------------------------------------
        // 1. Initialize authors and backers
        // --------------------------------------------------------------------
        // ALICE and BOB are authors.
        // CHARLIE and MIKE act as direct backers.
        // ALAN backs authors indirectly through an index.
        authors_test_ext().execute_with(|| {
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, INITIAL_BALANCE, STANDARD_HOLD)
                .unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            // --------------------------------------------------------------------
            // 2. Enroll authors
            // --------------------------------------------------------------------
            // Both authors enroll with sufficient collateral to accept funding.
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();
            Pallet::enroll(&BOB, 100, Fortitude::Force).unwrap();

            // --------------------------------------------------------------------
            // 3. Initial direct funding
            // --------------------------------------------------------------------
            // Each author receives direct backing to establish baseline funding.
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                150,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();
            Pallet::fund(
                &BOB,
                &Funder::Direct(MIKE),
                150,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            // --------------------------------------------------------------------
            // 4. Index creation and configuration
            // --------------------------------------------------------------------
            // An index is created with weighted entries:
            // - ALICE receives 60%
            // - BOB receives 40%
            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];

            prepare_and_initiate_index(MIKE, FUNDING.into(), &entries, INDEX_DIGEST).unwrap();

            // --------------------------------------------------------------------
            // 5. Index-based funding
            // --------------------------------------------------------------------
            // ALAN funds the index. The funds must be split proportionally
            // across all indexed authors.
            let by = Funder::Index {
                digest: INDEX_DIGEST,
                backer: ALAN,
            };
            Pallet::fund(&ALICE, &by, 200, Precision::Exact, Fortitude::Force).unwrap();

            // --------------------------------------------------------------------
            // 6. Assertions
            // --------------------------------------------------------------------
            // ALAN's hold reflects the index funding.
            // ALICE receives 60% and BOB receives 40% of the funded amount.
            assert_eq!(get_user_hold_balance(&ALAN), 50);
            assert_eq!(Pallet::get_fund(&ALICE, &by), Ok(120));
            assert_eq!(Pallet::get_fund(&BOB, &by), Ok(80));
        })
    }

    #[test]
    fn pool_funding_to_authors() {
        authors_test_ext().execute_with(|| {
            // --------------------------------------------------------------------
            // 1. Initialize authors, backers, and pool participants
            // --------------------------------------------------------------------
            // This test extends index funding by introducing a pool with commission.
            initiate_key_and_set_balance_and_hold(&ALICE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(&BOB, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(&CHARLIE, INITIAL_BALANCE, STANDARD_HOLD)
                .unwrap();
            initiate_key_and_set_balance_and_hold(&MIKE, INITIAL_BALANCE, STANDARD_HOLD).unwrap();
            initiate_key_and_set_balance_and_hold(&ALAN, INITIAL_BALANCE, STANDARD_HOLD).unwrap();

            // --------------------------------------------------------------------
            // 2. Enroll authors
            // --------------------------------------------------------------------
            Pallet::enroll(&ALICE, 100, Fortitude::Force).unwrap();
            Pallet::enroll(&BOB, 100, Fortitude::Force).unwrap();

            // --------------------------------------------------------------------
            // 3. Initial direct funding
            // --------------------------------------------------------------------
            Pallet::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                150,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();
            Pallet::fund(
                &BOB,
                &Funder::Direct(MIKE),
                150,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            // --------------------------------------------------------------------
            // 4. Pool creation with commission
            // --------------------------------------------------------------------
            // A pool wraps an index and introduces a commission paid
            // to the pool operator (MIKE).
            let alice_digest = gen_author_digest(&ALICE).unwrap();
            let bob_digest = gen_author_digest(&BOB).unwrap();
            let entries = vec![(alice_digest.clone(), 60), (bob_digest.clone(), 40)];
            let commission = Perbill::from_percent(5);
            prepare_and_initiate_pool(
                MIKE,
                FUNDING.into(),
                &entries,
                INDEX_DIGEST,
                POOL_DIGEST,
                commission,
            )
            .unwrap();

            // --------------------------------------------------------------------
            // 5. Pool-based funding
            // --------------------------------------------------------------------
            // ALAN funds the pool. Funds are split according to index weights,
            // and commission is reserved for the pool operator.
            let by = Funder::Pool {
                digest: POOL_DIGEST,
                backer: ALAN,
            };
            Pallet::fund(&ALICE, &by, 200, Precision::Exact, Fortitude::Force).unwrap();

            // --------------------------------------------------------------------
            // 6. Assertions before withdrawal
            // --------------------------------------------------------------------
            assert_eq!(get_user_hold_balance(&ALAN), 50);
            assert_eq!(Pallet::get_fund(&ALICE, &by), Ok(120));
            assert_eq!(Pallet::get_fund(&BOB, &by), Ok(80));

            // --------------------------------------------------------------------
            // 7. Pool withdrawal and commission settlement
            // --------------------------------------------------------------------
            // Upon withdrawal:
            // - ALAN receives remaining funds
            // - MIKE receives pool commission
            Pallet::draw(&ALICE, &by).unwrap();

            assert_eq!(get_user_balance(&ALAN), 290);
            assert_eq!(get_user_balance(&MIKE), 110);
        })
    }
}
