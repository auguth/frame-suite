// This is free and unencumbered software released into the public domain.
//
// Anyone is free to copy, modify, publish, use, compile, sell, or
// distribute this software, either in source code form or as a compiled
// binary, for any purpose, commercial or non-commercial, and by any
// means.
//
// In jurisdictions that recognize copyright laws, the author or authors
// of this software dedicate any and all copyright interest in the
// software to the public domain. We make this dedication for the benefit
// of the public at large and to the detriment of our heirs and
// successors. We intend this dedication to be an overt act of
// relinquishment in perpetuity of all present and future rights to this
// software under copyright law.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
// OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
// OTHER DEALINGS IN THE SOFTWARE.
//
// For more information, please refer to <http://unlicense.org>

use codec::Encode;
use core::marker::PhantomData;

use frame_support::{
    derive_impl,
    pallet_prelude::TransactionPriority,
    parameter_types,
    traits::{ConstBool, ConstU128, ConstU32, ConstU64, ConstU8, VariantCountOf},
    weights::{
        constants::{RocksDbWeight, WEIGHT_REF_TIME_PER_SECOND},
        IdentityFee, Weight,
    },
};
use frame_system::{
    limits::{BlockLength, BlockWeights},
};

use frame_suite::{plugin_context, Disposition, Ignore};

use frame_plugins::{
    penalty::{ThresholdPenalty, ThresholdPenaltyConfig},
    elections::{TopDownFairModel, TopDownFlatModel},
    influence::LinearModel,
    rewards::{
        payee::SharesPay,
        payout::{ConstantPayout, ConstantPayoutConfig},
    },
    balances::{ShareBalanceFamily, ShareBalanceContext},
};

use sp_runtime::{
    generic::Era,
    traits::{One, OpaqueKeys, Verify},
    Perbill, FixedU64, MultiAddress
};
use sp_version::RuntimeVersion;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;

use pallet_transaction_payment::{
    ChargeTransactionPayment, ConstFeeMultiplier, FungibleAdapter, Multiplier,
};
use pallet_session::PeriodicSessions;
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use pallet_chain_manager::crypto::sr25519::AffidavitCryptoSr25519;

// Local module imports
use super::{
	AccountId, Aura, Balance, Balances, Block, BlockNumber, Hash, Nonce, PalletInfo, Runtime,
	RuntimeCall, RuntimeEvent, RuntimeFreezeReason, RuntimeHoldReason, RuntimeOrigin, RuntimeTask,
	System, EXISTENTIAL_DEPOSIT, SLOT_DURATION, VERSION, HOURS, Xp, Commitment, Authors, ChainManager, ImOnline,
	Offences, SessionKeys, UncheckedExtrinsic, Signature,
};

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

parameter_types! {
	pub const BlockHashCount: BlockNumber = 2400;
	pub const Version: RuntimeVersion = VERSION;

	/// We allow for 2 seconds of compute with a 6 second average block time.
	pub RuntimeBlockWeights: BlockWeights = BlockWeights::with_sensible_defaults(
		Weight::from_parts(2u64 * WEIGHT_REF_TIME_PER_SECOND, u64::MAX),
		NORMAL_DISPATCH_RATIO,
	);
	pub RuntimeBlockLength: BlockLength = BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub const SS58Prefix: u8 = 42;
}

/// The default types are being injected by [`derive_impl`](`frame_support::derive_impl`) from
/// [`SoloChainDefaultConfig`](`struct@frame_system::config_preludes::SolochainDefaultConfig`),
/// but overridden as needed.
#[derive_impl(frame_system::config_preludes::SolochainDefaultConfig)]
impl frame_system::Config for Runtime {
	/// The block type for the runtime.
	type Block = Block;
	/// Block & extrinsics weights: base values and limits.
	type BlockWeights = RuntimeBlockWeights;
	/// The maximum length of a block (in bytes).
	type BlockLength = RuntimeBlockLength;
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The type for storing how many extrinsics an account has signed.
	type Nonce = Nonce;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	/// The weight of database operations that the runtime can invoke.
	type DbWeight = RocksDbWeight;
	/// Version of the runtime.
	type Version = Version;
	/// The data to be stored in an account.
	type AccountData = pallet_balances::AccountData<Balance>;
	/// This is used as an identifier of the chain. 42 is the generic substrate prefix.
	type SS58Prefix = SS58Prefix;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type DisabledValidators = ();
	type MaxAuthorities = ConstU32<32>;
	type AllowMultipleBlocksPerSlot = ConstBool<false>;
	type SlotDuration = pallet_aura::MinimumPeriodTimesTwo<Runtime>;
}

