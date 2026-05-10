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
// ````````````````````````````` XP INTEGRATION TESTS ````````````````````````````
// ===============================================================================

//! **Integration tests for the XP pallet.**
//!
//! Validates XP lifecycle, reward scaling, state transitions,
//! and supporting features such as locking, reserving, and ownership.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::mock::*;

// --- FRAME Suite ---
use frame_suite::{
    keys::{test_utils, KeySeedFor},
    xp::*,
};

// --- FRAME Support ---
use frame_support::{assert_err, assert_ok, traits::tokens::Precision};

// --- Substrate primitives ---
use sp_runtime::traits::BlakeTwo256;

// ===============================================================================
// `````````````````````````````` INTEGRATION TESTS ``````````````````````````````
// ===============================================================================

#[test]
fn create_and_earn_xp() {
    // Scenario: Create a new XP entry and simulate earning XP over multiple blocks.
    // Covers:
    //   - Initial XP creation and verification of default balances.
    //   - Earning XP in discrete block intervals, testing pulse logic and scaling.
    //   - Pulse increment after minimum block intervals (DiscreteAccumulator).
    //   - Earning XP after reaching min_pulse, verifying scaled rewards.
    //   - Locking XP for staking and testing aggressive earning logic.
    //   - Pulse increment and aggressive earning when lock is present.
    //
    // Ensures XP earning is time-bound, pulse-based, and lock-aware.
    xp_test_ext().execute_with(|| {
        // Create new XP entry for ALICE
        Pallet::new_xp(&ALICE, &XP_ALPHA);
        assert_ok!(Pallet::xp_exists(&XP_ALPHA));
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, InitXp::get());
        assert_eq!(xp.pulse.value, 0);

        // Earn XP in block 2 (no pulse increment yet)
        System::set_block_number(2);
        Pallet::earn_xp(&XP_ALPHA, XP_REWARD_COMMENT).unwrap();
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 10);
        assert_eq!(xp.pulse.value, 0);

        // Continue earning XP in subsequent blocks
        System::set_block_number(3);
        Pallet::earn_xp(&XP_ALPHA, XP_REWARD_COMMENT).unwrap();
        System::set_block_number(5);
        Pallet::earn_xp(&XP_ALPHA, XP_REWARD_COMMENT).unwrap();
        System::set_block_number(10);
        Pallet::earn_xp(&XP_ALPHA, XP_REWARD_PROPOSAL).unwrap();

        // At 5th earn, pulse is incremented to 1 (DiscreteAccumulator logic)
        System::set_block_number(15);
        Pallet::earn_xp(&XP_ALPHA, XP_REWARD_COMMENT).unwrap();
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 10);
        assert_eq!(xp.pulse.value, 1);

        // Now earning XP applies scaling due to min_pulse
        System::set_block_number(17);
        Pallet::earn_xp(&XP_ALPHA, XP_REWARD_PROPOSAL).unwrap();
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 20);
        assert_eq!(xp.pulse.value, 1);

        // Lock XP for staking reason, which boosts pulse increment and earning rate
        System::set_block_number(18);
        Pallet::lock_xp(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 10);
        assert_eq!(xp.pulse.value, 1);
        assert_eq!(xp.lock, 10);

        // Earn XP with lock present, pulse remains at 1
        System::set_block_number(20);
        Pallet::earn_xp(&XP_ALPHA, XP_REWARD_COMMENT).unwrap();
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 15);
        assert_eq!(xp.pulse.value, 1);

        // Continue earning and pulse increments to 2, boosting rewards
        System::set_block_number(22);
        Pallet::earn_xp(&XP_ALPHA, XP_REWARD_COMMENT).unwrap();
        System::set_block_number(27);
        Pallet::earn_xp(&XP_ALPHA, XP_REWARD_COMMENT).unwrap();
        System::set_block_number(29);
        Pallet::earn_xp(&XP_ALPHA, XP_REWARD_PROPOSAL).unwrap();
        // pulse is incremented to 2
        System::set_block_number(34);
        Pallet::earn_xp(&XP_ALPHA, XP_REWARD_COMMENT).unwrap();
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 40);
        assert_eq!(xp.pulse.value, 2);

        // Aggressive earning with increaced pulse
        System::set_block_number(40);
        Pallet::earn_xp(&XP_ALPHA, XP_REWARD_PROPOSAL).unwrap();
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 60);
        assert_eq!(xp.pulse.value, 2);
    });
}

