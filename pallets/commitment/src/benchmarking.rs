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

#![cfg(feature = "runtime-benchmarks")]
// ===============================================================================
// ````````````````````````````````` IMPORTS `````````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use super::*;
use crate::{types::*};

// --- FRAME Suite ---
use frame_suite::{commitment::*, Directive};

// --- FRAME Support ---
use frame_support::pallet_prelude::*;
use frame_support::traits::{
    fungible::{Inspect, InspectHold, Mutate, UnbalancedHold},
    tokens::{Fortitude, Precision},
};

// --- FRAME System ---
use frame_system::RawOrigin;

// --- FRAME Benchmarking ---
use frame_benchmarking::{account, v2::*};

// --- Substrate crates ---
use sp_runtime::PerThing;

// --- External crates ---
use scale_info::prelude::vec;

// ===============================================================================
// ``````````````````````````````````` ALIASES ```````````````````````````````````
// ===============================================================================
type System<T> = frame_system::Pallet<T>;

#[benchmarks]
mod benchmarks {
    use super::*;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````````` Constants ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    const SEED: u32 = 1;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` HELPERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    fn initiate_key_and_set_balance_and_hold<T: Config>(
        who: &Proprietor<T>,
        amount: AssetOf<T, ()>,
        amount_to_hold: AssetOf<T, ()>,
    ) -> DispatchResult {
        <T as Config>::Asset::set_balance(&who, amount);
        let hold_reason: <T as Config>::AssetHold = crate::HoldReason::PrepareForCommit.into();
        <T as Config>::Asset::set_balance_on_hold(&hold_reason, &who, amount_to_hold)?;
        Ok(())
    }

    fn get_balance<T: Config>(who: &Proprietor<T>) -> AssetOf<T, ()> {
        <T as Config>::Asset::balance(who)
    }

    fn get_balance_on_hold<T: Config>(who: &Proprietor<T>) -> AssetOf<T, ()> {
        let hold_reason: <T as Config>::AssetHold = crate::HoldReason::PrepareForCommit.into();
        <T as Config>::Asset::balance_on_hold(&hold_reason, who)
    }

    fn initiate_digest_with_default_balance<T: Config>(
        reason: &CommitReason<T, ()>,
        digest: &Digest<T>,
    ) -> DispatchResult {
        let mut digest_info = DigestInfo::default();
        // Initialize with default balance for Affirmative variant (index 0)
        digest_info
            .init_balance(&Default::default())
            .map_err(|_| "Failed to push default variant balance")?;
        DigestMap::<T>::insert((reason, digest), digest_info);
        Ok(())
    }

    fn prepare_and_initiate_index<T: Config>(
        who: &Proprietor<T>,
        reason: &CommitReason<T, ()>,
        entries: &[(EntryDigest<T>, T::Shares)],
        index_of: &IndexDigest<T>,
    ) -> DispatchResult {
        let index =
            <Pallet<T> as CommitIndex<Proprietor<T>>>::prepare_index(&who, &reason, entries)?;
        <Pallet<T> as CommitIndex<Proprietor<T>>>::set_index(&who, &reason, &index, &index_of)?;
        Ok(())
    }

    fn prepare_and_initiate_pool<T: Config>(
        who: &Proprietor<T>,
        reason: &CommitReason<T, ()>,
        entries: &[(EntryDigest<T>, T::Shares)],
        index_of: &IndexDigest<T>,
        pool_of: &PoolDigest<T>,
        commission: T::Commission,
    ) -> DispatchResult {
        prepare_and_initiate_index::<T>(&who, &reason, &entries, &index_of)?;
        <Pallet<T> as CommitPool<Proprietor<T>>>::set_pool(
            &who, &reason, &pool_of, &index_of, commission,
        )?;
        Ok(())
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````````` BENCHMARKS `````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[benchmark]
    fn deposit_reserve() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        initiate_key_and_set_balance_and_hold::<T>(&caller, 50u32.into(), 100u32.into()).unwrap();
        // --- Extrinsic call ---
        #[extrinsic_call]
        deposit_reserve(
            RawOrigin::Signed(caller.clone()),
            60u32.into(),
            PrecisionWrapper::BestEffort,
        );
        // --- Assert ---
        let current_hold_bal = get_balance_on_hold::<T>(&caller);
        assert_eq!(current_hold_bal, 150u32.into());
        let current_bal = get_balance::<T>(&caller);
        assert_eq!(current_bal, 0u32.into());
    }

