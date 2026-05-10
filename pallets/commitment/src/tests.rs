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
// ``````````````````````````````` INTEGRATION TESTS `````````````````````````````
// ===============================================================================

//! **Integration tests for the Commitment pallet.**
//!
//! Integration tests covering commitment lifecycle across direct digests,
//! indexes, and pools. Validates deposit, raise, resolve flows along with
//! reward/penalty distribution and balance invariants.

// ===============================================================================
// ```````````````````````````````````` IMPORTS ``````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::mock::*;

// --- FRAME Suite ---
use frame_suite::{commitment::*, Directive};

// --- FRAME Support ---
use frame_support::{
    assert_err, assert_ok,
    traits::{
        fungible::{Inspect, InspectFreeze, InspectHold},
        tokens::{Fortitude, Precision},
    },
};

// ===============================================================================
// ````````````````````````` DIRECT DIGEST COMMIT LIFECYCLE ``````````````````````
// ===============================================================================

#[test]
fn commitment_lifecycle_staking_scenario() {
    commit_test_ext().execute_with(|| {
            // Setup: Initialize Alice's account with free balance and hold balance for future commitments
            initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
            // Generate a unique commitment digest for Alice's staking agreement
            let staking_agreement_digest = Pallet::gen_digest(&ALICE).unwrap();
            // Verify digest and commitment don't exist yet (expected initial state)
            assert_err!{Pallet::digest_exists(&STAKING, &staking_agreement_digest), Error::DigestNotFound };
            assert_err!{Pallet::commit_exists(&staking_agreement_digest, &STAKING), Error::CommitNotFound};
            // Prepare to place initial commitment
            let init_stake_amount = 15;
            //  Verify Alice has sufficient funds to make this commitment
            assert_ok!(
                Pallet::can_place_commit(&ALICE, &STAKING, &staking_agreement_digest, init_stake_amount, &Default::default())
            );
            // Execute the commitment - Alice stakes 15 tokens for staking purposes
            Pallet::place_commit(&ALICE, &STAKING, &staking_agreement_digest, init_stake_amount, &Directive::new(Precision::BestEffort, Fortitude::Polite)).unwrap();
            // Verify balance states after initial commitment
            assert_eq!(
                AssetOf::balance(&ALICE), LARGE_VALUE
            );  // Total balance remains the same (20)
            assert_eq!(
                AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE), 5
            );  // Remaining held funds: previous_hold - committed = 20 - 15 = 5
            assert_eq!(
                AssetOf::balance_frozen(&STAKING, &ALICE), init_stake_amount
            ); // Newly frozen amount for staking = 15
            // Verify commitment and digest now exist in the system
            assert_ok!{Pallet::digest_exists(&STAKING, &staking_agreement_digest)};
            assert_ok!{Pallet::commit_exists(&ALICE, &STAKING)};
            //  Verify commitment and digest values match expected amounts
            let actual_commitment_value = Pallet::get_commit_value(&ALICE, &STAKING).unwrap();
            assert_eq!(actual_commitment_value, init_stake_amount);
            let actual_digest_value = Pallet::get_digest_value(&STAKING, &staking_agreement_digest).unwrap();
            assert_eq!(actual_digest_value, init_stake_amount);
            // Prepare to increase the commitment (raise stake)
            let add_stake_amount = 10;
            assert_ok!(
                Pallet::can_raise_commit(&ALICE, &STAKING, add_stake_amount, &Default::default())
            );
            // Execute commitment raise - Alice adds 10 more tokens to her stake
            Pallet::raise_commit(&ALICE, &STAKING, add_stake_amount, &Directive::new(Precision::BestEffort, Fortitude::Force)).unwrap();
            //  Verify balance states after raising commitment
            assert_eq!(
                AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE), 0
            ); // All held funds used: 5 - 5 = 0 (remaining 5 taken from free balance)
            assert_eq!(
                AssetOf::balance(&ALICE), 15
            ); // Free balance reduced: 20 - 5 = 15 (additional 5 taken from free balance)
            assert_eq!(
                AssetOf::balance_frozen(&STAKING, &ALICE), 25
            ); // Total frozen: initial_stake + additional_stake = 15 + 10 = 25
            // Verify updated commitment and digest values
            let total_stake_amount = init_stake_amount + add_stake_amount;
            let updated_commitment_value = Pallet::get_commit_value(&ALICE, &STAKING).unwrap();
            assert_eq!(updated_commitment_value, total_stake_amount);
            let updated_digest_total_value = Pallet::get_digest_value(&STAKING, &staking_agreement_digest).unwrap();
            assert_eq!(updated_digest_total_value, total_stake_amount);
            // Prepare for commitment resolution (unstaking)
            assert_ok!(Pallet::can_resolve_commit(&ALICE, &STAKING));
            //  Execute commitment resolution - Alice completes her staking commitment
            Pallet::resolve_commit(&ALICE, &STAKING).unwrap();
            // Verify final balance states after resolution
            assert_eq!(
                AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE), 0
            ); // No funds remain on hold
            assert_eq!(
                AssetOf::balance(&ALICE), 40
            ); // All funds returned: previous_free + resolved_amount = 15 + 25 = 40
            assert_eq!(
                AssetOf::balance_frozen(&STAKING, &ALICE), 0
            ); // No funds remain frozen for staking
            // Verify commitment lifecycle completion
            assert_ok!{Pallet::digest_exists(&STAKING, &staking_agreement_digest)};
            assert_err!{Pallet::commit_exists(&staking_agreement_digest, &STAKING), Error::CommitNotFound};
            // Clean up empty digest - reap the digest since it has no remaining committed funds
            assert_ok!(Pallet::reap_digest(&staking_agreement_digest, &STAKING));
            // Verify digest has been successfully removed from storage
            assert_err!{Pallet::digest_exists(&STAKING, &staking_agreement_digest), Error::DigestNotFound};

        })
}

