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
// ```````````````````````````` BALANCES MOCK RUNTIME ````````````````````````````
// ===============================================================================

//! Mock runtime using [`pallet_balances::Pallet`] as the primary asset adapter
//! implementing fungible traits.

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
use frame_suite::Disposition;

// --- FRAME Support ---
use frame_support::{derive_impl, pallet_prelude::*, traits::VariantCountOf};

// --- FRAME System ---
use frame_system::{
    mocking::MockUncheckedExtrinsic,
    offchain::{CreateInherent, CreateSignedTransaction, CreateTransactionBase, SigningTypes},
};

// --- External pallets ---
use pallet_session::PeriodicSessions;

// --- Substrate primitives ---
use sp_core::{ConstBool, ConstU64};
use sp_runtime::{BuildStorage, FixedU64, MultiSigner};

// ===============================================================================
// ``````````````````````````````````` ALIASES ```````````````````````````````````
// ===============================================================================

/// Mock block type for runtime testing
pub type Block = frame_system::mocking::MockBlock<BalancesTest>;

// ===============================================================================
// `````````````````````````````````` CONSTANTS ``````````````````````````````````
// ===============================================================================

/// Base unit for balances (smallest indivisible denomination).
pub const UNIT: Asset = 1_000_000_000_000;

/// One-thousandth of a [`UNIT`] (10^-3 UNIT).
pub const MILLI_UNIT: Asset = 1_000_000_000;

/// One-millionth of a [`UNIT`] (10^-6 UNIT).
pub const MICRO_UNIT: Asset = 1_000_000;

/// Minimum balance required to keep an account alive.
///
/// Accounts with a balance below this value may be reaped.
pub const EXISTENTIAL_DEPOSIT: Asset = MILLI_UNIT;

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
    pub struct BalancesTest;

    #[runtime::pallet_index(0)]
    pub type System = frame_system::Pallet<BalancesTest>;

    #[runtime::pallet_index(1)]
    pub type Commitment = pallet_commitment::Pallet<BalancesTest>;

    #[runtime::pallet_index(2)]
    pub type Balances = pallet_balances::Pallet<BalancesTest>;

    #[runtime::pallet_index(3)]
    pub type Authors = pallet_authors::Pallet<BalancesTest>;

    #[runtime::pallet_index(4)]
    pub type Session = pallet_session::Pallet<BalancesTest>;

    #[runtime::pallet_index(5)]
    pub type Historical = pallet_session::historical::Pallet<BalancesTest>;

    #[runtime::pallet_index(6)]
    pub type ImOnline = pallet_im_online::Pallet<BalancesTest>;

    #[runtime::pallet_index(7)]
    pub type Offences = pallet_offences::Pallet<BalancesTest>;

    #[runtime::pallet_index(8)]
    pub type Authorship = pallet_authorship::Pallet<BalancesTest>;

    #[runtime::pallet_index(9)]
    pub type ChainManager = pallet_chain_manager::Pallet<BalancesTest>;

    #[runtime::pallet_index(10)]
    pub type TimeStamp = pallet_timestamp::Pallet<BalancesTest>;
}