#[test]
fn earn_xp_overflow() {
    // Scenario: Attempt to earn XP when the liquid balance is already at maximum.
    // Covers:
    //   - Creating a new XP entry and directly setting its liquid XP to SATURATED_MAX.
    //   - Earning XP in multiple blocks, which should succeed until overflow occurs.
    //   - Asserting that further earn_xp calls fail with ArithmeticError::Overflow.
    //
    // Ensures the pallet enforces upper bounds on liquid XP and handles overflow correctly.
    xp_test_ext().execute_with(|| {
        // Create new XP entry for ALICE
        Pallet::new_xp(&ALICE, &XP_ALPHA);
        // Directly set liquid XP to maximum value (for test intent)
        Pallet::set_xp(&ALICE, SATURATED_MAX).unwrap();

        // Attempt to earn XP in subsequent blocks; should succeed until overflow
        System::set_block_number(3);
        Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
        System::set_block_number(6);
        Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
        System::set_block_number(8);
        Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
        System::set_block_number(10);
        Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
        System::set_block_number(12);
        Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS).unwrap();
        System::set_block_number(15);
        // Fail: Attempt to earn should overflow and return error
        assert_err!(
            Pallet::earn_xp(&XP_ALPHA, DEFAULT_POINTS),
            Error::XpCapOverflowed
        );
    });
}

#[test]
fn transfer_owner() {
    // Scenario: Transfer XP ownership between accounts.
    // Covers:
    //   - Creating XP entries for two accounts.
    //   - Transferring ownership of an XP key from ALICE to BOB.
    //   - Verifying ownership is updated and XP keys are correctly listed for the new owner.
    //
    // Ensures XP ownership can be safely transferred and access control is enforced.
    xp_test_ext().execute_with(|| {
        // Create XP entry for ALICE and verify initial ownership
        Pallet::new_xp(&ALICE, &XP_ALPHA);
        assert_ok!(Pallet::is_owner(&ALICE, &XP_ALPHA));
        // Creat XP entry for BOB
        Pallet::new_xp(&BOB, &XP_BETA);
        // Transfer XP_ALPHA ownership from ALICE to BOB
        let _ = Pallet::transfer_owner(&ALICE, &XP_ALPHA, &BOB);
        // Verify ALICE is no longer owner and BOB is now owner of XP_ALPHA
        assert_err!(Pallet::is_owner(&ALICE, &XP_ALPHA), Error::InvalidXpOwner);
        assert_ok!(Pallet::is_owner(&BOB, &XP_ALPHA));
        // Verify BOB owns both XP_ALPHA and XP_BETA after transfer
        let actual = Pallet::xp_of_owner(&BOB).unwrap();
        let expected = vec![XP_ALPHA, XP_BETA];
        assert_eq!(expected, actual);
    });
}

