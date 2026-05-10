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
// ``````````````````````````````` COMMITMENT TYPES ``````````````````````````````
// ===============================================================================

//! **Core types and aliases for the Commitment system.**
//!
//! This module defines the primary structures and type aliases used by
//! [`pallet_commitment`](crate). These types are publicly exposed and used across
//! the pallet's APIs for representing Commitment-related data.
//!
//! Trait implementations provided by this crate's [`crate::Pallet`] can use these types
//! via trait-bound equality constraints to ensure type alignment with this pallet's
//! concrete implementations if necessary.
//!
//! ## Invariants & Access
//!
//! All structures in this module encapsulate their fields as private to enforce
//! invariants during creation, mutation, and access. As a result, interaction with
//! these types is performed exclusively through inherent methods, which provide
//! both internal mutation capabilities and safe external (read/query) access.
//!
//! ## Example
//!
//! ```ignore
//! mod pallet {
//!     use pallet_commitment::types::IndexInfo;
//!
//!     pub trait Config<I: 'static>: frame_system::Config {
//!         type CommitmentAdapter: CommitIndex<Index = IndexInfo<Self, I>>;
//!     }
//! }
//! ```

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{balance::ProductType, Config, Error};

// --- Core ---
use core::{fmt::Debug, marker::PhantomData};

// --- Scale-codec crates ---
use codec::DecodeWithMemTracking;
use scale_info::{prelude::vec, TypeInfo};

// --- Derive Macros ---
use derive_more::Constructor;

// --- FRAME Suite ---
use frame_suite::{assets::*, misc::PositionIndex, plugins::ModelContext};

// --- FRAME Support ---
use frame_support::{
    dispatch::DispatchResult,
    ensure,
    traits::{
        fungible::{Inspect, InspectFreeze},
        tokens::Precision,
        VariantCountOf,
    },
};

// --- FRAME System ---
use frame_system::pallet;

// --- Substrate primitives ---
use sp_core::{Decode, Encode, Get, MaxEncodedLen};
use sp_runtime::{
    traits::{CheckedAdd, Zero},
    BoundedVec, DispatchError, RuntimeDebug, Vec, WeakBoundedVec,
};
use sp_std::collections::btree_set::BTreeSet;

// ===============================================================================
// ``````````````````````````````````` ALIASES ```````````````````````````````````
// ===============================================================================

/// The **primary digest type** used to uniquely identify a commitment entity.
///
/// This type is reused across direct, index, and pool commitments.
pub type Digest<T> = <T as pallet::Config>::AccountId;

/// Represents the **unique identifier** for a direct digest.
///
/// A direct digest is neither an index nor a pool (i.e. not an indirect digest).
pub type DirectDigest<T> = Digest<T>;

/// Represents the **unique identifier** for an index.
pub type IndexDigest<T> = Digest<T>;

/// Represents the **unique identifier** for a pool.
pub type PoolDigest<T> = Digest<T>;

/// Represents the **unique identifier** for an entry within an index.
pub type EntryDigest<T> = Digest<T>;

/// Represents the **unique identifier** for a slot within a pool.
pub type SlotDigest<T> = Digest<T>;

/// Represents the **source for generating a digest**,
/// typically the runtime-caller's `AccountId` that seeds it.
pub type DigestSource<T> = <T as pallet::Config>::AccountId;

/// Represents the **owner of an asset or commitment**.  
pub type Proprietor<T> = <T as pallet::Config>::AccountId;

/// The fungible **balance type** for assets handled by the pallet.
///
/// Derived from the pallet's [`Config::Asset`] type and associated with the [`Proprietor`].
pub type AssetOf<T, I = ()> = <<T as Config<I>>::Asset as Inspect<Proprietor<T>>>::Balance;

/// Represents a **lazy-evaluated balance** for commitments.  
///
/// Doesn't specialize for commit-variants [`Config::Position`] as its implemented at
/// higher level for commits, digests, indexes, pools, etc individually.
pub type LazyBalanceOf<T, I = ()> = VirtualBalance<T, I>;

/// Represents a **single commit instance** created by a commit operation.
///
/// This is a thin wrapper over [`VirtualReceipt`], capturing a receipt of the
/// deposit at the time of commitment-similar to a bill that is later required
/// during withdrawal resolution.
///
/// A commitment may accumulate multiple commit instances over time. Each
/// instance is immutable, with aggregation and evaluation performed at
/// higher levels.
pub type CommitInstance<T, I = ()> = VirtualReceipt<T, I>;

/// Combined identifier representing the **reason for freezing or locking a balance**.
///
/// Typically derived from the runtime's composite freeze-reason enum associated
/// with [`Config::Asset`]. It is used to distinguish between different contexts
/// in which balances are held, such as commitments, freezes, or other locking
/// mechanisms.
pub type CommitReason<T, I = ()> = <<T as Config<I>>::Asset as InspectFreeze<Proprietor<T>>>::Id;

/// Alias to the pallet-defined balance execution context.
///
/// This represents the **type-level environment** configured by the runtime,
/// providing all bounds, extensions, and error definitions required to
/// materialize a lazy balance [`plugin`](frame_suite::plugins) family model.
pub type BalanceContext<T, I = ()> = <T as Config<I>>::BalanceContext;

/// Concrete [`plugin`](frame_suite::plugins)-model/family context derived
/// from [`BalanceContext`].
///
/// This resolves the **plugin execution context**, supplying runtime-specific
/// parameters and dependencies required by balance operations.
pub type BalanceModelContext<T, I = ()> = <BalanceContext<T, I> as ModelContext>::Context;

/// A generic [`virtual`](frame_suite::virtuals) structure. It acts as the
/// core building block for all lazy balance-related virtual types.
pub type LazyVirtual<T, A, R, Ti, Ad, I = ()> =
    ProductType<T, I, BalanceModelContext<T, I>, A, R, Ti, Ad>;

/// Virtual representation of a live lazy-balance.
///
/// Backed by the lazy balance model, meaning storage layouts are interpreted
/// dynamically by the caller rather than the implementor.
pub type VirtualBalance<T, I = ()> =
    LazyVirtual<T, BalanceAsset, BalanceRational, BalanceTime, BalanceAddon, I>;

/// Virtual representation of a balance snapshot.
///
/// Captures balance state at a specific point in time. Used for historical
/// views and proportional calculations.
pub type VirtualSnapShot<T, I = ()> =
    LazyVirtual<T, SnapShotAsset, SnapShotRational, SnapShotTime, SnapShotAddon, I>;

/// Virtual representation of a receipt (claim).
///
/// Represents a deferred claim over balance value in the lazy model:
/// - created on deposit
/// - resolved on withdrawal
///
/// Its value is computed dynamically based on global balance state.
pub type VirtualReceipt<T, I = ()> =
    LazyVirtual<T, ReceiptAsset, ReceiptRational, ReceiptTime, ReceiptAddon, I>;

// ===============================================================================
// ``````````````````````````` DIGEST BALANCES VECTOR ````````````````````````````
// ===============================================================================

