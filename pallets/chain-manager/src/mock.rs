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
// ````````````````````````` CHAIN-MANAGER MOCK RUNTIME ``````````````````````````
// ===============================================================================

//! Mock runtime and test utilities for the Chain Manager pallet.

#![cfg(feature = "std")]
#![allow(unused)]

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Core / Std ---
use core::cell::RefCell;
use logtest::Logger;
use parking_lot::RwLock;
use std::sync::{Arc, Once};

// --- Local crate imports ---
use crate::{self as pallet_chain_manager, crypto::*, routines::*, types::SessionIndex};

// --- FRAME Suite ---
use frame_suite::{ForksHandler, blockchain::*, misc::*, plugin_context, roles::*, routines::*};

// --- FRAME Plugins ---
use frame_plugins::{
    balances::{ShareBalanceContext, ShareBalanceFamily},
    elections::{fair, flat},
    influence::LinearModel,
    penalty::{ThresholdPenalty, ThresholdPenaltyConfig},
    rewards::{
        payee::SharesPay,
        payout::{ConstantPayout, ConstantPayoutConfig},
    },
};

// --- FRAME Support ---
use frame_support::{
    derive_impl,
    dispatch::DispatchResult,
    ensure,
    pallet_prelude::*,
    parameter_types,
    traits::{
        fungible::{Mutate, UnbalancedHold},
        tokens::{Fortitude, Precision},
        FindAuthor, Hooks, OnTimestampSet,
    },
};

// --- FRAME System ---
use frame_system::{
    mocking::MockUncheckedExtrinsic,
    offchain::{
        AppCrypto, CreateInherent, CreateSignedTransaction, CreateTransactionBase, SigningTypes,
    },
};

// --- External pallets ---
use pallet_session::PeriodicSessions;
use pallet_xp::types::GenesisAcc;

// --- Substrate primitives ---
use sp_core::{
    crypto::ByteArray,
    offchain::{
        testing::{TestOffchainExt, TestTransactionPoolExt},
        OffchainDbExt, OffchainWorkerExt, TransactionPoolExt,
    },
    ConstBool, ConstU64,
};

use sp_io::TestExternalities;

use sp_keystore::{testing::MemoryKeystore, KeystoreExt};

use sp_runtime::{
    traits::{BlakeTwo256, Hash, Convert, IdentifyAccount},
    impl_opaque_keys, AccountId32, BuildStorage, ConsensusEngineId, 
    DispatchError, FixedU64, MultiSigner, Perbill, Percent, 
};

// ===============================================================================
// ``````````````````````````````````` ALIASES ```````````````````````````````````
// ===============================================================================

// --- Core Runtime Types ---

/// Mock block type for runtime testing
pub type Block = frame_system::mocking::MockBlock<Test>;

/// Account identifier type (32-byte)
pub type AccountId = AccountId32;

/// Block number type for tests
pub type BlockNumber = u64;

// --- Pallet & Core Aliases ---

/// Mock pallet alias
pub type Pallet = crate::Pallet<Test>;

/// Mock pallet error type
pub type Error = crate::Error<Test>;

/// Mock pallet event type
pub type Event = crate::Event<Test>;

/// Mock pallet call type
pub type Call = crate::Call<Test>;

/// Mock internals (election logic)
pub type Internals = crate::Internals<Test>;

// --- Author & Funding ----

/// Asset / balance type for tests
pub type Asset = u64;

/// Commission type (per-billion) for pools
pub type Commission = Perbill;

/// Author type alias
pub type AuthorOf = crate::types::AuthorOf<Test>;

pub type PenaltyRatio = crate::types::PenaltyRatio;

// --- Offchain Crypto & Signing Types ---

/// Unchecked extrinsic type for tests
pub type UncheckedExtrinsic = MockUncheckedExtrinsic<Test>;

/// Runtime app public (affidavit crypto)
pub type RuntimeAppPublic = <<Test as crate::Config>::AffidavitCrypto as AppCrypto<
    <Test as frame_system::offchain::SigningTypes>::Public,
    <Test as frame_system::offchain::SigningTypes>::Signature,
>>::RuntimeAppPublic;

/// Generic public key type
pub type GenericPublic = <<Test as crate::Config>::AffidavitCrypto as AppCrypto<
    <Test as frame_system::offchain::SigningTypes>::Public,
    <Test as frame_system::offchain::SigningTypes>::Signature,
>>::GenericPublic;

/// Public key type
pub type Public = <Test as frame_system::offchain::SigningTypes>::Public;

/// Signature type
pub type Signature = <Test as frame_system::offchain::SigningTypes>::Signature;

/// Affidavit crypto implementation
pub type AffidavitCrypto = <Test as crate::Config>::AffidavitCrypto;

// --- Config Adapters & Traits ---

/// Asset type from config
pub type AssetOf = <Test as crate::Config>::Asset;

/// Role adapter from config
pub type RoleAdapter = <Test as crate::Config>::RoleAdapter;

/// Points adapter from config
pub type PointsAdapter = <Test as crate::Config>::PointsAdapter;

/// Session rotation logic
pub type NextSessionRotation = <Test as crate::Config>::NextSessionRotation;

