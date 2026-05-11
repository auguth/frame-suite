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
// ``````````````````````````````` XP MOCK RUNTIME ```````````````````````````````
// ===============================================================================

//! Mock runtime using [`pallet_xp::Pallet`] as the primary asset adapter
//! to assert failure of supply-based inflation.
//!
//! ## Purpose
//!
//! This module exists solely to verify that enabling
//! [`pallet_chain_manager::Config::InflateViaSupply`] with a non-issuance-based
//! asset results in a panic.
//!
//! ## Context
//!
//! The configuration mirrors the primary mock runtime, with the only difference
//! being the asset adapter:
//!
//! - [`pallet_balances::Pallet`] (originally safe): replaced by [`pallet_xp::Pallet`]
//! - [`pallet_chain_manager::Config::InflateViaSupply`] is set to `true`
//!
//! ## Rationale
//!
//! [`pallet_xp::Pallet`] does not support issuance-based queries via
//! [`frame_support::traits::fungible::Inspect::total_issuance`].
//!
//! When the reward model attempts to derive payout from total supply,
//! this leads to a panic through the fungible adapter implementation.
//!
//! ## Note
//!
//! [`pallet_xp::Pallet`] must use
//! [`pallet_chain_manager::Config::InflateViaSupply`] set to`false`,
//! as it does not support issuance-based queries. Enabling it may lead to
//! runtime panics when total supply is accessed.

#![cfg(feature = "std")]
#![allow(unused)]

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{
    self as pallet_chain_manager,
    crypto::AffidavitCryptoSr25519,
    mock::{
        AccountId, Asset, Commission, FairElection, ImOnlineUnsignedPriority, MaxKeys,
        MaxPeerInHeartbeats, MockFindAuthor, MockOnTimestampSet, Moment, MyBalanceContext,
        MyConstantPayoutContext, MyPenaltyThresholdContext, Offset, Period, SessionKeys, Signature,
        ALAN, ALICE, AMY, BOB, CHARLIE, DAVE, JAKE, JIM, LAYA, MIKE, NIX, PAUL,
    },
};

// --- FRAME Plugins ---
use frame_plugins::{
    balances::ShareBalanceFamily,
    elections::{fair, flat},
    influence::LinearModel,
    penalty::ThresholdPenalty,
    rewards::{payee::SharesPay, payout::ConstantPayout},
};

// --- FRAME Suite ---
use frame_suite::{Disposition, Ignore};

// --- FRAME Support ---
use frame_support::{derive_impl, pallet_prelude::*, traits::VariantCountOf};

// --- FRAME System ---
use frame_system::{
    mocking::MockUncheckedExtrinsic,
    offchain::{CreateInherent, CreateSignedTransaction, CreateTransactionBase, SigningTypes},
};

// --- External pallets ---
use pallet_session::PeriodicSessions;

use pallet_xp::types::GenesisAcc;
// --- Substrate primitives ---
use sp_core::{ConstBool, ConstU64};
use sp_runtime::{BuildStorage, FixedU64, MultiSigner};

// ===============================================================================
// ``````````````````````````````````` ALIASES ```````````````````````````````````
// ===============================================================================

/// Mock block type for runtime testing
pub type Block = frame_system::mocking::MockBlock<XpTest>;

// ===============================================================================
// `````````````````````````````````` CONSTANTS ``````````````````````````````````
// ===============================================================================

// ===============================================================================
// ``````````````````````````````````` RUNTIME ```````````````````````````````````
// ===============================================================================

#[frame_support::runtime]
pub mod runtime {
    #[runtime::runtime]
    #[runtime::derive(
        RuntimeCall,
        RuntimeEvent,
        RuntimeError,
        RuntimeOrigin,
        RuntimeFreezeReason,
        RuntimeHoldReason,
        RuntimeSlashReason,
        RuntimeLockId,
        RuntimeTask,
        RuntimeViewFunction
    )]
    pub struct XpTest;

    #[runtime::pallet_index(0)]
    pub type System = frame_system::Pallet<XpTest>;

    #[runtime::pallet_index(1)]
    pub type Commitment = pallet_commitment::Pallet<XpTest>;

    #[runtime::pallet_index(2)]
    pub type Xp = pallet_xp::Pallet<XpTest>;

    #[runtime::pallet_index(3)]
    pub type Authors = pallet_authors::Pallet<XpTest>;

    #[runtime::pallet_index(4)]
    pub type Session = pallet_session::Pallet<XpTest>;

    #[runtime::pallet_index(5)]
    pub type Historical = pallet_session::historical::Pallet<XpTest>;

    #[runtime::pallet_index(6)]
    pub type ImOnline = pallet_im_online::Pallet<XpTest>;

    #[runtime::pallet_index(7)]
    pub type Offences = pallet_offences::Pallet<XpTest>;

    #[runtime::pallet_index(8)]
    pub type Authorship = pallet_authorship::Pallet<XpTest>;

    #[runtime::pallet_index(9)]
    pub type ChainManager = pallet_chain_manager::Pallet<XpTest>;

    #[runtime::pallet_index(10)]
    pub type TimeStamp = pallet_timestamp::Pallet<XpTest>;
}