/// Stores balance information for each variant of a digest.
///
/// A digest may have multiple semantic variants (e.g. `Affirmative`, `Contrary`, etc),
/// each maintaining its own balance. This structure tracks the corresponding
/// [`LazyBalanceOf`] for every variant.
///
/// Internally, it is backed by a [`BoundedVec`] whose length is fixed to the
/// number of semantic variants defined by [`Config::Position`] via the
/// `VariantCount` bound. This guarantees a **single, stable slot per variant**.
///
/// In some scenarios, a higher-indexed variant may be initialized before its
/// preceding variants. In such cases, the earlier slots are filled with a default
/// lazy balance to preserve positional invariants. While eagerly initializing all
/// slots would also be invariant-safe, it could increase storage usage when many
/// digests exist or when commitments do not utilize all variants.
///
/// To keep storage usage minimal, it is expected that the default variant
/// (`[`Default`]` for [`Config::Position`]) occupies index `0`, as defined by
/// [`PositionIndex`].
#[derive(
    Encode,
    Decode,
    Clone,
    RuntimeDebug,
    PartialEq,
    Eq,
    MaxEncodedLen,
    TypeInfo,
    DecodeWithMemTracking,
)]
#[scale_info(skip_type_params(T, I))]
pub struct DigestInfo<T: Config<I>, I: 'static = ()>(
    BoundedVec<LazyBalanceOf<T, I>, VariantCountOf<T::Position>>,
);

// ===============================================================================
// ``````````````````` DIGEST BALANCES VECTOR INHERENT METHODS ```````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> DigestInfo<T, I> {
    /// Returns the actively funded lazy balances of commitment digests along with
    /// their corresponding semantic positions (commit variants).
    ///
    /// Returns `DispatchError` if lookup or decoding fails.
    pub fn balances(&self) -> Result<Vec<(T::Position, LazyBalanceOf<T, I>)>, DispatchError> {
        let bound = &self.0;
        let mut collect = Vec::new();
        for (i, balance) in bound.iter().enumerate() {
            if *balance == Default::default() {
                continue;
            }
            let position = <T::Position as PositionIndex>::position_of(i);
            debug_assert!(
                position.is_some(),
                "commit-variant invalid position found for index {:?}, 
                an example default of the position type for debugging is {:?}",
                i,
                T::Position::default()
            );
            let position = position.ok_or(Error::<T, I>::InvalidCommitVariantIndex)?;
            collect.push((position, balance.clone()));
        }
        Ok(collect)
    }

    pub(crate) fn mut_balance(
        &mut self,
        variant: &T::Position,
    ) -> Option<&mut LazyBalanceOf<T, I>> {
        // Since we store variant balances as a vector, we need to deterministically
        // determine an index associated with the given variant
        let idx = variant.index();
        self.0.get_mut(idx)
    }

    pub fn get_balance(&self, variant: &T::Position) -> Option<&LazyBalanceOf<T, I>> {
        let idx = variant.index();
        self.0.get(idx)
    }

    pub fn reveal(&self) -> BoundedVec<LazyBalanceOf<T, I>, VariantCountOf<T::Position>> {
        self.0.clone()
    }

    pub(crate) fn init_balance(&mut self, variant: &T::Position) -> Result<(), DispatchError> {
        let idx = variant.index();
        // If the variant does not exist, create default variant balances up to the requested index
        let vec = &mut self.0;
        for i in 0..=idx {
            if let None = vec.get(i) {
                // Push default variant balances for missing variants
                let result = vec.try_push(Default::default());
                debug_assert!(
                    result.is_ok(),
                    "default commit-variants push results bad, where pushed 
                    index {:?} is lesser than or equal to expected variant (position) 
                    {:?} whoose index is {:?}",
                    i,
                    variant,
                    idx
                );
                result.map_err(|_| Error::<T, I>::VariantsExhausted)?;
            }
        }
        return Ok(());
    }
}

// ===============================================================================
// ``````````````````````````````` COMMITS VECTOR ````````````````````````````````
// ===============================================================================

/// Represents a collection of individual commit instances of a proprietor
/// for a specific digest (direct/index/pool) and commitment reason.
///
/// The association with a single **digest** and **reason** is not structurally
/// enforced at this level; instead, it is guaranteed by higher-level structures
/// (typically [`CommitInfo`]).
///
/// Each commit is stored as a [`CommitInstance`] within a [`WeakBoundedVec`],
/// bounding the number of commits per `(digest, reason)` pair to
/// [`Config::MaxCommits`].
#[derive(Encode, Decode, Clone, RuntimeDebug, MaxEncodedLen, TypeInfo, DecodeWithMemTracking)]
#[scale_info(skip_type_params(T, I))]
pub struct Commits<T: Config<I>, I: 'static = ()>(
    WeakBoundedVec<CommitInstance<T, I>, T::MaxCommits>,
);

// ===============================================================================
// ``````````````````````` COMMITS VECTOR INHERENT METHODS ```````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> Commits<T, I> {
    /// Initializes a new collection of commits with a single [`CommitInstance`],
    /// typically derived from [`LazyBalance`] deposit operations.
    ///
    /// This establishes the initial commit, after which additional commits
    /// may be appended via [`Commits::add_commit`].
    pub(crate) fn new(instance: CommitInstance<T, I>) -> Result<Self, DispatchError> {
        let max = T::MaxCommits::get();
        ensure!(!max.is_zero(), Error::<T, I>::ZeroMaxCommits);
        let vec = vec![instance];
        let commits = WeakBoundedVec::<CommitInstance<T, I>, T::MaxCommits>::try_from(vec);
        debug_assert!(
            commits.is_ok(),
            "single commit-instance vec to weak-vec of 
            max-commit {} is non-zero failed but shouldn't be",
            T::MaxCommits::get()
        );
        let commits = commits.map_err(|_| Error::<T, I>::CommitConstructionFailed)?;
        return Ok(Commits(commits));
    }

    /// Adds a new commitment instance to the existing
    /// commits collection.
    ///
    /// Returns `DispatchError` if the bounded vector capacity
    /// is exhausted.
    pub(crate) fn add_commit(
        &mut self,
        instance: CommitInstance<T, I>,
    ) -> Result<(), DispatchError> {
        debug_assert!(
            !self.0.is_empty(),
            "empty commits constructed without a single 
            commit-instance, attempting to add a new-instance {:?}",
            instance
        );
        ensure!(!self.0.is_empty(), Error::<T, I>::EmptyCommitsNotAllowed);
        let vec = &mut self.0;
        vec.try_push(instance)
            .map_err(|_| Error::<T, I>::MaxCommitsReached)?;
        Ok(())
    }

    pub fn commits(&self) -> WeakBoundedVec<CommitInstance<T, I>, T::MaxCommits> {
        debug_assert!(
            !self.0.is_empty(),
            "empty commits constructed without 
            a single commit-instance"
        );
        // no need to ensure for empty commits since this is a query function
        // which can return empty vector without Result<T, DispatchError>
        // ensure!(!self.0.is_empty(), Error::<T, I>::EmptyCommitsNotAllowed);
        self.0.clone()
    }
}

// ===============================================================================
// ``````````````````````````` SINGLE COMMIT META-DATA ```````````````````````````
// ===============================================================================

