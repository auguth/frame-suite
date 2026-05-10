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
// ```````````````````````````````` AUTHORS TYPES ````````````````````````````````
// ===============================================================================

//! **Core types and aliases for the Authors system.**
//!
//! This module defines the primary structures and type aliases used by
//! [`pallet_authors`](crate). These types are publicly exposed and used across
//! the pallet's APIs for representing Author-related data.
//!
//! Trait implementations provided by this crate's [`Pallet`] can use these types
//! via trait-bound equality constraints to ensure type alignment with this pallet's
//! concrete implementations if necessary.
//!
//! ## Example
//!
//! ```ignore
//! mod pallet {
//!     use pallet_authors::types::AuthorInfo;
//!
//!     pub trait Config: frame_system::Config {
//!         type RoleAdapter: RoleManager<Meta = AuthorInfo<Self>>;
//!     }
//! }
//! ```

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{Config, Pallet};

// --- Scale-codec crates ---
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

// --- Core / Std ---
use core::{
    cmp::Ordering,
    fmt::{Debug, Formatter},
};

// --- FRAME Suite ---
use frame_suite::{commitment::*, roles::*};

// --- FRAME Support ---
use frame_support::{
    traits::{
        tokens::{Fortitude, Precision},
        VariantCount,
    },
    RuntimeDebugNoBound,
};

// --- FRAME System ---
use frame_system::pallet_prelude::BlockNumberFor;

// --- Substrate primitives ---
use sp_core::RuntimeDebug;
use sp_runtime::Vec;

// ===============================================================================
// ``````````````````````````````````` ALIASES ```````````````````````````````````
// ===============================================================================

/// Represents an on-chain account that can hold an `Author` role.
pub type Author<T> = <T as frame_system::pallet::Config>::AccountId;

/// Represents the **asset or balance type** associated with an `Author`
/// in the context of funding, staking, or collateral.
///
/// Typically used for collateral management, reward calculation, and fund tracking.
pub type AuthorAsset<T> = <<T as Config>::CommitmentAdapter as InspectAsset<Author<T>>>::Asset;

/// Commission Type used by the Pallet for Pool Commissions.
pub type Commission<T> = <<T as Config>::CommitmentAdapter as CommitPool<Author<T>>>::Commission;

/// Penalty ratio type used by the Author Roles subsystem.
pub type Ratio<T> = <Pallet<T> as CompensateRoles<Author<T>>>::Ratio;

/// Shares Type used by the Pallet for Index/Pool shares.
pub type Shares<T> = <<T as Config>::CommitmentAdapter as CommitIndex<Author<T>>>::Shares;

/// Represents a cryptographic digest, a unique identifier for an `Author`.
///
/// Useful for referencing authors via commitments (funding) instead of
/// its direct enrollment account IDs.
pub type AuthorDigest<T> = <<T as Config>::CommitmentAdapter as Commitment<Author<T>>>::Digest;

/// A list of authors selected by an election round.
///
/// Represents the final election output returned by the configured
/// election model.
pub type ElectedAuthors<T> = Vec<Author<T>>;

/// Election input for **influence-based selection**.
///
/// Each entry pairs an [`Author`] with the influence values considered
/// for that author during election.
///
/// Used by **flat election models** that operate on influence rather than
/// individual funding relationships.
pub type ElectViaInfluence<T> = Vec<(Author<T>, Vec<<T as Config>::Influence>)>;

/// A single backing contribution used in **backing-based election**.
///
/// Pairs a [`Funder`] with the asset value attributed to that funder
/// for election weighting.
pub type BackingElectionWeight<T> = (Funder<T>, AuthorAsset<T>);

/// Election input for **backing-based selection**.
///
/// Each entry pairs an [`Author`] with the backing contributions
/// considered for that author during election.
///
/// Used by **fair election models** that preserve individual funder
/// contributions instead of aggregating them into a single value.
pub type ElectViaBacking<T> = Vec<(Author<T>, Vec<BackingElectionWeight<T>>)>;

/// Represents a **digest identifier** for an indexed commitment (unmanaged pools)
/// backing an [`Author`] role.
pub type IndexDigest<T> = <<T as Config>::CommitmentAdapter as Commitment<Author<T>>>::Digest;

