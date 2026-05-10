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
// ````````````````````````````` AUTHORS MOCK RUNTIME ````````````````````````````
// ===============================================================================

//! Mock runtime and test utilities for the Authors pallet.

#![cfg(feature = "std")]
#![allow(unused)]

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Core / Std ---
use core::{cell::RefCell, u64};

// --- Local crate imports ---
use crate as pallet_authors;

// --- FRAME Plugins ---
use frame_plugins::{
    balances::{ShareBalanceContext, ShareBalanceFamily},
    elections::{fair, flat},
    influence::LinearModel,
    penalty::ThresholdPenaltyConfig,
    rewards::payout::ConstantPayoutConfig,
};

// --- FRAME Suite ---
use frame_suite::{
    commitment::{CommitIndex, CommitPool, Commitment as Commit},
    plugin_context,
    roles::RoleActivity,
    Disposition, Ignore,
};

// --- FRAME Support ---
use frame_support::{
    derive_impl,
    pallet_prelude::*,
    traits::fungible::{Inspect, InspectHold, Mutate, UnbalancedHold},
};

// --- External pallets ---
use pallet_xp::types::GenesisAcc;

// --- Substrate primitives ---
use sp_core::ConstBool;
use sp_runtime::{AccountId32, BuildStorage, FixedU64, Perbill};

// ===============================================================================
// ``````````````````````````````````` ALIASES ```````````````````````````````````
// ===============================================================================

/// Mock block type used for testing runtime
pub type Block = frame_system::mocking::MockBlock<Test>;

/// Account identifier type (32-byte address)
pub type AccountId = AccountId32;

/// Commission rate type (percentage)
pub type Commission = Perbill;

/// Asset/balance type used in tests
pub type Asset = u64;

/// Mock asset type for the pallet.
pub type AuthorAsset = <Test as crate::Config>::Asset;

/// Mock commitment traits adapter for the pallet.
pub type CommitAdapter = <Test as crate::Config>::CommitmentAdapter;

/// Mock pallet type.
pub type Pallet = crate::Pallet<Test>;

/// Mock pallet error type.
pub type Error = crate::Error<Test>;

/// Mock pallet event type.
pub type Event = crate::Event<Test>;

/// Mock Digest representing an author's commitment identity
pub type AuthorDigest = crate::types::AuthorDigest<Test>;

/// Mock Digest representing an index (grouped commitments)
pub type IndexDigest = crate::types::IndexDigest<Test>;

/// Mock Digest representing a pool (managed commitments with commission)
pub type PoolDigest = crate::types::PoolDigest<Test>;

/// Mock Mapping from author digest -> author account
pub type AuthorsDigest = crate::AuthorsDigest<Test>;

/// Mock Storage map holding author metadata/state
pub type AuthorsMap = crate::AuthorsMap<Test>;

/// Mock Global maximum funding exposure per author
pub type MaxExposure = crate::MaxExposure<Test>;

/// Mock Global minimum funding required to back an author
pub type MinFund = crate::MinFund<Test>;

/// Mock Mapping of (author, backer) -> funding details
pub type AuthorFunders = crate::AuthorFunders<Test>;

/// Mock Storage of scheduled penalties for authors
pub type AuthorPenalties = crate::AuthorPenalties<Test>;

/// Mock Storage of scheduled rewards for authors
pub type AuthorRewards = crate::AuthorRewards<Test>;

/// Mock Global probation period for authors
pub type ProbationPeriod = crate::ProbationPeriod<Test>;

/// Mock Blocks reduced from probation on good behavior
pub type ReduceProbationBy = crate::ReduceProbationBy<Test>;

/// Mock Blocks added to probation on bad behavior
pub type IncreaseProbationBy = crate::IncreaseProbationBy<Test>;

/// Mock Buffer delay before rewards are applied
pub type RewardsBuffer = crate::RewardsBuffer<Test>;

/// Mock Buffer delay before penalties are applied
pub type PenaltiesBuffer = crate::PenaltiesBuffer<Test>;

/// Mock Maximum number of authors that can be elected
pub type MaxElected = crate::MaxElected<Test>;

/// Mock Minimum number of authors required for election
pub type MinElected = crate::MinElected<Test>;

/// Mock Flag to enforce strict max elected limit
pub type ForceMaxElected = crate::ForceMaxElected<Test>;

/// Mock Minimum collateral required to become an author
pub type MinCollateral = crate::MinCollateral<Test>;

/// Mock Flat election model (influence-based)
pub type FlatElection = crate::FlatElection<Test>;

/// Mock Fair election model (backing-based)
pub type FairElection = crate::FairElection<Test>;

// ===============================================================================
// ``````````````````````````````````` CONST FN ``````````````````````````````````
// ===============================================================================