/// Represents a commitment associated with a specific **digest** and reason.
///
/// This structure tracks commitments at the lowest level. The referenced
/// `digest` is intentionally unclassified and may correspond to a direct,
/// index, or pool digest, as those are higher-level abstractions built over
/// the same commitment model.
///
/// Each [`CommitInfo`] aggregates multiple [`CommitInstance`] values produced
/// over time for the same `(digest, reason)` pair, representing successive
/// commitments raised by the proprietor.
#[derive(
    Encode,
    Decode,
    Clone,
    RuntimeDebug,
    MaxEncodedLen,
    TypeInfo,
    PartialEq,
    Eq,
    DecodeWithMemTracking,
)]
#[scale_info(skip_type_params(T, I))]
pub struct CommitInfo<T: Config<I>, I: 'static = ()> {
    /// The target digest this commitment is associated with.
    ///
    /// The digest is intentionally unclassified and may refer to a
    /// direct, index, or pool digest.
    digest: Digest<T>,

    /// Collection of commit instances ([`CommitInstance`])
    /// associated with this `digest`.
    ///
    /// This collection is internally mutated via [`Commits::add_commit`]
    /// whenever new commit instances are appended.
    commits: Commits<T, I>,

    /// The semantic disposition (variant) of the commitment
    /// (e.g. `Affirmative`, `Contrary`, etc).
    ///
    /// This is semantically meaningful only for direct digests. For index
    /// and pool digests, this field acts as a structural placeholder, as
    /// those abstractions manage their own variant information through
    /// entries and slots respectively.
    variant: T::Position,
}

// ===============================================================================
// `````````````````` SINGLE COMMIT META-DATA INHERENT METHODS ```````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> CommitInfo<T, I> {
    /// Creates a new [`CommitInfo`] with an initial commit instance.
    ///
    /// This constructor initializes the internal [`Commits`] collection
    /// in a controlled manner and establishes the initial commitment
    /// state for the given digest and reason.
    pub(crate) fn new(
        digest: Digest<T>,
        instance: CommitInstance<T, I>,
        variant: T::Position,
    ) -> Result<Self, DispatchError> {
        let commits = Commits::<T, I>::new(instance)?;
        let try_position = <T::Position as PositionIndex>::position_of(
            <T::Position as PositionIndex>::index(&variant),
        );
        debug_assert!(
            try_position.is_some(),
            "cannot equalize new-commit's given variant {:?} and its derived 
            positional index (not consistent) when creating new commit-info for 
            proprietor towards non-classified-digest {:?}",
            variant,
            digest
        );
        let position = try_position.ok_or(Error::<T, I>::InvalidCommitVariantIndex)?;
        debug_assert!(
            position == variant,
            "new-commit's given variant {:?} and its derived
            positional index (not consistent) variant is not same, 
            found {:?} when creating new commit-info for 
            proprietor towards non-classified-digest {:?}",
            variant,
            position,
            digest
        );
        ensure!(
            position == variant,
            Error::<T, I>::InvalidCommitVariantIndex
        );
        Ok(Self {
            digest,
            commits,
            variant,
        })
    }

    /// Returns the individual commit instances of the proprietor.
    #[inline]
    pub fn commits(&self) -> WeakBoundedVec<CommitInstance<T, I>, T::MaxCommits> {
        Commits::<T, I>::commits(&self.commits)
    }

    /// Returns the digest proprietor committed to.
    pub fn digest(&self) -> Digest<T> {
        self.digest.clone()
    }

    /// Returns the digest's variant proprietor committed to.
    pub fn variant(&self) -> T::Position {
        self.variant.clone()
    }

    /// Adds a new commitment instance to the existing commits
    /// collection of the proprietor's commit-info for a digest.
    ///
    /// Returns `DispatchError` if the bounded vector capacity
    /// is exhausted.
    #[inline]
    pub(crate) fn add_commit(
        &mut self,
        instance: CommitInstance<T, I>,
    ) -> Result<(), DispatchError> {
        self.commits.add_commit(instance)
    }
}

// ===============================================================================
// ````````````````````````` INDEX SINGLE-ENTRY META-DATA ````````````````````````
// ===============================================================================

/// Represents a single entry within an index.
///
/// Each entry maps a direct digest to a non-zero share allocation and a
/// semantic variant. This allows index commitments to be proportionally
/// distributed across multiple underlying digests.
#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, DecodeWithMemTracking)]
#[scale_info(skip_type_params(T, I))]
pub struct EntryInfo<T: Config<I>, I: 'static = ()> {
    /// The direct digest identifying this entry.
    digest: EntryDigest<T>,

    /// Number of shares (must be non-zero) associated with this entry.
    shares: T::Shares,

    /// Semantic variant/disposition of this entry (e.g. `Affirmative`, `Contrary`, etc).
    ///
    /// Commitments placed through this entry are credited to the corresponding
    /// variant balance of the underlying direct digest.
    variant: T::Position,
}

// ===============================================================================
// ````````````````````` INDEX SINGLE-ENTRY INHERENT METHODS `````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> EntryInfo<T, I> {
    /// Creates a new [`EntryInfo`] for an index.
    ///
    /// Validates that the provided shares are non-zero and that the variant
    /// is semantically valid.
    ///
    /// Returns `DispatchError` if validation fails.
    pub fn new(
        digest: EntryDigest<T>,
        shares: T::Shares,
        variant: T::Position,
    ) -> Result<Self, DispatchError> {
        ensure!(!shares.is_zero(), Error::<T, I>::ShareCannotBeZero);
        let try_position = <T::Position as PositionIndex>::position_of(
            <T::Position as PositionIndex>::index(&variant),
        );
        debug_assert!(
            try_position.is_some(),
            "cannot equalize new-commit's given variant {:?} and its derived 
            positional index (not consistent) when creating new entry-info for 
            entry-digest {:?} of shares {:?}",
            variant,
            digest,
            shares
        );
        let position = try_position.ok_or(Error::<T, I>::InvalidCommitVariantIndex)?;
        debug_assert!(
            position == variant,
            "new-commit's given variant {:?} and its derived
            positional index (not consistent) variant is not same, 
            found {:?} when creating new entry-info for entry-digest 
            {:?} of shares {:?}",
            variant,
            position,
            digest,
            shares
        );
        ensure!(
            position == variant,
            Error::<T, I>::InvalidCommitVariantIndex
        );
        Ok(Self {
            digest,
            shares,
            variant,
        })
    }

    /// Return the share value of this entry.
    ///
    /// `DispatchError` if inconsistency detected.
    pub fn shares(&self) -> T::Shares {
        self.shares
    }

    /// Returns the direct digest associated with this entry.
    pub fn digest(&self) -> Digest<T> {
        self.digest.clone()
    }

    /// Returns the variant associated with this entry's direct digest.
    pub fn variant(&self) -> T::Position {
        self.variant.clone()
    }
}

impl<T: Config<I>, I: 'static> Clone for EntryInfo<T, I> {
    fn clone(&self) -> Self {
        Self {
            digest: self.digest.clone(),
            shares: self.shares,
            variant: self.variant.clone(),
        }
    }
}

// ===============================================================================
// ```````````````````````````` INDEX ENTRIES VECTOR `````````````````````````````
// ===============================================================================