// ===============================================================================
// `````````````````````````` INDEX DIGEST COMMIT LIFECYCLE ``````````````````````
// ===============================================================================

#[test]
fn commitment_index_lifecycle() {
    commit_test_ext().execute_with(|| {
        // Setup - Initialize multiple accounts with free balance and hold balance for future commitments
        initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
        initiate_key_and_set_balance_and_hold(BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
        initiate_key_and_set_balance_and_hold(CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
        // Generate individual commitment digests for underlying staking positions
        let digest_a123 = Pallet::gen_digest(&ALICE).unwrap();
        let digest_b456 = Pallet::gen_digest(&BOB).unwrap();
        // Place individual commitments - creating the underlying positions for the index
        let alice_stake_amount = 10;
        Pallet::place_commit(
            &ALICE,
            &STAKING,
            &digest_a123,
            alice_stake_amount,
            &Directive::new(Precision::BestEffort, Fortitude::Polite),
        )
        .unwrap();
        let bob_stake_amount = 15;
        Pallet::place_commit(
            &BOB,
            &STAKING,
            &digest_b456,
            bob_stake_amount,
            &Directive::new(Precision::BestEffort, Fortitude::Polite),
        )
        .unwrap();
        // Prepare index structure - defining entries and their proportional shares
        let entries = [(digest_a123.clone().clone(), 30), (digest_b456.clone(), 20)];
        let index = Pallet::prepare_index(&CHARLIE, &STAKING, &entries).unwrap();
        // Generate unique digest for the prepared index
        let index_digest = Pallet::gen_index_digest(&CHARLIE, &STAKING, &index).unwrap();
        //  Verify index and entries don't exist yet (expected initial state)
        assert_err!(
            Pallet::index_exists(&STAKING, &index_digest),
            Error::IndexNotFound
        );
        assert_err!(
            Pallet::entry_exists(&STAKING, &index_digest, &digest_a123),
            Error::IndexNotFound
        );
        assert_err!(
            Pallet::entry_exists(&STAKING, &index_digest, &digest_b456),
            Error::IndexNotFound
        );
        // Set the index - officially creating the index structure in storage
        Pallet::set_index(&CHARLIE, &STAKING, &index, &index_digest).unwrap();
        // Verify index and entries now exist in the system
        assert_ok!(Pallet::index_exists(&STAKING, &index_digest));
        assert_ok!(Pallet::entry_exists(&STAKING, &index_digest, &digest_a123));
        assert_ok!(Pallet::entry_exists(&STAKING, &index_digest, &digest_b456));
        // Verify initial index state - no commitments placed yet, so all values should be zero
        let expected_index_value = 0;
        let actual_index_value = Pallet::get_index_value(&STAKING, &index_digest).unwrap();
        assert_eq!(actual_index_value, expected_index_value);
        let expected_entries_values = vec![(digest_a123.clone(), 0), (digest_b456.clone(), 0)];
        let actual_entries_values = Pallet::get_entries_value(&STAKING, &index_digest).unwrap();
        assert_eq!(actual_entries_values, expected_entries_values);
        // Place commitment to the index - Charlie commits to the diversified index
        let init_stake_amount = 10;
        Pallet::place_commit(
            &CHARLIE,
            &STAKING,
            &index_digest,
            init_stake_amount,
            &Directive::new(Precision::BestEffort, Fortitude::Polite),
        )
        .unwrap();
        // Verify index value distribution after initial commitment
        // Share ratio 30:20 = 60%:40% distribution
        let expected_index_value = 10;
        let actual_index_value = Pallet::get_index_value(&STAKING, &index_digest).unwrap();
        assert_eq!(actual_index_value, expected_index_value);
        let expected_entries_values = vec![(digest_a123.clone(), 6), (digest_b456.clone(), 4)];
        let actual_entries_values = Pallet::get_entries_value(&STAKING, &index_digest).unwrap();
        assert_eq!(actual_entries_values, expected_entries_values);
        // Raise index commitment - Charlie increases his index position
        let add_stake_amount = 10;
        Pallet::raise_commit(
            &CHARLIE,
            &STAKING,
            add_stake_amount,
            &Directive::new(Precision::BestEffort, Fortitude::Polite),
        )
        .unwrap();
        // Verify index value distribution after raising commitment
        let expected_index_value = 20;
        let actual_index_value = Pallet::get_index_value(&STAKING, &index_digest).unwrap();
        assert_eq!(actual_index_value, expected_index_value);
        let expected_entries_values = vec![(digest_a123.clone(), 12), (digest_b456.clone(), 8)];
        let actual_entries_values = Pallet::get_entries_value(&STAKING, &index_digest).unwrap();
        assert_eq!(actual_entries_values, expected_entries_values);
        // Verify Charlie's balance states after raising commitment
        assert_eq!(AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &CHARLIE), 0); // All funds used
        assert_eq!(AssetOf::balance(&CHARLIE), 20);
        assert_eq!(AssetOf::balance_frozen(&STAKING, &CHARLIE), 20); // Total frozen for index commitment

        // Resolve index commitment - Charlie completes his index commitment
        Pallet::resolve_commit(&CHARLIE, &STAKING).unwrap();
        // Verify index values after resolution - all values return to zero
        let expected_index_value = 0;
        let actual_index_value = Pallet::get_index_value(&STAKING, &index_digest).unwrap();
        assert_eq!(actual_index_value, expected_index_value);
        let expected_entries_values = vec![(digest_a123.clone(), 0), (digest_b456.clone(), 0)];
        let actual_entries_values = Pallet::get_entries_value(&STAKING, &index_digest).unwrap();
        assert_eq!(actual_entries_values, expected_entries_values);
        // Verify Charlie's final balance states after resolution
        assert_eq!(AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &CHARLIE), 0); // No funds remain on hold
        assert_eq!(AssetOf::balance(&CHARLIE), 40); // All funds returned: 20 + 20 = 40
        assert_eq!(AssetOf::balance_frozen(&STAKING, &CHARLIE), 0); // No funds remain frozen

        // Verify index lifecycle completion - index exists but no active commitment
        assert_ok!(Pallet::index_exists(&STAKING, &index_digest));
        assert_err!(
            Pallet::commit_exists(&CHARLIE, &STAKING),
            Error::CommitNotFound
        );
        // Update entry shares - Charlie modifies the index structure (creates new index)
        // This updates the entry digest_a123 and since it's existing and share is non-zero
        // Its expected to removed and added at end of entries
        let new_index_digest = Pallet::set_entry_shares(
            &CHARLIE,
            &STAKING,
            &index_digest,
            &digest_a123,
            20, // Changed from 30 to 20, making it equal shares (20:20 = 50%:50%)
        )
        .unwrap();
        //  Verify new index initial state - no commitments yet to the updated index
        let expected_index_value = 0;
        let actual_index_value = Pallet::get_index_value(&STAKING, &new_index_digest).unwrap();
        assert_eq!(actual_index_value, expected_index_value);
        // Since digest_a123 is set later by setting entry shares, its reflected at last
        let expected_entries_values = vec![(digest_b456.clone(), 0), (digest_a123.clone(), 0)];
        let actual_entries_values = Pallet::get_entries_value(&STAKING, &new_index_digest).unwrap();
        assert_eq!(actual_entries_values, expected_entries_values);
        // Place commitment to updated index - testing the new share structure
        let init_stake_amount = 10;
        Pallet::place_commit(
            &CHARLIE,
            &STAKING,
            &new_index_digest,
            init_stake_amount,
            &Directive::new(Precision::BestEffort, Fortitude::Force),
        )
        .unwrap();
        // Verify updated share structure - now equal 50%:50% distribution
        let expected_entries_shares = vec![(digest_b456.clone(), 20), (digest_a123.clone(), 20)];
        let actual_entries_shares =
            Pallet::get_entries_shares(&STAKING, &new_index_digest).unwrap();
        assert_eq!(actual_entries_shares, expected_entries_shares);
        //  Verify equal distribution of commitment value
        let expected_index_value = 10;
        let actual_index_value = Pallet::get_index_value(&STAKING, &new_index_digest).unwrap();
        assert_eq!(actual_index_value, expected_index_value);
        let expected_entries_values = vec![(digest_b456.clone(), 5), (digest_a123.clone(), 5)];
        let actual_entries_values = Pallet::get_entries_value(&STAKING, &new_index_digest).unwrap();
        assert_eq!(actual_entries_values, expected_entries_values);
        // Attempt to reap active index - should fail due to active commitments
        assert_err!(
            Pallet::reap_index(&STAKING, &new_index_digest),
            Error::IndexHasFunds
        );
        // Clean up old empty index - reap the original index since it has no active commitments
        assert_ok!(Pallet::reap_index(&STAKING, &index_digest));
        // Verify old index has been successfully removed from storage
        assert_err!(
            Pallet::index_exists(&STAKING, &index_digest),
            Error::IndexNotFound
        );
    })
}