/// Represents a **digest identifier** for a pool commitment (managed collective commitment)
/// funding source for an [`Author`] role.
pub type PoolDigest<T> = <<T as Config>::CommitmentAdapter as Commitment<Author<T>>>::Digest;

/// Represents the account that provides backing to an [`Author`],
/// either directly or through an index or pool funding mechanism.
pub type Backer<T> = <T as frame_system::Config>::AccountId;

// ===============================================================================
// ````````````````````````````````` STRUCTURES ``````````````````````````````````
// ===============================================================================

/// Represents the **lifecycle state** of an `Author` role within the runtime.
///
/// This enum defines discrete states that an author (e.g., a block producer)
/// can transition through during its lifecycle.
///
/// ## Decentralization Philosophy
///
/// The `Author` role deliberately **does not include a suspension state**.
/// Instead of centralized or privileged suspension control, accountability
/// is enforced through penalties only.
///
/// This ensures that the author ecosystem remains **decentralized and self-governing**,  
/// avoiding unilateral suspension or authority over active participants.
#[derive(
    Encode,
    Decode,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    MaxEncodedLen,
    DecodeWithMemTracking,
    TypeInfo,
)]
pub enum AuthorStatus {
    /// The author is active and in good standing.
    Active,

    /// The author is temporarily under performance review or penalty watch.
    ///
    /// The author cannot resign while in this status to ensure accountability
    /// until the probation period is resolved.
    Probation,

    /// The author has left the role, either voluntarily or by fulfilling
    /// all obligations necessary for resignation.
    Resigned,
}

/// Represents the **entity responsible for backing or funding** an [`Author`] role.
///
/// A `Funder` abstracts away different possible funding sources - individual accounts,
/// indexed commitments, or pooled backing mechanisms - allowing the runtime to treat
/// all funders uniformly.
///
/// ## Design Rationale
///
/// The `Funder` type enables **composable funding logic** by decoupling
/// the origin of funds from their operational treatment.  
/// Whether funds come directly from an account, a collective pool, or an
/// indexing mechanism, they can all be processed consistently by the same logic.
///
/// ## Transparency
///
/// - Funding is open to multiple origins, encouraging *decentralized capital flow*.  
/// - Digest-based identifiers ensure verifiable commitment integrity without
///   requiring central registries.
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, MaxEncodedLen, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub enum Funder<T: Config> {
    /// A direct account-based funder (standard single backer).
    Direct(Backer<T>),

    /// An indexed commitment-based funder.
    /// Contains the index's digest and the account through which it is backed.
    Index {
        /// Index Digest
        digest: IndexDigest<T>,
        /// Index's Backer/Funder
        backer: Backer<T>,
    },

    /// A pool funder, representing collective or group-backed collateral.
    /// Contains the pool's digest and the account through which it is backed.
    Pool {
        /// Pool Digest
        digest: PoolDigest<T>,
        /// Pool's Backer/Funder
        backer: Backer<T>,
    },
}

/// Represents the **on-chain record of an [`Author`] role** within the runtime.
///
/// This struct captures all essential metadata for managing an author's lifecycle,
/// funding, and status.
///
/// ## Design Rationale
///
/// - **Lifecycle tracking:** `since`, `stale_since`, and `frozen_until` allow the
///   runtime to enforce penalties,
///   rewards, or inactivity handling without central authority.
/// - **Decentralization:** No suspension logic is provided; accountability is
///   enforced through penalties and voluntary resignation.
/// - **Composability:** `funders` is a bounded collection of [`Funder`], allowing
///   flexible funding models (direct, index, or pool).
/// - **Auditability:** All timestamps and digests provide verifiable on-chain evidence
///   of the author's role state.
#[derive(Encode, Decode, PartialEq, Eq, Clone, MaxEncodedLen, TypeInfo, DecodeWithMemTracking)]
#[scale_info(skip_type_params(T))]
pub struct AuthorInfo<T: Config> {
    /// Unique commitment digest/hash or derived identifier for the author
    /// (not the author's accountId).
    pub digest: AuthorDigest<T>,