/// Represents a collection of entries within an index.
///
/// Backed by a [`WeakBoundedVec`] to enforce an upper bound on the number of
/// entries (via [`Config::MaxIndexEntries`]) while maintaining efficient,
/// bounded storage.
///
/// This serves as the low-level container for all [`EntryInfo`] items that
/// constitute an index.
#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, DecodeWithMemTracking)]
#[scale_info(skip_type_params(T, I))]
pub struct Entries<T: Config<I>, I: 'static = ()>(
    WeakBoundedVec<EntryInfo<T, I>, T::MaxIndexEntries>,
);

// ===============================================================================
// ```````````````````` INDEX ENTRIES VECTOR INHERENT METHODS ````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> Entries<T, I> {
    /// Creates a new [`Entries`] collection for an index from a vector of
    /// validated [`EntryInfo`] items.
    ///
    /// Returns `DispatchError` if any inconsistencies detected.
    pub fn new(entries: Vec<EntryInfo<T, I>>) -> Result<Self, DispatchError> {
        let max = T::MaxIndexEntries::get();
        ensure!(!max.is_zero(), Error::<T, I>::TriedCreatingHaltedIndexes);
        ensure!(!entries.is_empty(), Error::<T, I>::EmptyEntriesNotAllowed);
        let mut seen = BTreeSet::new();
        for entry in &entries {
            ensure!(
                seen.insert(entry.digest.clone()),
                Error::<T, I>::DuplicateEntry
            );
        }
        let entries = WeakBoundedVec::<EntryInfo<T, I>, T::MaxIndexEntries>::try_from(entries)
            .map_err(|_| Error::<T, I>::MaxEntriesReached)?;
        return Ok(Entries(entries));
    }

    /// Returns all [`EntryInfo`] items contained in this list as a owned vector.
    pub fn entries(&self) -> Vec<EntryInfo<T, I>> {
        let bounded = &self.0;
        let mut collect = Vec::new();
        for entry in bounded {
            collect.push(entry.clone())
        }
        debug_assert!(
            !collect.is_empty(),
            "empty entries-list initiated which 
            should not be for indexes"
        );
        collect
    }

    /// Adds a new [`EntryInfo`] to the list of entries.
    ///
    /// Returns `DispatchError` if vector bound exhausted
    /// or duplicate found.
    pub fn add_entry(&mut self, entry: EntryInfo<T, I>) -> Result<(), DispatchError> {
        debug_assert!(
            !self.0.is_empty(),
            "empty entries constructed without a single 
            commit-instance, attempting to add a new-entry",
        );
        ensure!(!self.0.is_empty(), Error::<T, I>::EmptyEntriesNotAllowed);
        let vec = &mut self.0;
        vec.try_push(entry)
            .map_err(|_| Error::<T, I>::MaxEntriesReached)?;
        let mut seen = BTreeSet::new();
        for entry in vec {
            ensure!(
                seen.insert(entry.digest.clone()),
                Error::<T, I>::DuplicateEntry
            );
        }
        Ok(())
    }

    /// Removes an existing [`EntryInfo`] from the entries-list.
    ///
    /// Returns `DispatchError` if entry of digest not found.
    pub fn remove_entry(&mut self, entry: &EntryDigest<T>) -> Result<(), DispatchError> {
        debug_assert!(
            !self.0.is_empty(),
            "empty entries constructed without a single 
            commit-instance, attempting to remove an existing-entry {:?}",
            entry
        );
        ensure!(
            (!self.0.is_empty() && self.0.len() > 1),
            Error::<T, I>::EmptyEntriesNotAllowed
        );
        debug_assert!(
            self.0.len() > 1,
            "attempting to remove an existing-entry {:?}, which 
            will result in zero-length entries",
            entry
        );
        let mut entry_idx = None;
        for (i, entry_of) in self.0.iter().enumerate() {
            if entry_of.digest == *entry {
                entry_idx = Some(i);
                break;
            }
        }

        match entry_idx {
            Some(idx) => {
                self.0.remove(idx);
            }
            None => {
                return Err(Error::<T, I>::EntryOfIndexNotFound)?;
            }
        }
        Ok(())
    }
}

// ===============================================================================
// ``````````````````````````````` INDEX META-DATA ```````````````````````````````
// ===============================================================================

/// Represents an index containing multiple entries.
///
/// An `IndexInfo` tracks the overall capital, total balance, and the entries themselves.
///
#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, DecodeWithMemTracking)]
#[scale_info(skip_type_params(T, I))]
pub struct IndexInfo<T: Config<I>, I: 'static = ()> {
    /// Total asset depositted to this index
    ///
    /// This does not qualify as real-time value of the index,
    /// since entries are finite digests that should be queried
    /// for such cases.
    principal: AssetOf<T, I>,

    /// Total shares/capital across all entries in this index
    capital: T::Shares,

    /// The collection of entries making up this index
    entries: Entries<T, I>,
}

// ===============================================================================
// ``````````````````````` INDEX META-DATA INHERENT METHODS ``````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> IndexInfo<T, I> {
    /// Creates a new [`IndexInfo`] from a collection of entries.
    ///
    /// This function calculates the total capital by summing the shares
    /// of all entries. It retains only entries of non-zero shares.
    ///
    /// Returns `DispatchError` if any of invariant fails
    /// - Total Capital cannot be zero
    /// - Entries should not be empty
    pub(crate) fn new(entries: &mut Entries<T, I>) -> Result<Self, DispatchError> {
        debug_assert!(!entries.0.is_empty(), "entries is constructed empty");
        ensure!(!entries.0.is_empty(), Error::<T, I>::EmptyEntriesNotAllowed);
        let mut total_capital = T::Shares::zero();
        for entry in &entries.0 {
            let shares = entry.shares;
            debug_assert!(
                !shares.is_zero(),
                "entry for digest {:?} of variant {:?} share is constructed zero",
                entry.digest,
                entry.variant
            );
            ensure!(!shares.is_zero(), Error::<T, I>::ShareCannotBeZero);
            total_capital = total_capital
                .checked_add(&shares)
                .ok_or(Error::<T, I>::CapitalOverflowed)?;
        }
        debug_assert!(
            !total_capital.is_zero(),
            "total capital is zero while its entry shares isn't"
        );
        ensure!(!total_capital.is_zero(), Error::<T, I>::CapitalCannotBeZero);
        Ok(Self {
            principal: AssetOf::<T, I>::zero(),
            capital: total_capital,
            entries: entries.clone(),
        })
    }

    /// Returns the index's capital - total shares.
    pub fn capital(&self) -> T::Shares {
        let value = self.capital;
        debug_assert!(!value.is_zero(), "index capital is constructed zero");
        value
    }

    /// Returns the index's principal, i.e. the total amount
    /// deposited by proprietors.
    pub fn principal(&self) -> AssetOf<T, I> {
        self.principal
    }

    /// Returns the index's entries vector list.
    #[inline]
    pub fn entries(&self) -> Vec<EntryInfo<T, I>> {
        Entries::<T, I>::entries(&self.entries)
    }

    /// Reveal the actual entries [`Entries`]
    pub fn reveal_entries(&self) -> Entries<T, I> {
        self.entries.clone()
    }

    /// Checks if an entry of digest exists in the index.
    pub fn entry_exists(&self, entry: &EntryDigest<T>) -> DispatchResult {
        let entries = &self.entries;
        debug_assert!(!entries.0.is_empty(), "entries is constructed empty");
        ensure!(!entries.0.is_empty(), Error::<T, I>::EmptyEntriesNotAllowed);
        let mut idx = None;
        // Locate the target slot.
        for (i, entry_of) in entries.0.iter().enumerate() {
            if entry_of.digest == *entry {
                idx = Some(i);
            }
        }

        // If no matching slot exists, nothing to remove.
        if let Some(_) = idx {
            return Ok(());
        };

        Err(Error::<T, I>::EntryOfIndexNotFound.into())
    }

    /// Sets the index principal to the provided value, replacing the existing balance.
    pub(crate) fn set_balance(&mut self, principal: AssetOf<T, I>) {
        self.principal = principal
    }
}