// ===============================================================================
// ````````````````````````` POOL DIGEST COMMIT LIFECYCLE ````````````````````````
// ===============================================================================

#[test]
fn commitment_pool_lifecycle() {
    commit_test_ext().execute_with(|| {
        // Setup - Initialize multiple accounts with free balance and hold balance for future commitments
        initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
        initiate_key_and_set_balance_and_hold(BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
        initiate_key_and_set_balance_and_hold(CHARLIE, LARGE_VALUE, LARGE_VALUE).unwrap();
        initiate_key_and_set_balance_and_hold(ALAN, LARGE_VALUE, LARGE_VALUE).unwrap();
        initiate_key_and_set_balance_and_hold(MIKE, LARGE_VALUE, LARGE_VALUE).unwrap();
        initiate_key_and_set_balance_and_hold(NIX, LARGE_VALUE, LARGE_VALUE).unwrap();
        // Generate individual commitment digests for underlying staking positions
        let digest_a123 = Pallet::gen_digest(&ALICE).unwrap();
        let digest_b456 = Pallet::gen_digest(&BOB).unwrap();
        let digest_c789 = Pallet::gen_digest(&MIKE).unwrap();
        let digest_d285 = Pallet::gen_digest(&NIX).unwrap();
        // Place individual commitments - creating the underlying positions for the index
        let alice_stake_amount = 10;
        Pallet::place_commit(
            &ALICE,
            &STAKING,
            &digest_a123,
            alice_stake_amount,
            &Directive::new(Precision::BestEffort, Fortitude::Polite),
        )
        .unwrap();
        let bob_stake_amount = 15;
        Pallet::place_commit(
            &BOB,
            &STAKING,
            &digest_b456,
            bob_stake_amount,
            &Directive::new(Precision::BestEffort, Fortitude::Polite),
        )
        .unwrap();
        let nix_stake_amount = 10;
        Pallet::place_commit(
            &NIX,
            &STAKING,
            &digest_d285,
            nix_stake_amount,
            &Directive::new(Precision::BestEffort, Fortitude::Polite),
        )
        .unwrap();
        let mike_stake_amount = 5;
        Pallet::place_commit(
            &MIKE,
            &STAKING,
            &digest_c789,
            mike_stake_amount,
            &Directive::new(Precision::BestEffort, Fortitude::Polite),
        )
        .unwrap();
        // Prepare index structure - defining entries and their proportional shares
        let entries = [(digest_a123.clone(), 30), (digest_b456.clone(), 20)];
        let index = Pallet::prepare_index(&CHARLIE, &STAKING, &entries).unwrap();
        // Generate unique digest for the prepared index
        let index_digest = Pallet::gen_index_digest(&CHARLIE, &STAKING, &index).unwrap();
        // Set the index - officially creating the index structure in storage
        Pallet::set_index(&CHARLIE, &STAKING, &index, &index_digest).unwrap();
        // Place commitment to the index - Charlie commits to the diversified index
        let init_stake_amount = 10;
        Pallet::place_commit(
            &CHARLIE,
            &STAKING,
            &index_digest,
            init_stake_amount,
            &Directive::new(Precision::BestEffort, Fortitude::Polite),
        )
        .unwrap();
        //  Generate pool digest - Alan creates a managed pool based on Charlie's index with commission
        let init_commission = COMMISSION_STANDARD;
        let pool_alan_digest =
            Pallet::gen_pool_digest(&ALAN, &STAKING, &index_digest, init_commission).unwrap();
        // Verify pool and slots don't exist yet (expected initial state)
        assert_err!(
            Pallet::pool_exists(&STAKING, &pool_alan_digest),
            Error::PoolNotFound
        );
        // Create the managed pool - Alan officially establishes the pool with commission structure
        Pallet::set_pool(
            &ALAN,
            &STAKING,
            &pool_alan_digest,
            &index_digest,
            init_commission,
        )
        .unwrap();
        // Verify pool and slots now exist in the system
        assert_ok!(Pallet::pool_exists(&STAKING, &pool_alan_digest));
        assert_ok!(Pallet::slot_exists(
            &STAKING,
            &pool_alan_digest,
            &digest_a123
        ));
        assert_ok!(Pallet::slot_exists(
            &STAKING,
            &pool_alan_digest,
            &digest_b456
        ));
        // Verify initial pool state - inherits index structure without index commitment balance
        let expected_slots_shares = vec![(digest_a123.clone(), 30), (digest_b456.clone(), 20)];
        let actual_slots_shares = Pallet::get_slots_shares(&STAKING, &pool_alan_digest).unwrap();
        assert_eq!(actual_slots_shares, expected_slots_shares);
        let expected_pool_value = 0;
        let actual_pool_value = Pallet::get_pool_value(&STAKING, &pool_alan_digest).unwrap();
        assert_eq!(actual_pool_value, expected_pool_value);
        let expected_slots_value = vec![(digest_a123.clone(), 0), (digest_b456.clone(), 0)];
        let actual_slots_value = Pallet::get_slots_value(&STAKING, &pool_alan_digest).unwrap();
        assert_eq!(expected_slots_value, actual_slots_value);
        // Place initial commitment to the pool - Alan invests in his own managed pool
        let init_pool_commit = 10;
        Pallet::place_commit(
            &ALAN,
            &STAKING,
            &pool_alan_digest,
            init_pool_commit,
            &Directive::new(Precision::BestEffort, Fortitude::Force),
        )
        .unwrap();
        // Verify pool value distribution after initial investment
        // Total: 10, Share ratio 30:20 = 60%:40% distribution across slots
        let expected_pool_value = 10;
        let actual_pool_value = Pallet::get_pool_value(&STAKING, &pool_alan_digest).unwrap();
        assert_eq!(actual_pool_value, expected_pool_value);
        let expected_slots_value = vec![(digest_a123.clone(), 6), (digest_b456.clone(), 4)];
        let actual_slots_value = Pallet::get_slots_value(&STAKING, &pool_alan_digest).unwrap();
        assert_eq!(expected_slots_value, actual_slots_value);
        // Raise pool commitment - Alan increases his investment in the managed pool
        let add_pool_commit = 10;
        Pallet::raise_commit(
            &ALAN,
            &STAKING,
            add_pool_commit,
            &Directive::new(Precision::BestEffort, Fortitude::Force),
        )
        .unwrap();
        // Verify pool value distribution after raising investment
        let expected_pool_value = 20;
        let actual_pool_value = Pallet::get_pool_value(&STAKING, &pool_alan_digest).unwrap();
        assert_eq!(actual_pool_value, expected_pool_value);
        let expected_slots_value = vec![(digest_a123.clone(), 12), (digest_b456.clone(), 8)];
        let actual_slots_value = Pallet::get_slots_value(&STAKING, &pool_alan_digest).unwrap();
        assert_eq!(expected_slots_value, actual_slots_value);

        //------------------------- Creation of new pool due to mutation ----------------------//

        // Update commission structure - Alan creates new pool with different management fee
        let updated_commission = COMMISSION_HIGH;
        let new_pool_alan_diget =
            Pallet::set_commission(&ALAN, &STAKING, &index_digest, updated_commission).unwrap();
        //  Verify new pool structure with updated commission
        let actual_commission = Pallet::get_commission(&STAKING, &new_pool_alan_diget).unwrap();
        assert_eq!(actual_commission, updated_commission);
        let expected_slots_shares = vec![(digest_a123.clone(), 30), (digest_b456.clone(), 20)]; // Inherits same structure
        let actual_slots_shares = Pallet::get_slots_shares(&STAKING, &new_pool_alan_diget).unwrap();
        assert_eq!(actual_slots_shares, expected_slots_shares);
        let expected_pool_value = 0; // Fresh pool with no commitment yet
        let actual_pool_value = Pallet::get_pool_value(&STAKING, &new_pool_alan_diget).unwrap();
        assert_eq!(actual_pool_value, expected_pool_value);
        let expected_slots_value = vec![(digest_a123.clone(), 0), (digest_b456.clone(), 0)]; // No distributions in new pool
        let actual_slots_value = Pallet::get_slots_value(&STAKING, &new_pool_alan_diget).unwrap();
        assert_eq!(expected_slots_value, actual_slots_value);
        // Prepare Mike for pool commitment - resolve his previous individual commitment
        Pallet::resolve_commit(&MIKE, &STAKING).unwrap();
        // Mike commits in the new pool
        Pallet::place_commit(
            &MIKE,
            &STAKING,
            &new_pool_alan_diget,
            10,
            &Directive::new(Precision::BestEffort, Fortitude::Force),
        )
        .unwrap();
        // Verify pool value distribution after Mike's investment
        // Total: 10, Share ratio 30:20 = 60%:40% distribution across existing slots
        let expected_pool_value = 10;
        let actual_pool_value = Pallet::get_pool_value(&STAKING, &new_pool_alan_diget).unwrap();
        assert_eq!(actual_pool_value, expected_pool_value);
        let expected_slots_value = vec![(digest_a123.clone(), 6), (digest_b456.clone(), 4)];
        let actual_slots_value = Pallet::get_slots_value(&STAKING, &new_pool_alan_diget).unwrap();
        assert_eq!(expected_slots_value, actual_slots_value);
        // Dynamic pool expansion - Alan adds Nix's position as a new slot
        Pallet::set_slot_shares(
            &ALAN,
            &STAKING,
            &new_pool_alan_diget,
            &digest_d285,
            50, // Adding significant weight to diversify the pool further
        )
        .unwrap();
        // Verify pool structure after adding new slot - redistribution occurs
        let expected_slots_shares = vec![
            (digest_a123.clone(), 30),
            (digest_b456.clone(), 20),
            (digest_d285.clone(), 50),
        ]; // New slot added with 50% share weight
        let actual_slots_shares = Pallet::get_slots_shares(&STAKING, &new_pool_alan_diget).unwrap();
        assert_eq!(actual_slots_shares, expected_slots_shares);
        let expected_pool_value = 10;
        let actual_pool_value = Pallet::get_pool_value(&STAKING, &new_pool_alan_diget).unwrap();
        assert_eq!(actual_pool_value, expected_pool_value);
        // Verify Share redistribution
        let expected_slots_value = vec![
            (digest_a123.clone(), 3),
            (digest_b456.clone(), 2),
            (digest_d285.clone(), 5),
        ];
        let actual_slots_value = Pallet::get_slots_value(&STAKING, &new_pool_alan_diget).unwrap();
        assert_eq!(expected_slots_value, actual_slots_value);

        //--------------------------------------//

        // Transfer pool ownership - Alan delegates management of original pool to Charlie
        Pallet::set_pool_manager(&STAKING, &pool_alan_digest, &CHARLIE).unwrap();
        // Verify ownership transfer
        let actual_manager = Pallet::get_manager(&STAKING, &pool_alan_digest).unwrap();
        assert_eq!(actual_manager, CHARLIE);

        //  Verify Alan's balance states after pool operations
        assert_eq!(AssetOf::balance(&ALAN), LARGE_VALUE); // Total balance unchanged (20)
        assert_eq!(AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALAN), 0); // All hold funds used for pool commitments
        assert_eq!(AssetOf::balance_frozen(&STAKING, &ALAN), 20); // All 20 tokens frozen in pool commitment

        // Verify Charlie's balance states after receiving pool management
        assert_eq!(AssetOf::balance(&CHARLIE), LARGE_VALUE); // Total balance unchanged (20)
        assert_eq!(AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &CHARLIE), 10); // Remaining held funds: 20 - 10 = 10 (from original index commitment)
        assert_eq!(AssetOf::balance_frozen(&STAKING, &CHARLIE), 10); // 10 tokens frozen in index commitment

        //  Resolve pool commitment - Alan exits his investment position
        Pallet::resolve_commit(&ALAN, &STAKING).unwrap();

        // Verify commitment resolution status
        assert_err!(
            Pallet::commit_exists(&ALAN, &STAKING),
            Error::CommitNotFound
        );
        assert_ok!(Pallet::pool_exists(&STAKING, &pool_alan_digest));

        // Verify Alan's final balance states after exiting pool investment
        assert_eq!(AssetOf::balance(&ALAN), 38); // Returns with commission deducted: 20 + 20 - 2 = 38 (10% of 20)
        assert_eq!(AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALAN), 0); // No funds remain on hold
        assert_eq!(AssetOf::balance_frozen(&STAKING, &ALAN), 0); // No funds remain frozen

        // Verify Charlie's balance states after receiving management commission
        assert_eq!(AssetOf::balance(&CHARLIE), 22); // Receives commission: 20 + 2 = 22 (10% of 20)
        assert_eq!(AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &CHARLIE), 10); // Held funds unchanged from index commitment
        assert_eq!(AssetOf::balance_frozen(&STAKING, &CHARLIE), 10); // Frozen funds unchanged from index commitment
                                                                     //  Clean up empty pool - reap the pool since it has no active investments
        Pallet::reap_pool(&STAKING, &pool_alan_digest).unwrap();
        // Verify pool has been successfully removed from storage
        assert_err!(
            Pallet::pool_exists(&STAKING, &pool_alan_digest),
            Error::PoolNotFound
        );
    })
}