// --- Storage Aliases ---

/// Session start storage
pub type SessionStartsAt = crate::SessionStartAt<Test>;

/// Current session storage
pub type CurrentSession = crate::CurrentSession<Test>;

/// Affidavit enable flag storage
pub type AllowAffidavits = crate::AllowAffidavits<Test>;

/// Affidavit start percentage
pub type AffidavitBeginsAt = crate::AffidavitBeginsAt<Test>;

/// Election start percentage
pub type ElectionBeginsAt = crate::ElectionBeginsAt<Test>;

/// Affidavit end percentage
pub type AffidavitEndsAt = crate::AffidavitEndsAt<Test>;

/// Affidavit key mapping storage
pub type AffidavitKeys = crate::AffidavitKeys<Test>;

/// Election runner tracking storage
pub type ElectsPreparedBy = crate::ElectsPreparedBy<Test>;

/// Election runner points storage
pub type ElectionRunnerPoints = crate::ElectionRunnerPoints<Test>;

/// Election runner points upgrade
pub type ElectionRunnerPointsUpgrade = crate::ElectionRunnerPointsUpgrade<Test>;

/// Finality ticks config
pub type FinalityTicks = crate::FinalityTicks<Test>;

/// Finality time config
pub type FinalityAfter = crate::FinalityAfter<Test>;

/// Author affidavit handler
pub type AuthorOfAffidavits = crate::AuthorAffidavits<Test>;

// --- Affidavit/Election Types ---

/// Affidavit identifier type
pub type AffidavitId = AccountId;

/// Affidavit window type
pub type AffidavitWindow = crate::types::AffidavitWindow<Test>;

/// Election window type
pub type ElectionWindow = crate::types::ElectionWindow<Test>;

pub type Duration = crate::types::Duration;

// --- Author Activity ---

/// Author activity tracking
pub type AuthorActivity = crate::roles::AuthorActivity<Test>;

// --- Unsigned Transactions ---

/// Validate tx priority type
pub type ValidateTxPriority = crate::ValidateTxPriority<Test>;

/// Affidavit tx priority type
pub type AffidavitTxPriority = crate::AffidavitTxPriority<Test>;

/// Election tx priority type
pub type ElectionTxPriority = crate::ElectionTxPriority<Test>;

// --- Payload Types ---

/// Validate payload type
pub type ValidatePayloadOf = crate::types::ValidatePayloadOf<Test>;

/// Affidavit payload type
pub type AffidavitPayloadOf = crate::types::AffidavitPayloadOf<Test>;

/// Election payload type
pub type ElectionPayloadOf = crate::types::ElectionPayloadOf<Test>;

// --- Routine Types (OCW) ---

/// Election routine context
pub type TryElection = crate::types::TryElection<Test>;

/// Affidavit key initiate routine context
pub type InitAffidavitKey = crate::types::InitAffidavitKey<Test>;

/// Declare affidavit routine
pub type DeclareAffidavit = crate::types::DeclareAffidavit<Test>;

/// Rotate affidavit key routine
pub type RotateAffidavitKey = crate::types::RotateAffidavitKey<Test>;

// --- Offchain Storage Models ---

/// Finalized storage for affidavit key init
pub type FinalizedInitAfdtKey = Finalized<Test, AffidavitId, InitAffidavitKey, Pallet>;

/// Fork-aware storage for affidavit key init
pub type ForkAwareInitAfdtKey = ForkAware<Test, ValueHash, InitAffidavitKey, Pallet>;

/// Persistent storage for affidavit key init
pub type PersistentInitAfdtKey = Persistent<Test, Ledger<Test, AccountId>, InitAffidavitKey>;

// --- External / Integration Types ---

/// Funder type alias
pub type Funder = pallet_authors::types::Funder<Test>;

/// Session validator set
pub type Validators = pallet_session::Validators<Test>;

/// Recent elected tracking
pub type RecentElectedOn = pallet_authors::RecentElectedOn<Test>;

/// Elected authors set
pub type Elected = pallet_authors::Elected<Test>;

/// Maximum elected authors bound
pub type ForceMaxElected = pallet_authors::ForceMaxElected<Test>;

// --- Offchain Test Environment Types ---

/// Mock transaction pool state
pub type PoolState = Arc<RwLock<sp_core::offchain::testing::PoolState>>;

/// Mock offchain state
pub type OffchainState = Arc<RwLock<sp_core::offchain::testing::OffchainState>>;

/// Mock keystore
pub type KeyStore = Arc<sp_keystore::testing::MemoryKeystore>;

// --- Genesis Config ---

/// System genesis config
pub type SystemGenesis = frame_system::GenesisConfig<Test>;

/// XP pallet genesis config
pub type XpGenesis = pallet_xp::GenesisConfig<Test>;

/// Authors pallet genesis config
pub type AuthorsGenesis = pallet_authors::GenesisConfig<Test>;

/// Chain manager genesis config
pub type ChainManagerGenesis = pallet_chain_manager::GenesisConfig<Test>;

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
// `````````````````````````````````` CONSTANTS ``````````````````````````````````
// ===============================================================================