// ===============================================================================
// ```````````````````````````` SINGLE SLOT META-DATA ````````````````````````````
// ===============================================================================

/// Represents a slot within a pool, derived from an index entry.
///
/// A `SlotInfo` tracks the underlying digest, the allocated shares, the
/// slot's single [`CommitInstance`] (a collective receipt)
/// and the variant/disposition. It is primarily used when creating or
/// managing pools derived from index entries.
#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, DecodeWithMemTracking)]
#[scale_info(skip_type_params(T, I))]
pub struct SlotInfo<T: Config<I>, I: 'static = ()> {
    /// Unique identifier for the slot
    digest: SlotDigest<T>,

    /// Shares allocated to this slot
    shares: T::Shares,

    /// Commit-Instance associated with this slot
    ///
    /// Since pools collectively manages funds, slots also
    /// inherit such behaviour. Hence it acts similar to a proprietor
    /// holding a deposit receipt from digest for their commitment.
    commit: CommitInstance<T, I>,

    /// Disposition of this slot (e.g., Affirmative, Contrary, Awaiting)
    variant: T::Position,
}

// ===============================================================================
// ``````````````````````` SINGLE SLOT META-DATA INHERENTS ```````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> SlotInfo<T, I> {
    /// Returns the pool's slot's total shares.
    pub fn shares(&self) -> T::Shares {
        self.shares
    }

    /// Returns the pool's slot's commit-instance
    /// (as a pseudo-proprietor).
    pub fn commit(&self) -> CommitInstance<T, I> {
        self.commit.clone()
    }

    /// Returns the pool's slot's digest.
    pub fn digest(&self) -> SlotDigest<T> {
        self.digest.clone()
    }

    /// Returns the pool's slot's variant.
    pub fn variant(&self) -> T::Position {
        self.variant.clone()
    }

    /// Updates the [`CommitInstance`] associated with this slot.
    ///
    /// This method **replaces the existing commit instance** with the provided one.
    /// It does not perform any validation, aggregation, or merging of commits,
    /// the caller is responsible for ensuring correctness and consistency.
    ///
    /// ## Parameters
    /// - `commit`: The new [`CommitInstance`] to associate with this slot.
    ///
    /// ## Invariants
    /// - This operation assumes the slot is already initialized.
    /// - The provided commit should be semantically valid for this slot's
    ///   digest, shares, and variant.
    ///
    /// ## Note
    /// This is an internal mutation helper and is not intended to enforce
    /// higher-level commitment rules.
    fn set_slot_commit(&mut self, commit: CommitInstance<T, I>) {
        self.commit = commit
    }
}

// ===============================================================================
// ```````````````````````` ENTRY-TO-SLOT SAFE CONVERSION ````````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> From<EntryInfo<T, I>> for SlotInfo<T, I> {
    /// Converts an `EntryInfo` into a `SlotInfo`.
    ///
    /// This is used when creating a pool from an index, as each index entry
    /// corresponds to a pool slot. The deposit-receipt is initialized to empty
    /// as slot doesn't hold a commit currently.
    ///
    /// ## Returns
    /// A `SlotInfo` with the same digest, shares, and variant.
    fn from(entry: EntryInfo<T, I>) -> Self {
        Self {
            digest: entry.digest,
            shares: entry.shares,
            commit: Default::default(),
            variant: entry.variant,
        }
    }
}

// ===============================================================================
// ````````````````````````````````` SLOTS VECTOR ````````````````````````````````
// ===============================================================================

/// Represents a collection of slots within a pool.
///
/// `Slots` is essentially a bounded vector of [`SlotInfo`] instances, enforcing
/// a maximum number of slots defined by `MaxIndexEntries`.
///
/// This type ensures that pools derived from indexes cannot exceed the maximum
/// allowed number of slots.
#[derive(Encode, Decode, MaxEncodedLen, TypeInfo)]
#[scale_info(skip_type_params(T, I))]
pub struct Slots<T: Config<I>, I: 'static = ()>(WeakBoundedVec<SlotInfo<T, I>, T::MaxIndexEntries>);

// ===============================================================================
// ```````````````````````` SLOTS VECTOR INHERENT METHODS ````````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> Slots<T, I> {
    /// Returns the pools's individual slots as a vector.
    pub fn slots(&self) -> Vec<SlotInfo<T, I>> {
        let bounded = &self.0;
        let mut collect = Vec::new();
        for slot in bounded {
            collect.push(slot.clone())
        }
        debug_assert!(
            !collect.is_empty(),
            "empty slots initiated which should not be for pools"
        );
        collect
    }

    /// Adds a new [`EntryInfo`] to the list of slots, since slots can only
    /// be derived from entry.
    ///
    /// Returns `DispatchError` if vector bound exhausted or duplicate found.
    fn add_slot(&mut self, entry: EntryInfo<T, I>) -> Result<(), DispatchError> {
        debug_assert!(
            !self.0.is_empty(),
            "empty slots constructed without a single 
            slot, attempting to add a new-slot via entry",
        );
        ensure!(!self.0.is_empty(), Error::<T, I>::EmptySlotsNotAllowed);
        let vec = &mut self.0;
        vec.try_push(entry.into())
            .map_err(|_| Error::<T, I>::MaxSlotsReached)?;
        let mut seen = BTreeSet::new();
        for slot in vec {
            ensure!(
                seen.insert(slot.digest.clone()),
                Error::<T, I>::DuplicateSlot,
            );
        }
        Ok(())
    }

    /// Removes an existing [`SlotInfo`] from the slots-list.
    ///
    /// Returns `DispatchError` if slot of digest not found.
    fn remove_slot(&mut self, slot: &SlotDigest<T>) -> Result<(), DispatchError> {
        debug_assert!(
            !self.0.is_empty(),
            "empty slots constructed without a single 
            slot, attempting to remove an existing-slot {:?}",
            slot,
        );
        ensure!(
            (!self.0.is_empty() && self.0.len() > 1),
            Error::<T, I>::EmptySlotsNotAllowed
        );
        debug_assert!(
            self.0.len() > 1,
            "attempting to remove an existing-slot {:?}, which 
            will result in zero-length slots",
            slot
        );
        let mut slot_idx = None;
        for (i, slot_of) in self.0.iter().enumerate() {
            if slot_of.digest == *slot {
                slot_idx = Some(i);
                break;
            }
        }

        match slot_idx {
            Some(idx) => {
                self.0.remove(idx);
            }
            None => {
                return Err(Error::<T, I>::SlotOfPoolNotFound)?;
            }
        }
        Ok(())
    }

    /// Updates the commit instance of a slot identified by the given `digest`.
    ///
    /// This function performs a linear search over the internal slots collection
    /// to locate a slot whose `digest` matches the provided `digest`.
    ///
    /// - If a matching slot is found:
    ///     - Its associated [`CommitInstance`] is **replaced** with the provided `commit`.
    /// - If no matching slot exists:
    ///     - Returns [`Error::SlotOfPoolNotFound`].
    fn set_slot_commit(
        &mut self,
        digest: &SlotDigest<T>,
        commit: CommitInstance<T, I>,
    ) -> Result<(), DispatchError> {
        debug_assert!(
            !self.0.is_empty(),
            "empty slots constructed without a single 
            slot {:?}",
            self,
        );

        let mut slot_idx = None;
        for (i, slot_of) in self.0.iter().enumerate() {
            if slot_of.digest == *digest {
                slot_idx = Some(i);
                break;
            }
        }

        match slot_idx {
            Some(idx) => {
                let slot_of = self
                    .0
                    .get_mut(idx)
                    .ok_or(Error::<T, I>::SlotOfPoolNotFound)?;
                slot_of.set_slot_commit(commit);
            }
            None => {
                return Err(Error::<T, I>::SlotOfPoolNotFound)?;
            }
        }
        Ok(())
    }
}