// ===============================================================================
// ```````````````````````````` DIGEST REWARD LIFECYCLE ``````````````````````````
// ===============================================================================

#[test]
fn direct_reward_distribution() {
    commit_test_ext().execute_with(|| {
        // Setup - Initialize multiple accounts with free balance and hold balance for future commitments
        initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
        initiate_key_and_set_balance_and_hold(BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
        // Generate a unique staking agreement digest using Bob's account as the source
        let staking_agreement_digest = Pallet::gen_digest(&BOB).unwrap();

        let stake_amount = 10;
        // Alice places a commitment of 10 tokens to the staking agreement digest
        Pallet::place_commit(
            &ALICE,
            &STAKING,
            &staking_agreement_digest,
            stake_amount,
            &Directive::new(Precision::BestEffort, Fortitude::Force),
        )
        .unwrap();

        // Verify Alice's balance states after placing the initial commitment
        assert_eq!(AssetOf::balance(&ALICE), LARGE_VALUE); // Total balance remains the same (20)
        assert_eq!(AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE), 10); // Remaining held funds: previous_hold - committed = 20 - 10 = 10
        assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), stake_amount); // Newly frozen amount for staking = 10

        // // Verify that Alice's commitment value matches what she committed
        // let actual_commitment_value =
        //     Pallet::get_commit_value(&ALICE, &STAKING)
        //         .unwrap();
        // assert_eq!(actual_commitment_value, stake_amount);
        // // Verify that the digest's total value matches Alice's commitment
        // // (since Alice is the only one committed to this digest)
        // let actual_digest_value = Pallet::get_digest_value(
        //     &STAKING,
        //     &staking_agreement_digest,
        // )
        // .unwrap();
        // assert_eq!(actual_digest_value, stake_amount);

        // Simulate a reward scenario: increase the digest value from 10 to 15
        // This represents external rewards (e.g., staking rewards, yield, etc.)
        // being applied to the commitment digest
        let stake_amount_with_reward = 15; // Original 10 + 5 reward = 15
        Pallet::set_digest_value(
            &STAKING,
            &staking_agreement_digest,
            stake_amount_with_reward,
            &Default::default(),
        )
        .unwrap();

        // Verify that Alice's commitment value now reflects the increased digest value
        // Her commitment should now be worth 15 instead of the original 10
        let actual_commitment_value = Pallet::get_commit_value(&ALICE, &STAKING).unwrap();
        assert_eq!(actual_commitment_value, stake_amount_with_reward);
        // Verify that the digest's total value matches the updated amount
        let actual_digest_value =
            Pallet::get_digest_value(&STAKING, &staking_agreement_digest).unwrap();
        assert_eq!(actual_digest_value, stake_amount_with_reward);

        // Alice resolves (withdraws) her commitment, which should include the rewards
        Pallet::resolve_commit(&ALICE, &STAKING).unwrap();

        // Verify Alice's final balance after resolving with rewards
        // Final balance: original_free(20) + resolved_commitment_with_rewards(15) = 35
        // Alice gained 5 extra tokens as rewards
        assert_eq!(AssetOf::balance(&ALICE), 35);
        assert_eq!(AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE), 10); // Held balance unchanged: still 10 tokens remain held for future commitments
        assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), 0);
        // Frozen balance back to 0: commitment resolved, no longer frozen for staking
    })
}