#[test]
fn reap_xp() {
    // Scenario: Finalize (reap) an XP entry that is dormant and meets the minimum timestamp criteria.
    // Covers:
    //   - Creating a new XP entry and verifying existence.
    //   - Advancing block number and updating min_time_stamp to simulate dormancy.
    //   - Using ReapSupport::try_reap to safely finalize the XP entry.
    //   - Asserting XP entry is removed from storage and marked as reaped.
    //
    // Ensures dormant XP entries can be safely finalized and cannot be reused.
    xp_test_ext().execute_with(|| {
        System::set_block_number(2);
        // Create new XP entry
        Pallet::new_xp(&ALICE, &XP_ALPHA);
        assert_ok!(Pallet::xp_exists(&XP_ALPHA));
        System::set_block_number(4);
        System::set_block_number(6);
        System::set_block_number(12);
        // Update minimum timestamp to simulate dormancy
        Pallet::force_genesis_config(
            RuntimeOrigin::root(),
            crate::types::ForceGenesisConfig::MinTimeStamp(10),
        )
        .unwrap();
        System::set_block_number(20);
        // Attempt to reap dormant XP entry
        Pallet::try_reap(&XP_ALPHA).unwrap();
        // Assert that XP is removed and marked as reaped
        assert_err!(Pallet::xp_exists(&XP_ALPHA), Error::XpNotFound);
        assert_ok!(Pallet::is_reaped(&XP_ALPHA));
    });
}

#[test]
fn reserve_and_withdraw_xp() {
    // Scenario: Reserve XP points and test withdrawal logic.
    // Covers:
    //   - Reserving XP and verifying liquid/reserve balances.
    //   - Failing to reserve more than available liquid XP.
    //   - Partial withdrawal (BestEffort) and balance update.
    //   - Failing to withdraw more than reserved (Exact).
    //   - Full withdrawal and final balance check.
    //
    // Ensures XP Reserving is safe, error-aware, and reserve lifecycle is enforced.
    xp_test_ext().execute_with(|| {
        // Create new XP entry and set initial free XP.
        Pallet::new_xp(&ALICE, &XP_ALPHA);
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 10);
        assert_eq!(xp.reserve, 0);

        let new_points = 50;
        // Directly set free XP (for test intent).
        Pallet::set_xp(&ALICE, new_points).unwrap();
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 50);

        let reserve_points = 40;
        // Reserve XP for governance: liquid decreases, reserve increases.
        Pallet::reserve_xp(&XP_ALPHA, &GOVERNANCE, reserve_points).unwrap();
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 10);
        assert_eq!(xp.reserve, reserve_points);

        let underflow_reserve_points = 30;
        // Fail: Attempt to reserve more than available liquid XP.
        assert_err!(
            Pallet::reserve_xp(&XP_ALPHA, &GOVERNANCE, underflow_reserve_points),
            Error::InsufficientLiquidXp
        );

        let partial_withdraw = 20;
        // Partial withdrawal (BestEffort): liquid increases, reserve decreases.
        Pallet::withdraw_reserve_partial(
            &XP_ALPHA,
            &GOVERNANCE,
            partial_withdraw,
            Precision::BestEffort,
        )
        .unwrap();
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 30);
        assert_eq!(xp.reserve, 20);

        let underflow_withdraw = 40;
        // Fail: Attempt to withdraw more than reserved (Exact).
        assert_err!(
            Pallet::withdraw_reserve_partial(
                &XP_ALPHA,
                &GOVERNANCE,
                underflow_withdraw,
                Precision::Exact
            ),
            Error::InsufficientReserveXp
        );

        // Full withdrawal: all reserved XP returned to liquid.
        Pallet::withdraw_reserve(&XP_ALPHA, &GOVERNANCE).unwrap();
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 50);
        assert_eq!(xp.reserve, 0);
    });
}

#[test]
fn reserve_overflow() {
    // Scenario: Attempt to reserve XP when the reserve balance is already at maximum.
    // Covers:
    //   - Setting reserve XP to SATURATED_MAX for a governance reason.
    //   - Failing to reserve additional XP due to overflow.
    //
    // Ensures the pallet enforces upper bounds on reserve XP and handles overflow correctly.
    xp_test_ext().execute_with(|| {
        Pallet::new_xp(&ALICE, &XP_ALPHA);
        // Directly sets the reserve XP to its maximum value (for test intent).
        Pallet::set_reserve(&ALICE, &GOVERNANCE, SATURATED_MAX).unwrap();
        // Fail: Attempt to reserve more XP should fail due to overflow.
        assert_err!(
            Pallet::reserve_xp(&XP_ALPHA, &GOVERNANCE, DEFAULT_POINTS),
            Error::XpReserveCapOverflowed
        );
    });
}