// ===============================================================================
// `````````````````````` SLOT-TO-ENTRY FALLIBLE CONVERSION ``````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> TryFrom<Entries<T, I>> for Slots<T, I> {
    type Error = DispatchError;

    /// Converts a collection of `Entries` into `Slots`.
    ///
    /// Each [`EntryInfo`] is converted into a [`SlotInfo`] using the `From<EntryInfo>` implementation.
    ///
    /// ## Returns
    /// - `Ok(Slots)` if the conversion succeeds within the maximum allowed slots.
    /// - `Err(DispatchError)` if the resulting collection exceeds `MaxIndexEntries`.
    fn try_from(entries: Entries<T, I>) -> Result<Self, Self::Error> {
        let raw_vec: Vec<SlotInfo<T, I>> =
            entries.0.into_iter().map(|entry| entry.into()).collect();

        let entries = WeakBoundedVec::try_from(raw_vec)
            .map(Slots)
            .map_err(|_| Error::<T, I>::MaxSlotsReached.into());
        debug_assert!(
            entries.is_ok(),
            "both entries and slots have same upper weak-bound
            but slots cannot be tried from entries"
        );
        entries
    }
}

// ===============================================================================
// ```````````````````````````````` POOL META-DATA ```````````````````````````````
// ===============================================================================

/// Represents a managed pool derived from an index.
///
/// `PoolInfo` aggregates capital, commission, and the collection of slots,
/// each of which represents a portion of the pool linked to underlying entries.
/// Unlike indexes, pools are mutable and can have their slot balances adjusted dynamically.
///
/// Pools are created from an [`IndexInfo`] (or [`Entries`]), allowing the
/// shares of each entry to be translated into slots within the pool.
///
/// Pool's slot shares/variants can then be adjusted over time, while the
/// pool's commission and structure remain consistent.
///
#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, DecodeWithMemTracking)]
#[scale_info(skip_type_params(T, I))]
pub struct PoolInfo<T: Config<I>, I: 'static = ()> {
    /// Real-time balance of the pool.
    ///
    /// Stored as [`LazyBalanceOf`] which allows efficient updates and tracking
    /// proprietor depositted balances internally within itself at higher level.
    balance_of: LazyBalanceOf<T, I>,

    /// Total capital of the pool.
    ///
    /// Computed as the sum of all shares in the pool's slots.
    /// Represents the total weight or stake across all slots.
    capital: T::Shares,

    /// Commission rate charged by the pool manager.
    ///
    /// The manager earns this percentage of rewards or profits
    /// generated by the pool.
    commission: T::Commission,

    /// Collection of slots representing the underlying assets
    /// or commitments.
    ///
    /// Each slot corresponds to an entry from the index that formed
    /// this pool. Slots track individual shares, its deposit receipt,
    /// and disposition [`Disposition`](frame_suite::Disposition).
    slots: Slots<T, I>,
}

// ===============================================================================
// ``````````````````````` POOL META-DATA INHERENT METHODS ```````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> PoolInfo<T, I> {
    /// Creates a new pool from a list of index entries and a commission rate.
    ///
    /// - Converts the [`Entries`] into [`Slots`].
    /// - Calculates the total capital by summing all entry shares.
    /// - Initializes the pool balance to zero.
    ///
    /// ## Returns
    /// - `Ok(PoolInfo)` if successful.
    /// - `Err(DispatchError)` otherwise.
    pub(crate) fn new(
        index_entries: Entries<T, I>,
        commission: T::Commission,
    ) -> Result<Self, DispatchError> {
        let total_capital = index_entries
            .0
            .iter()
            .try_fold(T::Shares::zero(), |acc, slot| {
                acc.checked_add(&slot.shares)
                    .ok_or(Error::<T, I>::CapitalOverflowed)
            })?;

        let slots = index_entries.try_into()?;

        Ok(Self {
            balance_of: Default::default(),
            capital: total_capital,
            commission,
            slots,
        })
    }

    /// Returns the pool's lazy balance.
    pub fn balance(&self) -> LazyBalanceOf<T, I> {
        self.balance_of.clone()
    }

    /// Set the pool balance usually only after release.
    pub(crate) fn set_balance(&mut self, balance: LazyBalanceOf<T, I>) {
        self.balance_of = balance;
    }

    /// Returns the pool's total capital i.e., total shares of all slots.
    pub fn capital(&self) -> T::Shares {
        let value = self.capital;
        debug_assert!(!value.is_zero(), "index capital is constructed zero");
        value
    }

    /// Returns the pool's commission i.e., manager's share while resolving the pool's commit.
    pub fn commission(&self) -> T::Commission {
        self.commission
    }

    /// Returns the pools's individual slots as a vector.
    pub fn slots(&self) -> Vec<SlotInfo<T, I>> {
        self.slots.slots()
    }

    /// Resets the pools's top-level lazy balance.
    ///
    /// This lazy balance acts like a pseudo-direct-digest
    /// for proprietors structurally for pool-commitments.
    pub(crate) fn balance_reset(&mut self) {
        self.balance_of = Default::default();
        for slot in &mut self.slots.0 {
            slot.commit = Default::default();
        }
    }

    /// Replaces the commit instance of the slot identified by `digest`
    /// by delegating to the underlying `slots`.
    #[inline]
    pub(crate) fn set_slot_commit(
        &mut self,
        digest: &SlotDigest<T>,
        commit: CommitInstance<T, I>,
    ) -> Result<(), DispatchError> {
        self.slots.set_slot_commit(digest, commit)
    }

    /// Adds a new [`EntryInfo`] to the list of pool's slots,
    /// as slots [`SlotInfo`] can only be derived from an entry.
    ///
    /// Unlike indexes which are immutable, pools are mutable hence requires
    /// slot management via higher-structures.
    ///
    /// Returns `DispatchError` otherwise.
    pub(crate) fn add_slot(&mut self, entry: EntryInfo<T, I>) -> DispatchResult {
        self.slots.add_slot(entry)?;
        let total_capital = self
            .slots
            .0
            .iter()
            .try_fold(T::Shares::zero(), |acc, slot| {
                acc.checked_add(&slot.shares)
                    .ok_or(Error::<T, I>::CapitalOverflowed)
            })?;
        self.capital = total_capital;
        Ok(())
    }

    /// Removes an existing slot from the pool.
    ///
    /// Unlike indexes which are immutable, pools are mutable hence requires
    /// slot management via higher-structures.
    ///
    /// Returns `DispatchError` otherwise.
    pub(crate) fn remove_slot(&mut self, slot: &SlotDigest<T>) -> DispatchResult {
        self.slots.remove_slot(slot)?;
        ensure!(
            !self.slots.0.is_empty(),
            Error::<T, I>::EmptySlotsNotAllowed
        );
        let total_capital = self
            .slots
            .0
            .iter()
            .try_fold(T::Shares::zero(), |acc, slot| {
                acc.checked_add(&slot.shares)
                    .ok_or(Error::<T, I>::CapitalOverflowed)
            })?;
        self.capital = total_capital;
        Ok(())
    }

    /// Checks if a slot of digest exists in the pool.
    pub fn slot_exists(&self, slot: &SlotDigest<T>) -> DispatchResult {
        let slots = &self.slots;
        debug_assert!(!slots.0.is_empty(), "slots are constructed empty");
        let mut idx = None;
        // Locate the target slot.
        for (i, slot_of) in slots.0.iter().enumerate() {
            if slot_of.digest == *slot {
                idx = Some(i);
            }
        }

        // If no matching slot exists, nothing to remove.
        if let Some(_) = idx {
            return Ok(());
        };

        Err(Error::<T, I>::SlotOfPoolNotFound.into())
    }
}

