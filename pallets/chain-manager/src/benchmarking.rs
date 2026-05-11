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
use crate::{*, types::* , Pallet};

// --- FRAME Suite ---
use frame_suite::{
    roles::RoleManager, ForkLocalDepot,
    ElectAuthors, ElectionAffidavits,
    Finalized, KeyValueStore, ForksHandler
};
 
// --- FRAME Support ---
use frame_support::{
    traits::{fungible::Mutate, tokens::Fortitude, Hooks},
    ensure,
};

// --- FRAME System ---
use frame_system::{
    pallet_prelude::BlockNumberFor,
    offchain::{SigningTypes, AppCrypto},
    RawOrigin
};

// --- FRAME Benchmarking ---
use frame_benchmarking::v2::*;

// --- Substrate crates ---
use sp_runtime::{
    traits::{IdentifyAccount, Convert, Hash},
    MultiSignature, Vec, DispatchResult,
    Weight, Permill, MultiSigner,
    DispatchError, SaturatedConversion,
};
use sp_staking::offence::{OffenceDetails, OnOffenceHandler};

// --- External crates ---
use codec::Encode;
use scale_info::prelude::vec;

// ===============================================================================
// ``````````````````````````````````` ALIASES ```````````````````````````````````
// ===============================================================================

type System<T> = frame_system::Pallet<T>;

type ValidatorId<T> = <T as pallet_session::Config>::ValidatorId;

type RoleAdapter<T> = <T as crate::Config>::RoleAdapter;

type Public<T> = <T as frame_system::offchain::SigningTypes>::Public;

type Signature<T> = <T as frame_system::offchain::SigningTypes>::Signature;

type AffidavitCrypto<T> = <T as crate::Config>::AffidavitCrypto;

type FinalizedInitAfdtKey<T> = Finalized<T, AffidavitId<T>, InitAffidavitKey<T>, Pallet<T>>;

type RuntimeAppPublic<T> = <<T as crate::Config>::AffidavitCrypto as AppCrypto<
    <T as frame_system::offchain::SigningTypes>::Public,
        <T as frame_system::offchain::SigningTypes>::Signature,
>>::RuntimeAppPublic;

type GenericPublic<T> = <<T as crate::Config>::AffidavitCrypto as AppCrypto<
    <T as frame_system::offchain::SigningTypes>::Public,
    <T as frame_system::offchain::SigningTypes>::Signature,
>>::GenericPublic;


// ===============================================================================
// `````````````````````````````````` BENCHMARKS `````````````````````````````````
// ===============================================================================

#[benchmarks(
    where
        <T as SigningTypes>::Public: From<MultiSigner>,
        <T as SigningTypes>::Signature: From<MultiSignature>,
        AuthorOf<T>: From<ValidatorId<T>>,
)]
mod benchmarks {
    use super::*;
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````````` Constants ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~    
    const SEED: u32 = 1;
    const ACTIVE_AFDT_KEY: &'static [u8] = b"ACTIVE_AFDT_KEY";
    const LOG_TARGET_AFDT: Option<&'static str> = Some("AFFIDAVIT");
    const NEXT_AFDT_KEY: &'static [u8] = b"NEXT_AFDT_KEY";
    // --- Block Periods ---
    const _SESSION_START: u32 = 1;
    const AFDT_SUBMISSION_START: u32 = 121;
    const _AFDT_SUBMISSION_END: u32 = 481;
    const ELECTION_START: u32 = 301;
    const _SESSION_END: u32 = 601;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` HELPERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    fn set_balance<T: Config>(who: &T::AccountId, amount: AssetOf<T>) {
        <T as Config>::Asset::set_balance(who, amount);
        // let hold_reason: <T as Config>::RoleA::AssetFreeze = FreezeReason::AuthorCollateral.into();
        // <T as Config>::Asset::set_balance_on_hold(&hold_reason, &who, amount)?;
    }

    fn set_session_config<T: Config>() {
        CurrentSession::<T>::put(1u32);
        AllowAffidavits::<T>::put(true);
        SessionStartAt::<T>::put(BlockNumberFor::<T>::from(1u32));
        AffidavitBeginsAt::<T>::put(Duration::from_rational(2u32, 10u32));
        AffidavitEndsAt::<T>::put(Duration::from_rational(8u32, 10u32));
        ElectionBeginsAt::<T>::put(Duration::from_rational(5u32, 10u32));
    }

