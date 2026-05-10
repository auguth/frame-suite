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
// ``````````````````````````` COMMITMENT MOCK RUNTIME ```````````````````````````
// ===============================================================================

//! Mock runtime and test utilities for the Commitment pallet.

#![cfg(feature = "std")]
#![allow(unused)]

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate as pallet_commitment;
use crate::CommitHelpers;

// --- FRAME Plugins ---
use frame_plugins::balances::{ShareBalanceContext, ShareBalanceFamily};

// --- FRAME Suite ---
use frame_suite::{
    commitment::*,
    misc::{Disposition, Ignore},
    plugin_context,
    xp::XpMutate,
};

// --- FRAME Support ---
use frame_support::{
    derive_impl,
    pallet_prelude::*,
    traits::{
        fungible::{Mutate, UnbalancedHold},
        VariantCount,
    },
};

// --- FRAME Pallets ---
use pallet_xp::types::GenesisAcc;

// --- Serialization crates ---
use serde::{Deserialize, Serialize};

// --- Substrate primitives ---
use sp_core::ConstBool;
use sp_runtime::{AccountId32, BuildStorage, DispatchResult, FixedU64, Perbill};

// ===============================================================================
// ``````````````````````````````````` ALIASES ```````````````````````````````````
// ===============================================================================

/// Block type for the mock runtime.
pub type Block = frame_system::mocking::MockBlock<Test>;

/// AccountId type for the mock runtime.
pub type AccountId = AccountId32;

/// System account storage type for the mock runtime.
pub type Account = frame_system::Account<Test>;

/// Asset balance type used for commitments.
pub type Asset = u128;

/// Share unit type used for index/pool distributions.
pub type Shares = u32;

/// Commission `PerThing` type used for pool managements.
pub type Commission = Perbill;

/// Mock pallet type.
pub type Pallet = crate::Pallet<Test>;

/// Mock pallet error type.
pub type Error = crate::Error<Test>;

/// Mock pallet event type.
pub type Event = crate::Event<Test>;

/// Mock pallet internal helper utilities.
pub type CommitHelper = CommitHelpers<Test>;

// --- Config Types ---

/// Configured asset type for the pallet.
pub type AssetOf = <Test as crate::Config>::Asset;

/// Configured position type (commit variants).
pub type Position = <Test as crate::Config>::Position;

// --- Storage Types ---

/// Total committed asset value per commit reason.
pub type ReasonValue = crate::ReasonValue<Test>;

/// Digest information storage (reason, digest -> digest info).
pub type DigestMap = crate::DigestMap<Test>;

/// Commitment storage (proprietor, reason -> commit info).
pub type CommitMap = crate::CommitMap<Test>;

/// Entry-level commit storage (reason, index, entry, proprietor -> commits).
pub type EntryMap = crate::EntryMap<Test>;

/// Index information storage (reason, index -> index info).
pub type IndexMap = crate::IndexMap<Test>;

/// Pool information storage (reason, pool -> pool info).
pub type PoolMap = crate::PoolMap<Test>;

/// Tracks total assets pending issuance.
pub type AssetToIssue = crate::AssetToIssue<Test>;

/// Tracks total assets pending reaping (burn).
pub type AssetToReap = crate::AssetToReap<Test>;

// --- Structs Alias ---

/// Commitment metadata (digest, commits, variant).
pub type CommitInfo = crate::types::CommitInfo<Test>;

/// Index entry metadata (digest, shares, variant).
pub type EntryInfo = crate::types::EntryInfo<Test>;

/// Index metadata (entries, capital, balance).
pub type IndexInfo = crate::types::IndexInfo<Test>;

/// Pool metadata (capital, balance, commission, slots).
pub type PoolInfo = crate::types::PoolInfo<Test>;

/// Pool slot metadata (digest, shares/allocation, variant).
pub type SlotInfo = crate::types::SlotInfo<Test>;