// ===============================================================================
// ````````````````````````` KEY-GENERATION SEED STRUCTS `````````````````````````
// ===============================================================================

/// A composite structure combining a commit reason with an index.
///
/// This struct is primarily used as a **key generation seed** for creating
/// unique digests associated with an index under a specific reason. By combining
/// both the `reason` and the [`IndexInfo`] itself, the resulting hash is unique
/// and deterministic for the given combination.
///
/// Used in functions like `gen_index_digest` to ensure that the same index under
/// the same reason always produces the same digest, while different reasons or
/// different index contents yield different digests.
#[derive(
    Encode,
    Decode,
    RuntimeDebug,
    MaxEncodedLen,
    TypeInfo,
    Constructor,
    PartialEq,
    Eq,
    DecodeWithMemTracking,
)]
#[scale_info(skip_type_params(T, I))]
pub struct IndexOfReason<T: Config<I>, I: 'static = ()> {
    /// The reason or context under which this index is associated.
    pub reason: CommitReason<T, I>,

    /// The index information being combined with the reason for key generation.
    pub index: IndexInfo<T, I>,
}

/// A composite structure combining a commit reason with a pool.
///
/// This struct is used as a **key generation seed** for creating unique digests
/// associated with a pool under a specific reason. By combining the `reason` and
/// the [`PoolInfo`], the digest is deterministic and unique for the pool's
/// composition and context.
///
/// Primarily utilized in functions like `gen_pool_digest`, allowing consistent
/// and collision-resistant digest generation for pools derived from an index.
#[derive(
    Encode,
    Decode,
    RuntimeDebug,
    MaxEncodedLen,
    TypeInfo,
    Constructor,
    PartialEq,
    Eq,
    DecodeWithMemTracking,
)]
#[scale_info(skip_type_params(T, I))]
pub struct PoolOfReason<T: Config<I>, I: 'static = ()> {
    /// The reason or context under which this pool is associated.
    pub reason: CommitReason<T, I>,

    /// The pool information being combined with the reason for key generation.
    pub pool: PoolInfo<T, I>,
}

// ===============================================================================
// ``````````````````````` IMBALANCE CARRIER (ASSET-DELTA) ```````````````````````
// ===============================================================================

/// Represents a net change (delta) in assets for a particular operation.
///
/// This struct is used to track **both deposits and withdrawals** in a single structure,
/// which is particularly useful when working with unbalanced fungible traits.
///
/// Since the pallet manually mints and burns assets to maintain equilibrium,
/// both `deposit` and `withdraw` fields are necessary.
///
/// ## Usage
/// - **Withdrawal operations**: Used to record the assets that need to be withdrawn
/// from a pool or digest.
/// - **Deposit/Recovery**: Tracks any leftover assets that must be deposited back to
/// maintain total system balance.
/// - **Equilibrium maintenance**: Helps reconcile unbalanced operations by explicitly
/// separating minting and burning.
#[derive(
    Encode,
    Decode,
    MaxEncodedLen,
    RuntimeDebug,
    TypeInfo,
    PartialEq,
    Clone,
    Constructor,
    Eq,
    DecodeWithMemTracking,
    Copy,
)]
#[scale_info(skip_type_params(T, I))]
pub(crate) struct AssetDelta<T: Config<I>, I: 'static = ()> {
    /// The amount of assets to be taken (decreased or burned) to the system or account.
    pub deposit: AssetOf<T, I>,
    /// The amount of assets to be given (increased or minted) to the system or account.
    pub withdraw: AssetOf<T, I>,
}

// ===============================================================================
// ```````````````````````````` EXTRINSIC PARAMETERS `````````````````````````````
// ===============================================================================

/// Choose a valid digest model for [`ChooseDigest::digest_model`] safe construction.
#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, DecodeWithMemTracking, Clone, Debug, Copy)]
pub enum ChooseDigest {
    Direct,
    Index,
    Pool,
}

impl ChooseDigest {
    pub fn digest_model<T: Config<I>, I: 'static>(&self, digest: Digest<T>) -> DigestVariant<T, I> {
        match self {
            ChooseDigest::Direct => DigestVariant::Direct(digest),
            ChooseDigest::Index => DigestVariant::Index(digest),
            ChooseDigest::Pool => DigestVariant::Pool(digest),
        }
    }
}

/// Represents a generic digest variant in the commitment system.
///
/// Note: Usage of PhantomData variant in runtime will result in `panic!`
/// in debug-builds. Use [`ChooseDigest::digest_model`] for safe constructions.
///
/// This enum distinguishes between the different types of digests that
/// can exist in the pallet. It allows functions and events to operate
/// generically over digests without losing context about their source or type.
///
/// ### Usage:
/// - Used in events like `CommitPlaced`, `DigestInfo`, etc., to convey
///   which type of digest is being referred to.
/// - Enables trait and function implementations to handle multiple digest types
///   in a type-safe and g||eneric way.
#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, DecodeWithMemTracking)]
#[scale_info(skip_type_params(T, I))]
pub enum DigestVariant<T: Config<I>, I: 'static = ()> {
    /// A digest that refers directly to a commitment.
    Direct(Digest<T>),
    /// A digest that represents an index of multiple entries.
    Index(Digest<T>),
    /// A digest that represents a managed pool of slots.
    Pool(Digest<T>),
    /// Phantom variant to ensure the instance parameter `I` is used.
    /// This variant is never constructed.
    #[codec(skip)]
    __Ignore(PhantomData<I>),
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
    RuntimeDebug,
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