    fn setup_author<T: Config>(
        caller: T::AccountId,
        amount: AssetOf<T>,
        collateral: AssetOf<T>,
    ) -> Result<(), DispatchResult> {
        set_balance::<T>(&caller, amount);
        RoleAdapter::<T>::enroll(
            &caller,
            collateral,
            Fortitude::Force,
        )?;
        Ok(())
    }

    fn ext_validate<T: Config>(author: AuthorOf<T>, afdt_pub: AffidavitId<T>) -> DispatchResult {
        RoleAdapter::<T>::role_exists(&author)?;
        RoleAdapter::<T>::is_available(&author)?;
        let for_session = CurrentSession::<T>::get() + 1;
        AffidavitKeys::<T>::insert((for_session, afdt_pub), author);
        Ok(())
    }

    struct BenchAfdtPayload<T: Config> {
        pub active_afdt_pub: AffidavitId<T>,
        pub next_afdt_pub: AffidavitId<T>,
    }

    fn ext_declare_affidavit<T: Config>(author: AuthorOf<T>, payload: BenchAfdtPayload<T>) -> DispatchResult {
        let active_afdt = payload.active_afdt_pub;
        let rotate = payload.next_afdt_pub;
        let for_session = CurrentSession::<T>::get() + 1;
        let afdt_author =
            AffidavitKeys::<T>::get((for_session, &active_afdt)).ok_or(Error::<T>::AffidavitAuthorNotFound)?;
        ensure!(afdt_author == author, Error::<T>::AuthorNotAffidavitOwner);
        Pallet::<T>::process_affidavit(&active_afdt.clone())?;
        AffidavitKeys::<T>::insert((for_session + 1, rotate), author);
        Ok(())
    }

    fn ext_elect_authors<T: Config>(
        author: AuthorOf<T>,
        afdt_pub: AffidavitId<T>,
    ) -> Result<Vec<AuthorOf<T>>, DispatchError> {
        let for_session = CurrentSession::<T>::get() + 1;
        let afdt_author = AffidavitKeys::<T>::get((for_session + 1, &afdt_pub.clone()))
            .ok_or(Error::<T>::AffidavitAuthorNotFound)?;
        ensure!(afdt_author == author, Error::<T>::AuthorNotAffidavitOwner);
        Internals::<T>::prepare_election(&Some(author.clone()))?;
        let current_block = System::<T>::block_number();
        ElectsPreparedBy::<T>::insert(for_session, (author, current_block));
        let elected = Internals::<T>::reveal().unwrap();
        Ok(elected.into_iter().collect())
    }

    fn generate_affidavit_id<T: Config>() -> AffidavitId<T> {
        let key = <RuntimeAppPublic<T> as sp_runtime::RuntimeAppPublic>::generate_pair(None);
        let generic_pub: GenericPublic<T> = key.into();
        let public: Public<T> = generic_pub.into();
        public.into_account().into()
    }

    fn bootstrap_fork_graph<T: Config>() {
        for i in 0u32..=3u32 {
            let b = BlockNumberFor::<T>::from(i);
            let hash = <T as frame_system::Config>::Hashing::hash(&i.to_le_bytes());
            frame_system::BlockHash::<T>::insert(b, hash);
        }
        System::<T>::set_block_number(2u32.into());
        <Pallet<T> as ForksHandler<T, ForkLocalDepot>>::start(None, None, || {});
        System::<T>::set_block_number(3u32.into());
        <Pallet<T> as ForksHandler<T, ForkLocalDepot>>::start(None, None, || {});
    }

    fn insert_active_afdt_key<T: Config>(key: AffidavitId<T>) -> DispatchResult {
        FinalizedInitAfdtKey::<T>::insert(ACTIVE_AFDT_KEY, &key, LOG_TARGET_AFDT, None)?;
        Ok(())
    }

    fn sign_payload<T: Config>(payload: &[u8], public: Public<T>) -> Signature<T> {
        AffidavitCrypto::<T>::sign(payload, public)
            .expect("test keystore should contain affidavit signing key")
    }