// ===============================================================================
// ``````````````````````````````````` CONFIGS ```````````````````````````````````
// ===============================================================================

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// ``````````````````````````````````` SYSTEM ````````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for BalancesTest {
    type Block = Block;
    type AccountId = AccountId;
    type Lookup = sp_runtime::traits::IdentityLookup<Self::AccountId>;
    type AccountData = pallet_balances::AccountData<Asset>;
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````` COMMITMENT `````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_commitment::Config for BalancesTest {
    type RuntimeEvent = RuntimeEvent;
    type Shares = u64;
    type Bias = FixedU64;
    type Asset = pallet_balances::Pallet<Self>;
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
// ``````````````````````````````````` BALANCES ``````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_balances::Config for BalancesTest {
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ();
    type ReserveIdentifier = [u8; 8];
    type Balance = Asset;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ConstU64<EXISTENTIAL_DEPOSIT>;
    type AccountStore = System;
    type WeightInfo = ();
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type DoneSlashHandler = ();
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// ``````````````````````````````````` AUTHORS ```````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_authors::Config for BalancesTest {
    type RuntimeEvent = RuntimeEvent;
    type CommitmentAdapter = pallet_commitment::Pallet<Self>;
    type AssetFreeze = RuntimeFreezeReason;
    type Asset = pallet_balances::Pallet<Self>;
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

impl pallet_session::Config for BalancesTest {
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

impl pallet_session::historical::Config for BalancesTest {
    type FullIdentification = AccountId;
    type FullIdentificationOf = ChainManager;
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// ```````````````````````````````` CHAIN-MANAGER ````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_chain_manager::Config for BalancesTest {
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type RewardContext = ();
    type RewardModel = SharesPay;
    type InflationContext = MyConstantPayoutContext;
    type InflationModel = ConstantPayout;
    type RoleAdapter = Authors;
    type Asset = pallet_balances::Pallet<Self>;
    type WeightInfo = ();
    type InflateViaSupply = ConstBool<true>;
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

impl SigningTypes for BalancesTest {
    type Public = MultiSigner;
    type Signature = sp_runtime::MultiSignature;
}

impl CreateTransactionBase<RuntimeCall> for BalancesTest {
    type Extrinsic = MockUncheckedExtrinsic<BalancesTest>;
    type RuntimeCall = RuntimeCall;
}

impl CreateSignedTransaction<RuntimeCall> for BalancesTest {
    fn create_signed_transaction<
        C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>,
    >(
        call: RuntimeCall,
        _public: <Signature as sp_runtime::traits::Verify>::Signer,
        account: <BalancesTest as frame_system::Config>::AccountId,
        _nonce: <BalancesTest as frame_system::Config>::Nonce,
    ) -> Option<MockUncheckedExtrinsic<BalancesTest>> {
        Some(MockUncheckedExtrinsic::<BalancesTest>::new_signed(
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

impl pallet_im_online::Config for BalancesTest {
    type AuthorityId = pallet_im_online::sr25519::AuthorityId;
    type RuntimeEvent = RuntimeEvent;
    type NextSessionRotation = PeriodicSessions<Period, Offset>;
    type ValidatorSet = pallet_session::historical::Pallet<BalancesTest>;
    type ReportUnresponsiveness = Offences;
    type UnsignedPriority = ImOnlineUnsignedPriority;
    type WeightInfo = ();
    type MaxKeys = MaxKeys;
    type MaxPeerInHeartbeats = MaxPeerInHeartbeats;
}

impl CreateTransactionBase<pallet_im_online::Call<BalancesTest>> for BalancesTest {
    type Extrinsic = MockUncheckedExtrinsic<BalancesTest>;
    type RuntimeCall = RuntimeCall;
}

impl CreateInherent<pallet_im_online::Call<BalancesTest>> for BalancesTest {
    fn create_inherent(call: Self::RuntimeCall) -> Self::Extrinsic {
        MockUncheckedExtrinsic::<BalancesTest>::new_bare(call)
    }
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````` OFFENCES ```````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_offences::Config for BalancesTest {
    type RuntimeEvent = RuntimeEvent;
    type IdentificationTuple = pallet_session::historical::IdentificationTuple<BalancesTest>;
    type OnOffenceHandler = ChainManager;
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````` AUTHORSHIP `````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_authorship::Config for BalancesTest {
    type FindAuthor = MockFindAuthor;
    type EventHandler = ImOnline;
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````` TIMESTAMP ``````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_timestamp::Config for BalancesTest {
    type Moment = Moment;
    type OnTimestampSet = MockOnTimestampSet;
    type MinimumPeriod = ConstU64<5>;
    type WeightInfo = ();
}

// ===============================================================================
// ````````````````````````` TEST ENVIRONMENT HELPER FNS `````````````````````````
// ===============================================================================

pub fn balances_mock_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<BalancesTest>::default()
        .build_storage()
        .unwrap();
    pallet_authors::GenesisConfig::<BalancesTest> {
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

// ===============================================================================
// ```````````````````````````````````` TESTS ````````````````````````````````````
// ===============================================================================

#[cfg(test)]
mod tests {

    // ===============================================================================
    // ``````````````````````````````````` IMPORTS ```````````````````````````````````
    // ===============================================================================

    // --- Local crate imports ---
    use super::*;
    use crate::Internals;

    // --- FRAME Suite ---
    use frame_suite::RewardAuthors;

    // --- FRAME Support ---
    use frame_support::traits::fungible::Unbalanced;

    // ===============================================================================
    // `````````````````````````````` INFLATE VIA SUPPLY `````````````````````````````
    // ===============================================================================

    /// Validates payout derivation from total issuance when
    /// [`pallet_chain_manager::Config::InflateViaSupply`] is enabled (`true`).
    ///
    /// ## Pipeline
    ///
    /// - [`pallet_balances`]: provides issuance-backed asset  
    /// - [`pallet_commitment`]: manages holds/locks  
    /// - [`pallet_authors`]: derives author collateral and funding  
    /// - [`pallet_chain_manager`]: computes rewards via  
    ///   [`pallet_chain_manager::Config::InflationModel`]  
    ///   (uses [`frame_support::traits::fungible::Inspect::total_issuance`])
    ///
    /// ## Motivation
    ///
    /// Some assets (e.g. [`XP`](pallet_xp)) do not support issuance-based queries
    /// and may panic when total supply is accessed via their
    /// [`fungible adapter`](pallet_xp::fungible).
    ///
    /// This test ensures that with an issuance-backed asset
    /// ([`pallet_balances`]), payout correctly derives from total issuance
    /// under supply-based inflation.
    #[test]
    fn payout_uses_total_issuance_when_inflate_via_supply_enabled() {
        balances_mock_test_ext().execute_with(|| {
            Balances::set_total_issuance(100000);
            let payout = Internals::<BalancesTest>::payout_via();
            assert_eq!(payout, 100000);

            Balances::set_total_issuance(250000);
            let payout = Internals::<BalancesTest>::payout_via();
            assert_eq!(payout, 250000);
        })
    }
}