// --- Mock Accounts ---

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

/// AccountId derived from seed `7` (Dave).
pub const DAVE: AccountId = account_frm_seed(7);

/// AccountId derived from seed `8` (Laya).
pub const LAYA: AccountId = account_frm_seed(8);

/// AccountId derived from seed `9` (Jake).
pub const JAKE: AccountId = account_frm_seed(9);

/// AccountId derived from seed `10` (Jim).
pub const JIM: AccountId = account_frm_seed(10);

/// AccountId derived from seed `11` (Paul).
pub const PAUL: AccountId = account_frm_seed(11);

/// AccountId derived from seed `12` (Amy).
pub const AMY: AccountId = account_frm_seed(12);

// --- Block Intervals ---

/// Milliseconds per block (6 seconds block time).
pub const MILLI_SECS_PER_BLOCK: BlockNumber = 6000;

/// Number of blocks per minute.
pub const MINUTES: BlockNumber = 60_000 / MILLI_SECS_PER_BLOCK;

/// Number of blocks per hour.
pub const HOURS: BlockNumber = MINUTES * 60;

/// Number of blocks per day.
pub const DAYS: BlockNumber = HOURS * 24;

// --- Asset & Commitment Value ---

/// Initial balance assigned to test accounts.
pub const INITIAL_BALANCE: Asset = 1000;

/// Standard amount reserved for holds (pre-commitment reserve).
pub const STANDARD_HOLD: Asset = 500;

/// Standard collateral amount for typical author enrollment.
pub const STANDARD_COLLATERAL: Asset = 250;

/// Large collateral amount for high-stake scenarios.
pub const LARGE_COLLATERAL: Asset = 500;

/// Small collateral amount for low-stake testing.
pub const SMALL_COLLATERAL: Asset = 100;

/// Minimum collateral required by the system.
pub const MIN_COLLATERAL: Asset = 50;

/// Standard funding amount for backing an author.
pub const STANDARD_FUND: Asset = 250;

/// Large funding amount for high-exposure scenarios.
pub const LARGE_FUND: Asset = 500;

/// Small funding amount for minimal backing.
pub const SMALL_FUND: Asset = 100;

/// Minimum funding required to support an author.
pub const MIN_FUND: Asset = 25;

/// Zero-value constant (no balance or amount).
pub const VALUE_ZERO: Asset = 0;

/// Maximum possible asset value (upper bound).
pub const VALUE_MAX: Asset = Asset::MAX;

/// Hold reason used for reserving balance during commitment preparation
pub const COMMITMENT_HOLD: pallet_commitment::HoldReason =
    pallet_commitment::HoldReason::PrepareForCommit;

// --- Block Periods ---

/// Block at which the session begins
pub const SESSION_START: BlockNumber = 1;

/// Block at which affidavit submission starts
pub const AFDT_SUBMISSION_START: BlockNumber = 121;

/// Block at which affidavit submission ends
pub const AFDT_SUBMISSION_END: BlockNumber = 481;

/// Block at which election starts. Ends at [`AFDT_SUBMISSION_END`]
pub const ELECTION_START: BlockNumber = 301;

/// Block at which the session ends
pub const SESSION_END: BlockNumber = 601;

// --- Affidavit Keys ---

/// Predefined affidavit key A (seed 101)
pub const AFFIDAVIT_KEY_A: AccountId = account_frm_seed(101);

/// Predefined affidavit key B (seed 102)
pub const AFFIDAVIT_KEY_B: AccountId = account_frm_seed(102);

/// Predefined affidavit key C (seed 103)
pub const AFFIDAVIT_KEY_C: AccountId = account_frm_seed(103);

// ===============================================================================
// ````````````````````````` TEST ENVIRONMENT HELPER FNS `````````````````````````
// ===============================================================================