    fn get_public_key<T: Config>(afdt_key: AffidavitId<T>) -> Option<Public<T>> {
        let all_keys = <RuntimeAppPublic<T> as sp_runtime::RuntimeAppPublic>::all();
        for key in all_keys.into_iter() {
            let generic_pub: GenericPublic<T> = key.into();
            let public: Public<T> = generic_pub.into();
            let account: AffidavitId<T> = public.clone().into_account().into();

            if account == afdt_key {
                return Some(public);
            }
        }
        None
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````````` BENCHMARK `````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[benchmark]
    fn validate() {
        set_session_config::<T>();
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let collateral: AssetOf<T> = 100u32.into();
        let xp: AssetOf<T> = 250u32.into();
        setup_author::<T>(caller.clone(), xp, collateral).unwrap();
        let afdt_id = generate_affidavit_id::<T>();
        let public = get_public_key::<T>(afdt_id.clone())
            .expect("generated affidavit key must exist in keystore");
        let payload = ValidatePayloadOf::<T> {
            public: public.clone(),
        };
        let signature = sign_payload::<T>(&payload.encode(), public);
        System::<T>::set_block_number((AFDT_SUBMISSION_START - 1u32).into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        validate(RawOrigin::Signed(caller), payload, signature);
        // --- Assert ---
        let next_session = CurrentSession::<T>::get() + 1;
        assert!(AffidavitKeys::<T>::contains_key((next_session, afdt_id)));
    }

    #[benchmark]
    fn chill () {
        set_session_config::<T>();
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let collateral: AssetOf<T> = 100u32.into();
        let xp: AssetOf<T> = 250u32.into();
        setup_author::<T>(caller.clone(), xp, collateral).unwrap();
        let afdt_id = generate_affidavit_id::<T>(); 
        ext_validate::<T>(caller.clone(), afdt_id.clone()).unwrap(); 
        let bfr_afdt = AFDT_SUBMISSION_START - 25u32;
        System::<T>::set_block_number(bfr_afdt.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        chill(RawOrigin::Signed(caller), afdt_id);              
    }

    #[benchmark]
    fn declare() {
        set_session_config::<T>();
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let collateral: AssetOf<T> = 100u32.into();
        let xp: AssetOf<T> = 250u32.into();
        setup_author::<T>(caller.clone(), xp, collateral).unwrap();
        let afdt_id = generate_affidavit_id::<T>(); 
        ext_validate::<T>(caller.clone(), afdt_id.clone()).unwrap(); 
        let next_adft_id = generate_affidavit_id::<T>();
        let public = get_public_key::<T>(afdt_id.clone()).unwrap();
        let affidavit_payload = AffidavitPayloadOf::<T> {
            public: public.clone(),
            rotate: next_adft_id.clone(),
        };
        let signature = sign_payload::<T>(&affidavit_payload.encode(), public);
        System::<T>::set_block_number(AFDT_SUBMISSION_START.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        declare(RawOrigin::None, affidavit_payload, signature);
    }

    #[benchmark]
    fn elect() {
        set_session_config::<T>();
        let collateral: AssetOf<T> = 100u32.into();
        let xp: AssetOf<T> = 250u32.into();
        let authors: Vec<T::AccountId> = vec![
            account("validator_id", 0, SEED),
            account("validator_id", 1, SEED),
            account("validator_id", 2, SEED),
            account("validator_id", 3, SEED),
            account("validator_id", 4, SEED),
            account("validator_id", 5, SEED),
            account("validator_id", 6, SEED),
            account("validator_id", 7, SEED),
            account("validator_id", 8, SEED),
            account("validator_id", 9, SEED),
        ];
        System::<T>::set_block_number(AFDT_SUBMISSION_START.into());
        for author in authors.iter() {
            setup_author::<T>(author.clone(), xp, collateral).unwrap();
            let afdt_id = generate_affidavit_id::<T>(); 
            ext_validate::<T>(author.clone(), afdt_id.clone()).unwrap(); 
            let next_adft_id = generate_affidavit_id::<T>();
            let affidavit_payload = BenchAfdtPayload::<T> {
                active_afdt_pub: afdt_id.clone(),
                next_afdt_pub: next_adft_id.clone(),
            };
            ext_declare_affidavit(author.clone(), affidavit_payload).unwrap();
        }
        // Block Author
        let alice: T::AccountId = account("alice_id", 0, SEED);
        setup_author::<T>(alice.clone(), xp, collateral).unwrap();
        let afdt_id = generate_affidavit_id::<T>(); 
        ext_validate::<T>(alice.clone(), afdt_id.clone()).unwrap(); 
        let next_adft_id = generate_affidavit_id::<T>();
        let affidavit_payload = BenchAfdtPayload::<T> {
            active_afdt_pub: afdt_id.clone(),
            next_afdt_pub: next_adft_id.clone(),
        };
        ext_declare_affidavit(alice.clone(), affidavit_payload).unwrap();

        let public = get_public_key::<T>(next_adft_id).unwrap();
        let payload = ElectionPayloadOf::<T> {
            public: public.clone(),
        };
        let signature = sign_payload::<T>(&payload.encode(), public);
        System::<T>::set_block_number(ELECTION_START.into());
        // --- Extrinsic call ---
        #[extrinsic_call]
        elect(RawOrigin::None, payload, signature);        
    }

    #[benchmark]
    fn force_allow_affidavits() {
        let allow_aff = false;
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::AllowAffidavits(allow_aff),
        );
    }

    #[benchmark]
    fn force_affidavit_begins_at() {
        let aff_begins = Permill::from_rational(30u32, 100u32);
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::AffidavitBeginsAt(aff_begins),
        );
    }

    #[benchmark]
    fn force_affidavit_ends_at() {
        let aff_ends = Permill::from_rational(70u32, 100u32);
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::AffidavitEndsAt(aff_ends),
        );
    }

    #[benchmark]
    fn force_election_begins_at() {
        let elect_begins = Permill::from_rational(45u32, 100u32);
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::ElectionBeginsAt(elect_begins),
        );
    }