/// Creates deterministic test accounts from a seed value.
///
/// Places seed in the last byte for easy debugging - account addresses
/// will be `0x000...00{seed}`, making them visually distinct in test output.
pub const fn account_frm_seed(seed: u8) -> AccountId {
    let mut data = [0u8; 32];
    data[31] = seed;
    AccountId::new(data)
}

// ===============================================================================
// ``````````````````````````````````` CONSTS ````````````````````````````````````
// ===============================================================================

/// AccountId derived from seed `1` (Alice).
pub const ALICE: AccountId = account_frm_seed(1);

/// AccountId derived from seed `2` (Bob).
pub const BOB: AccountId = account_frm_seed(2);

/// AccountId derived from seed `3` (Charlie).
pub const CHARLIE: AccountId = account_frm_seed(3);

/// AccountId derived from seed `4` (Alan).
pub const ALAN: AccountId = account_frm_seed(4);

/// AccountId derived from seed `5` (Mike).
pub const MIKE: AccountId = account_frm_seed(5);

/// AccountId derived from seed `6` (Nix).
pub const NIX: AccountId = account_frm_seed(6);

/// AccountId derived from seed `7` (AMY).
pub const AMY: AccountId = account_frm_seed(7);

/// Hold reason used to pre-authorize funds before creating commitments.
pub const COMMITMENT_RESERVE: pallet_commitment::HoldReason =
    pallet_commitment::HoldReason::PrepareForCommit;

/// Freeze reason representing author-funding/backing related commitments.
pub const FUNDING: crate::FreezeReason = crate::FreezeReason::AuthorFunding;

/// Freeze reason representing author enrollment collateral commitment.
pub const COLLATERAL: crate::FreezeReason = crate::FreezeReason::AuthorCollateral;

/// Initial balance assigned to test accounts.
pub const INITIAL_BALANCE: Asset = 100;

/// Standard amount reserved for holds.
pub const STANDARD_HOLD: Asset = 250;

/// Index digest identifier.
pub const INDEX_DIGEST: AccountId = account_frm_seed(11);

/// Pool digest identifier.
pub const POOL_DIGEST: AccountId = account_frm_seed(12);

/// Large asset amount.
pub const LARGE_VALUE: Asset = 100;

/// Standard asset amount.
pub const STANDARD_VALUE: Asset = 50;

/// Small asset amount.
pub const SMALL_VALUE: Asset = 25;

/// Minimum asset amount.
pub const MIN_VALUE: Asset = 10;

// ===============================================================================
// ````````````````````````` TEST ENVIRONMENT HELPER FNS `````````````````````````
// ===============================================================================

/// Builds test environment with system and pallet genesis state initialized.
pub fn authors_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_authors::GenesisConfig::<Test> {
        min_collateral: 50,
        min_fund: 25,
        max_exposure: 1000,
        min_elected: 6,
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    pallet_xp::GenesisConfig::<Test> {
        genesis_acc: vec![
            GenesisAcc {
                owner: ALICE,
                id: ALICE,
            },
            GenesisAcc {
                owner: BOB,
                id: BOB,
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
            GenesisAcc {
                owner: NIX,
                id: NIX,
            },
        ],
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}

// Sets user balance and places a portion on hold for commitments
pub fn initiate_key_and_set_balance_and_hold(
    who: &AccountId,
    amount: u64,
    amount_to_hold: u64,
) -> DispatchResult {
    AuthorAsset::set_balance(&who, amount);
    AuthorAsset::set_balance_on_hold(&COMMITMENT_RESERVE.into(), &who, amount_to_hold)?;
    Ok(())
}

// Returns total balance of the user
pub fn get_user_balance(who: &AccountId) -> u64 {
    AuthorAsset::balance(who)
}

// Returns balance currently held for commitments
pub fn get_user_hold_balance(who: &AccountId) -> u64 {
    AuthorAsset::balance_on_hold(&COMMITMENT_RESERVE.into(), who)
}

// Generates a unique digest for an author (used for commitments)
pub fn gen_author_digest(who: &AccountId) -> Result<AccountId, DispatchError> {
    let digest = CommitAdapter::gen_digest(who)?;
    Ok(digest)
}

// Prepares an index from entries and registers it under a digest
pub fn prepare_and_initiate_index(
    who: AccountId,
    reason: RuntimeFreezeReason,
    entries: &[(AccountId, u64)], //(entry_digest, shares)
    index_of: AccountId,
) -> DispatchResult {
    let index = CommitAdapter::prepare_index(&who, &reason, entries)?;
    CommitAdapter::set_index(&who, &reason, &index, &index_of)?;
    Ok(())
}

// Creates a pool from an index with a commission configuration
pub fn prepare_and_initiate_pool(
    who: AccountId,
    reason: RuntimeFreezeReason,
    entries: &[(AccountId, u64)],
    index_of: AccountId,
    pool_of: AccountId,
    commission: Perbill,
) -> DispatchResult {
    prepare_and_initiate_index(who.clone(), reason, entries, index_of.clone())?;
    CommitAdapter::set_pool(&who, &reason, &pool_of, &index_of, commission).unwrap();
    Ok(())
}

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
    pub struct Test;

    #[runtime::pallet_index(0)]
    pub type System = frame_system::Pallet<Test>;

    #[runtime::pallet_index(1)]
    pub type Commitment = pallet_commitment::Pallet<Test>;

    #[runtime::pallet_index(2)]
    pub type Xp = pallet_xp::Pallet<Test>;

    #[runtime::pallet_index(3)]
    pub type Authors = pallet_authors::Pallet<Test>;
}

// ===============================================================================
// ``````````````````````````````````` CONFIGS ```````````````````````````````````
// ===============================================================================

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = AccountId;
    type Lookup = sp_runtime::traits::IdentityLookup<Self::AccountId>;
}