/// Asset imbalance tracking (to re-issue, to mint and to reap).
pub type AssetDelta = crate::types::AssetDelta<Test>;

/// Collection of commit instances (bounded list).
pub type Commits = crate::types::Commits<Test>;

/// Collection of index entries (bounded list).
pub type Entries = crate::types::Entries<Test>;

/// Collection of pool slots (bounded list).
pub type Slots = crate::types::Slots<Test>;

/// Single commit instance (deposit receipt).
pub type CommitInstance = crate::types::CommitInstance<Test>;

/// Digest Distinguisher for direct, indexes, and pools.
pub type DigestVariant = crate::types::DigestVariant<Test>;

/// Digest balances across variants (variant -> lazy balance).
pub type DigestInfo = crate::types::DigestInfo<Test>;

/// Lazy balance representation for a digest-variant.
pub type LazyBalance = crate::types::LazyBalanceOf<Test>;

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

// --- Hold / Freeze Reasons ---

/// Hold reason used to pre-authorize funds before creating commitments.
pub const PREPARE_FOR_COMMIT: HoldReason = HoldReason::PrepareForCommit;

/// Hold reason used for external or auxiliary balance holds.
pub const EXTERNAL_HOLD: HoldReason = HoldReason::External;

/// Freeze reason representing staking-related commitments.
pub const STAKING: FreezeReason = FreezeReason::Staking;

/// Freeze reason representing governance participation commitments.
pub const GOVERNANCE: FreezeReason = FreezeReason::Governance;

/// Freeze reason representing escrow-based commitments.
pub const ESCROW: FreezeReason = FreezeReason::Escrow;

// --- Core Test Values ---

/// Initial balance assigned to test accounts.
pub const INITIAL_BALANCE: Asset = 1000;

/// Standard amount reserved for holds.
pub const STANDARD_HOLD: Asset = 500;

/// Standard commitment amount.
pub const STANDARD_COMMIT: Asset = 250;

/// Large commitment amount.
pub const LARGE_COMMIT: Asset = 500;

/// Small commitment amount.
pub const SMALL_COMMIT: Asset = 100;

/// Standard reward value.
pub const STANDARD_REWARD: Asset = 50;

/// Small reward value.
pub const SMALL_REWARD: Asset = 10;

/// Standard penalty value.
pub const STANDARD_PENALTY: Asset = 100;

/// Small penalty value.
pub const SMALL_PENALTY: Asset = 10;

/// Large asset value.
pub const LARGE_VALUE: Asset = 20;

/// Standard asset value.
pub const STANDARD_VALUE: Asset = 10;

/// Small asset value.
pub const SMALL_VALUE: Asset = 5;

/// Zero asset value.
pub const ZERO_VALUE: Asset = 0;

/// Maximum asset value.
pub const MAX_VALUE: Asset = Asset::MAX;

// --- Shares & Commission ---

/// Dominant share value for distribution tests.
pub const SHARE_DOMINANT: Shares = 60;

/// Major share value for distribution tests.
pub const SHARE_MAJOR: Shares = 40;

/// Equal share value for distribution tests.
pub const SHARE_EQUAL: Shares = 100;

/// Maximum possible share value.
pub const MAX_SHARES: Shares = Shares::MAX;

/// Zero share value (invalid case).
pub const ZERO_SHARE: Shares = 0;

/// Zero commission rate.
pub const COMMISSION_ZERO: Commission = Commission::from_percent(0);

/// One Percent commission rate.
pub const COMMISSION_ONE: Commission = Commission::from_percent(1);

/// Low commission rate.
pub const COMMISSION_LOW: Commission = Commission::from_percent(5);

/// Standard commission rate.
pub const COMMISSION_STANDARD: Commission = Commission::from_percent(10);

/// High commission rate.
pub const COMMISSION_HIGH: Commission = Commission::from_percent(15);

// MAX commission rate.
pub const COMMISSION_MAX: Commission = Commission::from_percent(100);