#[test]
fn lock_and_withdraw_xp() {
    // Scenario: Lock XP points and test withdrawal logic.
    // Covers:
    //   - Creating a new XP entry and verifying initial balances.
    //   - Locking XP for a staking reason, ensuring liquid XP decreases and lock balance increases.
    //   - Failing to lock more XP than available (InsufficientLiquidXp).
    //   - Failing to lock zero points (CannotLockZero).
    //   - Withdrawing the lock, restoring XP to liquid and burning the lock (no partial withdrawal).
    //   - Verifying lock is removed after withdrawal.
    //
    // Ensures XP locking is safe, error-aware, and lock lifecycle is enforced.
    xp_test_ext().execute_with(|| {
        // Create new XP entry and verify initial balances
        Pallet::new_xp(&ALICE, &XP_ALPHA);
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 10);
        assert_eq!(xp.lock, 0);

        let new_points = 50;
        // Directly set free XP for test setup
        Pallet::set_xp(&ALICE, new_points).unwrap();
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 50);

        let lock_points = 30;
        // Lock XP for staking: liquid decreases, lock increases
        Pallet::lock_xp(&XP_ALPHA, &STAKING, lock_points).unwrap();
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 20);
        assert_eq!(xp.lock, lock_points);

        let underflow_lock_points = 30;
        // Fail: Attempt to lock more than available liquid XP
        assert_err!(
            Pallet::lock_xp(&XP_ALPHA, &STAKING, underflow_lock_points),
            Error::InsufficientLiquidXp
        );
        // Fail: Attempt to lock zero points
        assert_err!(
            Pallet::lock_xp(&XP_ALPHA, &STAKING, INVALID_POINTS),
            Error::CannotLockZero
        );

        // Withdraw lock: locked XP returned to liquid, lock is burned
        Pallet::withdraw_lock(&XP_ALPHA, &STAKING).unwrap();
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 50);
        assert_eq!(xp.lock, 0);
        // Verify lock is removed
        assert_err!(
            Pallet::lock_exists(&XP_ALPHA, &STAKING),
            Error::XpLockNotFound
        );
    });
}

#[test]
fn lock_overflow() {
    // Scenario: Attempt to lock XP when the lock balance is already at maximum.
    // Covers:
    //   - Creating a new XP entry and setting its lock to SATURATED_MAX.
    //   - Failing to lock additional XP due to arithmetic overflow.
    //
    // Ensures the pallet enforces upper bounds on locked XP and handles overflow correctly.
    xp_test_ext().execute_with(|| {
        Pallet::new_xp(&ALICE, &XP_ALPHA);
        // Directly set locked XP to maximum value (for runtime intent)
        Pallet::set_lock(&ALICE, &STAKING, SATURATED_MAX).unwrap();
        // Attempt to lock additional XP should fail due to overflow
        assert_err!(
            Pallet::lock_xp(&XP_ALPHA, &STAKING, DEFAULT_POINTS),
            Error::XpLockCapOverflowed
        );
    });
}