// ===============================================================================
// ``````````````````````````````````` CONFIGS ```````````````````````````````````
// ===============================================================================

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// ``````````````````````````````````` SYSTEM ````````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for XpTest {
    type Block = Block;
    type AccountId = AccountId;
    type Lookup = sp_runtime::traits::IdentityLookup<Self::AccountId>;
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````` COMMITMENT `````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_commitment::Config for XpTest {
    type RuntimeEvent = RuntimeEvent;
    type Shares = u64;
    type Bias = FixedU64;
    type Asset = pallet_xp::Pallet<Self>;
    type Position = Disposition;
    type AssetHold = RuntimeHoldReason;
    type AssetFreeze = RuntimeFreezeReason;
    type MaxIndexEntries = ConstU32<3>;
    type MaxCommits = ConstU32<3>;
    type Commission = Commission;
    type Time = u32;
    type WeightInfo = ();
    type BalanceFamily<'a> = ShareBalanceFamily<'a>;
    type BalanceContext = MyBalanceContext<Commitment>;
    type EmitEvents = ConstBool<true>;
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````````` XP `````````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_xp::Config for XpTest {
    type Xp = u64;
    type Pulse = u32;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type LockReason = RuntimeFreezeReason;
    type ReserveReason = RuntimeHoldReason;
    type Extensions = Ignore<Xp>;
    type WeightInfo = ();
    type EmitEvents = ConstBool<true>;
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// ``````````````````````````````````` AUTHORS ```````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_authors::Config for XpTest {
    type RuntimeEvent = RuntimeEvent;
    type CommitmentAdapter = pallet_commitment::Pallet<Self>;
    type AssetFreeze = RuntimeFreezeReason;
    type Asset = pallet_xp::Pallet<Self>;
    type Influence = u64;
    type InfluenceContext = ();
    type InfluenceModel = LinearModel;
    type FlatElectionContext = ();
    type FlatElectionModel = flat::TopDownFlatModel;
    type FairElectionContext = ();
    type FairElectionModel = fair::TopDownFairModel;
    type ActivityProvider = ChainManager;
    type WeightInfo = ();
    type EmitEvents = ConstBool<true>;
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// ``````````````````````````````````` SESSION ```````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_session::Config for XpTest {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = AccountId;
    type ValidatorIdOf = ChainManager;
    type ShouldEndSession = PeriodicSessions<Period, Offset>;
    type NextSessionRotation = PeriodicSessions<Period, Offset>;
    type SessionManager = ChainManager;
    type SessionHandler = (ImOnline,);
    type Keys = SessionKeys;
    type WeightInfo = ();
    type DisablingStrategy = ();
}

impl pallet_session::historical::Config for XpTest {
    type FullIdentification = AccountId;
    type FullIdentificationOf = ChainManager;
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// ```````````````````````````````` CHAIN-MANAGER ````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_chain_manager::Config for XpTest {
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type RewardContext = ();
    type RewardModel = SharesPay;
    type InflationContext = MyConstantPayoutContext;
    type InflationModel = ConstantPayout;
    type RoleAdapter = Authors;
    type Asset = pallet_xp::Pallet<Self>;
    type WeightInfo = ();
    type InflateViaSupply = ConstBool<true>; // For Panic Test
    type PenaltyContext = MyPenaltyThresholdContext;
    type PenaltyModel = ThresholdPenalty;
    type NextSessionRotation = PeriodicSessions<Period, Offset>;
    type MaxAffidavitWeights = ConstU32<500>;
    type AffidavitCrypto = AffidavitCryptoSr25519;
    type ElectionAdapter = FairElection<Self>;
    type EmitEvents = ConstBool<true>;
    type Points = u64;
    type PointsAdapter = ChainManager;
    const MAX_FORKS: u32 = 10;
    const MAX_FORK_RECOVERY_TRAVERSAL: u32 = 30;
}

impl SigningTypes for XpTest {
    type Public = MultiSigner;
    type Signature = sp_runtime::MultiSignature;
}

impl CreateTransactionBase<RuntimeCall> for XpTest {
    type Extrinsic = MockUncheckedExtrinsic<XpTest>;
    type RuntimeCall = RuntimeCall;
}

impl CreateSignedTransaction<RuntimeCall> for XpTest {
    fn create_signed_transaction<
        C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>,
    >(
        call: RuntimeCall,
        _public: <Signature as sp_runtime::traits::Verify>::Signer,
        account: <XpTest as frame_system::Config>::AccountId,
        _nonce: <XpTest as frame_system::Config>::Nonce,
    ) -> Option<MockUncheckedExtrinsic<XpTest>> {
        Some(MockUncheckedExtrinsic::<XpTest>::new_signed(
            call,
            account,
            (),
            (), // SignedExtra
        ))
    }
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// ````````````````````````````````` I'M ONLINE ``````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_im_online::Config for XpTest {
    type AuthorityId = pallet_im_online::sr25519::AuthorityId;
    type RuntimeEvent = RuntimeEvent;
    type NextSessionRotation = PeriodicSessions<Period, Offset>;
    type ValidatorSet = pallet_session::historical::Pallet<XpTest>;
    type ReportUnresponsiveness = Offences;
    type UnsignedPriority = ImOnlineUnsignedPriority;
    type WeightInfo = ();
    type MaxKeys = MaxKeys;
    type MaxPeerInHeartbeats = MaxPeerInHeartbeats;
}

impl CreateTransactionBase<pallet_im_online::Call<XpTest>> for XpTest {
    type Extrinsic = MockUncheckedExtrinsic<XpTest>;
    type RuntimeCall = RuntimeCall;
}

impl CreateInherent<pallet_im_online::Call<XpTest>> for XpTest {
    fn create_inherent(call: Self::RuntimeCall) -> Self::Extrinsic {
        MockUncheckedExtrinsic::<XpTest>::new_bare(call)
    }
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````` OFFENCES ```````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_offences::Config for XpTest {
    type RuntimeEvent = RuntimeEvent;
    type IdentificationTuple = pallet_session::historical::IdentificationTuple<XpTest>;
    type OnOffenceHandler = ChainManager;
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````` AUTHORSHIP `````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_authorship::Config for XpTest {
    type FindAuthor = MockFindAuthor;
    type EventHandler = ImOnline;
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````` TIMESTAMP ``````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_timestamp::Config for XpTest {
    type Moment = Moment;
    type OnTimestampSet = MockOnTimestampSet;
    type MinimumPeriod = ConstU64<5>;
    type WeightInfo = ();
}

// ===============================================================================
// ````````````````````````` TEST ENVIRONMENT HELPER FNS `````````````````````````
// ===============================================================================

pub fn issuance_xp_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<XpTest>::default()
        .build_storage()
        .unwrap();
    pallet_xp::GenesisConfig::<XpTest> {
        genesis_acc: vec![
            GenesisAcc {
                owner: ALICE,
                id: ALICE,
            },
            GenesisAcc {
                owner: MIKE,
                id: MIKE,
            },
            GenesisAcc {
                owner: CHARLIE,
                id: CHARLIE,
            },
            GenesisAcc {
                owner: ALAN,
                id: ALAN,
            },
        ],
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    pallet_authors::GenesisConfig::<XpTest> {
        min_collateral: 50,
        min_fund: 25,
        max_exposure: 1000,
        min_elected: 3,
        max_elected: 6,
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::*;
    use crate::{Config, Internals as ChainInternals};
    use frame_suite::{roles::*, RewardAuthors};
    use frame_support::traits::tokens::{Fortitude, Precision};
    use pallet_authors::types::Funder;
    use sp_runtime::AccountId32;

    /// Validates guaranteed failure of payout derivation from total issuance when
    /// [`pallet_chain_manager::Config::InflateViaSupply`] is enabled (`true`).
    ///
    /// ## Pipeline
    ///
    /// - [`pallet_xp::Pallet`]: provides non-issuance-based asset  
    /// - [`pallet_commitment::Pallet`]: manages holds/locks  
    /// - [`pallet_authors::Pallet`]: derives author collateral and funding  
    /// - [`pallet_chain_manager::Pallet`]: computes rewards via  
    ///   [`pallet_chain_manager::Config::InflationModel`]  
    ///   (attempts to use [`frame_support::traits::fungible::Inspect::total_issuance`])
    ///
    /// ## Motivation
    ///
    /// [`pallet_xp::Pallet`] do not support issuance-based queries
    /// and may panic when total supply is accessed via their fungible adapter.
    ///
    /// This test-function ensures that with such an asset, enabling supply-based inflation
    /// results in a panic, rather than producing an incorrect payout.
    #[test]
    #[should_panic]
    fn payout_via_panics_when_inflate_via_supply_is_enabled_with_xp_asset() {
        issuance_xp_test_ext().execute_with(|| {
            set_user_balance_and_hold(ALICE, 250, 250).unwrap();
            set_user_balance_and_hold(CHARLIE, 250, 250).unwrap();
            set_user_balance_and_hold(ALAN, 250, 250).unwrap();
            set_user_balance_and_hold(MIKE, 250, 250).unwrap();

            RoleAdapter::enroll(&ALICE, 200, Fortitude::Force).unwrap();
            RoleAdapter::fund(
                &ALICE,
                &Funder::Direct(CHARLIE),
                100,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();
            RoleAdapter::fund(
                &ALICE,
                &Funder::Direct(ALAN),
                150,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();
            RoleAdapter::fund(
                &ALICE,
                &Funder::Direct(MIKE),
                125,
                Precision::Exact,
                Fortitude::Force,
            )
            .unwrap();

            let _ = ChainInternals::<XpTest>::payout_via();
        })
    }
}
