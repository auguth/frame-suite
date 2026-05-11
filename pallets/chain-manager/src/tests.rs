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
// `````````````````````````````` INTEGRATION TESTS ``````````````````````````````
// ===============================================================================

//! **Integration tests for the Chain Manager pallet.**
//!
//! Covers end-to-end workflows including:
//! - Author lifecycle (enroll, fund, withdraw)
//! - Affidavit registration and submission
//! - Election execution and result validation
//! - Block production and point tracking
//! - Offence handling and penalties
//! - Reward distribution and session transitions
//! - Offchain worker (OCW) pipeline
//!
//! Ensures correct interaction between on-chain logic and offchain processes
//! across session boundaries in a deterministic test environment.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::mock::*;

// --- Scale-codec crates ---
use codec::{Decode, Encode};

// --- FRAME Suite ---
use frame_suite::{blockchain::*, roles::*};

// --- FRAME Support ---
use frame_support::{
    assert_ok,
    pallet_prelude::TransactionSource,
    traits::tokens::{Fortitude, Precision},
};

// --- External pallets ---
use pallet_session::SessionManager;

// --- Substrate primitives ---
use sp_application_crypto::Pair;
use sp_core::sr25519;
use sp_runtime::{
    traits::{IdentifyAccount, ValidateUnsigned},
    MultiSignature, MultiSigner,
};

// --- Substrate staking ---
use sp_staking::offence::{OffenceDetails, OnOffenceHandler};

// ===============================================================================
// ``````````````````````````````` AUTHOR LIFECYCLE ``````````````````````````````
// ===============================================================================