#[test]
fn begin_xp() {
    // Scenario: Safely initialize or earn XP using BeginXp, with lifecycle and reaping checks.
    // Covers:
    //   - Creating a new XP entry if it does not exist.
    //   - Earning XP if the entry exists and is not reaped.
    //   - Pulse logic and scaling as XP is earned over block intervals.
    //   - Updating min_pulse and verifying scaled rewards.
    //   - Reaping the XP entry and ensuring further begin_xp calls fail.
    //
    // Ensures XP lifecycle is respected, and begin_xp is safe against re-initialization of finalized entries.
    xp_test_ext().execute_with(|| {
        // Create new XP using begin_xp (entry does not exist)
        System::set_block_number(2);
        assert_err!(Pallet::xp_exists(&XP_ALPHA), Error::XpNotFound);
        Pallet::begin_xp(&ALICE, &XP_ALPHA, DEFAULT_POINTS).unwrap();
        assert_ok!(Pallet::xp_exists(&XP_ALPHA));

        // Earn XP using begin_xp (entry exists, not reaped)
        System::set_block_number(4);
        Pallet::begin_xp(&ALICE, &XP_ALPHA, DEFAULT_POINTS).unwrap();
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 10);
        assert_eq!(xp.pulse.value, 0);

        // Continue earning XP in subsequent blocks
        System::set_block_number(6);
        Pallet::begin_xp(&ALICE, &XP_ALPHA, DEFAULT_POINTS).unwrap();
        System::set_block_number(8);
        Pallet::begin_xp(&ALICE, &XP_ALPHA, DEFAULT_POINTS).unwrap();
        System::set_block_number(10);
        Pallet::begin_xp(&ALICE, &XP_ALPHA, DEFAULT_POINTS).unwrap();

        // Pulse is incremented after enough earns
        System::set_block_number(13);
        Pallet::begin_xp(&ALICE, &XP_ALPHA, DEFAULT_POINTS).unwrap();
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 10);
        assert_eq!(xp.pulse.value, 1);
        // Earning XP after min_pulse applies scaling
        System::set_block_number(15);
        Pallet::begin_xp(&ALICE, &XP_ALPHA, DEFAULT_POINTS).unwrap();
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
        assert_eq!(xp.free, 20);
        assert_eq!(xp.pulse.value, 1);

        // Reap the XP entry to finalize it
        System::set_block_number(15);
        Pallet::reap_xp(&XP_ALPHA).unwrap();
        assert_ok!(Pallet::is_reaped(&XP_ALPHA));

        // Attempting to begin_xp on a reaped entry should fail
        assert_err!(
            Pallet::begin_xp(&ALICE, &XP_ALPHA, DEFAULT_POINTS),
            Error::XpAlreadyReaped
        );
    });
}

// ===============================================================================
// `````````````````````````````` XP KEY-GEN TESTS ```````````````````````````````
// ===============================================================================

#[test]
fn generic_key_gen_deterministic_check() {
    xp_test_ext().execute_with(|| {
        type Id = AccountId;
        type Item = MockXp;
        type Salt = u32;
        type Hasher = BlakeTwo256;
        type Impl = KeySeedFor<Id, Item, Salt, Hasher, Test>;

        Pallet::new_xp(&ALICE, &XP_ALPHA);
        let salt = System::account_nonce(ALICE);
        let xp = Pallet::get_xp(&XP_ALPHA).unwrap();

        test_utils::run_keygen_deterministic_check::<Id, Item, Salt, Hasher, Test, Impl>(
            ALICE, xp, salt,
        );
    });
}

#[test]
fn generic_key_gen_collision_check() {
    xp_test_ext().execute_with(|| {
        type Id = AccountId;
        type Item = MockXp;
        type Salt = u32;
        type Hasher = BlakeTwo256;
        type Impl = KeySeedFor<Id, Item, Salt, Hasher, Test>;

        System::set_block_number(2);
        Pallet::new_xp(&ALICE, &XP_ALPHA);
        let xp_alpha = Pallet::get_xp(&XP_ALPHA).unwrap();
        let salt_alpha = System::account_nonce(ALICE);

        System::set_block_number(4);
        Pallet::new_xp(&BOB, &XP_BETA);
        let xp_beta = Pallet::get_xp(&XP_BETA).unwrap();
        Account::mutate(BOB, |info| {
            info.nonce = 4;
        });
        let salt_beta = System::account_nonce(BOB);

        assert_ne!(xp_alpha, xp_beta);
        assert_ne!(salt_alpha, salt_beta);
        assert_ne!(ALICE, BOB);

        test_utils::run_keygen_collision_check::<Id, Item, Salt, Hasher, Test, Impl>(
            ALICE, xp_alpha, salt_alpha, BOB, xp_beta, salt_beta,
        );
    });
}