// --- Block Timeline ---

/// Starting block number.
pub const BLOCK_START: u64 = 1;

/// Early-phase block number.
pub const BLOCK_EARLY: u64 = 100;

/// Mid-phase block number.
pub const BLOCK_MID: u64 = 300;

/// Late-phase block number.
pub const BLOCK_LATE: u64 = 500;

// --- Origin Accounts ---

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

/// AccountId derived from seed `7` (Dev).
pub const DAVE: AccountId = account_frm_seed(7);

/// AccountId derived from seed `7` (Dev).
pub const AMY: AccountId = account_frm_seed(8);

// --- Direct Digests ---

/// Direct digest identifier Alpha.
pub const ALPHA_DIGEST: AccountId = account_frm_seed(11);

/// Direct digest identifier Beta.
pub const BETA_DIGEST: AccountId = account_frm_seed(12);

/// Direct digest identifier Gamma.
pub const GAMMA_DIGEST: AccountId = account_frm_seed(13);

/// Direct digest identifier Delta.
pub const DELTA_DIGEST: AccountId = account_frm_seed(14);

// --- Entry Digests (Index Components) ---

/// Entry digest identifier Alpha.
pub const ALPHA_ENTRY_DIGEST: AccountId = account_frm_seed(21);

/// Entry digest identifier Beta.
pub const BETA_ENTRY_DIGEST: AccountId = account_frm_seed(22);

/// Entry digest identifier Gamma.
pub const GAMMA_ENTRY_DIGEST: AccountId = account_frm_seed(23);

/// Entry digest identifier Delta.
pub const DELTA_ENTRY_DIGEST: AccountId = account_frm_seed(24);

// --- Index Digests ---

/// Index digest identifier Alpha.
pub const ALPHA_INDEX_DIGEST: AccountId = account_frm_seed(31);

/// Index digest identifier Beta.
pub const BETA_INDEX_DIGEST: AccountId = account_frm_seed(32);

/// Index digest identifier Gamma.
pub const GAMMA_INDEX_DIGEST: AccountId = account_frm_seed(33);

/// Index digest identifier Delta.
pub const DELTA_INDEX_DIGEST: AccountId = account_frm_seed(34);

// --- Pool Digests ---

/// Pool digest identifier Alpha.
pub const ALPHA_POOL_DIGEST: AccountId = account_frm_seed(41);

/// Pool digest identifier Beta.
pub const BETA_POOL_DIGEST: AccountId = account_frm_seed(42);

/// Pool digest identifier Gamma.
pub const GAMMA_POOL_DIGEST: AccountId = account_frm_seed(43);

/// Pool digest identifier Delta.
pub const DELTA_POOL_DIGEST: AccountId = account_frm_seed(44);

// --- Staking (Controllers) ---

/// Controller origin for validator Alpha.
pub const VALIDATOR_ALPHA: AccountId = account_frm_seed(51);

/// Controller origin for validator Beta.
pub const VALIDATOR_BETA: AccountId = account_frm_seed(52);

/// Controller origin for validator Gamma.
pub const VALIDATOR_GAMMA: AccountId = account_frm_seed(53);

/// Controller origin for validator Delta.
pub const VALIDATOR_DELTA: AccountId = account_frm_seed(54);

// --- Governance (Proposal Digests) ---

/// Digest identifier for runtime upgrade proposal.
pub const PROPOSAL_RUNTIME_UPGRADE: AccountId = account_frm_seed(61);

/// Digest identifier for treasury spend proposal.
pub const PROPOSAL_TREASURY_SPEND: AccountId = account_frm_seed(62);

// --- Escrow (Contract Digests) ---

/// Digest identifier for freelance escrow contract.
pub const CONTRACT_FREELANCE: AccountId = account_frm_seed(71);

/// Digest identifier for supply chain escrow contract.
pub const CONTRACT_SUPPLY_CHAIN: AccountId = account_frm_seed(72);