    /// Timestamp when the author was enrolled.
    pub since: BlockNumberFor<T>,

    /// Current status of the author.
    pub status: AuthorStatus,

    /// Timestamp when the author status was updated.
    pub status_since: BlockNumberFor<T>,

    /// Timestamp until which the author is at risk to the system.
    ///
    /// This indicates, that till this time the author cannot
    /// do few runtime actions.
    ///
    /// If in the past, the author is considered safe.
    pub risk_until: BlockNumberFor<T>,

    /// Locally defined minimum fund to support this author.
    pub min_fund: Option<AuthorAsset<T>>,

    /// Locally defined maximum fund exposure for this author.
    pub max_fund: Option<AuthorAsset<T>>,
}

/// Specifies the destination or mechanism by which funds are allocated
/// within the authors / roles system.
///
/// A `FundingTarget` determines *how* a contribution is routed:
/// - directly to a single author,
/// - indirectly via an index abstraction,
/// - or pooled into a shared funding vehicle.
///
/// This enum is used by funding and compensation logic to resolve
/// the final recipients and exposure semantics of a funding action.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    RuntimeDebugNoBound,
    Clone,
    PartialEq,
    Eq,
    MaxEncodedLen,
    TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub enum FundingTarget<T: Config> {
    /// Funds are allocated directly to a specific author.
    Direct(Author<T>),

    /// Funds are allocated via an index abstraction.
    Index(IndexDigest<T>),

    /// Funds are allocated into a shared funding pool.
    Pool(PoolDigest<T>),
}

/// Enumerates configurable runtime-stored parameters that
/// influences probation, rewards, penalties, election constraints,
/// or economic limits may be forcibly overridden
/// at runtime through privileged (root/governance) operations.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    RuntimeDebugNoBound,
    Clone,
    PartialEq,
    Eq,
    MaxEncodedLen,
    TypeInfo,
)]
#[scale_info(skip_type_params(T))]
pub enum ForceGenesisConfig<T: Config> {
    /// Updates the number of blocks authors must remain in probation.
    ProbationPeriod(BlockNumberFor<T>),
    /// Updates how much probation is reduced on good behavior.
    ReduceProbationBy(BlockNumberFor<T>),
    /// Updates how much probation is increased on misbehavior.
    IncreaseProbationBy(BlockNumberFor<T>),
    // Updates the delay (in blocks) before rewards are finalized.
    RewardsBuffer(BlockNumberFor<T>),
    /// Updates the delay (in blocks) before penalties are enforced.
    PenaltiesBuffer(BlockNumberFor<T>),
    /// Updates the maximum number of authors that can be elected.
    MaxElected(u32),
    /// Updates the minimum number of authors required for a valid election.
    MinElected(u32),
    /// Toggles strict enforcement of `MaxElected`.
    EnforceMaxElected(bool),
    /// Update the  minimum funding required per backing operation.
    MinFund(AuthorAsset<T>),
    /// Updates the maximum allowed exposure per funding operation.
    MaxExposure(AuthorAsset<T>),
    /// Updates the minimum collateral required for authors.
    MinCollateral(AuthorAsset<T>),
}

/// Wrapper type for [`Fortitude`] used in extrinsics.
///
/// ## Variants
/// - `Force`: Enforces the operation strictly, potentially overriding
///   softer constraints or preferences.
/// - `Polite`: Applies the operation in a non-strict manner, respecting
///   constraints and failing if conditions are not ideal.
///
/// ## Notes
/// - This type exists to decouple the extrinsic API from the internal
///   [`Fortitude`] type.
/// - It is converted into [`Fortitude`] internally before execution.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    RuntimeDebugNoBound,
    Clone,
    PartialEq,
    Eq,
    MaxEncodedLen,
    TypeInfo,
)]
pub enum FortitudeWrapper {
    Force,
    Polite,
}