    #[benchmark]
    fn withdraw_reserve() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let _hold_reason: <T as Config>::AssetHold = crate::HoldReason::PrepareForCommit.into();
        initiate_key_and_set_balance_and_hold::<T>(&caller, 50u32.into(), 100u32.into()).unwrap();
        // --- Extrinsic call ---
        #[extrinsic_call]
        withdraw_reserve(RawOrigin::Signed(caller.clone()), None);
        // --- Assert ---
        let current_bal = get_balance::<T>(&caller);
        assert_eq!(current_bal, 150u32.into());
        let current_hold_bal = get_balance_on_hold::<T>(&caller);
        assert_eq!(current_hold_bal, 0u32.into());
    }

    #[benchmark]
    fn withdraw_reserve_partial() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let _hold_reason: <T as Config>::AssetHold = crate::HoldReason::PrepareForCommit.into();
        initiate_key_and_set_balance_and_hold::<T>(&caller, 50u32.into(), 100u32.into()).unwrap();
        // --- Extrinsic call ---
        #[extrinsic_call]
        withdraw_reserve(RawOrigin::Signed(caller.clone()), Some(60u32.into()));
        // --- Assert ---
        let current_bal = get_balance::<T>(&caller);
        assert_eq!(current_bal, 110u32.into());
        let current_hold_bal = get_balance_on_hold::<T>(&caller);
        assert_eq!(current_hold_bal, 40u32.into());
    }

    #[benchmark]
    fn inspect_digest_model() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let digest_alpha: Digest<T> = account::<T::AccountId>("digest_alpha", 0, SEED);
        let reason: CommitReason<T, ()> = crate::FreezeReason::BenchTestReason.into();
        initiate_key_and_set_balance_and_hold::<T>(&caller, 100u32.into(), 100u32.into()).unwrap();
        System::<T>::set_block_number(2u32.into());
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &caller,
            &reason,
            &digest_alpha,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        System::<T>::set_block_number(6u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        inspect_digest_model(
            RawOrigin::Signed(caller.clone()),
            digest_alpha.clone(),
            reason,
        );
        // -- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::DigestModel {
                digest: DigestVariant::Direct(digest_alpha),
            })
            .into(),
        );
    }

    #[benchmark]
    fn inspect_commit_value() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let digest_alpha: Digest<T> = account::<T::AccountId>("digest_alpha", 0, SEED);
        let reason: CommitReason<T, ()> = crate::FreezeReason::BenchTestReason.into();
        initiate_key_and_set_balance_and_hold::<T>(&caller, 100u32.into(), 100u32.into()).unwrap();
        System::<T>::set_block_number(2u32.into());
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &caller,
            &reason,
            &digest_alpha,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        System::<T>::set_block_number(6u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        inspect_commit_value(RawOrigin::Signed(caller.clone()), reason);
        // -- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::CommitValue {
                model: DigestVariant::Direct(digest_alpha),
                reason: reason,
                value: 50u32.into(),
            })
            .into(),
        );
    }

    #[benchmark]
    fn inspect_index_value() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let digest_alpha: Digest<T> = account::<T::AccountId>("digest_alpha", 0, SEED);
        let digest_beta: Digest<T> = account::<T::AccountId>("digest_beta", 0, SEED);
        let index_digest: Digest<T> = account::<T::AccountId>("index_digest", 0, SEED);
        let reason: CommitReason<T, ()> = crate::FreezeReason::BenchTestReason.into();
        initiate_digest_with_default_balance::<T>(&reason, &digest_alpha).unwrap();
        initiate_digest_with_default_balance::<T>(&reason, &digest_beta).unwrap();
        initiate_key_and_set_balance_and_hold::<T>(&caller, 100u32.into(), 100u32.into()).unwrap();
        let entries = vec![
            (digest_alpha.clone(), 60u8.into()),
            (digest_beta.clone(), 40u8.into()),
        ];
        prepare_and_initiate_index::<T>(&caller, &reason, &entries, &index_digest).unwrap();
        System::<T>::set_block_number(2u32.into());
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &caller,
            &reason,
            &index_digest,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        System::<T>::set_block_number(6u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        inspect_index_value(
            RawOrigin::Signed(caller.clone()),
            reason,
            index_digest.clone(),
        );
        // -- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::IndexValue {
                index_of: index_digest,
                reason: reason,
                value: 50u32.into(),
            })
            .into(),
        );
    }

    #[benchmark]
    fn inspect_entry_value() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let digest_alpha: Digest<T> = account::<T::AccountId>("digest_alpha", 0, SEED);
        let digest_beta: Digest<T> = account::<T::AccountId>("digest_beta", 0, SEED);
        let index_digest: Digest<T> = account::<T::AccountId>("index_digest", 0, SEED);
        let reason: CommitReason<T, ()> = crate::FreezeReason::BenchTestReason.into();
        initiate_digest_with_default_balance::<T>(&reason, &digest_alpha).unwrap();
        initiate_digest_with_default_balance::<T>(&reason, &digest_beta).unwrap();
        initiate_key_and_set_balance_and_hold::<T>(&caller, 100u32.into(), 100u32.into()).unwrap();
        let entries = vec![
            (digest_alpha.clone(), 60u8.into()),
            (digest_beta.clone(), 40u8.into()),
        ];
        prepare_and_initiate_index::<T>(&caller, &reason, &entries, &index_digest).unwrap();
        System::<T>::set_block_number(2u32.into());
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &caller,
            &reason,
            &index_digest,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        System::<T>::set_block_number(6u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        inspect_entry_value(
            RawOrigin::Signed(caller.clone()),
            reason,
            index_digest.clone(),
            digest_alpha.clone(),
        );
        // -- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::IndexEntryValue {
                index_of: index_digest,
                reason: reason,
                entry_of: digest_alpha,
                value: 30u32.into(),
            })
            .into(),
        );
    }

    #[benchmark]
    fn inspect_entries_value() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let digest_alpha: Digest<T> = account::<T::AccountId>("digest_alpha", 0, SEED);
        let digest_beta: Digest<T> = account::<T::AccountId>("digest_beta", 0, SEED);
        let index_digest: Digest<T> = account::<T::AccountId>("index_digest", 0, SEED);
        let reason: CommitReason<T, ()> = crate::FreezeReason::BenchTestReason.into();
        initiate_digest_with_default_balance::<T>(&reason, &digest_alpha).unwrap();
        initiate_digest_with_default_balance::<T>(&reason, &digest_beta).unwrap();
        initiate_key_and_set_balance_and_hold::<T>(&caller, 100u32.into(), 100u32.into()).unwrap();
        let entries = vec![
            (digest_alpha.clone(), 60u8.into()),
            (digest_beta.clone(), 40u8.into()),
        ];
        prepare_and_initiate_index::<T>(&caller, &reason, &entries, &index_digest).unwrap();
        System::<T>::set_block_number(2u32.into());
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &caller,
            &reason,
            &index_digest,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        System::<T>::set_block_number(6u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        inspect_entries_value(
            RawOrigin::Signed(caller.clone()),
            reason,
            index_digest.clone(),
        );
        // -- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::IndexEntriesValue {
                index_of: index_digest,
                reason: reason,
                entries: vec![(digest_alpha, 30u32.into()), (digest_beta, 20u32.into())],
            })
            .into(),
        );
    }

    #[benchmark]
    fn inspect_pool_value() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let digest_alpha: Digest<T> = account::<T::AccountId>("digest_alpha", 0, SEED);
        let digest_beta: Digest<T> = account::<T::AccountId>("digest_beta", 0, SEED);
        let index_digest: Digest<T> = account::<T::AccountId>("index_digest", 0, SEED);
        let pool_digest: Digest<T> = account::<T::AccountId>("pool_digest", 0, SEED);
        let reason: CommitReason<T, ()> = crate::FreezeReason::BenchTestReason.into();
        initiate_key_and_set_balance_and_hold::<T>(&caller, 100u32.into(), 100u32.into()).unwrap();
        initiate_key_and_set_balance_and_hold::<T>(&bob, 100u32.into(), 100u32.into()).unwrap();
        initiate_key_and_set_balance_and_hold::<T>(&charlie, 100u32.into(), 100u32.into()).unwrap();
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &bob,
            &reason,
            &digest_alpha,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &charlie,
            &reason,
            &digest_beta,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        let entries = vec![
            (digest_alpha.clone(), 60u8.into()),
            (digest_beta.clone(), 40u8.into()),
        ];
        let commission = T::Commission::from_percent(5.into());
        prepare_and_initiate_pool::<T>(
            &caller,
            &reason,
            &entries,
            &index_digest,
            &pool_digest,
            commission,
        )
        .unwrap();
        System::<T>::set_block_number(2u32.into());
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &caller,
            &reason,
            &pool_digest,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        // --- Extrinsic call ---
        System::<T>::set_block_number(6u32.into());
        // --- Assert ---
        #[extrinsic_call]
        inspect_pool_value(
            RawOrigin::Signed(caller.clone()),
            reason,
            pool_digest.clone(),
        );

        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::PoolValue {
                pool_of: pool_digest,
                reason: reason,
                value: 50u32.into(),
            })
            .into(),
        );
    }

    #[benchmark]
    fn inspect_slot_value() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let digest_alpha: Digest<T> = account::<T::AccountId>("digest_alpha", 0, SEED);
        let digest_beta: Digest<T> = account::<T::AccountId>("digest_beta", 0, SEED);
        let index_digest: Digest<T> = account::<T::AccountId>("index_digest", 0, SEED);
        let pool_digest: Digest<T> = account::<T::AccountId>("pool_digest", 0, SEED);
        let reason: CommitReason<T, ()> = crate::FreezeReason::BenchTestReason.into();
        initiate_key_and_set_balance_and_hold::<T>(&caller, 100u32.into(), 100u32.into()).unwrap();
        initiate_key_and_set_balance_and_hold::<T>(&bob, 100u32.into(), 100u32.into()).unwrap();
        initiate_key_and_set_balance_and_hold::<T>(&charlie, 100u32.into(), 100u32.into()).unwrap();
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &bob,
            &reason,
            &digest_alpha,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &charlie,
            &reason,
            &digest_beta,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        let entries = vec![
            (digest_alpha.clone(), 60u8.into()),
            (digest_beta.clone(), 40u8.into()),
        ];
        let commission = T::Commission::from_percent(5.into());
        prepare_and_initiate_pool::<T>(
            &caller,
            &reason,
            &entries,
            &index_digest,
            &pool_digest,
            commission,
        )
        .unwrap();
        System::<T>::set_block_number(2u32.into());
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &caller,
            &reason,
            &pool_digest,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        System::<T>::set_block_number(6u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        inspect_slot_value(
            RawOrigin::Signed(caller.clone()),
            reason,
            pool_digest.clone(),
            digest_beta.clone(),
        );
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::PoolSlotValue {
                pool_of: pool_digest,
                reason: reason,
                slot_of: digest_beta,
                value: 20u32.into(),
            })
            .into(),
        );
    }

    #[benchmark]
    fn inspect_slots_value() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let digest_alpha: Digest<T> = account::<T::AccountId>("digest_alpha", 0, SEED);
        let digest_beta: Digest<T> = account::<T::AccountId>("digest_beta", 0, SEED);
        let index_digest: Digest<T> = account::<T::AccountId>("index_digest", 0, SEED);
        let pool_digest: Digest<T> = account::<T::AccountId>("pool_digest", 0, SEED);
        let reason: CommitReason<T, ()> = crate::FreezeReason::BenchTestReason.into();
        initiate_key_and_set_balance_and_hold::<T>(&caller, 100u32.into(), 100u32.into()).unwrap();
        initiate_key_and_set_balance_and_hold::<T>(&bob, 100u32.into(), 100u32.into()).unwrap();
        initiate_key_and_set_balance_and_hold::<T>(&charlie, 100u32.into(), 100u32.into()).unwrap();
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &bob,
            &reason,
            &digest_alpha,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &charlie,
            &reason,
            &digest_beta,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        let entries = vec![
            (digest_alpha.clone(), 60u8.into()),
            (digest_beta.clone(), 40u8.into()),
        ];
        let commission = T::Commission::from_percent(5.into());
        prepare_and_initiate_pool::<T>(
            &caller,
            &reason,
            &entries,
            &index_digest,
            &pool_digest,
            commission,
        )
        .unwrap();
        System::<T>::set_block_number(2u32.into());
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &caller,
            &reason,
            &pool_digest,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        System::<T>::set_block_number(6u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        inspect_slots_value(
            RawOrigin::Signed(caller.clone()),
            reason,
            pool_digest.clone(),
        );
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::PoolSlotsValue {
                pool_of: pool_digest,
                reason: reason,
                slots: vec![(digest_alpha, 30u32.into()), (digest_beta, 20u32.into())],
            })
            .into(),
        );
    }

    #[benchmark]
    fn inspect_pool_commission() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let digest_alpha: Digest<T> = account::<T::AccountId>("digest_alpha", 0, SEED);
        let digest_beta: Digest<T> = account::<T::AccountId>("digest_beta", 0, SEED);
        let index_digest: Digest<T> = account::<T::AccountId>("index_digest", 0, SEED);
        let pool_digest: Digest<T> = account::<T::AccountId>("pool_digest", 0, SEED);
        let reason: CommitReason<T, ()> = crate::FreezeReason::BenchTestReason.into();
        initiate_key_and_set_balance_and_hold::<T>(&caller, 100u32.into(), 100u32.into()).unwrap();
        initiate_key_and_set_balance_and_hold::<T>(&bob, 100u32.into(), 100u32.into()).unwrap();
        initiate_key_and_set_balance_and_hold::<T>(&charlie, 100u32.into(), 100u32.into()).unwrap();
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &bob,
            &reason,
            &digest_alpha,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &charlie,
            &reason,
            &digest_beta,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        let entries = vec![
            (digest_alpha.clone(), 60u8.into()),
            (digest_beta.clone(), 40u8.into()),
        ];
        let commission = T::Commission::from_percent(5.into());
        prepare_and_initiate_pool::<T>(
            &caller,
            &reason,
            &entries,
            &index_digest,
            &pool_digest,
            commission,
        )
        .unwrap();
        System::<T>::set_block_number(2u32.into());
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &caller,
            &reason,
            &pool_digest,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        System::<T>::set_block_number(6u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        inspect_pool_commission(
            RawOrigin::Signed(caller.clone()),
            reason,
            pool_digest.clone(),
        );
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::PoolCommission {
                pool_of: pool_digest,
                reason: reason,
                commission: commission,
            })
            .into(),
        );
    }

    #[benchmark]
    fn inspect_pool_manager() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let digest_alpha: Digest<T> = account::<T::AccountId>("digest_alpha", 0, SEED);
        let digest_beta: Digest<T> = account::<T::AccountId>("digest_beta", 0, SEED);
        let index_digest: Digest<T> = account::<T::AccountId>("index_digest", 0, SEED);
        let pool_digest: Digest<T> = account::<T::AccountId>("pool_digest", 0, SEED);
        let reason: CommitReason<T, ()> = crate::FreezeReason::BenchTestReason.into();
        initiate_key_and_set_balance_and_hold::<T>(&caller, 100u32.into(), 100u32.into()).unwrap();
        initiate_key_and_set_balance_and_hold::<T>(&bob, 100u32.into(), 100u32.into()).unwrap();
        initiate_key_and_set_balance_and_hold::<T>(&charlie, 100u32.into(), 100u32.into()).unwrap();
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &bob,
            &reason,
            &digest_alpha,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &charlie,
            &reason,
            &digest_beta,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        let entries = vec![
            (digest_alpha.clone(), 60u8.into()),
            (digest_beta.clone(), 40u8.into()),
        ];
        let commission = T::Commission::from_percent(5.into());
        prepare_and_initiate_pool::<T>(
            &caller,
            &reason,
            &entries,
            &index_digest,
            &pool_digest,
            commission,
        )
        .unwrap();
        System::<T>::set_block_number(2u32.into());
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &caller,
            &reason,
            &pool_digest,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        System::<T>::set_block_number(6u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        inspect_pool_manager(
            RawOrigin::Signed(caller.clone()),
            reason,
            pool_digest.clone(),
        );
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::PoolManager {
                pool_of: pool_digest,
                reason: reason,
                manager: caller,
            })
            .into(),
        );
    }

    #[benchmark]
    fn inspect_asset_to_issue() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let digest_alpha: Digest<T> = account::<T::AccountId>("digest_alpha", 0, SEED);
        let reason: CommitReason<T, ()> = crate::FreezeReason::BenchTestReason.into();
        initiate_key_and_set_balance_and_hold::<T>(&caller, 100u32.into(), 100u32.into()).unwrap();
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &caller,
            &reason,
            &digest_alpha,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        <Pallet<T> as Commitment<Proprietor<T>>>::set_digest_value(
            &reason,
            &digest_alpha,
            100u32.into(),
            &Directive::new(Precision::BestEffort, Fortitude::Polite),
        )
        .unwrap();
        // --- Extrinsic call ---
        #[extrinsic_call]
        inspect_asset_to_issue(RawOrigin::Signed(caller.clone()));
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::AssetIssuable {
                asset: 50u32.into(),
            })
            .into(),
        );
    }

    #[benchmark]
    fn inspect_asset_to_reap() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let digest_alpha: Digest<T> = account::<T::AccountId>("digest_alpha", 0, SEED);
        let reason: CommitReason<T, ()> = crate::FreezeReason::BenchTestReason.into();
        initiate_key_and_set_balance_and_hold::<T>(&caller, 100u32.into(), 100u32.into()).unwrap();
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &caller,
            &reason,
            &digest_alpha,
            50u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        <Pallet<T> as Commitment<Proprietor<T>>>::set_digest_value(
            &reason,
            &digest_alpha,
            35u32.into(),
            &Directive::new(Precision::BestEffort, Fortitude::Polite),
        )
        .unwrap();
        // --- Extrinsic call ---
        #[extrinsic_call]
        inspect_asset_to_reap(RawOrigin::Signed(caller.clone()));
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::AssetReapable {
                asset: 15u32.into(),
            })
            .into(),
        );
    }

    #[benchmark]
    fn inspect_reason_value() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let digest_alpha: Digest<T> = account::<T::AccountId>("digest_alpha", 0, SEED);
        let reason: CommitReason<T, ()> = crate::FreezeReason::BenchTestReason.into();
        initiate_key_and_set_balance_and_hold::<T>(&caller, 100u32.into(), 100u32.into()).unwrap();
        <Pallet<T> as Commitment<Proprietor<T>>>::place_commit(
            &caller,
            &reason,
            &digest_alpha,
            60u32.into(),
            &Directive::new(Precision::Exact, Fortitude::Polite),
        )
        .unwrap();
        // --- Extrinsic call ---
        #[extrinsic_call]
        inspect_reason_value(RawOrigin::Signed(caller.clone()), reason);
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::ReasonValuation {
                reason: reason,
                value: 60u32.into(),
            })
            .into(),
        );
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::commit_test_ext(), crate::mock::Test);
}