impl pallet_xp::Config for Test {
    type Xp = Asset;
    type Pulse = u32;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type LockReason = RuntimeFreezeReason;
    type ReserveReason = RuntimeHoldReason;
    type Extensions = Ignore<Xp>;
    type WeightInfo = ();
    type EmitEvents = ConstBool<true>;
}

impl pallet_commitment::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Commission = Commission;
    type Shares = u64;
    type Bias = FixedU64;
    type Asset = pallet_xp::Pallet<Self>;
    type Position = Disposition;
    type AssetHold = RuntimeHoldReason;
    type AssetFreeze = RuntimeFreezeReason;
    type WeightInfo = ();
    type MaxIndexEntries = ConstU32<3>;
    type MaxCommits = ConstU32<3>;
    type Time = u32;
    type BalanceFamily<'a> = ShareBalanceFamily<'a>;
    type BalanceContext = MyBalanceContext<Commitment>;
    type EmitEvents = ConstBool<true>;
}

plugin_context!(
    name: pub MyBalanceContext,
    context: ShareBalanceContext<T>,
    marker: [T,],
    value: ShareBalanceContext(PhantomData)
);

impl pallet_authors::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type CommitmentAdapter = pallet_commitment::Pallet<Self>;
    type AssetFreeze = RuntimeFreezeReason;
    type Influence = u64;
    type Asset = pallet_xp::Pallet<Self>;
    type InfluenceContext = ();
    type InfluenceModel = LinearModel;
    type FlatElectionContext = ();
    type FlatElectionModel = flat::TopDownFlatModel;
    type FairElectionContext = ();
    type FairElectionModel = fair::TopDownFairModel;
    type ActivityProvider = DummyActivityProvider;
    type WeightInfo = ();
    type EmitEvents = ConstBool<true>;
}

plugin_context!(
    name: MyConstantPayoutContext,
    context: ConstantPayoutConfig<u64>,
    value: ConstantPayoutConfig::<u64> {
        payout: 100u64
    }
);

plugin_context!(
    name: MyPenaltyThresholdContext,
    context: ThresholdPenaltyConfig<Perbill>,
    value: ThresholdPenaltyConfig::<Perbill> {
        threshold: Perbill::from_percent(70)
    }
);

// ===============================================================================
// ``````````````````````` AUTHOR DUMMY ACTIVITY PROVIDER ````````````````````````
// ===============================================================================

pub struct DummyActivityProvider;

#[derive(
    Encode,
    Decode,
    RuntimeDebug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    TypeInfo,
    MaxEncodedLen,
    DecodeWithMemTracking,
)]
pub enum DummyActivity {
    AuthorIsActive,
}

impl From<DummyActivity> for DispatchError {
    fn from(e: DummyActivity) -> DispatchError {
        match e {
            DummyActivity::AuthorIsActive => DispatchError::Other("AuthorIsActive"),
        }
    }
}

impl RoleActivity<AccountId, u64> for DummyActivityProvider {
    type Activity = DummyActivity;

    fn is_idle(_who: &AccountId) -> Result<(), DummyActivity> {
        ACTIVITY_STATE.with(|state| match *state.borrow() {
            true => Err(DummyActivity::AuthorIsActive),
            false => Ok(()),
        })
    }
}

thread_local! {
    pub static ACTIVITY_STATE: RefCell<bool> = RefCell::new(false);
}

/// Set author's dummy activity (duty)
pub fn set_activity_state(is_active: bool) {
    ACTIVITY_STATE.with(|state| *state.borrow_mut() = is_active);
}