impl From<PrecisionWrapper> for Precision {
    fn from(value: PrecisionWrapper) -> Self {
        match value {
            PrecisionWrapper::Exact => Precision::Exact,
            PrecisionWrapper::BestEffort => Precision::BestEffort,
        }
    }
}

// ===============================================================================
// ````````````````````````````````` DERIVE IMPLS ````````````````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> Default for DigestInfo<T, I> {
    /// Creates an empty `DigestInfo` with no variant balances.
    /// Variant slots are initialized lazily and filled safely on demand.
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Config<I>, I: 'static> PartialEq for Commits<T, I> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: Config<I>, I: 'static> Eq for Commits<T, I> {}

impl<T: Config<I>, I: 'static> PartialEq for EntryInfo<T, I> {
    fn eq(&self, other: &Self) -> bool {
        self.digest == other.digest && self.shares == other.shares && self.variant == other.variant
    }
}

impl<T: Config<I>, I: 'static> Eq for EntryInfo<T, I> {}

impl<T: Config<I>, I: 'static> core::fmt::Debug for EntryInfo<T, I> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EntryInfo")
            .field("digest", &self.digest)
            .field("shares", &self.shares)
            .field("variant", &self.variant)
            .finish()
    }
}

impl<T: Config<I>, I: 'static> PartialEq for Entries<T, I> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: Config<I>, I: 'static> Eq for Entries<T, I> {}

impl<T: Config<I>, I: 'static> core::fmt::Debug for Entries<T, I> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Entries").field(&self.0).finish()
    }
}

impl<T: Config<I>, I: 'static> Clone for Entries<T, I> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: Config<I>, I: 'static> core::fmt::Debug for IndexInfo<T, I> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IndexInfo")
            .field("principal", &self.principal)
            .field("capital", &self.capital)
            .field("entries", &self.entries)
            .finish()
    }
}

impl<T: Config<I>, I: 'static> PartialEq for IndexInfo<T, I> {
    fn eq(&self, other: &Self) -> bool {
        self.principal == other.principal
            && self.capital == other.capital
            && self.entries == other.entries
    }
}

impl<T: Config<I>, I: 'static> Eq for IndexInfo<T, I> {}

impl<T: Config<I>, I: 'static> Clone for IndexInfo<T, I> {
    fn clone(&self) -> Self {
        Self {
            principal: self.principal,
            capital: self.capital,
            entries: self.entries.clone(),
        }
    }
}

impl<T: Config<I>, I: 'static> PartialEq for SlotInfo<T, I> {
    fn eq(&self, other: &Self) -> bool {
        self.digest == other.digest
            && self.shares == other.shares
            && self.commit == other.commit
            && self.variant == other.variant
    }
}

impl<T: Config<I>, I: 'static> Eq for SlotInfo<T, I> {}

impl<T: Config<I>, I: 'static> core::fmt::Debug for SlotInfo<T, I> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SlotInfo")
            .field("digest", &self.digest)
            .field("shares", &self.shares)
            .field("commit", &self.commit)
            .field("variant", &self.variant)
            .finish()
    }
}

impl<T: Config<I>, I: 'static> Clone for SlotInfo<T, I> {
    fn clone(&self) -> Self {
        Self {
            digest: self.digest.clone(),
            shares: self.shares,
            commit: self.commit.clone(),
            variant: self.variant.clone(),
        }
    }
}

impl<T: Config<I>, I: 'static> PartialEq for Slots<T, I> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: Config<I>, I: 'static> core::fmt::Debug for Slots<T, I> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Slots").field(&self.0.as_slice()).finish()
    }
}

impl<T: Config<I>, I: 'static> Eq for Slots<T, I> {}

impl<T: Config<I>, I: 'static> Clone for Slots<T, I> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: Config<I>, I: 'static> PartialEq for PoolInfo<T, I> {
    fn eq(&self, other: &Self) -> bool {
        self.balance_of == other.balance_of
            && self.capital == other.capital
            && self.commission == other.commission
            && self.slots == other.slots
    }
}

impl<T: Config<I>, I: 'static> Eq for PoolInfo<T, I> {}

impl<T: Config<I>, I: 'static> Clone for PoolInfo<T, I> {
    fn clone(&self) -> Self {
        Self {
            balance_of: self.balance_of.clone(),
            capital: self.capital,
            commission: self.commission,
            slots: self.slots.clone(),
        }
    }
}

impl<T: Config<I>, I: 'static> core::fmt::Debug for PoolInfo<T, I> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PoolInfo")
            .field("balance_of", &self.balance_of)
            .field("capital", &self.capital)
            .field("commission", &self.commission)
            .field("slots", &self.slots)
            .finish()
    }
}

impl<T: Config<I>, I: 'static> Clone for IndexOfReason<T, I> {
    fn clone(&self) -> Self {
        Self {
            reason: self.reason,
            index: self.index.clone(),
        }
    }
}

impl<T: Config<I>, I: 'static> Clone for PoolOfReason<T, I> {
    fn clone(&self) -> Self {
        Self {
            reason: self.reason,
            pool: self.pool.clone(),
        }
    }
}

impl<T: Config<I>, I: 'static> Clone for DigestVariant<T, I> {
    fn clone(&self) -> Self {
        match self {
            DigestVariant::Direct(d) => DigestVariant::Direct(d.clone()),
            DigestVariant::Index(d) => DigestVariant::Index(d.clone()),
            DigestVariant::Pool(d) => DigestVariant::Pool(d.clone()),
            DigestVariant::__Ignore(_) => {
                debug_assert!(false, "digest variant phantom variant accessed");
                DigestVariant::__Ignore(PhantomData)
            }
        }
    }
}

impl<T: Config<I>, I: 'static> PartialEq for DigestVariant<T, I> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (DigestVariant::Direct(a), DigestVariant::Direct(b)) => a == b,
            (DigestVariant::Index(a), DigestVariant::Index(b)) => a == b,
            (DigestVariant::Pool(a), DigestVariant::Pool(b)) => a == b,
            (DigestVariant::__Ignore(_), DigestVariant::__Ignore(_)) => {
                debug_assert!(false, "digest variant phantom variant accessed");
                true
            }
            (DigestVariant::__Ignore(_), _) => {
                debug_assert!(false, "digest variant phantom variant accessed");
                false
            }
            (_, DigestVariant::__Ignore(_)) => {
                debug_assert!(false, "digest variant phantom variant accessed");
                false
            }
            _ => false,
        }
    }
}

impl<T: Config<I>, I: 'static> Eq for DigestVariant<T, I> {}

impl<T: Config<I>, I: 'static> Debug for DigestVariant<T, I> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DigestVariant::Direct(d) => write!(f, "Direct({:?})", d),
            DigestVariant::Index(d) => write!(f, "Index({:?})", d),
            DigestVariant::Pool(d) => write!(f, "Pool({:?})", d),
            DigestVariant::__Ignore(_) => {
                debug_assert!(false, "digest variant phantom variant accessed");
                write!(f, "Invalid Digest Variant")
            }
        }
    }
}