impl pallet_grandpa::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;

	type WeightInfo = ();
	type MaxAuthorities = ConstU32<32>;
	type MaxNominators = ConstU32<0>;
	type MaxSetIdSessionEntries = ConstU64<0>;

	type KeyOwnerProof = sp_core::Void;
	type EquivocationReportSystem = ();
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = Aura;
	type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
	type WeightInfo = ();
}

impl pallet_balances::Config for Runtime {
	type MaxLocks = ConstU32<50>;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<EXISTENTIAL_DEPOSIT>;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
	type FreezeIdentifier = RuntimeFreezeReason;
	type MaxFreezes = VariantCountOf<RuntimeFreezeReason>;
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type DoneSlashHandler = ();
}

parameter_types! {
	pub FeeMultiplier: Multiplier = Multiplier::one();
}

impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = FungibleAdapter<Balances, ()>;
	type OperationalFeeMultiplier = ConstU8<5>;
	type WeightToFee = IdentityFee<Balance>;
	type LengthToFee = IdentityFee<Balance>;
	type FeeMultiplierUpdate = ConstFeeMultiplier<FeeMultiplier>;
	type WeightInfo = pallet_transaction_payment::weights::SubstrateWeight<Runtime>;
}

impl pallet_sudo::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type WeightInfo = pallet_sudo::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const Period: u32 = HOURS;
    pub const Offset: u32 = 0;
}

impl pallet_session::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = AccountId;
    type ValidatorIdOf = ChainManager;
    type ShouldEndSession = PeriodicSessions<Period, Offset>;
    type NextSessionRotation = PeriodicSessions<Period, Offset>;
    type SessionManager = ChainManager;
    type SessionHandler = <SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
    type Keys = SessionKeys;
    type WeightInfo = pallet_session::weights::SubstrateWeight<Runtime>;
    type DisablingStrategy = ();
}

impl pallet_session::historical::Config for Runtime {
    type FullIdentification = AccountId;
    type FullIdentificationOf = ChainManager;
}

parameter_types! {
    pub const ImOnlineUnsignedPriority: TransactionPriority = TransactionPriority::MAX;
    pub const MaxKeys: u32 = 10_000;
    pub const MaxPeerInHeartbeats: u32 = 10_000;
}

impl pallet_im_online::Config for Runtime {
    type AuthorityId = ImOnlineId;
    type RuntimeEvent = RuntimeEvent;
    type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
    type ValidatorSet = pallet_session::historical::Pallet<Runtime>;
    type ReportUnresponsiveness = Offences;
    type UnsignedPriority = ImOnlineUnsignedPriority;
    type WeightInfo = pallet_im_online::weights::SubstrateWeight<Runtime>;
    type MaxKeys = MaxKeys;
    type MaxPeerInHeartbeats = MaxPeerInHeartbeats;
}

impl pallet_offences::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type IdentificationTuple = pallet_session::historical::IdentificationTuple<Runtime>;
    type OnOffenceHandler = ChainManager;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct BenchmarkAuthorFinder;

#[cfg(feature = "runtime-benchmarks")]
impl FindAuthor<AccountId> for BenchmarkAuthorFinder {
    fn find_author<'a, I>(_: I) -> Option<AccountId> 
    where
    I: 'a + IntoIterator<Item = (sp_runtime::ConsensusEngineId, &'a [u8])>,
    {
        Some(frame_benchmarking::account("alice_id", 0, 1))
    }
}

impl pallet_authorship::Config for Runtime {
    #[cfg(feature = "runtime-benchmarks")]
    type FindAuthor = BenchmarkAuthorFinder;

    #[cfg(not(feature = "runtime-benchmarks"))]
    type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
    type EventHandler = ImOnline;
}

impl pallet_xp::Config for Runtime {
    type Xp = Balance;
    type Pulse = BlockNumber;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type ReserveReason = RuntimeHoldReason;
    type LockReason = RuntimeFreezeReason;
    type Extensions = Ignore<Xp>;
    type EmitEvents = ConstBool<false>;
    type WeightInfo = pallet_xp::weights::SubstrateWeight<Runtime>;
}

impl pallet_commitment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Shares = u64;
    type Asset = pallet_xp::Pallet<Self>;
    type Position = Disposition;
    type MaxIndexEntries = ConstU32<200>;
    type Bias = FixedU64;
    type MaxCommits = ConstU32<30>;
    type AssetHold = RuntimeHoldReason;
    type AssetFreeze = RuntimeFreezeReason;
    type Commission = Perbill;
    type Time = u32;
    type BalanceFamily<'a> = ShareBalanceFamily<'a>;
    type BalanceContext = MyBalanceContext<Commitment>;
    type EmitEvents = ConstBool<true>;
    type WeightInfo = pallet_commitment::weights::SubstrateWeight<Runtime>;
}