    #[benchmark]
    fn force_election_runner_points_upgrade() {
        let points_upgrade: T::Points = 20u8.into();
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::ElectionRunnerPointsUpgrade(Some(points_upgrade)),
        );
    }

    #[benchmark]
    fn force_validate_tx_priority() {
        let tx_priority = 10_000u32.into();
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::ValidateTxPriority(tx_priority),
        );
    }

    #[benchmark]
    fn force_election_tx_priority() {
        let tx_priority = 5_000u32.into();
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::ElectionTxPriority(tx_priority),
        );
    }

    #[benchmark]
    fn force_affidavit_tx_priority() {
        let tx_priority = 7_500u32.into();
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::AffidavitTxPriority(tx_priority),
        );
    }

    #[benchmark]
    fn force_finality_after() {
        let finalty_after = 120000u32.into();
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::FinalityAfter(finalty_after),
        );
    }

    #[benchmark]
    fn force_finality_ticks() {
        let finalty_ticks = 120000u32.into();
        // --- Extrinsic call ---
        #[extrinsic_call]
        force_genesis_config(
            RawOrigin::Root,
            ForceGenesisConfig::FinalityTicks(finalty_ticks),
        );
    }

    #[benchmark]
    fn inspect_elects() {
        set_session_config::<T>();
        let collateral: AssetOf<T> = 100u32.into();
        let xp: AssetOf<T> = 250u32.into();
        let authors: Vec<T::AccountId> = vec![
            account("validator_id", 0, SEED),
            account("validator_id", 1, SEED),
            account("validator_id", 2, SEED),
            account("validator_id", 3, SEED),
            account("validator_id", 4, SEED),
            account("validator_id", 5, SEED),
            account("validator_id", 6, SEED),
            account("validator_id", 7, SEED),
            account("validator_id", 8, SEED),
            account("validator_id", 9, SEED),
        ];
        let mut caller = None;
        let mut caller_nxt_afdt_id = None;
        System::<T>::set_block_number(AFDT_SUBMISSION_START.into());
        for (i, author) in authors.iter().enumerate() {
            setup_author::<T>(author.clone(), xp, collateral).unwrap();
            let afdt_id = generate_affidavit_id::<T>(); 
            ext_validate::<T>(author.clone(), afdt_id.clone()).unwrap(); 
            let next_adft_id = generate_affidavit_id::<T>();
            let affidavit_payload = BenchAfdtPayload::<T> {
                active_afdt_pub: afdt_id.clone(),
                next_afdt_pub: next_adft_id.clone(),
            };
            ext_declare_affidavit::<T>(author.clone(), affidavit_payload).unwrap();
            if i == 0 {
                caller = Some(author);
                caller_nxt_afdt_id = Some(next_adft_id);
            }
        }
        System::<T>::set_block_number(ELECTION_START.into()); 
        ext_elect_authors::<T>(caller.unwrap().clone(), caller_nxt_afdt_id.unwrap()).unwrap();
        // --- Extrinsic call ---
        #[extrinsic_call]
        inspect_elects(RawOrigin::Signed(caller.unwrap().clone()));         
    }

    #[benchmark]
    fn prepare_validation_payload() {
        set_session_config::<T>();
        bootstrap_fork_graph::<T>();
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let collateral: AssetOf<T> = 100u32.into();
        let xp: AssetOf<T> = 250u32.into();
        setup_author::<T>(caller.clone(), xp, collateral).unwrap();
        let afdt_id = generate_affidavit_id::<T>();
        insert_active_afdt_key::<T>(afdt_id.clone()).unwrap();
        let finality_ticks = FinalityTicks::<T>::get();
        let ticks_u32: u32 = finality_ticks.saturated_into();
        let target_block: u32 = 10 + ticks_u32 + 5;
        for i in 1..=target_block {
            let b = BlockNumberFor::<T>::from(i);
            let hash = <T as frame_system::Config>::Hashing::hash(&i.to_le_bytes());
            frame_system::BlockHash::<T>::insert(b, hash);
            System::<T>::set_block_number(b);
            pallet_timestamp::Now::<T>::put(
                T::Moment::saturated_from(6_000u64 * i as u64)
            );
            <Pallet<T> as ForksHandler<T, ForkLocalDepot>>::start(None, None, || {});
            let _ = FinalizedInitAfdtKey::<T>::get(ACTIVE_AFDT_KEY, LOG_TARGET_AFDT, None);
            let _ = FinalizedInitAfdtKey::<T>::get(NEXT_AFDT_KEY, LOG_TARGET_AFDT, None);
        }
        // --- Extrinsic call ---
        #[extrinsic_call]
        prepare_validation_payload(RawOrigin::Signed(caller));
    }

    #[benchmark]
    fn inspect_affidavit() {
        set_session_config::<T>();
        let caller: T::AccountId = account("whitelisted_caller", 0, SEED);
        let collateral: AssetOf<T> = 100u32.into();
        let xp: AssetOf<T> = 250u32.into();
        setup_author::<T>(caller.clone(), xp, collateral).unwrap();
        let afdt_id = generate_affidavit_id::<T>(); 
        ext_validate::<T>(caller.clone(), afdt_id.clone()).unwrap(); 
        let next_adft_id = generate_affidavit_id::<T>();
        let affidavit_payload = BenchAfdtPayload::<T> {
            active_afdt_pub: afdt_id.clone(),
            next_afdt_pub: next_adft_id.clone(),
        };
        System::<T>::set_block_number(AFDT_SUBMISSION_START.into()); 
        ext_declare_affidavit::<T>(caller.clone(), affidavit_payload).unwrap();
        // --- Extrinsic call ---
        #[extrinsic_call]
        inspect_affidavit(RawOrigin::Signed(caller), afdt_id);

    }

    // Benchmark for `OnOffenceHandler::on_offence`
    //
    // Measures cost scaling linearly with the number of offenders (`n`).
    #[benchmark]
    fn on_offence(n: Linear<1, 100>) {
        let alice: AuthorOf<T> = account("alice_id", 0, SEED);
        let bob: AuthorOf<T> = account("bob_id", 0, SEED);
        let charlie: AuthorOf<T> = account("charlie_id", 0, SEED);

        let mut offence_details: Vec<OffenceDetails<OffenceReporter<T>, Offender<T>>> =
            Vec::with_capacity(n as usize);

        let mut slash_fractions: Vec<PenaltyRatio> = Vec::with_capacity(n as usize);

        let collateral: AssetOf<T> = 100u32.into();
        let xp: AssetOf<T> = 250u32.into();
        setup_author::<T>(alice.clone(), xp, collateral).unwrap();
        setup_author::<T>(bob.clone(), xp, collateral).unwrap();
        setup_author::<T>(charlie.clone(), xp, collateral).unwrap();
        for i in 0..n {
            let validator: AuthorOf<T> = account("validator_id", i, SEED);
            setup_author::<T>(validator.clone(), xp, collateral).unwrap();
            let validator_id =
                <Pallet<T> as Convert<AuthorOf<T>, Option<SessionId<T>>>>::convert(validator)
                    .unwrap();
            let identification = T::FullIdentificationOf::convert(validator_id.clone()).unwrap();

            offence_details.push(OffenceDetails {
                offender: (validator_id, identification),
                reporters: vec![alice.clone(), bob.clone(), charlie.clone()],
            });
            slash_fractions.push(PenaltyRatio::from_percent(10));
        }
        // --- Call ---
        #[block]
        {
            <Pallet<T> as OnOffenceHandler<OffenceReporter<T>, Offender<T>, Weight>>::on_offence(
                &offence_details,
                &slash_fractions,
                0,
            );
        }
    }

    #[benchmark]
    fn on_initialize_with_author() {
        let author: AuthorOf<T> = account("alice_id", 0, SEED);
        set_balance::<T>(&author, 500u32.into());
        <T as Config>::RoleAdapter::enroll(&author, 250u32.into(), Fortitude::Force).unwrap();
        System::<T>::set_block_number(20u32.into());
        // --- Call ---
        #[block]
        {
            <Pallet<T> as Hooks<BlockNumberFor<T>>>::on_initialize(20u32.into());
        }
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::chain_manager_test_ext(),
        crate::mock::Test
    );
}
