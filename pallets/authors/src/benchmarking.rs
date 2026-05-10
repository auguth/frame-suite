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
use crate::{types::*, MaxElected};

// --- FRAME Suite ---
use frame_suite::{
    commitment::*,
    roles::{CompensateRoles, FundRoles, RoleManager},
};
// --- FRAME Support ---
use frame_support::traits::{
    fungible::{Mutate},
    tokens::{Fortitude, Precision},
};
use frame_support::{assert_ok, pallet_prelude::*};

// --- FRAME System ---
use frame_system::{pallet_prelude::BlockNumberFor, RawOrigin};

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

    fn set_balance<T: Config>(
        who: &T::AccountId,
        amount: AuthorAsset<T>,
    ) -> DispatchResult {
        <T as Config>::Asset::set_balance(who, amount);
        Ok(())
    }

    fn prepare_and_initiate_index<T: Config>(
        who: &T::AccountId,
        reason: &FreezeReason,
        entries: &[(
            <<T as Config>::CommitmentAdapter as Commitment<T::AccountId>>::Digest,
            <<T as Config>::CommitmentAdapter as CommitIndex<T::AccountId>>::Shares,
        )],
        index_of: &IndexDigest<T>,
    ) -> DispatchResult {
        let reason_conv = <T as Config>::AssetFreeze::from(*reason);
        let index = <T as Config>::CommitmentAdapter::prepare_index(&who, &reason_conv, entries)?;
        <T as Config>::CommitmentAdapter::set_index(&who, &reason_conv, &index, &index_of)?;
        Ok(())
    }

    fn prepare_and_initiate_pool<T: Config>(
        who: &T::AccountId,
        reason: &FreezeReason,
        entries: &[(
            <<T as Config>::CommitmentAdapter as Commitment<T::AccountId>>::Digest,
            <<T as Config>::CommitmentAdapter as CommitIndex<T::AccountId>>::Shares,
        )],
        index_of: &IndexDigest<T>,
        pool_of: &PoolDigest<T>,
        commission: <<T as Config>::CommitmentAdapter as CommitPool<T::AccountId>>::Commission,
    ) -> DispatchResult {
        prepare_and_initiate_index::<T>(&who, &reason, &entries, &index_of)?;
        let reason_conv = <T as Config>::AssetFreeze::from(*reason);
        <T as Config>::CommitmentAdapter::set_pool(
            &who,
            &reason_conv,
            &pool_of,
            &index_of,
            commission,
        )?;
        Ok(())
    }

    fn gen_digest<T: Config>(
        target: &T::AccountId,
    ) -> Result<<<T as Config>::CommitmentAdapter as Commitment<T::AccountId>>::Digest, DispatchError>
    {
        let digest =
            <<T as Config>::CommitmentAdapter as Commitment<T::AccountId>>::gen_digest(target)?;
        Ok(digest)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````````` BENCHMARKS `````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[benchmark]
    fn enlist() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let collateral: AuthorAsset<T> = 100u32.into();
        set_balance::<T>(&caller, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        enlist(
            RawOrigin::Signed(caller.clone()),
            collateral,
            FortitudeWrapper::Force,
        );
        // --- Asserts ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::AuthorEnlisted {
                author: caller.clone(),
                collateral: collateral,
            })
            .into(),
        );
        assert_ok!(<Pallet<T> as RoleManager<Author<T>>>::role_exists(&caller));
    }

    #[benchmark]
    fn demit() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let collateral: AuthorAsset<T> = 100u32.into();
        set_balance::<T>(&caller, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(
            &caller.clone(),
            collateral,
            Fortitude::Force,
        )
        .unwrap();
        System::<T>::set_block_number(15u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::set_status(&caller.clone(), AuthorStatus::Active)
            .unwrap();
        System::<T>::set_block_number(20u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        demit(RawOrigin::Signed(caller.clone()));
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::AuthorResigned {
                author: caller.clone(),
                released: collateral,
            })
            .into(),
        );
    }

    #[benchmark]
    fn refill() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let collateral: AuthorAsset<T> = 100u32.into();
        set_balance::<T>(&caller, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(
            &caller.clone(),
            collateral,
            Fortitude::Force,
        )
        .unwrap();
        let raise: AuthorAsset<T> = 50u32.into();
        System::<T>::set_block_number(6u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        refill(
            RawOrigin::Signed(caller.clone()),
            raise,
            FortitudeWrapper::Force,
        );
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::AuthorCollateralRaised {
                author: caller.clone(),
                raised: raise,
            })
            .into(),
        );
    }

    #[benchmark]
    fn my_collateral() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&caller, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(
            &caller.clone(),
            collateral,
            Fortitude::Force,
        )
        .unwrap();
        System::<T>::set_block_number(6u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        my_collateral(RawOrigin::Signed(caller.clone()));
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::AuthorTotalCollateral {
                author: caller.clone(),
                collateral: collateral,
            })
            .into(),
        );
    }

    #[benchmark]
    fn direct_fund() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 250u32.into()).unwrap();
        set_balance::<T>(&charlie, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();
        System::<T>::set_block_number(15u32.into());
        let fund = 75u32.into();
        // --- Extrinsic call ---
        #[extrinsic_call]
        back(
            RawOrigin::Signed(charlie.clone()),
            FundingTarget::Direct(alice.clone()),
            fund,
            FortitudeWrapper::Force,
            PrecisionWrapper::Exact,
        );
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::AuthorFunded {
                author: alice,
                backer: charlie,
                amount: fund,
            })
            .into(),
        );
    }

    #[benchmark]
    fn index_fund() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let alan: T::AccountId = account::<T::AccountId>("alan_id", 0, SEED);
        let mike: T::AccountId = account::<T::AccountId>("mike_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let index_digest_account: T::AccountId = account::<T::AccountId>("index_digest", 0, SEED);
        let index_digest = gen_digest::<T>(&index_digest_account).unwrap();
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 250u32.into()).unwrap();
        set_balance::<T>(&bob, 300u32.into()).unwrap();
        set_balance::<T>(&alan, 300u32.into()).unwrap();
        set_balance::<T>(&mike, 500u32.into()).unwrap();
        set_balance::<T>(&charlie, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&bob, collateral, Fortitude::Force).unwrap();
        System::<T>::set_block_number(15u32.into());
        let alan_fund = 75u32.into();
        let by_alan = Funder::Direct(alan.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &by_alan,
            alan_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let charlie_fund = 75u32.into();
        let by_charlie = Funder::Direct(charlie);
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &bob,
            &by_charlie,
            charlie_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let alice_digest = gen_digest::<T>(&alice).unwrap();
        let bob_digest = gen_digest::<T>(&bob).unwrap();
        let entries = vec![(alice_digest, 40u8.into()), (bob_digest, 60u8.into())];
        prepare_and_initiate_index::<T>(
            &mike,
            &FreezeReason::AuthorFunding,
            &entries,
            &index_digest,
        )
        .unwrap();
        System::<T>::set_block_number(25u32.into());
        let mike_fund = 100u32;
        // --- Extrinsic call ---
        #[extrinsic_call]
        back(
            RawOrigin::Signed(mike.clone()),
            FundingTarget::Index(index_digest.clone()),
            mike_fund.into(),
            FortitudeWrapper::Force,
            PrecisionWrapper::Exact,
        );
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::IndexFunded {
                index: index_digest,
                backer: mike,
                amount: mike_fund.into(),
            })
            .into(),
        );
    }

    #[benchmark]
    fn pool_fund() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let alan: T::AccountId = account::<T::AccountId>("alan_id", 0, SEED);
        let mike: T::AccountId = account::<T::AccountId>("mike_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let index_digest_account: T::AccountId = account::<T::AccountId>("index_digest", 0, SEED);
        let index_digest = gen_digest::<T>(&index_digest_account).unwrap();
        let pool_digest_account: T::AccountId = account::<T::AccountId>("pool_digest", 0, SEED);
        let pool_digest = gen_digest::<T>(&pool_digest_account).unwrap();
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 250u32.into()).unwrap();
        set_balance::<T>(&bob, 300u32.into()).unwrap();
        set_balance::<T>(&alan, 300u32.into()).unwrap();
        set_balance::<T>(&mike, 500u32.into()).unwrap();
        set_balance::<T>(&charlie, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&bob, collateral, Fortitude::Force).unwrap();
        System::<T>::set_block_number(15u32.into());
        let alan_fund = 75u32.into();
        let by_alan = Funder::Direct(alan.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &by_alan,
            alan_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let charlie_fund = 75u32.into();
        let by_charlie = Funder::Direct(charlie);
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &bob,
            &by_charlie,
            charlie_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let alice_digest = gen_digest::<T>(&alice).unwrap();
        let bob_digest = gen_digest::<T>(&bob).unwrap();
        let entries = vec![(alice_digest, 40u8.into()), (bob_digest, 60u8.into())];
        prepare_and_initiate_pool::<T>(
            &mike,
            &FreezeReason::AuthorFunding,
            &entries,
            &index_digest,
            &pool_digest,
            Commission::<T>::from_percent(5.into()),
        )
        .unwrap();
        System::<T>::set_block_number(25u32.into());
        let mike_fund = 100u32;
        // --- Extrinsic call ---
        #[extrinsic_call]
        back(
            RawOrigin::Signed(mike.clone()),
            FundingTarget::Pool(pool_digest.clone()),
            mike_fund.into(),
            FortitudeWrapper::Force,
            PrecisionWrapper::Exact,
        );
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::PoolFunded {
                pool: pool_digest,
                backer: mike,
                amount: mike_fund.into(),
            })
            .into(),
        );
    }

    #[benchmark]
    fn release_direct_fund() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 250u32.into()).unwrap();
        set_balance::<T>(&charlie, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();
        System::<T>::set_block_number(15u32.into());
        let fund = 75u32.into();
        let fund_by: Funder<T> = Funder::Direct(charlie.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &fund_by,
            fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        System::<T>::set_block_number(20u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        exit(
            RawOrigin::Signed(charlie.clone()),
            FundingTarget::Direct(alice.clone()),
        );
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::AuthorDrawn {
                author: alice,
                backer: charlie,
                amount: fund,
            })
            .into(),
        );
    }

    #[benchmark]
    fn release_index_fund() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let alan: T::AccountId = account::<T::AccountId>("alan_id", 0, SEED);
        let mike: T::AccountId = account::<T::AccountId>("mike_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let index_digest_account: T::AccountId = account::<T::AccountId>("index_digest", 0, SEED);
        let index_digest = gen_digest::<T>(&index_digest_account).unwrap();
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 250u32.into()).unwrap();
        set_balance::<T>(&bob, 300u32.into()).unwrap();
        set_balance::<T>(&alan, 300u32.into()).unwrap();
        set_balance::<T>(&mike, 500u32.into()).unwrap();
        set_balance::<T>(&charlie, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&bob, collateral, Fortitude::Force).unwrap();
        System::<T>::set_block_number(15u32.into());
        let alan_fund = 75u32.into();
        let by_alan = Funder::Direct(alan.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &by_alan,
            alan_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let charlie_fund = 75u32.into();
        let by_charlie = Funder::Direct(charlie);
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &bob,
            &by_charlie,
            charlie_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let alice_digest = gen_digest::<T>(&alice).unwrap();
        let bob_digest = gen_digest::<T>(&bob).unwrap();
        let entries = vec![(alice_digest, 40u8.into()), (bob_digest, 60u8.into())];
        prepare_and_initiate_index::<T>(
            &mike,
            &FreezeReason::AuthorFunding,
            &entries,
            &index_digest,
        )
        .unwrap();
        let mike_fund = 100u32;
        let mike_by = Funder::Index {
            digest: index_digest.clone(),
            backer: mike.clone(),
        };
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &mike_by,
            mike_fund.into(),
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        System::<T>::set_block_number(25u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        exit(
            RawOrigin::Signed(mike.clone()),
            FundingTarget::Index(index_digest.clone()),
        );
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::IndexDrawn {
                index: index_digest,
                backer: mike,
                amount: mike_fund.into(),
            })
            .into(),
        );
    }

    #[benchmark]
    fn release_pool_fund() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let alan: T::AccountId = account::<T::AccountId>("alan_id", 0, SEED);
        let mike: T::AccountId = account::<T::AccountId>("mike_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let index_digest_account: T::AccountId = account::<T::AccountId>("index_digest", 0, SEED);
        let index_digest = gen_digest::<T>(&index_digest_account).unwrap();
        let pool_digest_account: T::AccountId = account::<T::AccountId>("pool_digest", 0, SEED);
        let pool_digest = gen_digest::<T>(&pool_digest_account).unwrap();
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 250u32.into()).unwrap();
        set_balance::<T>(&bob, 300u32.into()).unwrap();
        set_balance::<T>(&alan, 300u32.into()).unwrap();
        set_balance::<T>(&mike, 500u32.into()).unwrap();
        set_balance::<T>(&charlie, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&bob, collateral, Fortitude::Force).unwrap();
        System::<T>::set_block_number(15u32.into());
        let alan_fund = 75u32.into();
        let by_alan = Funder::Direct(alan.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &by_alan,
            alan_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let charlie_fund = 75u32.into();
        let by_charlie = Funder::Direct(charlie);
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &bob,
            &by_charlie,
            charlie_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let alice_digest = gen_digest::<T>(&alice).unwrap();
        let bob_digest = gen_digest::<T>(&bob).unwrap();
        let entries = vec![(alice_digest, 40u8.into()), (bob_digest, 60u8.into())];
        prepare_and_initiate_pool::<T>(
            &mike,
            &FreezeReason::AuthorFunding,
            &entries,
            &index_digest,
            &pool_digest,
            Commission::<T>::from_percent(5.into()),
        )
        .unwrap();
        let mike_fund = 100u32;
        let mike_by = Funder::Pool {
            digest: pool_digest.clone(),
            backer: mike.clone(),
        };
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &mike_by,
            mike_fund.into(),
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        System::<T>::set_block_number(25u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        exit(
            RawOrigin::Signed(mike.clone()),
            FundingTarget::Pool(pool_digest.clone()),
        );
        // --- Assert ---
        let released_fund = mike_fund - 5u32;
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::PoolDrawn {
                pool: pool_digest,
                backer: mike,
                amount: released_fund.into(),
            })
            .into(),
        );
    }

    #[benchmark]
    fn confirm() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 250u32.into()).unwrap();

        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();

        System::<T>::set_block_number(15u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        confirm(RawOrigin::Signed(alice.clone()));
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::AuthorStatus {
                author: alice,
                status: AuthorStatus::Active,
            })
            .into(),
        );
    }

    #[benchmark]
    fn create_index() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let alan: T::AccountId = account::<T::AccountId>("alan_id", 0, SEED);
        let mike: T::AccountId = account::<T::AccountId>("mike_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 300u32.into()).unwrap();
        set_balance::<T>(&bob, 300u32.into()).unwrap();
        set_balance::<T>(&alan, 300u32.into()).unwrap();
        set_balance::<T>(&mike, 300u32.into()).unwrap();
        set_balance::<T>(&charlie, 300u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&bob, collateral, Fortitude::Force).unwrap();
        System::<T>::set_block_number(15u32.into());
        let alan_fund = 75u32.into();
        let by_alan = Funder::Direct(alan.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &by_alan,
            alan_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let charlie_fund = 75u32.into();
        let by_charlie = Funder::Direct(charlie);
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &bob,
            &by_charlie,
            charlie_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let entries = vec![(alice, 40u8.into()), (bob, 60u8.into())];
        // --- Extrinsic call ---
        #[extrinsic_call]
        create_index(RawOrigin::Signed(mike), entries);
    }

    #[benchmark]
    fn create_pool() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let alan: T::AccountId = account::<T::AccountId>("alan_id", 0, SEED);
        let mike: T::AccountId = account::<T::AccountId>("mike_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let index_digest_account: T::AccountId = account::<T::AccountId>("index_digest", 0, SEED);
        let index_digest = gen_digest::<T>(&index_digest_account).unwrap();
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 250u32.into()).unwrap();
        set_balance::<T>(&bob, 300u32.into()).unwrap();
        set_balance::<T>(&alan, 300u32.into()).unwrap();
        set_balance::<T>(&mike, 500u32.into()).unwrap();
        set_balance::<T>(&charlie, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&bob, collateral, Fortitude::Force).unwrap();
        System::<T>::set_block_number(15u32.into());
        let alan_fund = 75u32.into();
        let by_alan = Funder::Direct(alan.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &by_alan,
            alan_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let charlie_fund = 75u32.into();
        let by_charlie = Funder::Direct(charlie);
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &bob,
            &by_charlie,
            charlie_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let alice_digest = gen_digest::<T>(&alice).unwrap();
        let bob_digest = gen_digest::<T>(&bob).unwrap();
        let entries = vec![(alice_digest, 40u8.into()), (bob_digest, 60u8.into())];
        prepare_and_initiate_index::<T>(
            &mike,
            &FreezeReason::AuthorFunding,
            &entries,
            &index_digest,
        )
        .unwrap();
        let commission = Commission::<T>::from_percent(10.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        create_pool(RawOrigin::Signed(mike), index_digest, commission);
    }

    #[benchmark]
    fn transfer_pool() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let alan: T::AccountId = account::<T::AccountId>("alan_id", 0, SEED);
        let mike: T::AccountId = account::<T::AccountId>("mike_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let index_digest_account: T::AccountId = account::<T::AccountId>("index_digest", 0, SEED);
        let index_digest = gen_digest::<T>(&index_digest_account).unwrap();
        let pool_digest_account: T::AccountId = account::<T::AccountId>("pool_digest", 0, SEED);
        let pool_digest = gen_digest::<T>(&pool_digest_account).unwrap();
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 250u32.into()).unwrap();
        set_balance::<T>(&bob, 300u32.into()).unwrap();
        set_balance::<T>(&alan, 300u32.into()).unwrap();
        set_balance::<T>(&mike, 500u32.into()).unwrap();
        set_balance::<T>(&charlie, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&bob, collateral, Fortitude::Force).unwrap();
        System::<T>::set_block_number(15u32.into());
        let alan_fund = 75u32.into();
        let by_alan = Funder::Direct(alan.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &by_alan,
            alan_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let charlie_fund = 75u32.into();
        let by_charlie = Funder::Direct(charlie.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &bob,
            &by_charlie,
            charlie_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let alice_digest = gen_digest::<T>(&alice).unwrap();
        let bob_digest = gen_digest::<T>(&bob).unwrap();
        let entries = vec![(alice_digest, 40u8.into()), (bob_digest, 60u8.into())];
        prepare_and_initiate_pool::<T>(
            &mike,
            &FreezeReason::AuthorFunding,
            &entries,
            &index_digest,
            &pool_digest,
            Commission::<T>::from_percent(5.into()),
        )
        .unwrap();
        let transfer_to = charlie;
        // --- Extrinsic call ---
        #[extrinsic_call]
        transfer_pool(RawOrigin::Signed(mike), pool_digest, transfer_to);
    }

    #[benchmark]
    fn update_commission() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let alan: T::AccountId = account::<T::AccountId>("alan_id", 0, SEED);
        let mike: T::AccountId = account::<T::AccountId>("mike_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let index_digest_account: T::AccountId = account::<T::AccountId>("index_digest", 0, SEED);
        let index_digest = gen_digest::<T>(&index_digest_account).unwrap();
        let pool_digest_account: T::AccountId = account::<T::AccountId>("pool_digest", 0, SEED);
        let pool_digest = gen_digest::<T>(&pool_digest_account).unwrap();
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 250u32.into()).unwrap();
        set_balance::<T>(&bob, 300u32.into()).unwrap();
        set_balance::<T>(&alan, 300u32.into()).unwrap();
        set_balance::<T>(&mike, 500u32.into()).unwrap();
        set_balance::<T>(&charlie, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&bob, collateral, Fortitude::Force).unwrap();
        System::<T>::set_block_number(15u32.into());
        let alan_fund = 75u32.into();
        let by_alan = Funder::Direct(alan.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &by_alan,
            alan_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let charlie_fund = 75u32.into();
        let by_charlie = Funder::Direct(charlie.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &bob,
            &by_charlie,
            charlie_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let alice_digest = gen_digest::<T>(&alice).unwrap();
        let bob_digest = gen_digest::<T>(&bob).unwrap();
        let entries = vec![(alice_digest, 40u8.into()), (bob_digest, 60u8.into())];
        prepare_and_initiate_pool::<T>(
            &mike,
            &FreezeReason::AuthorFunding,
            &entries,
            &index_digest,
            &pool_digest,
            Commission::<T>::from_percent(5.into()),
        )
        .unwrap();
        let new_commission = Commission::<T>::from_percent(10.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        update_commission(RawOrigin::Signed(mike), index_digest, new_commission);
    }

    #[benchmark]
    fn update_slot_shares() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let alan: T::AccountId = account::<T::AccountId>("alan_id", 0, SEED);
        let mike: T::AccountId = account::<T::AccountId>("mike_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let index_digest_account: T::AccountId = account::<T::AccountId>("index_digest", 0, SEED);
        let index_digest = gen_digest::<T>(&index_digest_account).unwrap();
        let pool_digest_account: T::AccountId = account::<T::AccountId>("pool_digest", 0, SEED);
        let pool_digest = gen_digest::<T>(&pool_digest_account).unwrap();
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 250u32.into()).unwrap();
        set_balance::<T>(&bob, 300u32.into()).unwrap();
        set_balance::<T>(&alan, 300u32.into()).unwrap();
        set_balance::<T>(&mike, 500u32.into()).unwrap();
        set_balance::<T>(&charlie, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&bob, collateral, Fortitude::Force).unwrap();
        System::<T>::set_block_number(15u32.into());
        let alan_fund = 75u32.into();
        let by_alan = Funder::Direct(alan.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &by_alan,
            alan_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let charlie_fund = 75u32.into();
        let by_charlie = Funder::Direct(charlie.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &bob,
            &by_charlie,
            charlie_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let alice_digest = gen_digest::<T>(&alice).unwrap();
        let bob_digest = gen_digest::<T>(&bob).unwrap();
        let entries = vec![
            (alice_digest.clone(), 40u8.into()),
            (bob_digest.clone(), 60u8.into()),
        ];
        prepare_and_initiate_pool::<T>(
            &mike,
            &FreezeReason::AuthorFunding,
            &entries,
            &index_digest,
            &pool_digest,
            Commission::<T>::from_percent(5.into()),
        )
        .unwrap();
        let new_slot_share = 60u8;
        // --- Extrinsic call ---
        #[extrinsic_call]
        update_slot_shares(
            RawOrigin::Signed(mike),
            pool_digest,
            alice_digest,
            new_slot_share.into(),
        );
    }

    #[benchmark]
    fn update_entry_shares() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let alan: T::AccountId = account::<T::AccountId>("alan_id", 0, SEED);
        let mike: T::AccountId = account::<T::AccountId>("mike_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let index_digest_account: T::AccountId = account::<T::AccountId>("index_digest", 0, SEED);
        let index_digest = gen_digest::<T>(&index_digest_account).unwrap();
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 250u32.into()).unwrap();
        set_balance::<T>(&bob, 300u32.into()).unwrap();
        set_balance::<T>(&alan, 300u32.into()).unwrap();
        set_balance::<T>(&mike, 500u32.into()).unwrap();
        set_balance::<T>(&charlie, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&bob, collateral, Fortitude::Force).unwrap();
        System::<T>::set_block_number(15u32.into());
        let alan_fund = 75u32.into();
        let by_alan = Funder::Direct(alan.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &by_alan,
            alan_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let charlie_fund = 75u32.into();
        let by_charlie = Funder::Direct(charlie.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &bob,
            &by_charlie,
            charlie_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let alice_digest = gen_digest::<T>(&alice).unwrap();
        let bob_digest = gen_digest::<T>(&bob).unwrap();
        let entries = vec![
            (alice_digest.clone(), 40u8.into()),
            (bob_digest.clone(), 60u8.into()),
        ];
        prepare_and_initiate_index::<T>(
            &mike,
            &FreezeReason::AuthorFunding,
            &entries,
            &index_digest,
        )
        .unwrap();
        let new_entry_share = 60u8;
        // --- Extrinsic call ---
        #[extrinsic_call]
        update_entry_shares(
            RawOrigin::Signed(mike),
            index_digest,
            alice_digest,
            new_entry_share.into(),
        );
    }

    #[benchmark]
    fn force_probation_period() {
        let new_prob_period: BlockNumberFor<T> = 15u32.into();

        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::ProbationPeriod(new_prob_period),
        );
    }

    #[benchmark]
    fn force_reduce_probation_by() {
        let new_reduce_probation_by: BlockNumberFor<T> = 2u32.into();
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::ReduceProbationBy(new_reduce_probation_by),
        );
    }

    #[benchmark]
    fn force_increase_probation_by() {
        let new_increase_probation_by: BlockNumberFor<T> = 2u32.into();
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::IncreaseProbationBy(new_increase_probation_by),
        );
    }

    #[benchmark]
    fn force_rewards_buffer() {
        let new_rewards_buffer: BlockNumberFor<T> = 3u32.into();
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::RewardsBuffer(new_rewards_buffer),
        );
    }

    #[benchmark]
    fn force_penalties_buffer() {
        let new_penalties_buffer: BlockNumberFor<T> = 5u32.into();
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::PenaltiesBuffer(new_penalties_buffer),
        );
    }

    #[benchmark]
    fn force_max_elected() {
        let new_max_elected: u32 = 150u32.into();
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::MaxElected(new_max_elected),
        );
    }

    #[benchmark]
    fn force_min_elected() {
        let new_min_elected: u32 = 5u32.into();
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::MinElected(new_min_elected),
        );
    }

    #[benchmark]
    fn force_enforce_max_elected() {
        let enforce: bool = true;
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::EnforceMaxElected(enforce),
        );
    }

    #[benchmark]
    fn force_min_fund() {
        let new_min_fund: AuthorAsset<T> = 150u32.into();
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(RawOrigin::Root, ForceGenesisConfig::MinFund(new_min_fund));
    }

    #[benchmark]
    fn force_max_exposure() {
        let new_max_exposure: AuthorAsset<T> = 100000u32.into();
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::MaxExposure(new_max_exposure),
        );
    }

    #[benchmark]
    fn force_min_collateral() {
        let new_min_collateral: AuthorAsset<T> = 200u32.into();
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::MinCollateral(new_min_collateral),
        );
    }

    #[benchmark]
    fn check_direct_fund() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 250u32.into()).unwrap();
        set_balance::<T>(&charlie, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();
        System::<T>::set_block_number(15u32.into());
        let fund = 75u32.into();
        let fund_by: Funder<T> = Funder::Direct(charlie.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &fund_by,
            fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        // --- Extrinsic call ---
        #[extrinsic_call]
        my_fund(
            RawOrigin::Signed(charlie.clone()),
            FundingTarget::Direct(alice.clone()),
        );
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::InspectAuthorFund {
                author: alice,
                backer: charlie,
                amount: fund,
            })
            .into(),
        );
    }

    #[benchmark]
    fn check_index_fund() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let alan: T::AccountId = account::<T::AccountId>("alan_id", 0, SEED);
        let mike: T::AccountId = account::<T::AccountId>("mike_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let index_digest_account: T::AccountId = account::<T::AccountId>("index_digest", 0, SEED);
        let index_digest = gen_digest::<T>(&index_digest_account).unwrap();
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 250u32.into()).unwrap();
        set_balance::<T>(&bob, 300u32.into()).unwrap();
        set_balance::<T>(&alan, 300u32.into()).unwrap();
        set_balance::<T>(&mike, 500u32.into()).unwrap();
        set_balance::<T>(&charlie, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&bob, collateral, Fortitude::Force).unwrap();
        System::<T>::set_block_number(15u32.into());
        let alan_fund = 75u32.into();
        let by_alan = Funder::Direct(alan.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &by_alan,
            alan_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let charlie_fund = 75u32.into();
        let by_charlie = Funder::Direct(charlie);
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &bob,
            &by_charlie,
            charlie_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let alice_digest = gen_digest::<T>(&alice).unwrap();
        let bob_digest = gen_digest::<T>(&bob).unwrap();
        let entries = vec![(alice_digest, 40u8.into()), (bob_digest, 60u8.into())];
        prepare_and_initiate_index::<T>(
            &mike,
            &FreezeReason::AuthorFunding,
            &entries,
            &index_digest,
        )
        .unwrap();
        let mike_fund = 100u32;
        let mike_by = Funder::Index {
            digest: index_digest.clone(),
            backer: mike.clone(),
        };
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &mike_by,
            mike_fund.into(),
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        System::<T>::set_block_number(25u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        my_fund(
            RawOrigin::Signed(mike.clone()),
            FundingTarget::Index(index_digest.clone()),
        );
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::InspectIndexFund {
                index: index_digest,
                backer: mike,
                amount: mike_fund.into(),
            })
            .into(),
        );
    }

    #[benchmark]
    fn check_index_fund_towards() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let alan: T::AccountId = account::<T::AccountId>("alan_id", 0, SEED);
        let mike: T::AccountId = account::<T::AccountId>("mike_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let index_digest_account: T::AccountId = account::<T::AccountId>("index_digest", 0, SEED);
        let index_digest = gen_digest::<T>(&index_digest_account).unwrap();
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 250u32.into()).unwrap();
        set_balance::<T>(&bob, 300u32.into()).unwrap();
        set_balance::<T>(&alan, 300u32.into()).unwrap();
        set_balance::<T>(&mike, 500u32.into()).unwrap();
        set_balance::<T>(&charlie, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&bob, collateral, Fortitude::Force).unwrap();
        System::<T>::set_block_number(15u32.into());
        let alan_fund = 75u32.into();
        let by_alan = Funder::Direct(alan.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &by_alan,
            alan_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let charlie_fund = 75u32.into();
        let by_charlie = Funder::Direct(charlie);
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &bob,
            &by_charlie,
            charlie_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let alice_digest = gen_digest::<T>(&alice).unwrap();
        let bob_digest = gen_digest::<T>(&bob).unwrap();
        let entries = vec![(alice_digest, 40u8.into()), (bob_digest, 60u8.into())];
        prepare_and_initiate_index::<T>(
            &mike,
            &FreezeReason::AuthorFunding,
            &entries,
            &index_digest,
        )
        .unwrap();
        let mike_fund = 100u32;
        let mike_by = Funder::Index {
            digest: index_digest.clone(),
            backer: mike.clone(),
        };
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &mike_by,
            mike_fund.into(),
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        System::<T>::set_block_number(25u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        my_author_fund(
            RawOrigin::Signed(mike.clone()),
            alice.clone(),
            FundingTarget::Index(index_digest),
        );
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::InspectFund {
                author: alice,
                funder: mike_by,
                amount: 40u32.into(),
            })
            .into(),
        );
    }

    #[benchmark]
    fn check_pool_fund() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let alan: T::AccountId = account::<T::AccountId>("alan_id", 0, SEED);
        let mike: T::AccountId = account::<T::AccountId>("mike_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let index_digest_account: T::AccountId = account::<T::AccountId>("index_digest", 0, SEED);
        let index_digest = gen_digest::<T>(&index_digest_account).unwrap();
        let pool_digest_account: T::AccountId = account::<T::AccountId>("pool_digest", 0, SEED);
        let pool_digest = gen_digest::<T>(&pool_digest_account).unwrap();
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 250u32.into()).unwrap();
        set_balance::<T>(&bob, 300u32.into()).unwrap();
        set_balance::<T>(&alan, 300u32.into()).unwrap();
        set_balance::<T>(&mike, 500u32.into()).unwrap();
        set_balance::<T>(&charlie, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&bob, collateral, Fortitude::Force).unwrap();
        System::<T>::set_block_number(15u32.into());
        let alan_fund = 75u32.into();
        let by_alan = Funder::Direct(alan.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &by_alan,
            alan_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let charlie_fund = 75u32.into();
        let by_charlie = Funder::Direct(charlie);
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &bob,
            &by_charlie,
            charlie_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let alice_digest = gen_digest::<T>(&alice).unwrap();
        let bob_digest = gen_digest::<T>(&bob).unwrap();
        let entries = vec![(alice_digest, 40u8.into()), (bob_digest, 60u8.into())];
        prepare_and_initiate_pool::<T>(
            &mike,
            &FreezeReason::AuthorFunding,
            &entries,
            &index_digest,
            &pool_digest,
            Commission::<T>::from_percent(5.into()),
        )
        .unwrap();
        let mike_fund = 100u32;
        let mike_by = Funder::Pool {
            digest: pool_digest.clone(),
            backer: mike.clone(),
        };
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &mike_by,
            mike_fund.into(),
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        System::<T>::set_block_number(25u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        my_fund(
            RawOrigin::Signed(mike.clone()),
            FundingTarget::Pool(pool_digest.clone()),
        );
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::InspectPoolFund {
                pool: pool_digest,
                backer: mike,
                amount: mike_fund.into(),
            })
            .into(),
        );
    }

    #[benchmark]
    fn check_pool_fund_towards() {
        let alice: T::AccountId = account::<T::AccountId>("alice_id", 0, SEED);
        let bob: T::AccountId = account::<T::AccountId>("bob_id", 0, SEED);
        let alan: T::AccountId = account::<T::AccountId>("alan_id", 0, SEED);
        let mike: T::AccountId = account::<T::AccountId>("mike_id", 0, SEED);
        let charlie: T::AccountId = account::<T::AccountId>("charlie_id", 0, SEED);
        let index_digest_account: T::AccountId = account::<T::AccountId>("index_digest", 0, SEED);
        let index_digest = gen_digest::<T>(&index_digest_account).unwrap();
        let pool_digest_account: T::AccountId = account::<T::AccountId>("pool_digest", 0, SEED);
        let pool_digest = gen_digest::<T>(&pool_digest_account).unwrap();
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&alice, 250u32.into()).unwrap();
        set_balance::<T>(&bob, 300u32.into()).unwrap();
        set_balance::<T>(&alan, 300u32.into()).unwrap();
        set_balance::<T>(&mike, 500u32.into()).unwrap();
        set_balance::<T>(&charlie, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&alice, collateral, Fortitude::Force)
            .unwrap();
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&bob, collateral, Fortitude::Force).unwrap();
        System::<T>::set_block_number(15u32.into());
        let alan_fund = 75u32.into();
        let by_alan = Funder::Direct(alan.clone());
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &by_alan,
            alan_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let charlie_fund = 75u32.into();
        let by_charlie = Funder::Direct(charlie);
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &bob,
            &by_charlie,
            charlie_fund,
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        let alice_digest = gen_digest::<T>(&alice).unwrap();
        let bob_digest = gen_digest::<T>(&bob).unwrap();
        let entries = vec![(alice_digest, 40u8.into()), (bob_digest, 60u8.into())];
        prepare_and_initiate_pool::<T>(
            &mike,
            &FreezeReason::AuthorFunding,
            &entries,
            &index_digest,
            &pool_digest,
            Commission::<T>::from_percent(5.into()),
        )
        .unwrap();
        let mike_fund = 100u32;
        let mike_by = Funder::Pool {
            digest: pool_digest.clone(),
            backer: mike.clone(),
        };
        <Pallet<T> as FundRoles<Author<T>>>::fund(
            &alice,
            &mike_by,
            mike_fund.into(),
            Precision::Exact,
            Fortitude::Force,
        )
        .unwrap();
        System::<T>::set_block_number(25u32.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        my_author_fund(
            RawOrigin::Signed(mike.clone()),
            alice.clone(),
            FundingTarget::Pool(pool_digest),
        );
        // --- Assert ---
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::InspectFund {
                author: alice,
                funder: mike_by,
                amount: 40u32.into(),
            })
            .into(),
        );
    }

    #[benchmark]
    fn shed_rewards() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&caller, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&caller, collateral, Fortitude::Force)
            .unwrap();
        System::<T>::set_block_number(8u32.into());
        let reward_a = 10u32.into();
        <Pallet<T> as CompensateRoles<Author<T>>>::reward(&caller, reward_a, Precision::Exact)
            .unwrap();
        System::<T>::set_block_number(9u32.into());
        let reward_b = 20u32.into();
        <Pallet<T> as CompensateRoles<Author<T>>>::reward(&caller, reward_b, Precision::Exact)
            .unwrap();
        // --- Extrinsic call ---
        #[extrinsic_call]
        shed_rewards(RawOrigin::Signed(caller.clone()));
        // --- Assert ---
        let expected_shed_rewards = vec![(10u32.into(), reward_a), (11u32.into(), reward_b)];
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::ScheduledRewards {
                author: caller,
                rewards: expected_shed_rewards,
            })
            .into(),
        );
    }

    #[benchmark]
    fn shed_penalties() {
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let collateral: AuthorAsset<T> = 150u32.into();
        set_balance::<T>(&caller, 250u32.into()).unwrap();
        System::<T>::set_block_number(4u32.into());
        <Pallet<T> as RoleManager<Author<T>>>::enroll(&caller, collateral, Fortitude::Force)
            .unwrap();
        System::<T>::set_block_number(8u32.into());
        let penalty_a = Ratio::<T>::from_percent(2);
        <Pallet<T> as CompensateRoles<Author<T>>>::penalize(&caller, penalty_a).unwrap();
        System::<T>::set_block_number(10u32.into());
        let penalty_b = Ratio::<T>::from_percent(5);
        <Pallet<T> as CompensateRoles<Author<T>>>::penalize(&caller, penalty_b).unwrap();
        // --- Extrinsic call ---
        #[extrinsic_call]
        shed_penalties(RawOrigin::Signed(caller.clone()));
        // --- Assert ---
        let expected_shed_penalty = vec![(12u32.into(), penalty_a), (14u32.into(), penalty_b)];
        System::<T>::assert_last_event(
            <T as pallet::Config>::RuntimeEvent::from(Event::<T>::ScheduledPenalties {
                author: caller,
                penalties: expected_shed_penalty,
            })
            .into(),
        );
    }

    #[benchmark]
    fn on_initialize_rewards_penalties(
        r: Linear<1, { MaxElected::<T>::get() }>,
        p: Linear<1, { MaxElected::<T>::get() }>,
    ) {
        let block: BlockNumberFor<T> = 1u32.into();
        // --- Rewards setup ---
        for i in 0..r {
            let author = account::<T::AccountId>("validator_id", i, SEED);
            let funder = account::<T::AccountId>("backer_id", i, SEED);
            let collateral: AuthorAsset<T> = 1000u32.into();
            let fund: AuthorAsset<T> = 500u32.into();
            set_balance::<T>(&author, 2000u32.into()).unwrap();
            set_balance::<T>(&funder, 2000u32.into()).unwrap();
            Pallet::<T>::enroll(&author, collateral, Fortitude::Force).unwrap();
            Pallet::<T>::fund(&author, &Funder::Direct(funder), fund, Precision::Exact, Fortitude::Force).unwrap();
            let reward: AuthorAsset<T> = 10u32.into();
            AuthorRewards::<T>::insert((block, author), reward);
        }
        // --- Penalties setup ---
        for i in 0..p {
            let offset = r + i;
            let author = account::<T::AccountId>("validator_id", offset, SEED);
            let funder = account::<T::AccountId>("backer_id", offset, SEED);
            let collateral: AuthorAsset<T> = 1000u32.into();
            let fund: AuthorAsset<T> = 500u32.into();
            set_balance::<T>(&author, 2000u32.into()).unwrap();
            set_balance::<T>(&funder, 2000u32.into()).unwrap();
            Pallet::<T>::enroll(&author, collateral, Fortitude::Force).unwrap();
            Pallet::<T>::fund(&author, &Funder::Direct(funder), fund, Precision::Exact, Fortitude::Force).unwrap();
            let penalty = Ratio::<T>::from_percent(10);
            AuthorPenalties::<T>::insert((block, author), penalty);
        }
        // --- Call ---
        #[block]
        {
            Pallet::<T>::on_initialize(block);
        }
        // --- Assert ---
        // All rewards and penalties for this block must be consumed
        assert!(AuthorRewards::<T>::iter_prefix((block,)).next().is_none());
        assert!(AuthorPenalties::<T>::iter_prefix((block,)).next().is_none());
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::authors_test_ext(), crate::mock::Test);
}