plugin_context!(
    name: pub MyBalanceContext,
    context: ShareBalanceContext<T>,
    marker: [T,],
    value: ShareBalanceContext(PhantomData)
);
macro_rules! const_assert_size_eq {
    ($a:ty, $b:ty) => {
        const _: () = {
            let _ = [0u8; core::mem::size_of::<$a>()];
            let _ = [0u8; core::mem::size_of::<$b>()];
            let _ = [0u8; core::mem::size_of::<$a>() - core::mem::size_of::<$b>()];
        };
    };
}

// Usage (inside runtime):
const_assert_size_eq!(AccountId, Hash);

impl pallet_authors::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type CommitmentAdapter = pallet_commitment::Pallet<Self>;
    type AssetFreeze = RuntimeFreezeReason;
    type Asset = pallet_xp::Pallet<Self>;
    type Influence = Balance;
    type InfluenceContext = ();
    type InfluenceModel = LinearModel;
    type FlatElectionContext = ();
    type FlatElectionModel = TopDownFlatModel;
    type FairElectionContext = ();
    type FairElectionModel = TopDownFairModel;
	type WeightInfo = pallet_authors::weights::SubstrateWeight<Runtime>;
    type ActivityProvider = ChainManager;
    type EmitEvents = ConstBool<true>;
}


plugin_context!(
    name: pub MyConstantPayoutContext,
    context: ConstantPayoutConfig<Balance>,
    value: ConstantPayoutConfig::<Balance> {
        payout: Balance::from(100u32)
    }
);

plugin_context!(
    name: pub MyPenaltyThresholdContext,
    context: ThresholdPenaltyConfig<Perbill>,
    value: ThresholdPenaltyConfig::<Perbill> {
        threshold: Perbill::from_percent(70)
    }
);

type _FlatElection<T> = pallet_authors::FlatElection<T>;
type FairElection<T> = pallet_authors::FairElection<T>;

impl pallet_chain_manager::Config for Runtime {
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type RewardContext = ();
    type RewardModel = SharesPay;
    type InflationContext = MyConstantPayoutContext;
    type InflationModel = ConstantPayout;
    type RoleAdapter = Authors;
    type Asset = pallet_xp::Pallet<Self>;
    type InflateViaSupply = ConstBool<false>;
    type WeightInfo = ();
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

impl frame_system::offchain::SigningTypes for Runtime {
    type Public = <Signature as Verify>::Signer;
    type Signature = Signature;
}

impl<LocalCall> frame_system::offchain::CreateTransactionBase<LocalCall> for Runtime
where
    RuntimeCall: From<LocalCall>,
{
    type Extrinsic = UncheckedExtrinsic;
    type RuntimeCall = RuntimeCall;
}

impl frame_system::offchain::CreateInherent<pallet_im_online::Call<Runtime>> for Runtime {
    fn create_inherent(call: RuntimeCall) -> UncheckedExtrinsic {
        UncheckedExtrinsic::new_bare(call)
    }
}

impl frame_system::offchain::CreateSignedTransaction<RuntimeCall> for Runtime {
    fn create_signed_transaction<
        C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>,
    >(
        call: RuntimeCall,
        public: <Signature as Verify>::Signer,
        account: <Runtime as frame_system::Config>::AccountId,
        nonce: <Runtime as frame_system::Config>::Nonce,
    ) -> Option<UncheckedExtrinsic> {
        let tip: u128 = 0;

        let extra = (
            frame_system::CheckNonZeroSender::<Runtime>::new(),
            frame_system::CheckSpecVersion::<Runtime>::new(),
            frame_system::CheckTxVersion::<Runtime>::new(),
            frame_system::CheckGenesis::<Runtime>::new(),
            frame_system::CheckEra::<Runtime>::from(Era::mortal(128, 0)),
            frame_system::CheckNonce::<Runtime>::from(nonce),
            frame_system::CheckWeight::<Runtime>::new(),
            ChargeTransactionPayment::<Runtime>::from(tip),
            frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(false),
            frame_system::WeightReclaim::<Runtime>::new(),
        );

        let raw_payload =
            sp_runtime::generic::SignedPayload::<RuntimeCall, _>::new(call, extra).ok()?;
        let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
        let (call, extra, _) = raw_payload.deconstruct();

        Some(UncheckedExtrinsic::new_signed(
            call,
            MultiAddress::Id(account),
            signature,
            extra,
        ))
    }
}