// --- Index Digests (Grouped Commitments) ---

/// Digest identifier for optimized staking index.
pub const INDEX_OPTIMIZED_STAKING: AccountId = account_frm_seed(81);

/// Digest identifier for balanced staking index.
pub const INDEX_BALANCED_STAKING: AccountId = account_frm_seed(82);

/// Digest identifier for governance index bundle.
pub const INDEX_GOVERNANCE_BUNDLE: AccountId = account_frm_seed(83);

/// Digest identifier for escrow distribution index.
pub const INDEX_ESCROW_DISTRIBUTION: AccountId = account_frm_seed(84);

// --- Pool Digests (Managed Commitments) ---

/// Digest identifier for managed staking pool.
pub const POOL_MANAGED_STAKING: AccountId = account_frm_seed(91);

/// Digest identifier for expert-managed governance pool.
pub const POOL_EXPERT_GOVERNANCE: AccountId = account_frm_seed(92);

/// Digest identifier for professional escrow pool.
pub const POOL_PROFESSIONAL_ESCROW: AccountId = account_frm_seed(93);

// ===============================================================================
// ````````````````````````` TEST ENVIRONMENT HELPER FNS `````````````````````````
// ===============================================================================

/// Builds test environment with system and pallet genesis state initialized.
pub fn commit_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    crate::GenesisConfig::<Test>::default()
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
            GenesisAcc {
                owner: DAVE,
                id: DAVE,
            },
        ],
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}

/// Sets a user's balance and pre-holds a portion for commitments.
pub fn set_user_balance_and_hold(who: AccountId, balance: Asset, hold: Asset) -> DispatchResult {
    AssetOf::set_balance(&who, balance);
    AssetOf::set_balance_on_hold(&PREPARE_FOR_COMMIT, &who, hold)?;
    Ok(())
}

/// Sets default balance and hold's a standard value for a user (common test setup).
pub fn set_default_user_balance_and_standard_hold(who: AccountId) -> DispatchResult {
    AssetOf::set_balance(&who, INITIAL_BALANCE);
    AssetOf::set_balance_on_hold(&PREPARE_FOR_COMMIT, &who, STANDARD_HOLD)?;
    Ok(())
}

/// Creates an asset account, sets balance, and pre-holds funds for commitments.
pub fn initiate_key_and_set_balance_and_hold(
    who: AccountId,
    balance: Asset,
    hold: Asset,
) -> DispatchResult {
    AssetOf::new_xp(&who, &who);
    AssetOf::set_balance(&who, balance);
    AssetOf::set_balance_on_hold(&PREPARE_FOR_COMMIT, &who, hold)?;
    Ok(())
}

/// Initializes a digest with a default variant balance (required before commits
/// for low-level helper utilities testing).
pub fn initiate_digest_with_default_balance(
    reason: FreezeReason,
    digest: AccountId,
) -> DispatchResult {
    let mut digest_info = DigestInfo::default();
    // Initialize with default balance for Affirmative variant (index 0)
    digest_info
        .init_balance(&Default::default())
        .map_err(|_| "Failed to push default variant balance")?;
    DigestMap::insert((reason, digest), digest_info);
    Ok(())
}

/// Prepares an index from entries and stores it under the given identifier.
pub fn prepare_and_initiate_index(
    who: AccountId,
    reason: FreezeReason,
    entries: &[(AccountId, Shares)], //(entry_digest, shares)
    index_digest: AccountId,
) -> DispatchResult {
    let index = Pallet::prepare_index(&who, &reason, entries)?;
    Pallet::set_index(&who, &reason, &index, &index_digest)?;
    Ok(())
}

