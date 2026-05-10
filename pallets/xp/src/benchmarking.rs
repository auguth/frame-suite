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

#[cfg(feature = "runtime-benchmarks")]
// ===============================================================================
// ````````````````````````````````` IMPORTS `````````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use super::*;
use crate::types::{ForceGenesisConfig, XpId};
use crate::Pallet as Xp;

// --- FRAME Suite ---
use frame_suite::xp::*;

// --- FRAME Support ---
use frame_support::{assert_err, assert_ok};

// --- FRAME System ---
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin};

// --- FRAME Benchmarking ---
use frame_benchmarking::{account, v2::*};

// --- Substrate crates ---
use sp_std::{boxed::Box, vec, vec::Vec};

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
    // `````````````````````````````````` BENCHMARKS `````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[benchmark]
    fn handover() {
        let caller: T::AccountId = account("caller", 0, 0);
        let key: XpId<T> = account::<T::AccountId>("xpid_alfa", 0, SEED).into();
        let dest: T::AccountId = account("dest", 0, 0);
        <Xp<T> as XpMutate>::new_xp(&caller, &key);
        System::<T>::set_block_number(2u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        handover(RawOrigin::Signed(caller.clone()), key.clone(), dest.clone());
        // -- Asserts ---
        assert_ok!(<Xp<T> as XpOwner>::is_owner(&dest.clone(), &key.clone()));
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::XpOwner {
                id: key,
                owner: dest,
            })
            .into(),
        );
    }

    #[benchmark]
    fn dispose() {
        let caller: T::AccountId = whitelisted_caller();
        let alice: T::AccountId = account("alice", 0, SEED);
        let key_alpha: T::AccountId = account::<T::AccountId>("xpid_alfa", 0, SEED).into();
        let key_beta: T::AccountId = account::<T::AccountId>("xpid_beta", 0, SEED).into();
        let keys: Vec<XpId<T>> = vec![key_alpha, key_beta.clone()];
        frame_system::Pallet::<T>::set_block_number(1u32.into());
        for key in &keys {
            <Xp<T> as XpMutate>::new_xp(&alice, &key);
        }
        MinTimeStamp::<T>::set(2u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        dispose(RawOrigin::Signed(caller.clone()), alice, key_beta.clone());
        // -- Asserts ---
        assert_err!(
            <Xp<T> as XpSystem>::xp_exists(&key_beta),
            Error::<T>::XpNotFound
        );

        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::XpReap { id: key_beta }).into(),
        );
    }

    #[benchmark]
    fn force_handover() {
        let alice: T::AccountId = account("alice", 0, SEED);
        let key: XpId<T> = account::<T::AccountId>("xpid_alfa", 0, SEED).into();
        let dest: T::AccountId = account("dest", 0, 0);
        <Xp<T> as XpMutate>::new_xp(&alice, &key);
        System::<T>::set_block_number(2u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_handover(RawOrigin::Root, alice.clone(), key.clone(), dest.clone());
        // -- Asserts ---
        assert_ok!(<Xp<T> as XpOwner>::is_owner(&dest.clone(), &key.clone()));
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::XpOwner {
                id: key,
                owner: dest,
            })
            .into(),
        );
    }

    #[benchmark]
    fn force_update_min_time_stamp() {
        let default_min_time_stamp: BlockNumberFor<T> = 0u32.into();
        let new_min_time_stamp: BlockNumberFor<T> = 2u32.into();
        assert_eq!(MinTimeStamp::<T>::get(), default_min_time_stamp);
        // --- Extrinsic call ---
        System::<T>::set_block_number(4u32.into());
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::MinTimeStamp(new_min_time_stamp),
        );
        // -- Assert ---
        assert_eq!(MinTimeStamp::<T>::get(), new_min_time_stamp);
    }

    #[benchmark]
    fn call() {
        let caller: T::AccountId = whitelisted_caller();
        let key_alpha: XpId<T> = account::<T::AccountId>("xpid_alfa", 0, SEED).into();
        <Xp<T> as XpMutate>::new_xp(&caller, &key_alpha);
        let runtime_call = Box::new(<T as Config>::RuntimeCall::from(
            frame_system::Call::<T>::remark { remark: vec![] },
        ));

        System::<T>::set_block_number(2u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        call(RawOrigin::Signed(caller), key_alpha.clone(), runtime_call);
    }

    #[benchmark]
    fn force_update_init_xp() {
        let default_init_xp: T::Xp = 1u32.into();
        let new_init_xp: T::Xp = 3u32.into();
        assert_eq!(InitXp::<T>::get(), default_init_xp);
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(RawOrigin::Root, ForceGenesisConfig::InitXp(new_init_xp));
        // -- Assert ---
        assert_eq!(InitXp::<T>::get(), new_init_xp);
    }

    #[benchmark]
    fn force_update_min_pulse() {
        let default_min_pulse: T::Pulse = 3u32.into();
        let new_min_pulse: T::Pulse = 5u32.into();
        assert_eq!(MinPulse::<T>::get(), default_min_pulse);
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(RawOrigin::Root, ForceGenesisConfig::MinPulse(new_min_pulse));
        // -- Assert ---
        assert_eq!(MinPulse::<T>::get(), new_min_pulse);
    }

    #[benchmark]
    fn force_update_pulse_factor() {
        let threshold: T::Pulse = 100u8.into();
        let per_count: T::Pulse = 10u8.into();
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::PulseFactor {
                threshold,
                per_count,
            },
        );
        // -- Asserts ---
        let stepper = PulseFactor::<T>::get();
        assert_eq!(threshold, stepper.threshold);
        assert_eq!(per_count, stepper.per_count);
    }

    #[benchmark]
    fn inspect_xp_keys_of() {
        let caller: T::AccountId = account::<T::AccountId>("caller", 0, SEED).into();
        let key_alpha: XpId<T> = account::<T::AccountId>("xpid_alfa", 0, SEED).into();
        let key_beta: XpId<T> = account::<T::AccountId>("xpid_beta", 0, SEED).into();
        <Xp<T> as XpMutate>::new_xp(&caller, &key_alpha);
        <Xp<T> as XpMutate>::new_xp(&caller, &key_beta);
        // --- Extrinsic call ---
        #[extrinsic_call]
        inspect_xp_keys_of(RawOrigin::Signed(caller.clone()), caller.clone());
        // -- Asserts ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::XpOfOwner {
                owner: caller.clone(),
                ids: vec![key_alpha.clone(), key_beta.clone()],
            })
            .into(),
        );
    }

    #[benchmark]
    fn inspect_my_xp() {
        let caller: T::AccountId = whitelisted_caller();
        let key: XpId<T> = account::<T::AccountId>("xpid_alfa", 0, SEED).into();
        let points: T::Xp = 50u32.into();
        <Xp<T> as XpMutate>::new_xp(&caller, &key);
        <Xp<T> as XpMutate>::set_xp(&key, points).unwrap();
        System::<T>::set_block_number(2u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        inspect_my_xp(RawOrigin::Signed(caller.clone()), key.clone());
        // --- Asserts ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::Xp {
                id: key,
                xp: points,
            })
            .into(),
        );
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::xp_test_ext(), crate::mock::Test);
}