#[test]
fn author_lifecycle_full_integration() {
    let (mut ext, _) = new_offchain_ext();
    ext.execute_with(|| {
        // ============================================================================
        // SESSION 1 - PHASE 1: INITIAL SETUP & SESSION START (Block 1)
        // ============================================================================
        System::set_block_number(1);
        Pallet::start_session(1);

        // Verify session initialization
        assert_eq!(CurrentSession::get(), 1);
        assert_eq!(SessionStartsAt::get(), 1);

        let callers = vec![
            ALICE, NIX, MIKE, BOB, CHARLIE, DAVE, LAYA, ALAN, JAKE, JIM, PAUL, AMY,
        ];

        for caller in callers {
            set_user_balance_and_hold(caller, 500, 500).unwrap();
        }

        // ============================================================================
        // SESSION 1 - PHASE 2: AUTHOR ENROLLMENT (Block 50)
        // ============================================================================
        System::set_block_number(50);

        RoleAdapter::enroll(&ALICE, 450, Fortitude::Force).unwrap();
        RoleAdapter::enroll(&NIX, 420, Fortitude::Force).unwrap();
        RoleAdapter::enroll(&MIKE, 400, Fortitude::Force).unwrap();
        RoleAdapter::enroll(&BOB, 350, Fortitude::Force).unwrap();
        RoleAdapter::enroll(&CHARLIE, 370, Fortitude::Force).unwrap();
        RoleAdapter::enroll(&DAVE, 200, Fortitude::Force).unwrap();
        RoleAdapter::enroll(&LAYA, 250, Fortitude::Force).unwrap();
        RoleAdapter::enroll(&ALAN, 300, Fortitude::Force).unwrap();

        // ============================================================================
        // SESSION 1 - PHASE 3: BACKER FUNDING (Block 150)
        // ============================================================================
        System::set_block_number(150);

        RoleAdapter::fund(
            &ALICE,
            &Funder::Direct(JAKE),
            200,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        RoleAdapter::fund(
            &ALAN,
            &Funder::Direct(JIM),
            150,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        RoleAdapter::fund(
            &NIX,
            &Funder::Direct(PAUL),
            300,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        RoleAdapter::fund(
            &MIKE,
            &Funder::Direct(AMY),
            200,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();

        set_session_config();

        // ============================================================================
        // SESSION 1 - PHASE 4: AFFIDAVIT KEY REGISTRATION (Block 200)
        // ============================================================================
        // Authors register cryptographic keypairs for session 2 via validate() extrinsic
        // These keys prove author identity during affidavit submission and elections
        System::set_block_number(200);

        // Generate sr25519 keypairs for each author
        let alice_keypair = sr25519::Pair::from_seed(&[1u8; 32]);
        let nix_keypair = sr25519::Pair::from_seed(&[2u8; 32]);
        let mike_keypair = sr25519::Pair::from_seed(&[3u8; 32]);
        let bob_keypair = sr25519::Pair::from_seed(&[4u8; 32]);
        let charlie_keypair = sr25519::Pair::from_seed(&[5u8; 32]);
        let dev_keypair = sr25519::Pair::from_seed(&[6u8; 32]);
        let laya_keypair = sr25519::Pair::from_seed(&[7u8; 32]);
        let alan_keypair = sr25519::Pair::from_seed(&[8u8; 32]);

        // Convert to MultiSigner for payload creation
        let alice_public = MultiSigner::from(alice_keypair.public());
        let nix_public = MultiSigner::from(nix_keypair.public());
        let mike_public = MultiSigner::from(mike_keypair.public());
        let bob_public = MultiSigner::from(bob_keypair.public());
        let charlie_public = MultiSigner::from(charlie_keypair.public());
        let dev_public = MultiSigner::from(dev_keypair.public());
        let laya_public = MultiSigner::from(laya_keypair.public());
        let alan_public = MultiSigner::from(alan_keypair.public());

        // Register each author's affidavit key via validate() extrinsic
        // Pattern: Create payload -> Sign payload -> Submit to runtime
        let alice_validate_payload = ValidatePayloadOf {
            public: alice_public.clone(),
        };
        let alice_validate_sig = alice_keypair.sign(alice_validate_payload.encode().as_slice());
        let alice_validate_sig: MultiSignature = alice_validate_sig.into();

        assert_ok!(Pallet::validate(
            RuntimeOrigin::signed(ALICE),
            alice_validate_payload,
            alice_validate_sig,
        ));

        let nix_validate_payload = ValidatePayloadOf {
            public: nix_public.clone(),
        };
        let nix_validate_sig = nix_keypair.sign(nix_validate_payload.encode().as_slice());
        let nix_validate_sig: MultiSignature = nix_validate_sig.into();

        assert_ok!(Pallet::validate(
            RuntimeOrigin::signed(NIX),
            nix_validate_payload,
            nix_validate_sig,
        ));

        let mike_validate_payload = ValidatePayloadOf {
            public: mike_public.clone(),
        };
        let mike_validate_sig = mike_keypair.sign(mike_validate_payload.encode().as_slice());
        let mike_validate_sig: MultiSignature = mike_validate_sig.into();

        assert_ok!(Pallet::validate(
            RuntimeOrigin::signed(MIKE),
            mike_validate_payload,
            mike_validate_sig,
        ));

        let bob_validate_payload = ValidatePayloadOf {
            public: bob_public.clone(),
        };
        let bob_validate_sig = bob_keypair.sign(bob_validate_payload.encode().as_slice());
        let bob_validate_sig: MultiSignature = bob_validate_sig.into();

        assert_ok!(Pallet::validate(
            RuntimeOrigin::signed(BOB),
            bob_validate_payload,
            bob_validate_sig,
        ));

        let charlie_validate_payload = ValidatePayloadOf {
            public: charlie_public.clone(),
        };
        let charlie_validate_sig =
            charlie_keypair.sign(charlie_validate_payload.encode().as_slice());
        let charlie_validate_sig: MultiSignature = charlie_validate_sig.into();

        assert_ok!(Pallet::validate(
            RuntimeOrigin::signed(CHARLIE),
            charlie_validate_payload,
            charlie_validate_sig,
        ));

        let dev_validate_payload = ValidatePayloadOf {
            public: dev_public.clone(),
        };
        let dev_validate_sig = dev_keypair.sign(dev_validate_payload.encode().as_slice());
        let dev_validate_sig: MultiSignature = dev_validate_sig.into();

        assert_ok!(Pallet::validate(
            RuntimeOrigin::signed(DAVE),
            dev_validate_payload,
            dev_validate_sig,
        ));

        let laya_validate_payload = ValidatePayloadOf {
            public: laya_public.clone(),
        };
        let laya_validate_sig = laya_keypair.sign(laya_validate_payload.encode().as_slice());
        let laya_validate_sig: MultiSignature = laya_validate_sig.into();

        assert_ok!(Pallet::validate(
            RuntimeOrigin::signed(LAYA),
            laya_validate_payload,
            laya_validate_sig,
        ));

        let alan_validate_payload = ValidatePayloadOf {
            public: alan_public.clone(),
        };
        let alan_validate_sig = alan_keypair.sign(alan_validate_payload.encode().as_slice());
        let alan_validate_sig: MultiSignature = alan_validate_sig.into();

        assert_ok!(Pallet::validate(
            RuntimeOrigin::signed(ALAN),
            alan_validate_payload,
            alan_validate_sig,
        ));

        // ============================================================================
        // SESSION 1 - PHASE 5: AFFIDAVIT SUBMISSION (Block 300)
        // ============================================================================
        // Authors submit affidavits declaring intent to participate in next session
        // Affidavits include rotation keys for session 3 participation
        System::set_block_number(300);
        AllowAffidavits::put(true);

        // Generate rotation keypairs for session 3 election
        // These will be used when authors run elections or participate in session 3
        let alice_rotate_keypair = sr25519::Pair::from_seed(&[11u8; 32]);
        let nix_rotate_keypair = sr25519::Pair::from_seed(&[12u8; 32]);
        let mike_rotate_keypair = sr25519::Pair::from_seed(&[13u8; 32]);
        let bob_rotate_keypair = sr25519::Pair::from_seed(&[14u8; 32]);
        let charlie_rotate_keypair = sr25519::Pair::from_seed(&[15u8; 32]);
        let dev_rotate_keypair = sr25519::Pair::from_seed(&[16u8; 32]);
        let laya_rotate_keypair = sr25519::Pair::from_seed(&[17u8; 32]);
        let alan_rotate_keypair = sr25519::Pair::from_seed(&[18u8; 32]);

        let alice_rotate_public = MultiSigner::from(alice_rotate_keypair.public());
        let nix_rotate_public = MultiSigner::from(nix_rotate_keypair.public());
        let mike_rotate_public = MultiSigner::from(mike_rotate_keypair.public());
        let bob_rotate_public = MultiSigner::from(bob_rotate_keypair.public());
        let charlie_rotate_public = MultiSigner::from(charlie_rotate_keypair.public());
        let dev_rotate_public = MultiSigner::from(dev_rotate_keypair.public());
        let laya_rotate_public = MultiSigner::from(laya_rotate_keypair.public());
        let alan_rotate_public = MultiSigner::from(alan_rotate_keypair.public());

        // Submit affidavits for all authors via declare() extrinsic
        // Create payload with rotation key -> Sign with current key -> Submit
        // Alice submits affidavit
        let alice_affidavit_payload = AffidavitPayloadOf {
            public: alice_public.clone(),
            rotate: alice_rotate_public.clone().into_account().into(),
        };
        let alice_affidavit_sig = alice_keypair.sign(alice_affidavit_payload.encode().as_slice());
        let alice_affidavit_sig: MultiSignature = alice_affidavit_sig.into();

        assert_ok!(Pallet::declare(
            RuntimeOrigin::none(),
            alice_affidavit_payload,
            alice_affidavit_sig,
        ));

        // NIX submits affidavit
        let nix_affidavit_payload = AffidavitPayloadOf {
            public: nix_public.clone(),
            rotate: nix_rotate_public.clone().into_account().into(),
        };
        let nix_affidavit_sig = nix_keypair.sign(nix_affidavit_payload.encode().as_slice());
        let nix_affidavit_sig: MultiSignature = nix_affidavit_sig.into();

        assert_ok!(Pallet::declare(
            RuntimeOrigin::none(),
            nix_affidavit_payload,
            nix_affidavit_sig,
        ));

        // MIKE submits affidavit
        let mike_affidavit_payload = AffidavitPayloadOf {
            public: mike_public.clone(),
            rotate: mike_rotate_public.clone().into_account().into(),
        };
        let mike_affidavit_sig = mike_keypair.sign(mike_affidavit_payload.encode().as_slice());
        let mike_affidavit_sig: MultiSignature = mike_affidavit_sig.into();

        assert_ok!(Pallet::declare(
            RuntimeOrigin::none(),
            mike_affidavit_payload,
            mike_affidavit_sig,
        ));

        // BOB submits affidavit
        let bob_affidavit_payload = AffidavitPayloadOf {
            public: bob_public.clone(),
            rotate: bob_rotate_public.clone().into_account().into(),
        };
        let bob_affidavit_sig = bob_keypair.sign(bob_affidavit_payload.encode().as_slice());
        let bob_affidavit_sig: MultiSignature = bob_affidavit_sig.into();

        assert_ok!(Pallet::declare(
            RuntimeOrigin::none(),
            bob_affidavit_payload,
            bob_affidavit_sig,
        ));

        // CHARLIE submits affidavit
        let charlie_affidavit_payload = AffidavitPayloadOf {
            public: charlie_public.clone(),
            rotate: charlie_rotate_public.clone().into_account().into(),
        };
        let charlie_affidavit_sig =
            charlie_keypair.sign(charlie_affidavit_payload.encode().as_slice());
        let charlie_affidavit_sig: MultiSignature = charlie_affidavit_sig.into();

        assert_ok!(Pallet::declare(
            RuntimeOrigin::none(),
            charlie_affidavit_payload,
            charlie_affidavit_sig,
        ));

        // DAVE submits affidavit
        let dev_affidavit_payload = AffidavitPayloadOf {
            public: dev_public.clone(),
            rotate: dev_rotate_public.clone().into_account().into(),
        };
        let dev_affidavit_sig = dev_keypair.sign(dev_affidavit_payload.encode().as_slice());
        let dev_affidavit_sig: MultiSignature = dev_affidavit_sig.into();

        assert_ok!(Pallet::declare(
            RuntimeOrigin::none(),
            dev_affidavit_payload,
            dev_affidavit_sig,
        ));

        // LAYA submits affidavit
        let laya_affidavit_payload = AffidavitPayloadOf {
            public: laya_public.clone(),
            rotate: laya_rotate_public.clone().into_account().into(),
        };
        let laya_affidavit_sig = laya_keypair.sign(laya_affidavit_payload.encode().as_slice());
        let laya_affidavit_sig: MultiSignature = laya_affidavit_sig.into();

        assert_ok!(Pallet::declare(
            RuntimeOrigin::none(),
            laya_affidavit_payload,
            laya_affidavit_sig,
        ));

        // ALAN submits affidavit
        let alan_affidavit_payload = AffidavitPayloadOf {
            public: alan_public.clone(),
            rotate: alan_rotate_public.clone().into_account().into(),
        };
        let alan_affidavit_sig = alan_keypair.sign(alan_affidavit_payload.encode().as_slice());
        let alan_affidavit_sig: MultiSignature = alan_affidavit_sig.into();

        assert_ok!(Pallet::declare(
            RuntimeOrigin::none(),
            alan_affidavit_payload,
            alan_affidavit_sig,
        ));

        // ============================================================================
        // SESSION 1 - PHASE 6: ELECTION EXECUTION (Block 350)
        // ============================================================================
        // Any author who submitted affidavit can run the election
        // Alice runs election using her rotation key via elect() extrinsic
        System::set_block_number(350);
        ForceMaxElected::put(true);
        ElectionRunnerPoints::put(8); // Bonus points for running election
        set_block_author(ALICE);
        let alice_election_payload = ElectionPayloadOf {
            public: alice_rotate_public.clone(),
        };
        let alice_election_sig =
            alice_rotate_keypair.sign(alice_election_payload.encode().as_slice());
        let alice_election_sig: MultiSignature = alice_election_sig.into();

        assert_ok!(Pallet::elect(
            RuntimeOrigin::none(),
            alice_election_payload,
            alice_election_sig,
        ));

        // Verify election results via ElectAuthors::reveal()
        let actual_elected = Internals::reveal().unwrap();
        let expected_elected = vec![BOB, MIKE, LAYA, NIX, ALICE, ALAN];
        assert_eq!(actual_elected.len(), 6);
        assert_eq!(actual_elected, expected_elected);

        // ============================================================================
        // SESSION 1 - PHASE 7: BLOCK PRODUCTION & POINT ACCUMULATION SIMULATION (Blocks 400-450)
        // ============================================================================
        // Elected authors produce blocks in round-robin fashion
        // Points tracked via AuthorPoints::add_point() for each produced block
        System::set_block_number(400);

        // ALICE produces 6 blocks (400-406)
        for _i in 0..6 {
            System::set_block_number(System::block_number() + 1);
            Pallet::add_point(&ALICE).unwrap();
        }

        // NIX produces 4 blocks (406-410)
        for _i in 0..4 {
            System::set_block_number(System::block_number() + 1);
            Pallet::add_point(&NIX).unwrap();
        }

        // MIKE produces 5 blocks (410-415)
        for _i in 0..5 {
            System::set_block_number(System::block_number() + 1);
            Pallet::add_point(&MIKE).unwrap();
        }

        // BOB produces 3 blocks (415-418)
        for _i in 0..3 {
            System::set_block_number(System::block_number() + 1);
            Pallet::add_point(&BOB).unwrap();
        }

        // DAVE produces 4 blocks (418-422)
        for _i in 0..4 {
            System::set_block_number(System::block_number() + 1);
            Pallet::add_point(&LAYA).unwrap();
        }

        // ALAN produces 6 blocks (422-428)
        for _i in 0..6 {
            System::set_block_number(System::block_number() + 1);
            Pallet::add_point(&ALAN).unwrap();
        }

        //  ALICE produces 7 more blocks (428-435)
        for _i in 0..7 {
            System::set_block_number(System::block_number() + 1);
            Pallet::add_point(&ALICE).unwrap();
        }

        // NIX produces 7 more blocks (435-442)
        for _i in 0..7 {
            System::set_block_number(System::block_number() + 1);
            Pallet::add_point(&NIX).unwrap();
        }

        // BOB produces 8 more blocks (442-450)
        for _i in 0..8 {
            System::set_block_number(System::block_number() + 1);
            Pallet::add_point(&BOB).unwrap();
        }

        // Verify accumulated points for session 1
        // Points stored in BlockPointsStore and accessible via AuthorPoints::points_of()
        let alice_points = PointsAdapter::points_of(&ALICE).unwrap();
        Pallet::points_of(&ALICE).unwrap();
        assert_eq!(alice_points, 13);
        let nix_points = Pallet::points_of(&NIX).unwrap();
        assert_eq!(nix_points, 11);
        let bob_points = Pallet::points_of(&BOB).unwrap();
        assert_eq!(bob_points, 11);
        let mike_points = Pallet::points_of(&MIKE).unwrap();
        assert_eq!(mike_points, 5);
        let laya_points = Pallet::points_of(&LAYA).unwrap();
        assert_eq!(laya_points, 4);
        let alan_points = Pallet::points_of(&ALAN).unwrap();
        assert_eq!(alan_points, 6);

        // ============================================================================
        // SESSION 1 - PHASE 8: OFFENCE DETECTION & PENALTY (Block 475)
        // ============================================================================
        // Simulate offence detection for NIX (e.g., equivocation, being offline)
        // Demonstrates OnOffenceHandler integration with penalty subsystem
        System::set_block_number(475);

        let offenders = vec![OffenceDetails {
            offender: (NIX, NIX),
            reporters: vec![CHARLIE, MIKE], // Who reported the offence
        }];
        let slash_fraction = vec![PenaltyRatio::from_percent(10)]; // 10% penalty

        // Trigger offence handling via OnOffenceHandler::on_offence()
        // This delegates to PenalizeAuthors::penalize_authors()
        Pallet::on_offence(&offenders, &slash_fraction, 1);

        // Verify penalty was scheduled via RoleAdapter
        // Penalty will be applied at block 479 (current_block + grace_period)
        let nix_penalties = RoleAdapter::get_penalties_of(&NIX).unwrap();
        assert_eq!(nix_penalties, vec![(479, PenaltyRatio::from_percent(10))]);

        // ============================================================================
        // SESSION 1 - PHASE 9: SESSION TRANSITION & REWARD DISTRIBUTION (Blocks 600-603)
        // ============================================================================
        System::set_block_number(600);

        // Prepare next session's validators via SessionManager::new_session()
        // This calls ElectAuthors::reveal() to get elected authors
        // Also awards election runner (Alice) with bonus points
        let mut next_session_authors = Pallet::new_session(2).unwrap();
        let mut expeected_authors = vec![BOB, MIKE, LAYA, ALICE, NIX, ALAN];
        next_session_authors.sort();
        expeected_authors.sort();
        assert_eq!(
            next_session_authors,
            expeected_authors
        );

        // Verify election runner received bonus points in session 1
        let alice_points = PointsAdapter::points_of(&ALICE).unwrap();
        assert_eq!(alice_points, 21); // 13 base + 8 election bonus

        System::set_block_number(601);

        // End session 1 - triggers reward distribution via SessionManager::end_session()
        // This calls RewardAuthors::reward_authors() to distribute rewards
        Pallet::end_session(1);

        // Query reward distribution information
        let payout_total = Internals::payout();
        assert_eq!(payout_total, 100); // Total rewards to distribute

        // Verify proportional distribution based on points
        // Total points: 21 + 11 + 11 + 5 + 4 + 6 = 58
        // Each author gets: (their_points / total_points) * payout_total
        let payout_for = Internals::payout_for();
        let expected_payout_for = vec![
            (BOB, 11),   // (11/58) * 100 = 19
            (MIKE, 5),   // (5/58) * 100 = 9
            (LAYA, 4),   // (4/58) * 100 = 7
            (NIX, 11),   // (11/58) * 100 = 19
            (ALICE, 21), // (21/58) * 100 = 36
            (ALAN, 6),   // (6/58) * 100 = 10
        ];
        assert_eq!(payout_for, expected_payout_for);

        // Verify rewards were scheduled via RoleAdapter for block 603
        let alice_scheduled_reward = RoleAdapter::get_rewards_of(&ALICE).unwrap();
        let expected_alice_rewards = vec![(603, 36)];
        assert_eq!(alice_scheduled_reward, expected_alice_rewards);

        let bob_scheduled_reward = RoleAdapter::get_rewards_of(&BOB).unwrap();
        let expected_bob_rewards = vec![(603, 19)];
        assert_eq!(bob_scheduled_reward, expected_bob_rewards);

        let mike_scheduled_reward = RoleAdapter::get_rewards_of(&MIKE).unwrap();
        let expected_mike_rewards = vec![(603, 9)];
        assert_eq!(mike_scheduled_reward, expected_mike_rewards);

        let nix_scheduled_reward = RoleAdapter::get_rewards_of(&NIX).unwrap();
        let expected_nix_rewards = vec![(603, 19)];
        assert_eq!(nix_scheduled_reward, expected_nix_rewards);

        let alan_scheduled_reward = RoleAdapter::get_rewards_of(&ALAN).unwrap();
        let expected_alan_rewards = vec![(603, 10)];
        assert_eq!(alan_scheduled_reward, expected_alan_rewards);

        let laya_scheduled_reward = RoleAdapter::get_rewards_of(&LAYA).unwrap();
        let expected_laya_rewards = vec![(603, 7)];
        assert_eq!(laya_scheduled_reward, expected_laya_rewards);

        // ============================================================================
        // SESSION 2 - PHASE 10: NEW SESSION INITIALIZATION (Block 602)
        // ============================================================================            
        System::set_block_number(602);
        Pallet::start_session(2);

        // Verify session 2 state
        assert_eq!(CurrentSession::get(), 2);
        assert_eq!(SessionStartsAt::get(), 602);

        // Verify new session has clean point tracking
        let session_2_alice_points = PointsAdapter::points_of(&ALICE);
        assert!(session_2_alice_points.is_err());

        // ============================================================================
        // SESSION 2 - PHASE 11: AUTHOR WITHDRAWAL (Block 620)
        // ============================================================================
        // MIKE and NIX decide to withdraw from session 3 participation
        // Use chill() extrinsic to remove their affidavit keys for next session
        System::set_block_number(620);

        let mike_affidavit_id: AffidavitId = mike_rotate_public.clone().into_account().into();
        assert_ok!(Pallet::chill(
            RuntimeOrigin::signed(MIKE),
            mike_affidavit_id.clone(),
        ));

        let nix_affidavit_id: AffidavitId = nix_rotate_public.clone().into_account().into();
        assert_ok!(Pallet::chill(
            RuntimeOrigin::signed(NIX),
            nix_affidavit_id.clone(),
        ));

        // Verify affidavit keys removed for session 3
        assert!(AffidavitKeys::get((3, mike_affidavit_id)).is_none());
        assert!(AffidavitKeys::get((3, nix_affidavit_id)).is_none());
    })
}

// ===============================================================================
// `````````````````````````` OFFCHAIN WORKER LIFECYCLE ``````````````````````````
// ===============================================================================

#[test]
fn ocw_hook_end_to_end_pipeline() {
    let mut env = new_ocw_env();
    env.ext.execute_with(|| {
        set_session_config();
        CurrentSession::put(1);
        let users = vec![ALICE, BOB];
        set_default_users_balance_and_hold(users).unwrap();
        enroll_author_with_default_collateral(ALICE).unwrap();
        direct_fund_author(BOB, ALICE, STANDARD_FUND).unwrap();

        ocw_step();

        let active_afdt_key = get_afdt_key();
        let nxt_afdt_key = get_next_afdt_key();
        assert!(active_afdt_key.is_none());
        assert!(nxt_afdt_key.is_none());
        assert_eq!(affidavit_key_count(), 0);

        ocw_step();

        let active_afdt_key = get_afdt_key();
        assert!(active_afdt_key.is_some());
        assert_eq!(affidavit_key_count(), 1);
        assert!(get_finalized_afdt_key().is_none());

        while get_finalized_afdt_key().is_none() {
            ocw_step();
        }

        let finalized_afdt_key = get_finalized_afdt_key();
        assert!(finalized_afdt_key.is_some());

        let (val_payload, sig) = Pallet::sign_validate_payload().unwrap();

        Pallet::validate(RuntimeOrigin::signed(ALICE), val_payload, sig).unwrap();

        let for_session = CurrentSession::get() + 1;
        let finalized_afdt_key = finalized_afdt_key.unwrap();
        assert!(AffidavitKeys::contains_key((
            for_session,
            finalized_afdt_key.clone()
        )));

        while System::block_number() < AFDT_SUBMISSION_START {
            ocw_step();
            assert_eq!(env.pool_state.read().transactions.len(), 0);
        }

        let current_block = System::block_number();
        assert!(current_block == AFDT_SUBMISSION_START);

        ocw_step();

        let tx = env.pool_state.read().transactions.clone();
        assert_eq!(tx.len(), 1);
        let submited_ext = env.pool_state.read().transactions.last().unwrap().clone();
        let ext_decode = UncheckedExtrinsic::decode(&mut &submited_ext[..]).unwrap();
        assert!(matches!(
            ext_decode.function,
            RuntimeCall::ChainManager(crate::Call::declare { .. })
        ));

        let next_afdt_key = get_finalized_next_afdt_key();
        assert!(next_afdt_key.is_some());

        let next_afdt_key = get_finalized_next_afdt_key().unwrap();
        let for_session_2 = CurrentSession::get() + 2;

        {
            let public = get_public_key(finalized_afdt_key.clone()).unwrap();
            let payload = AffidavitPayloadOf {
                public: public.clone(),
                rotate: next_afdt_key.clone(),
            };
            let signature = sign_payload(&payload.encode(), public.clone());

            let call = Call::declare { payload, signature };

            let validity = Pallet::validate_unsigned(TransactionSource::Local, &call);
            assert!(validity.is_ok());

            {
                let payload = AffidavitPayloadOf {
                    public: public.clone(),
                    rotate: next_afdt_key.clone(),
                };
                let signature = sign_payload(&payload.encode(), public);
                Pallet::declare(RuntimeOrigin::none(), payload, signature).unwrap();
            }
        }
        assert!(AffidavitKeys::contains_key((
            for_session,
            finalized_afdt_key
        )));
        assert!(AffidavitKeys::contains_key((
            for_session_2,
            next_afdt_key.clone()
        )));

        {
            let users = vec![MIKE, AMY, NIX, LAYA, JIM, DAVE];
            set_default_users_balance_and_hold(users).unwrap();
            let authors = vec![MIKE, AMY, NIX];
            enroll_authors_with_default_collateral(authors).unwrap();

            direct_fund_author(LAYA, MIKE, 250).unwrap();
            direct_fund_author(JIM, AMY, 350).unwrap();
            direct_fund_author(DAVE, NIX, 400).unwrap();

            let mike_afdt_key = generate_affidavit_id();
            let amy_afdt_key = generate_affidavit_id();
            let nix_afdt_key = generate_affidavit_id();

            ext_validate(MIKE, mike_afdt_key.clone()).unwrap();
            ext_validate(AMY, amy_afdt_key.clone()).unwrap();
            ext_validate(NIX, nix_afdt_key.clone()).unwrap();

            let mike_nxt_afdt_key = generate_affidavit_id();
            let amy_nxt_afdt_key = generate_affidavit_id();
            let nix_nxt_afdt_key = generate_affidavit_id();

            let mike_payload = TestAfdtPayload {
                active_afdt_pub: mike_afdt_key.clone(),
                next_afdt_pub: mike_nxt_afdt_key.clone(),
            };

            let amy_payload = TestAfdtPayload {
                active_afdt_pub: amy_afdt_key.clone(),
                next_afdt_pub: amy_nxt_afdt_key.clone(),
            };

            let nix_payload = TestAfdtPayload {
                active_afdt_pub: nix_afdt_key.clone(),
                next_afdt_pub: nix_nxt_afdt_key.clone(),
            };

            ext_declare_affidavit(MIKE, mike_payload).unwrap();
            ext_declare_affidavit(AMY, amy_payload).unwrap();
            ext_declare_affidavit(NIX, nix_payload).unwrap();
        }

        let tx = env.pool_state.read().transactions.clone();
        assert_eq!(tx.len(), 1);

        while System::block_number() < ELECTION_START {
            ocw_step();
            assert_eq!(env.pool_state.read().transactions.len(), 2);
        }
        let submited_ext = env.pool_state.read().transactions.last().unwrap().clone();
        let ext_decode = UncheckedExtrinsic::decode(&mut &submited_ext[..]).unwrap();
        assert!(matches!(
            ext_decode.function,
            RuntimeCall::ChainManager(crate::Call::declare { .. })
        ));

        let current_block = System::block_number();
        assert!(current_block == ELECTION_START);

        env.pool_state.write().transactions.clear();

        ocw_step();

        let tx = env.pool_state.read().transactions.clone();
        assert_eq!(tx.len(), 1);
        let submited_ext = env.pool_state.read().transactions.last().unwrap().clone();
        let ext_decode = UncheckedExtrinsic::decode(&mut &submited_ext[..]).unwrap();
        assert!(matches!(
            ext_decode.function,
            RuntimeCall::ChainManager(crate::Call::elect { .. })
        ));

        {
            let public = get_public_key(next_afdt_key.clone()).unwrap();
            let payload = ElectionPayloadOf {
                public: public.clone(),
            };
            let signature = sign_payload(&payload.encode(), public.clone());

            let call = Call::elect { payload, signature };

            let validity = Pallet::validate_unsigned(TransactionSource::Local, &call);
            assert!(validity.is_ok());

            let payload = ElectionPayloadOf {
                public: public.clone(),
            };
            let signature = sign_payload(&payload.encode(), public);
            set_block_author(ALICE);
            Pallet::elect(RuntimeOrigin::none(), payload, signature).unwrap();
        }

        env.pool_state.write().transactions.clear();

        while System::block_number() == SESSION_END {
            ocw_step();
            assert_eq!(env.pool_state.read().transactions.len(), 0);
        }

        let tx = env.pool_state.read().transactions.clone();
        assert_eq!(tx.len(), 0);
    })
}