/// Prepares an index and then creates a pool on top of it with commission.
pub fn prepare_and_initiate_pool(
    who: AccountId,
    reason: FreezeReason,
    entries: &[(AccountId, Shares)],
    index_digest: AccountId,
    pool_digest: AccountId,
    commission: Commission,
) -> DispatchResult {
    prepare_and_initiate_index(who.clone(), reason, entries, index_digest.clone())?;
    Pallet::set_pool(&who, &reason, &pool_digest, &index_digest, commission).unwrap();
    Ok(())
}

/// Asserts panic in debug builds or specific error in release builds.
#[macro_export]
macro_rules! assert_debug_panic_or_err {
    ($expr:expr, $err:expr) => {{
        use std::panic::{catch_unwind, AssertUnwindSafe};

        let result = catch_unwind(AssertUnwindSafe(|| $expr));

        #[cfg(debug_assertions)]
        {
            assert!(
                result.is_err(),
                "Expected panic in debug build, but call succeeded"
            );
        }

        #[cfg(not(debug_assertions))]
        {
            let inner = result.expect("Call panicked in release build");
            assert_err!(inner, $err);
        }
    }};
}

// ===============================================================================
// ``````````````````````````````````` RUNTIME ```````````````````````````````````
// ===============================================================================

/// Mock runtime configuration module.
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
}

// ===============================================================================
// ``````````````````````````````` COMPOSITE ENUMS ```````````````````````````````
// ===============================================================================

#[derive(
    Clone,
    Copy,
    PartialEq,
    PartialOrd,
    Ord,
    Eq,
    DecodeWithMemTracking,
    RuntimeDebug,
    Encode,
    Decode,
    TypeInfo,
    MaxEncodedLen,
    Serialize,
    Deserialize,
)]
/// Mock Hold Reason (Reserves)
pub enum HoldReason {
    PrepareForCommit,
    External,
}

impl VariantCount for HoldReason {
    const VARIANT_COUNT: u32 = 2;
}

#[derive(
    Clone,
    Copy,
    PartialEq,
    PartialOrd,
    Ord,
    Eq,
    DecodeWithMemTracking,
    RuntimeDebug,
    Encode,
    Decode,
    TypeInfo,
    MaxEncodedLen,
    Serialize,
    Deserialize,
)]
/// Mock Freeze Reason (Locks)
pub enum FreezeReason {
    Staking,
    Governance,
    Escrow,
    Benchmarks,
}

impl VariantCount for FreezeReason {
    const VARIANT_COUNT: u32 = 4;
}

impl From<pallet_commitment::HoldReason> for HoldReason {
    fn from(reason: pallet_commitment::HoldReason) -> Self {
        match reason {
            crate::HoldReason::PrepareForCommit => HoldReason::PrepareForCommit,
        }
    }
}

impl From<pallet_commitment::FreezeReason> for FreezeReason {
    fn from(reason: pallet_commitment::FreezeReason) -> Self {
        match reason {
            crate::FreezeReason::BenchTestReason => FreezeReason::Benchmarks,
        }
    }
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

impl pallet_commitment::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Shares = Shares;
    type Bias = FixedU64;
    type Asset = Xp;
    type Commission = Commission;
    type Position = Disposition;
    type AssetHold = HoldReason;
    type AssetFreeze = FreezeReason;
    type MaxIndexEntries = ConstU32<3>;
    type MaxCommits = ConstU32<3>;
    type Time = u32;
    type BalanceFamily<'a> = ShareBalanceFamily<'a>;
    type BalanceContext = MyBalanceContext<Commitment>;
    type WeightInfo = ();
    type EmitEvents = ConstBool<true>;
}

plugin_context!(
    name: pub MyBalanceContext,
    context: ShareBalanceContext<T>,
    marker: [T,],
    value: ShareBalanceContext(PhantomData)
);

impl pallet_xp::Config for Test {
    type Xp = Asset;
    type Pulse = u32;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type LockReason = FreezeReason;
    type ReserveReason = HoldReason;
    type Extensions = Ignore<Xp>;
    type WeightInfo = ();
    type EmitEvents = ConstBool<true>;
}