/// Wrapper type for [`Precision`] used in extrinsics.
///
/// This enum defines how precisely a funding operation should be executed,
/// particularly in scenarios where exact amounts may not be achievable due
/// to rounding, liquidity, or distribution constraints.
///
/// ## Variants
/// - `Exact`: Requires the operation to be executed with exact precision.
///   Fails if the exact value cannot be honored.
/// - `BestEffort`: Allows approximate execution, where the system will
///   attempt to fulfill the request as closely as possible.
///
/// ## Notes
/// - This type exists to decouple the extrinsic API from the internal
///   [`Precision`] type.
/// - It is converted into [`Precision`] internally before execution.
#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    RuntimeDebugNoBound,
    Clone,
    PartialEq,
    Eq,
    MaxEncodedLen,
    TypeInfo,
)]
pub enum PrecisionWrapper {
    Exact,
    BestEffort,
}

// ===============================================================================
// ```````````````````````````````` INHERENT IMPLS ```````````````````````````````
// ===============================================================================

impl<T: Config> AuthorInfo<T> {
    /// Creates a new `AuthorInfo` instance with default timestamps and status.
    ///
    /// - `status` starts as `Probation` to ensure initial monitoring and enforces
    ///    probation period.
    /// - `since`, `stale_since`, and `risk_until` are set to the current block number.
    pub fn new(digest: AuthorDigest<T>) -> Self {
        let current_block = frame_system::Pallet::<T>::block_number();
        Self {
            digest,
            since: current_block,
            status: AuthorStatus::Probation,
            status_since: current_block,
            risk_until: current_block,
            min_fund: None,
            max_fund: None,
        }
    }

    /// Recreates the author state for re-enrollment.
    ///
    /// - Resets lifecycle timestamps to the current block.
    /// - Sets status to `Probation` for re-evaluation.
    /// - Clears local funding constraints (`min_fund`, `max_fund`).
    /// - Retains the existing commitment digest.
    pub fn re_enroll(&self) -> Self {
        let current_block = frame_system::Pallet::<T>::block_number();
        Self {
            digest: self.digest.clone(),
            since: current_block,
            status: AuthorStatus::Probation,
            status_since: current_block,
            risk_until: current_block,
            min_fund: None,
            max_fund: None,
        }
    }
}

// ===============================================================================
// ````````````````````````````````` TRAIT IMPLS `````````````````````````````````
// ===============================================================================

impl VariantCount for AuthorStatus {
    /// The number of distinct variants for [`AuthorStatus`].
    const VARIANT_COUNT: u32 = 3;
}

impl From<PrecisionWrapper> for Precision {
    fn from(value: PrecisionWrapper) -> Self {
        match value {
            PrecisionWrapper::Exact => Precision::Exact,
            PrecisionWrapper::BestEffort => Precision::BestEffort,
        }
    }
}

impl From<FortitudeWrapper> for Fortitude {
    fn from(value: FortitudeWrapper) -> Self {
        match value {
            FortitudeWrapper::Force => Fortitude::Force,
            FortitudeWrapper::Polite => Fortitude::Polite,
        }
    }
}

// ===============================================================================
// ````````````````````````````````` DERIVE IMPLS ````````````````````````````````
// ===============================================================================

impl<T: Config> Debug for Funder<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Funder::Direct(d) => write!(f, "Direct({:?})", d),
            Funder::Index { digest, backer } => {
                write!(f, "Index(digest: {:?}, backer: {:?})", digest, backer)
            }
            Funder::Pool { digest, backer } => {
                write!(f, "Pool(digest: {:?}, backer: {:?})", digest, backer)
            }
        }
    }
}

/// [`BackingElectionWeight`] tuple only takes the other element [`AuthorAsset`] for ordering.
impl<T: Config> Ord for Funder<T> {
    fn cmp(&self, _other: &Self) -> Ordering {
        Ordering::Equal
    }
}

impl<T: Config> PartialOrd for Funder<T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Config> core::fmt::Debug for AuthorInfo<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AuthorInfo")
            .field("digest", &self.digest)
            .field("since", &self.since)
            .field("status", &self.status)
            .field("status_since", &self.status_since)
            .field("risk_until", &self.risk_until)
            .field("min_fund", &self.min_fund)
            .field("max_fund", &self.max_fund)
            .finish()
    }
}