// ===============================================================================
// ``````````````````````````` DIGEST PENALTY LIFECYCLE ``````````````````````````
// ===============================================================================

#[test]
fn direct_penalty_distribution() {
    commit_test_ext().execute_with(|| {
        // Setup - Initialize multiple accounts with free balance and hold balance for future commitments
        initiate_key_and_set_balance_and_hold(ALICE, LARGE_VALUE, LARGE_VALUE).unwrap();
        initiate_key_and_set_balance_and_hold(BOB, LARGE_VALUE, LARGE_VALUE).unwrap();
        // Generate a unique staking agreement digest using Bob's account as the source
        let staking_agreement_digest = Pallet::gen_digest(&BOB).unwrap();

        let stake_amount = 10;
        // Alice places a commitment of 10 tokens to the staking agreement digest
        Pallet::place_commit(
            &ALICE,
            &STAKING,
            &staking_agreement_digest,
            stake_amount,
            &Directive::new(Precision::BestEffort, Fortitude::Force),
        )
        .unwrap();

        // Verify Alice's balance states after placing the initial commitment
        assert_eq!(AssetOf::balance(&ALICE), LARGE_VALUE); // Total balance remains the same (20)
        assert_eq!(AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE), 10); // Remaining held funds: previous_hold - committed = 20 - 10 = 10
        assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), stake_amount); // Newly frozen amount for staking = 10

        // Verify that Alice's commitment value matches what she committed
        let actual_commitment_value = Pallet::get_commit_value(&ALICE, &STAKING).unwrap();
        assert_eq!(actual_commitment_value, stake_amount);
        // Verify that the digest's total value matches Alice's commitment
        // (since Alice is the only one committed to this digest)
        let actual_digest_value =
            Pallet::get_digest_value(&STAKING, &staking_agreement_digest).unwrap();
        assert_eq!(actual_digest_value, stake_amount);

        // Simulate a penalty scenario: decrease the digest value from 10 to 8
        // This represents external penalties (e.g., slashing, etc.)
        // being applied to the commitment digest
        let stake_amount_with_penalty = 8; // Original 10 - 2 penalty = 8
        Pallet::set_digest_value(
            &STAKING,
            &staking_agreement_digest,
            stake_amount_with_penalty,
            &Default::default(),
        )
        .unwrap();

        // Verify that Alice's commitment value now reflects the decreased digest value
        // Her commitment should now be worth 8 instead of the original 10
        let actual_commitment_value = Pallet::get_commit_value(&ALICE, &STAKING).unwrap();
        assert_eq!(actual_commitment_value, stake_amount_with_penalty);
        // Verify that the digest's total value matches the updated amount
        let actual_digest_value =
            Pallet::get_digest_value(&STAKING, &staking_agreement_digest).unwrap();
        assert_eq!(actual_digest_value, stake_amount_with_penalty);

        // Alice resolves (withdraws) her commitment, which should include the penalty
        Pallet::resolve_commit(&ALICE, &STAKING).unwrap();

        // Verify Alice's final balance after resolving with rewards
        // Final balance: original_free(20) + resolved_commitment_with_penalty(8) = 28
        // Alice looses 2 tokens as penalty
        assert_eq!(AssetOf::balance(&ALICE), 28);
        assert_eq!(AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE), 10); // Held balance unchanged: still 10 tokens remain held for future commitments
        assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), 0);
        // Frozen balance back to 0: commitment resolved, no longer frozen for staking
    })
}