/// Builds test environment with system and pallet genesis state initialized.
pub fn chain_manager_test_ext() -> sp_io::TestExternalities {
    let mut t = SystemGenesis::default().build_storage().unwrap();
    XpGenesis {
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
            GenesisAcc {
                owner: LAYA,
                id: LAYA,
            },
            GenesisAcc {
                owner: JAKE,
                id: JAKE,
            },
            GenesisAcc {
                owner: JIM,
                id: JIM,
            },
            GenesisAcc {
                owner: PAUL,
                id: PAUL,
            },
            GenesisAcc {
                owner: AMY,
                id: AMY,
            },
        ],
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    AuthorsGenesis {
        min_collateral: 50,
        min_fund: 25,
        max_exposure: 1000,
        min_elected: 3,
        max_elected: 6,
        force_max_elected: true,
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    ChainManagerGenesis {
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}

/// Sets the current block's author
pub fn set_block_author(author: AccountId) {
    CURRENT_AUTHOR.with(|a| {
        *a.borrow_mut() = Some(author);
    });
}

/// Clears the current block's author.
pub fn clear_block_author() {
    CURRENT_AUTHOR.with(|a| {
        *a.borrow_mut() = None;
    });
}

/// Sets balance and places the hold (commitment reserve) for a user
pub fn set_user_balance_and_hold(
    who: AccountId,
    amount: Asset,
    amount_to_hold: Asset,
) -> DispatchResult {
    AssetOf::set_balance(&who, amount);
    AssetOf::set_balance_on_hold(&COMMITMENT_HOLD.into(), &who, amount_to_hold)?;
    Ok(())
}

/// Sets default balance and hold (commitment reserve) amount for a user
pub fn set_default_user_balance_and_hold(who: AccountId) -> DispatchResult {
    AssetOf::set_balance(&who, INITIAL_BALANCE);
    AssetOf::set_balance_on_hold(&COMMITMENT_HOLD.into(), &who, STANDARD_HOLD)?;
    Ok(())
}

/// Sets default balance and commit-reserve hold amount for multiple users
pub fn set_default_users_balance_and_hold(users: Vec<AccountId>) -> DispatchResult {
    for user in users {
        AssetOf::set_balance(&user, INITIAL_BALANCE);
        AssetOf::set_balance_on_hold(&COMMITMENT_HOLD.into(), &user, STANDARD_HOLD)?;
    }
    Ok(())
}

/// Enrolls multiple authors with default collateral
pub fn enroll_authors_with_default_collateral(authors: Vec<AuthorOf>) -> DispatchResult {
    for author in authors {
        RoleAdapter::enroll(&author, STANDARD_COLLATERAL, Fortitude::Force)?;
    }
    Ok(())
}

/// Enrolls a single author with default collateral
pub fn enroll_author_with_default_collateral(author: AuthorOf) -> DispatchResult {
    RoleAdapter::enroll(&author, STANDARD_COLLATERAL, Fortitude::Force)?;
    Ok(())
}

/// Directly funds an author from a direct funder account for fair election weights
pub fn direct_fund_author(funder: AccountId, author: AuthorOf, amount: Asset) -> DispatchResult {
    RoleAdapter::fund(
        &author,
        &Funder::Direct(funder),
        amount,
        Precision::Exact,
        Fortitude::Force,
    )?;
    Ok(())
}

/// Submits affidavits for multiple authors
pub fn submit_affidavit_for_authors(authors: Vec<AffidavitId>) -> DispatchResult {
    for author in authors {
        Pallet::process_affidavit(&author)?;
    }
    Ok(())
}

/// Runs election using the given author as runner and returns elected authors
pub fn run_election_and_elect_authors(author: AuthorOf) -> Result<Vec<AuthorOf>, DispatchError> {
    Internals::prepare_election(&Some(author))?;
    let Some(elected) = Internals::reveal() else {
        return Ok(Vec::default());
    };
    Ok(elected)
}

/// Sets default session and election configuration
/// - Session start at block 1
/// - Affidavits enabled
/// - Affidavit begins at 20%
/// - Affidavit ends at 80%
/// - Election begins at 50%
///
/// ```text
/// Timeline (session-relative):
/// 0%        20%        50%        80%       100%
/// |---------|----------|----------|----------|
///           ^          ^          ^
///           |          |          |
///      Affidavit    Election   Affidavit/
///        start        start    Election end
/// ```
pub fn set_session_config() {
    SessionStartsAt::put(1);
    AllowAffidavits::put(true);
    AffidavitBeginsAt::put(Duration::from_rational(2u32, 10u32));
    ElectionBeginsAt::put(Duration::from_percent(50));
    AffidavitEndsAt::put(Duration::from_percent(80));
}

/// Inserts affidavit keys for authors for a given session
pub fn insert_affidavit_keys_for_authors(
    authors: Vec<AuthorOf>,
    session: SessionIndex,
) -> Vec<(SessionIndex, AuthorOf, AffidavitId)> {
    // derive starting seed from max last byte of given authors
    let mut i = authors
        .iter()
        .map(|acc| acc.as_slice()[31])
        .max()
        .unwrap_or(0)
        .saturating_add(1);

    let mut aff_pair = Vec::new();

    for author in authors {
        let aff_id = account_frm_seed(i);
        AffidavitKeys::insert((session, aff_id.clone()), author.clone());
        i = i.saturating_add(1);
        aff_pair.push((session, author, aff_id));
    }

    aff_pair
}

/// Inserts elected authors into the validator set
pub fn insert_into_validator_set(elected: Vec<AuthorOf>) -> DispatchResult {
    let mut validators = Vec::new();
    for author in elected {
        let session_id = Pallet::convert(author).unwrap();
        validators.push(session_id);
    }
    Validators::put(validators);
    Ok(())
}

/// Test environment holding
/// - externalities,
/// - transaction pool,
/// - offchain state, and
/// - keystore
pub struct OcwTestEnv {
    pub ext: TestExternalities,
    pub pool_state: PoolState,
    pub offchain_state: OffchainState,
    pub keystore: KeyStore,
}

/// Builds offchain test externalities with DB, tx pool, and keystore
/// - Enables storage access, signing, and transaction submission
/// - Does not enables offchain workers internally
pub fn new_offchain_ext() -> (TestExternalities, Arc<MemoryKeystore>) {
    let mut ext = chain_manager_test_ext();

    let (offchain, _state) = TestOffchainExt::new();
    let (pool, _) = TestTransactionPoolExt::new();
    let keystore = Arc::new(MemoryKeystore::new());

    ext.register_extension(OffchainDbExt::new(offchain));
    ext.register_extension(sp_keystore::KeystoreExt(keystore.clone()));
    ext.register_extension(sp_runtime::offchain::TransactionPoolExt::new(pool));
    (ext, keystore)
}

/// Builds full offchain worker test environment
/// - Initializes block to 1 and timestamp to 6000 ms
/// - Enables OCW execution, storage, tx pool, and signing
pub fn new_ocw_env() -> OcwTestEnv {
    let mut ext = chain_manager_test_ext();
    ext.execute_with(|| {
        System::set_block_number(1);
        TimeStamp::set_timestamp(6000);
    });
    // Offchain worker
    let (offchain, offchain_state) = TestOffchainExt::new();

    ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
    ext.register_extension(OffchainDbExt::new(offchain));

    // Transaction pool
    let (pool, pool_state) = TestTransactionPoolExt::new();
    ext.register_extension(TransactionPoolExt::new(pool));

    // Keystore (needed for signing transactions)
    let keystore = Arc::new(MemoryKeystore::new());
    ext.register_extension(KeystoreExt::new(Arc::new(keystore.clone())));

    OcwTestEnv {
        ext,
        pool_state,
        offchain_state,
        keystore,
    }
}

/// Ensures logger is initialized only once
static INIT_LOGGER: Once = Once::new();

/// Initializes logger for test execution
pub fn init_logger() -> Logger {
    INIT_LOGGER.call_once(|| {
        Logger::start();
    });
    Logger
}

/// Generates a new affidavit-id
pub fn generate_affidavit_id() -> AffidavitId {
    let key = <RuntimeAppPublic as sp_runtime::RuntimeAppPublic>::generate_pair(None);
    let generic_pub: GenericPublic = key.into();
    let public: Public = generic_pub.into();
    public.into_account().into()
}

/// Inserts active affidavit key into finalized storage
pub fn insert_active_afdt_key(key: AffidavitId) -> DispatchResult {
    FinalizedInitAfdtKey::insert(ACTIVE_AFDT_KEY, &key, LOG_TARGET_AFDT, None)?;
    Ok(())
}

/// Inserts next affidavit key into finalized storage
pub fn insert_next_afdt_key(key: AffidavitId) -> DispatchResult {
    FinalizedInitAfdtKey::insert(NEXT_AFDT_KEY, &key, LOG_TARGET_AFDT, None)?;
    Ok(())
}

/// Returns finalized active affidavit key if confidence is safe
pub fn get_finalized_afdt_key() -> Option<AffidavitId> {
    match FinalizedInitAfdtKey::get(ACTIVE_AFDT_KEY, LOG_TARGET_AFDT, None).ok()? {
        Some(Confidence::Safe(key)) => Some(key),
        _ => None,
    }
}

/// Returns active affidavit key regardless of confidence
pub fn get_afdt_key() -> Option<AffidavitId> {
    match FinalizedInitAfdtKey::get(ACTIVE_AFDT_KEY, LOG_TARGET_AFDT, None).ok()? {
        Some(Confidence::Safe(key)) => Some(key),
        Some(Confidence::Risky(key)) => Some(key),
        Some(Confidence::Unsafe(key)) => Some(key),
        _ => None,
    }
}

/// Returns finalized next affidavit key if confidence is safe
pub fn get_finalized_next_afdt_key() -> Option<AffidavitId> {
    match FinalizedInitAfdtKey::get(NEXT_AFDT_KEY, LOG_TARGET_AFDT, None).ok()? {
        Some(Confidence::Safe(key)) => Some(key),
        _ => None,
    }
}

/// Returns next affidavit key regardless of confidence
pub fn get_next_afdt_key() -> Option<AffidavitId> {
    match FinalizedInitAfdtKey::get(NEXT_AFDT_KEY, LOG_TARGET_AFDT, None).ok()? {
        Some(Confidence::Safe(key)) => Some(key),
        Some(Confidence::Risky(key)) => Some(key),
        Some(Confidence::Unsafe(key)) => Some(key),
        _ => None,
    }
}

/// Returns public key corresponding to an affidavit id
pub fn get_public_key(afdt_key: AffidavitId) -> Option<Public> {
    let all_keys = <RuntimeAppPublic as sp_runtime::RuntimeAppPublic>::all();
    for key in all_keys.into_iter() {
        let generic_pub: GenericPublic = key.into();
        let public: Public = generic_pub.into();
        let account: AffidavitId = public.clone().into_account().into();

        if account == afdt_key {
            return Some(public);
        }
    }
    None
}

/// Returns total number of available affidavit keys
pub fn affidavit_key_count() -> usize {
    <RuntimeAppPublic as sp_runtime::RuntimeAppPublic>::all().len()
}

/// Runs offchain worker for each block up to the given block number
/// - Updates timestamp by 6000 ms per block
pub fn ocw_run_to_block(n: u64) {
    let b = System::block_number();
    let current_ts = b.saturating_mul(6000);
    TimeStamp::set_timestamp(current_ts);
    while System::block_number() < n {
        let b = System::block_number();
        Pallet::offchain_worker(b);
        // move to next block
        System::set_block_number(b + 1);
        let ts = TimeStamp::get();
        // update timestamp
        let new_ts = ts.saturating_add(6000);
        TimeStamp::set_timestamp(new_ts);
    }
}

/// Advances to block `n` maintaining the fork graph without running OCW routines.
/// - Queries finalized storage for affidavit keys to improve confidence
/// - Updates timestamp by 6000 ms per block
/// - Requires `init_fork_graph()` first.
pub fn run_to_block(n: BlockNumber) {
    let b = System::block_number();
    let current_ts = b.saturating_mul(6000);
    TimeStamp::set_timestamp(current_ts);
    while System::block_number() < n {
        let b = System::block_number();
        register_block_hash(b);
        <Pallet as ForksHandler<Test, ForkLocalDepot>>::start(  None, None, || {});
        let _ = FinalizedInitAfdtKey::get(ACTIVE_AFDT_KEY, LOG_TARGET_AFDT, None);
        let _ = FinalizedInitAfdtKey::get(NEXT_AFDT_KEY, LOG_TARGET_AFDT, None);
        // move to next block
        System::set_block_number(b + 1);
        let ts = TimeStamp::get();
        // update timestamp
        let new_ts = ts.saturating_add(6000);
        TimeStamp::set_timestamp(new_ts);
    }
    register_block_hash(n);
    <Pallet as ForksHandler<Test, ForkLocalDepot>>::start(None, None, || {});
    let _ = FinalizedInitAfdtKey::get(ACTIVE_AFDT_KEY, LOG_TARGET_AFDT, None);
    let _ = FinalizedInitAfdtKey::get(NEXT_AFDT_KEY, LOG_TARGET_AFDT, None);
}

/// Like [`run_to_block`] but also accumulates observations for a custom `key`.
/// Requires `init_fork_graph()` first.
pub fn run_to_block_with_finalized_key(n: BlockNumber, key: &[u8]) {
    let b = System::block_number();
    let current_ts = b.saturating_mul(6000);
    TimeStamp::set_timestamp(current_ts);
    while System::block_number() < n {
        let b = System::block_number();
        register_block_hash(b);
        <Pallet as ForksHandler<Test, ForkLocalDepot>>::start(None, None, || {});
        let _ = FinalizedInitAfdtKey::get(key, None, None);
        let _ = FinalizedInitAfdtKey::get(ACTIVE_AFDT_KEY, LOG_TARGET_AFDT, None);
        let _ = FinalizedInitAfdtKey::get(NEXT_AFDT_KEY, LOG_TARGET_AFDT, None);
        // move to next block
        System::set_block_number(b + 1);
        let ts = TimeStamp::get();
        // update timestamp
        let new_ts = ts.saturating_add(6000);
        TimeStamp::set_timestamp(new_ts);
    }
    register_block_hash(n);
    <Pallet as ForksHandler<Test, ForkLocalDepot>>::start(None, None, || {});
    let _ = FinalizedInitAfdtKey::get(key, None, None);
    let _ = FinalizedInitAfdtKey::get(ACTIVE_AFDT_KEY, LOG_TARGET_AFDT, None);
    let _ = FinalizedInitAfdtKey::get(NEXT_AFDT_KEY, LOG_TARGET_AFDT, None);
}

/// Sets current session
pub fn set_session(session: u32) {
    CurrentSession::put(session);
}

/// Registers affidavit key for an author in a session
pub fn register_affidavit_key(session: SessionIndex, afdt: AffidavitId, author: AuthorOf) {
    AffidavitKeys::insert((session, afdt), author);
}

/// Generates a new affidavit keypair and returns the public key
pub fn generate_affidavit_keypair() -> Public {
    let key = <RuntimeAppPublic as sp_runtime::RuntimeAppPublic>::generate_pair(None);
    let generic_pub: GenericPublic = key.into();
    generic_pub.into()
}

/// Runs offchain worker for current block and advances to next block
/// - Updates timestamp by 6000 ms
pub fn ocw_step() {
    let b = System::block_number();
    Pallet::offchain_worker(b);
    System::set_block_number(b + 1);
    let ts = TimeStamp::get();
    TimeStamp::set_timestamp(ts.saturating_add(6000));
}

/// Fast-tracks extrinsic `validate` for the author
/// - Registers affidavit key for upcoming session
pub fn ext_validate(author: AuthorOf, afdt_pub: AffidavitId) -> DispatchResult {
    RoleAdapter::role_exists(&author)?;
    RoleAdapter::is_available(&author)?;
    let for_session = CurrentSession::get() + 1;
    AffidavitKeys::insert((for_session, afdt_pub), author);
    Ok(())
}

/// Affidavit payload containing active and next public keys for rotation
pub struct TestAfdtPayload {
    pub active_afdt_pub: AffidavitId,
    pub next_afdt_pub: AffidavitId,
}

/// Fast-tracks declare affidavit extrinsic for the author
/// - Processes active affidavit and registers next key for rotation
pub fn ext_declare_affidavit(author: AuthorOf, payload: TestAfdtPayload) -> DispatchResult {
    let active_afdt = payload.active_afdt_pub;
    let rotate = payload.next_afdt_pub;
    let for_session = CurrentSession::get() + 1;

    let afdt_author =
        AffidavitKeys::get((for_session, &active_afdt)).ok_or(Error::AffidavitAuthorNotFound)?;
    ensure!(afdt_author == author, Error::AuthorNotAffidavitOwner);
    Pallet::process_affidavit(&active_afdt.clone())?;
    AffidavitKeys::insert((for_session + 1, rotate), author);
    Ok(())
}

/// Fast-tracks elect authors extrinsic for the author
/// - Runs election and returns elected authors
pub fn ext_elect_authors(
    author: AuthorOf,
    afdt_pub: AffidavitId,
) -> Result<Vec<AuthorOf>, DispatchError> {
    let for_session = CurrentSession::get() + 1;
    let afdt_author = AffidavitKeys::get((for_session + 1, &afdt_pub.clone()))
        .ok_or(Error::AffidavitAuthorNotFound)?;
    ensure!(afdt_author == author, Error::AuthorNotAffidavitOwner);
    Internals::prepare_election(&Some(author.clone()))?;
    let current_block = System::block_number();
    ElectsPreparedBy::insert(for_session, (author, current_block));

    let elected = Internals::reveal().unwrap();
    Ok(elected.into_iter().collect())
}

/// Signs payload using affidavit key
pub fn sign_payload(payload: &[u8], public: Public) -> Signature {
    AffidavitCrypto::sign(payload, public)
        .expect("test keystore should contain affidavit signing key")
}

/// Computes the affidavit submission period in blocks of the current session
pub fn compute_affidavit_window() -> Result<AffidavitWindow, DispatchError> {
    crate::Pallet::compute_affidavit_window()
}

/// Computes the election period in blocks of the current session
pub fn compute_election_window() -> Result<ElectionWindow, DispatchError> {
    crate::Pallet::compute_election_window()
}

/// Returns a deterministic non-zero hash for block `n`.
pub fn mock_block_hash(n: BlockNumber) -> <Test as frame_system::Config>::Hash {
    BlakeTwo256::hash(&n.to_le_bytes())
}

/// Registers a deterministic block hash for `n` in `frame_system::BlockHash`.
pub fn register_block_hash(n: BlockNumber) {
    frame_system::BlockHash::<Test>::insert(n, mock_block_hash(n));
}

/// Seeds the fork graph for the fork-aware storage operations.
///
/// Registers hashes for blocks 0, 1, and 2, sets the current block to 2,
/// and calls `ForksHandler::start` with an empty closure. This creates
/// the first branch in the fork graph for block 1 without running any
/// OCW routines (no key generation, no transaction submission).
pub fn init_fork_graph() {
    for n in 0u64..=2 {
        register_block_hash(n);
    }
    System::set_block_number(2);
    TimeStamp::set_timestamp(12_000);
    <Pallet as ForksHandler<Test, ForkLocalDepot>>::start(None, None, || {});
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

    #[runtime::pallet_index(4)]
    pub type Session = pallet_session::Pallet<Test>;

    #[runtime::pallet_index(5)]
    pub type Historical = pallet_session::historical::Pallet<Test>;

    #[runtime::pallet_index(6)]
    pub type ImOnline = pallet_im_online::Pallet<Test>;

    #[runtime::pallet_index(7)]
    pub type Offences = pallet_offences::Pallet<Test>;

    #[runtime::pallet_index(8)]
    pub type Authorship = pallet_authorship::Pallet<Test>;

    #[runtime::pallet_index(9)]
    pub type ChainManager = pallet_chain_manager::Pallet<Test>;

    #[runtime::pallet_index(10)]
    pub type TimeStamp = pallet_timestamp::Pallet<Test>;
}

// ===============================================================================
// ``````````````````````````````````` CONFIGS ```````````````````````````````````
// ===============================================================================

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// ``````````````````````````````````` SYSTEM ````````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = AccountId;
    type Lookup = sp_runtime::traits::IdentityLookup<Self::AccountId>;
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````` COMMITMENT `````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_commitment::Config for Test {
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

plugin_context!(
    name: pub MyBalanceContext,
    context: ShareBalanceContext<T>,
    marker: [T,],
    value: ShareBalanceContext(PhantomData)
);

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````````` XP `````````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_xp::Config for Test {
    type Xp = u64;
    type Pulse = u32;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type LockReason = RuntimeFreezeReason;
    type ReserveReason = RuntimeHoldReason;
    type Extensions = Ignore<Xp>;
    type EmitEvents = ConstBool<true>;
    type WeightInfo = ();
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// ``````````````````````````````````` AUTHORS ```````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_authors::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type CommitmentAdapter = Commitment;
    type AssetFreeze = RuntimeFreezeReason;
    type Asset = Xp;
    type Influence = u64;
    type InfluenceContext = ();
    type InfluenceModel = LinearModel;
    type FlatElectionContext = ();
    type FlatElectionModel = flat::TopDownFlatModel;
    type FairElectionContext = ();
    type FairElectionModel = fair::TopDownFairModel;
    type ActivityProvider = ChainManager;
    type EmitEvents = ConstBool<true>;
    type WeightInfo = ();
}

pub type FlatElection<T> = pallet_authors::FlatElection<T>;

pub type FairElection<T> = pallet_authors::FairElection<T>;

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// ``````````````````````````````````` SESSION ```````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_session::Config for Test {
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

impl pallet_session::historical::Config for Test {
    type FullIdentification = AccountId;
    type FullIdentificationOf = ChainManager;
}

parameter_types! {
    pub const Period: u64 = 1 * HOURS;
    pub const Offset: u64 = 0;
}

impl_opaque_keys! {
    pub struct SessionKeys {
        pub im_online: pallet_im_online::sr25519::AuthorityId,
    }
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// ```````````````````````````````` CHAIN-MANAGER ````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_chain_manager::Config for Test {
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type RewardContext = ();
    type RewardModel = SharesPay;
    type InflationContext = MyConstantPayoutContext;
    type InflationModel = ConstantPayout;
    type RoleAdapter = Authors;
    type Asset = Xp;
    type WeightInfo = ();
    type InflateViaSupply = ConstBool<false>;
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

plugin_context!(
    name: pub MyConstantPayoutContext,
    context: ConstantPayoutConfig<u64>,
    value: ConstantPayoutConfig::<u64> {
        payout: 100u64
    }
);

plugin_context!(
    name: pub MyPenaltyThresholdContext,
    context: ThresholdPenaltyConfig<Perbill>,
    value: ThresholdPenaltyConfig::<Perbill> {
        threshold: Perbill::from_percent(70)
    }
);

// Offchain Workers Signing
impl SigningTypes for Test {
    type Public = MultiSigner;
    type Signature = sp_runtime::MultiSignature;
}

impl CreateTransactionBase<RuntimeCall> for Test {
    type Extrinsic = UncheckedExtrinsic;
    type RuntimeCall = RuntimeCall;
}

impl CreateSignedTransaction<RuntimeCall> for Test {
    fn create_signed_transaction<
        C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>,
    >(
        call: RuntimeCall,
        _public: <sp_runtime::MultiSignature as sp_runtime::traits::Verify>::Signer,
        account: <Test as frame_system::Config>::AccountId,
        _nonce: <Test as frame_system::Config>::Nonce,
    ) -> Option<UncheckedExtrinsic> {
        Some(UncheckedExtrinsic::new_signed(
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

impl pallet_im_online::Config for Test {
    type AuthorityId = pallet_im_online::sr25519::AuthorityId;
    type RuntimeEvent = RuntimeEvent;
    type NextSessionRotation = PeriodicSessions<Period, Offset>;
    type ValidatorSet = pallet_session::historical::Pallet<Test>;
    type ReportUnresponsiveness = Offences;
    type UnsignedPriority = ImOnlineUnsignedPriority;
    type WeightInfo = ();
    type MaxKeys = MaxKeys;
    type MaxPeerInHeartbeats = MaxPeerInHeartbeats;
}

parameter_types! {
    pub const ImOnlineUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
    pub const MaxKeys: u32 = 10_000;
    pub const MaxPeerInHeartbeats: u32 = 10_000;
}

impl CreateTransactionBase<pallet_im_online::Call<Test>> for Test {
    type Extrinsic = UncheckedExtrinsic;
    type RuntimeCall = RuntimeCall;
}

impl CreateInherent<pallet_im_online::Call<Test>> for Test {
    fn create_inherent(call: RuntimeCall) -> UncheckedExtrinsic {
        UncheckedExtrinsic::new_bare(call)
    }
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````` OFFENCES ```````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_offences::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type IdentificationTuple = pallet_session::historical::IdentificationTuple<Test>;
    type OnOffenceHandler = ChainManager;
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````` AUTHORSHIP `````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

impl pallet_authorship::Config for Test {
    type FindAuthor = MockFindAuthor;
    type EventHandler = ImOnline;
}

thread_local! {
    static CURRENT_AUTHOR: RefCell<Option<AccountId>> = RefCell::new(None);
}

pub struct MockFindAuthor;

impl FindAuthor<AccountId> for MockFindAuthor {
    fn find_author<'a, I>(_: I) -> Option<AccountId>
    where
        I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
    {
        CURRENT_AUTHOR.with(|a| a.borrow().clone())
    }
}

// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
// `````````````````````````````````` TIMESTAMP ``````````````````````````````````
// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

pub type Moment = u64;

impl pallet_timestamp::Config for Test {
    type Moment = Moment;
    type OnTimestampSet = MockOnTimestampSet;
    type MinimumPeriod = ConstU64<5>;
    type WeightInfo = ();
}

parameter_types! {
    pub static CapturedMoment: Option<Moment> = None;
}

pub struct MockOnTimestampSet;
impl OnTimestampSet<Moment> for MockOnTimestampSet {
    fn on_timestamp_set(moment: Moment) {
        CapturedMoment::mutate(|x| *x = Some(moment));
    }
}