// ===============================================================================
// `````````````````````````` STAKING LIFECYCLE SCENARIO `````````````````````````
// ===============================================================================

#[test]
fn staking_bonding_alike_scenario() {
    commit_test_ext().execute_with(|| {
        // Setup - Initialize multiple validators and nominators for comprehensive bonding scenarios
        initiate_key_and_set_balance_and_hold(ALICE, 100, 100).unwrap(); // Validator 1
        initiate_key_and_set_balance_and_hold(BOB, 100, 100).unwrap(); // Validator 2
        initiate_key_and_set_balance_and_hold(CHARLIE, 50, 50).unwrap(); // Nominator 1
        initiate_key_and_set_balance_and_hold(ALAN, 30, 30).unwrap(); // Nominator 2
        initiate_key_and_set_balance_and_hold(MIKE, 20, 20).unwrap(); // Nominator 3

        //-- Validator Self-Bonding --
        // Generate validator commitment digests for self-bonding
        let alice_validator_digest = Pallet::gen_digest(&ALICE).unwrap();
        let bob_validator_digest = Pallet::gen_digest(&BOB).unwrap();

        // Alice bonds as validator with 50 tokens
        let alice_bond_amount = 50;
        Pallet::place_commit(
            &ALICE,
            &STAKING,
            &alice_validator_digest,
            alice_bond_amount,
            &Directive::new(Precision::BestEffort, Fortitude::Force),
        )
        .unwrap();

        // Bob bonds as validator with 40 tokens
        let bob_bond_amount = 40;
        Pallet::place_commit(
            &BOB,
            &STAKING,
            &bob_validator_digest,
            bob_bond_amount,
            &Directive::new(Precision::BestEffort, Fortitude::Force),
        )
        .unwrap();

        // Verify validator self-bonding states
        assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), 50);
        assert_eq!(AssetOf::balance_frozen(&STAKING, &BOB), 40);

        let alice_validator_stake = Pallet::get_commit_value(&ALICE, &STAKING).unwrap();
        let bob_validator_stake = Pallet::get_commit_value(&BOB, &STAKING).unwrap();
        assert_eq!(alice_validator_stake, alice_bond_amount);
        assert_eq!(bob_validator_stake, bob_bond_amount);

        //---- Nominator Bonding to Individual Validators ----
        // Charlie nominates Alice with 20 tokens
        let charlie_nomination_amount = 20;
        Pallet::place_commit(
            &CHARLIE,
            &STAKING,
            &alice_validator_digest,
            charlie_nomination_amount,
            &Directive::new(Precision::BestEffort, Fortitude::Force),
        )
        .unwrap();

        // Alan nominates Bob with 15 tokens
        let alan_nomination_amount = 15;
        Pallet::place_commit(
            &ALAN,
            &STAKING,
            &bob_validator_digest,
            alan_nomination_amount,
            &Directive::new(Precision::BestEffort, Fortitude::Force),
        )
        .unwrap();

        // Verify nomination states
        assert_eq!(
            AssetOf::balance_frozen(&STAKING, &CHARLIE),
            charlie_nomination_amount
        );
        assert_eq!(
            AssetOf::balance_frozen(&STAKING, &ALAN),
            alan_nomination_amount
        );

        // Verify total stake per validator (self-bond + nominations)
        let actual_alice_total_stake =
            Pallet::get_digest_value(&STAKING, &alice_validator_digest).unwrap();
        let actual_bob_total_stake =
            Pallet::get_digest_value(&STAKING, &bob_validator_digest).unwrap();

        let expected_alice_total_stake = 70; // alice_bond_amount + charlie_nomination_amount
        let expected_bob_total_stake = 55; // bob_bond_amount + alan_nomination_amount
        assert_eq!(actual_alice_total_stake, expected_alice_total_stake); // 50 + 20 = 70
        assert_eq!(actual_bob_total_stake, expected_bob_total_stake); // 40 + 15 = 55

        //--- Multi-Validator Index for Diversified Nomination ---
        // Create index combining both validators for Mike's diversified nomination
        let validator_entries = [
            (alice_validator_digest.clone(), 60),
            (bob_validator_digest.clone(), 40),
        ]; // 60:40 ratio
        let diversified_index = Pallet::prepare_index(&MIKE, &STAKING, &validator_entries).unwrap();
        let diversified_index_digest =
            Pallet::gen_index_digest(&MIKE, &STAKING, &diversified_index).unwrap();

        Pallet::set_index(
            &MIKE,
            &STAKING,
            &diversified_index,
            &diversified_index_digest,
        )
        .unwrap();

        // Mike makes diversified nomination through index
        let mike_diversified_amount = 10;
        Pallet::place_commit(
            &MIKE,
            &STAKING,
            &diversified_index_digest,
            mike_diversified_amount,
            &Directive::new(Precision::BestEffort, Fortitude::Force),
        )
        .unwrap();

        // Verify diversified nomination distribution (60:40 split of 10 = 6:4)
        let expected_index_entries = vec![
            (alice_validator_digest.clone(), 6),
            (bob_validator_digest.clone(), 4),
        ];
        let actual_index_entries =
            Pallet::get_entries_value(&STAKING, &diversified_index_digest).unwrap();
        assert_eq!(actual_index_entries, expected_index_entries);

        // Phase 4: Stake Increases (Additional Bonding)
        // Alice increases her validator self-bond
        let alice_additional_bond = 20;
        Pallet::raise_commit(
            &ALICE,
            &STAKING,
            alice_additional_bond,
            &Directive::new(Precision::BestEffort, Fortitude::Force),
        )
        .unwrap();

        // Charlie increases nomination to Alice
        let charlie_additional_nomination = 10;
        Pallet::raise_commit(
            &CHARLIE,
            &STAKING,
            charlie_additional_nomination,
            &Directive::new(Precision::BestEffort, Fortitude::Force),
        )
        .unwrap();

        // Verify increased stakes
        let actual_alice_updated_stake = Pallet::get_commit_value(&ALICE, &STAKING).unwrap();
        let actual_charlie_updated_stake = Pallet::get_commit_value(&CHARLIE, &STAKING).unwrap();
        let expected_alice_updated_stake = 70; // 50 + 20
        let expected_bob_updated_stake = 30; // 20 + 10
        assert_eq!(actual_alice_updated_stake, expected_alice_updated_stake);
        assert_eq!(actual_charlie_updated_stake, expected_bob_updated_stake);

        //--- Reward and Penalty Distribution ---
        // Verify current total stake per validator (self-bond + nominations)
        let actual_alice_total_stake =
            Pallet::get_digest_value(&STAKING, &alice_validator_digest).unwrap();
        let actual_bob_total_stake =
            Pallet::get_digest_value(&STAKING, &bob_validator_digest).unwrap();
        let expected_alice_total_stake = 106; // alice_bond_amount + charlie_nomination_amount + alice_additional_bond + charlie_additional_nomination + mikes_diversfied_index (6)
        let expected_bob_total_stake = 59; // bob_bond_amount + alan_nomination_amount + mikes_diversfied_index (4)
        assert_eq!(actual_alice_total_stake, expected_alice_total_stake);
        assert_eq!(actual_bob_total_stake, expected_bob_total_stake);

        // Stimulate rewards for Validator Alice
        let alice_total_with_reward = 130; // 106 (total stake) + 24 (reward);
        Pallet::set_digest_value(
            &STAKING,
            &alice_validator_digest,
            alice_total_with_reward,
            &Default::default(),
        )
        .unwrap();

        // Stimulate penalty for Validator Bob
        let bob_total_with_penalty = 50; // 59 (total stake) - 9 (penalty);
        Pallet::set_digest_value(
            &STAKING,
            &bob_validator_digest,
            bob_total_with_penalty,
            &Default::default(),
        )
        .unwrap();
        //--- Unbond with reward and penalty ---
        // Charlie Unbond from alice
        Pallet::resolve_commit(&CHARLIE, &STAKING).unwrap();
        // Charlie balance state check
        let charlie_balance = AssetOf::balance(&CHARLIE);
        let charlie_frozen_balance = AssetOf::balance_frozen(&STAKING, &CHARLIE);
        let expected_charlie_balance = 86; // 50 (existing) + 30 (staked) + 6 (reward);
        assert_eq!(charlie_balance, expected_charlie_balance);
        assert_eq!(charlie_frozen_balance, 0);

        // Alan Unbond from bob
        Pallet::resolve_commit(&ALAN, &STAKING).unwrap();
        // Alan balance state check
        let alan_balance = AssetOf::balance(&ALAN);
        let alan_frozen_balance = AssetOf::balance_frozen(&STAKING, &ALAN);
        let expected_alan_balance = 42; // 30 (existing) + 15 (staked) - 3 (penalty);
        assert_eq!(alan_balance, expected_alan_balance);
        assert_eq!(alan_frozen_balance, 0);

        // Mike resolve his bond to the diversified_index
        Pallet::resolve_commit(&MIKE, &STAKING).unwrap();
        // Mike balance state check
        let mike_balance = AssetOf::balance(&MIKE);
        let mike_frozen_balance = AssetOf::balance_frozen(&STAKING, &MIKE);
        let expected_mike_balance = 30; // 20 (existing) + 10 (staked) + 0 ( 1.36 (reward) - 0.61 (penalty) );
        assert_eq!(mike_balance, expected_mike_balance);
        assert_eq!(mike_frozen_balance, 0);

        // Alice resolve his bond
        Pallet::resolve_commit(&ALICE, &STAKING).unwrap();
        // Alice balance state check
        let alice_balance = AssetOf::balance(&ALICE);
        let alice_frozen_balance = AssetOf::balance_frozen(&STAKING, &ALICE);
        let expected_alice_balance = 187; // 100 (existing) + 70 (staked) + 17 (reward);
        assert_eq!(alice_balance, expected_alice_balance);
        assert_eq!(alice_frozen_balance, 0);

        // Bob resolve his bond
        Pallet::resolve_commit(&BOB, &STAKING).unwrap();
        // Bob balance state check
        let bob_balance = AssetOf::balance(&BOB);
        let bob_frozen_balance = AssetOf::balance_frozen(&STAKING, &BOB);
        let expected_bob_balance = 135; // 100 (existing) + 40 (staked) - 5 (penalty);
        assert_eq!(bob_balance, expected_bob_balance);
        assert_eq!(bob_frozen_balance, 0);
    })
}
