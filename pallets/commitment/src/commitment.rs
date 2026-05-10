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
// ```````````````````````````` COMMITMENT TRAITS IMPL ```````````````````````````
// ===============================================================================

//! Implementation module of the [`Commitment Family`](frame_suite::commitment)
//! traits, where we utilize indexes, pools, and variants to create a flexible and
//! semantic commitment system.
//!
//! Low-level helper traits are defined in [`crate::traits`] within this crate. These
//! helpers provide fundamental functions that can be reused by other implementations
//! to offer a similar commitment system.
//!
//! The asset type is defined as the fungible trait's balance - i.e., a unit with
//! fungible behaviours. See the required trait bounds of [`Config::Asset`] to
//! understand which methods this system utilizes.
//!
//! Notably, this commitment system does not rely on standard balanced (safe) fungible methods.
//! Instead, it uses its own safe models via low-level methods provided by fungible traits.
//! Therefore, it does not query the total asset in circulation, as its scope is limited
//! to commitments for the particular asset holder.
//!
//! [`Pallet`] implements:
//! - [`InspectAsset`]
//! - [`DigestModel`]
//! - [`Commitment`]
//! - [`CommitIndex`]
//! - [`CommitPool`]
//! - [`CommitVariant`]
//! - [`IndexVariant`]
//! - [`PoolVariant`]
//! - and other helper traits include
//!     - [`CommitErrorHandler`]
//!
//! Local Tests for these traits are covered in `tests`.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{
    balance::*, traits::*, types::*, AssetToIssue, AssetToReap, CommitHelpers, CommitMap, Config,
    DigestMap, EntryMap, Error, Event, HoldReason, IndexMap, Pallet, PoolManager, PoolMap,
};

// --- Core ---
use core::cmp::Ordering;

// --- FRAME Suite ---
use frame_suite::{commitment::*, keys::*, misc::Extent};

// --- FRAME Support ---
use frame_support::{
    ensure,
    traits::{
        fungible::{Inspect, InspectHold},
        tokens::{Fortitude, Preservation},
    },
};

// --- Substrate primitives ---
use sp_core::Get;
use sp_runtime::{
    traits::{CheckedAdd, Saturating, Zero},
    DispatchError, DispatchResult, Vec,
};

// ===============================================================================
// ```````````````````````````````` INSPECT ASSET ````````````````````````````````
// ===============================================================================

/// Implements [`InspectAsset`] for the pallet, allowing the
/// commitment pallet to inspect a user's available funds
/// for commitment.
impl<T: Config<I>, I: 'static> InspectAsset<Proprietor<T>> for Pallet<T, I> {
    /// The asset type used in commitments, taken from the fungible `Inspect` trait.
    ///
    /// This allows any fungible implementation to be used as the commitment asset,
    /// providing flexibility in the type of value being committed while ensuring
    /// compatibility with the broader Substrate fungible ecosystem.
    type Asset = AssetOf<T, I>;

    /// Retrieves the total available funds for commitment for a given proprietor.
    ///
    /// Aggregates two balance sources:
    /// - Funds held under [`HoldReason::PrepareForCommit`] reason
    /// - Liquid balance reducible under [`Preservation::Preserve`] and
    /// [`Fortitude::Polite`] rules
    ///
    /// This aggregated view is used by commitment validation logic to determine whether
    /// a proprietor has sufficient funds to place, raise, or modify commitments.
    ///
    /// ## Returns
    /// `Asset` containing the total available balance
    fn available_funds(who: &Proprietor<T>) -> Self::Asset {
        let hold_reason: T::AssetHold = HoldReason::PrepareForCommit.into();

        // Funds specifically held for this commitment purpose.
        let held_balance = T::Asset::balance_on_hold(&hold_reason, who);

        // Liquid balance available for commitment.
        // We do not want the account to be dusted/killed/reaped.
        let liquid_balance =
            T::Asset::reducible_balance(who, Preservation::Preserve, Fortitude::Polite);

        held_balance.saturating_add(liquid_balance)
    }
}

// ===============================================================================
// ```````````````````````````````` DIGEST MODEL `````````````````````````````````
// ===============================================================================

/// Implements [`DigestModel`] for the pallet, allowing the system to
/// determine the specific model variant of a given digest.
///
/// Since all digests share the same base type, we need a wrapper
/// i.e., [`DigestVariant`] to distinguish between models such as Direct, Index, and Pool.
impl<T: Config<I>, I: 'static> DigestModel<Proprietor<T>> for Pallet<T, I> {
    /// The digest model type, wrapping the digest with its variant classification.
    ///
    /// This type distinguishes between three commitment models:
    /// - **Direct**: A standalone commitment to a specific digest
    /// - **Index**: A commitment distributed across multiple entries with weighted shares
    /// - **Pool**: A managed commitment structure with dynamic slot allocation and commission
    type Model = DigestVariant<T, I>;

    /// Determines which digest model a given digest belongs to.
    ///
    /// This method checks if the digest exists in each model variant in order:
    /// 1. Direct
    /// 2. Index
    /// 3. Pool
    ///
    /// If a matching model is found, it returns the wrapped variant.
    /// Otherwise, a DispatchError.
    ///
    /// Note: This method is **not suitable** for creating new digests (not
    /// registered in the system).
    ///
    /// New digests should be manually wrapped to avoid incorrect determination.
    fn determine_digest(
        digest: &Self::Digest,
        reason: &Self::Reason,
    ) -> Result<Self::Model, DispatchError> {
        if Self::digest_exists(reason, digest).is_ok() {
            return Ok(DigestVariant::Direct(digest.clone()));
        }

        if Self::index_exists(reason, digest).is_ok() {
            return Ok(DigestVariant::Index(digest.clone()));
        }

        if Self::pool_exists(reason, digest).is_ok() {
            return Ok(DigestVariant::Pool(digest.clone()));
        }

        Err(Error::<T, I>::DigestNotFoundToDetermine.into())
    }
}

// ===============================================================================
// ````````````````````````````````` COMMITMENT ``````````````````````````````````
// ===============================================================================

/// Implements the base [`Commitment`] trait for the pallet.
impl<T: Config<I>, I: 'static> Commitment<Proprietor<T>> for Pallet<T, I> {
    /// The source of a digest, used to determine the concrete digest.
    ///  
    /// In this implementation, the digest source is the the calling source
    /// i.e., [`frame_system::Config::AccountId`]. Must be using its account
    /// nonce for deterministic-randomness.
    ///
    /// This means all digests - whether direct, index, or pool - have a
    /// deterministic account ID generated by [`Commitment::gen_digest`] and similar
    /// methods.
    ///
    /// Internally, the methods enforces generating the [`Commitment::Digest`] same
    /// as the source type.
    type DigestSource = DigestSource<T>;

    /// The digest type used in this pallet, based on account ID type
    /// (from [`frame_system::Config::AccountId`]).
    ///
    /// This ensures all digests are tied to a consistent, predictable
    /// identity, same as every accounts. Much alike contract addresses.
    type Digest = Digest<T>;

    /// The reason associated with a commitment.
    ///
    /// This type is linked to the `Id` type of the `InspectFreeze` fungible trait,
    /// typically a composite enum constructed at runtime from all pallets
    /// that use fungible properties.
    ///
    /// Declaring `Reason` as the top-level key:
    /// - Encourages compile-time reasons for commitments.
    /// - Prevents creating new reasons at runtime without explicit intent.
    /// - Ensures other pallets adopt this commitment structure for consistent behavior.
    type Reason = CommitReason<T, I>;

    /// Commitment operation top level configuration.
    ///
    /// Enforces exactness and forcefullness towards a commit operation.
    type Intent = DispatchPolicy;

    /// Type representing the derived limits used for commitment validations.
    ///
    /// Encapsulates bounds (e.g. minimum, maximum, optimal) computed by the
    /// underlying balance model for deposit, mint, and reap operations.
    type Limits = LimitsProduct<T, I>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Checks whether a commitment exists for the given proprietor and reason.
    ///
    /// ## Returns
    /// - `Ok(())` if a commitment exists
    /// - `Err(DispatchError)` if no commitment exists
    fn commit_exists(who: &Proprietor<T>, reason: &Self::Reason) -> DispatchResult {
        ensure!(
            CommitMap::<T, I>::contains_key((who, reason)),
            Error::<T, I>::CommitNotFound
        );
        Ok(())
    }

    /// Checks whether a direct-digest exists for the given reason.
    ///
    /// This doesn't ensures existence for index or pool digests, as callers
    /// should use [`CommitIndex::index_exists`] or [`CommitPool::pool_exists`]
    ///
    /// ## Returns
    /// - `Ok(())` if the digest exists
    /// - `Err(DispatchError)` if the digest does not exist
    fn digest_exists(reason: &Self::Reason, digest: &Self::Digest) -> DispatchResult {
        ensure!(
            DigestMap::<T, I>::contains_key((reason, digest)),
            Error::<T, I>::DigestNotFound
        );
        Ok(())
    }

    /// Validates whether a new commitment can be placed with the
    /// variant's [`Config::Position`] default.
    ///
    /// This is a thin wrapper over [`Self::can_place_commit_of_variant`],
    /// using the default [`Config::Position`] for non-variant commitments.
    ///
    /// ## Returns
    /// - `Ok(())` if validation succeeds
    /// - `Err(DispatchError)` if validation fails
    #[inline]
    fn can_place_commit(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
        qualifier: &Self::Intent,
    ) -> DispatchResult {
        Self::can_place_commit_of_variant(
            who,
            reason,
            digest,
            &Default::default(),
            value,
            qualifier,
        )
    }

    /// Validates whether an existing commitment can be increased (raised).
    ///
    /// Same as the default trait validation, but extended with an additional
    /// check against the underlying balance model to ensure the deposit can
    /// actually be applied.
    ///
    /// In the lazy balance model, raising is equivalent to performing an
    /// additional deposit and the underlying system does not distinguish between
    /// placing and raising.
    ///
    /// ## Returns
    /// - `Ok(())` if validation succeeds
    /// - `Err(DispatchError)` if any constraint is violated
    fn can_raise_commit(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        value: Self::Asset,
        qualifier: &Self::Intent,
    ) -> DispatchResult {
        let digest = &Self::get_commit_digest(who, reason)?;
        let max = Self::available_funds(who);
        ensure!(max >= value, Error::<T, I>::InsufficientFunds);
        let variant = &Self::get_commit_variant(who, reason)?;
        // debug_assert!()
        let balance = DigestMap::<T, I>::get((reason, digest))
            .and_then(|digest_info| digest_info.get_balance(&variant).cloned())
            .unwrap_or_default();
        let limits = deposit_limits_of(&balance, &variant, digest, qualifier)?;
        ensure!(
            <Self::Limits as Extent>::contains(&limits, value),
            Error::<T, I>::PlacingOffLimits
        );
        can_deposit(&balance, variant, digest, &value, qualifier)
    }

    /// Validates whether a commitment can be resolved.
    ///
    /// Extends the default trait behavior by additionally validating that all
    /// underlying balances can support the required withdrawals.
    ///
    /// Since each digest model maintains balances differently:
    /// - **Direct / Index**: withdrawals are validated directly against their balances
    /// - **Pool**: validation is performed against the pool's current (unadjusted)
    ///   aggregate balance, without applying intermediate slot updates
    ///
    /// This is a lightweight validation step; full consistency (including pool
    /// rebalancing) is enforced during the actual resolution operation.
    ///
    /// ## Returns
    /// - `Ok(())` if all withdrawals are valid
    /// - `Err(DispatchError)` if any withdrawal is invalid
    fn can_resolve_commit(who: &Proprietor<T>, reason: &Self::Reason) -> DispatchResult {
        let digest = &Self::get_commit_digest(who, reason)?;
        let digest_model = &Self::determine_digest(digest, reason)?;
        // debug_assert!()
        match digest_model {
            DigestVariant::Direct(direct) => {
                let variant = &Self::get_commit_variant(who, reason)?;
                // debug_assert!()
                let digest_info = DigestMap::<T, I>::get((reason, direct))
                    .ok_or(Error::<T, I>::DigestNotFound)?;
                // debug_assert!()
                let balance = digest_info
                    .get_balance(variant)
                    // debug_assert!()
                    .ok_or(Error::<T, I>::DigestVariantBalanceNotFound)?;
                let commit_info =
                    CommitMap::<T, I>::get((who, reason)).ok_or(Error::<T, I>::CommitNotFound)?;
                // debug_assert!()
                for commit in commit_info.commits() {
                    can_withdraw(&balance, variant, digest, &commit)?;
                }
            }
            DigestVariant::Index(index) => {
                let index_info = Self::get_index(reason, index)?;
                // debug_assert!()
                for entry in index_info.entries() {
                    let digest = &entry.digest();
                    let Some(commits) = EntryMap::<T, I>::get((reason, index, digest, who)) else {
                        // If Zero Amount Depositted due to low shares
                        continue;
                    };
                    let variant = &entry.variant();
                    let digest_info = DigestMap::<T, I>::get((reason, digest))
                        // debug_assert!()
                        .ok_or(Error::<T, I>::EntryDigestNotFound)?;
                    let balance = digest_info
                        .get_balance(variant)
                        // debug_assert!()
                        .ok_or(Error::<T, I>::DigestVariantBalanceNotFound)?;
                    for commit in commits.commits() {
                        can_withdraw(&balance, variant, digest, &commit)?;
                    }
                }
            }
            DigestVariant::Pool(pool) => {
                let pool_info = Self::get_pool(reason, pool)?;
                // debug_assert!()
                let balance = pool_info.balance();
                let commit_info =
                    CommitMap::<T, I>::get((who, reason)).ok_or(Error::<T, I>::CommitNotFound)?;
                // debug_assert!()
                for commit in commit_info.commits() {
                    can_withdraw(&balance, &Default::default(), pool, &commit)?;
                }
            }
            _ => {
                debug_assert!(
                    false,
                    "digest-model marker variants {:?} are constructed, 
                    captured during can withdraw validation proprietor {:?} 
                    of reason {:?} are explicitly dis-allowed",
                    digest_model, who, reason
                );
                return Err(Error::<T, I>::InvalidDigestModel.into());
            }
        }
        Ok(())
    }

    /// Validates whether a digest's value can be set using the default variant.
    ///
    /// This is a thin wrapper over [`Self::can_set_digest_variant_value`],
    /// using the default [`Config::Position`] for single non-variant commitments.
    ///
    /// ## Returns
    /// - `Ok(())` if validation succeeds
    /// - `Err(DispatchError)` if validation fails
    #[inline]
    fn can_set_digest_value(
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
        qualifier: &Self::Intent,
    ) -> DispatchResult {
        Self::can_set_digest_variant_value(reason, digest, value, &Default::default(), qualifier)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Retrieves the digest associated with a proprietor's commitment.
    ///
    /// Since [`Commitment::Digest`] is opaque to identify as direct or index
    /// or pool. This function can return any digest model which later can be
    /// determined using [`DigestModel::determine_digest`].
    ///
    /// Since each reason can only have one active digest per proprietor,
    /// this directly returns the commitment's digest.
    ///
    /// ## Returns
    /// - `Ok(Digest)` containing the commitment's digest
    /// - `Err(DispatchError)` if no commitment exists
    fn get_commit_digest(
        who: &Proprietor<T>,
        reason: &Self::Reason,
    ) -> Result<Self::Digest, DispatchError> {
        let commit_info =
            CommitMap::<T, I>::get((who, reason)).ok_or(Error::<T, I>::CommitNotFound)?;
        let digest = commit_info.digest();
        debug_assert!(
            // cannot do `digest_exists` as this can get called by any digest model
            // `determine_digest` holds all checks
            Self::determine_digest(&digest, reason).is_ok(),
            "commit-exists for reason {:?} of digest {:?} for proprietor {:?}, 
            but internally digest doesn't really exist",
            reason,
            digest,
            who
        );
        Ok(digest)
    }

    /// Retrieves the total committed value across all proprietors for a reason.
    ///
    /// ## Returns
    /// - `Asset` containing the total committed value, or zero if unavailable
    fn get_total_value(reason: &Self::Reason) -> Self::Asset {
        CommitHelpers::<T, I>::value_of(None, reason).unwrap_or(Zero::zero())
    }

    /// Retrieves the real-time committed value for a specific proprietor and reason.
    ///
    /// This value reflects the current state including any applied rewards or penalties.
    /// Since each proprietor can only have one active digest per reason, this returns
    /// the aggregate value across all commitment instances for that digest.
    ///
    /// Internally, raising commits doesn't mutate an existing commitment-balance but
    /// instead accmulate new immutable balances as commit-instances which will be
    /// aggregated.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the proprietor's current committed value
    /// - `Err(DispatchError)` if no commitment exists
    fn get_commit_value(
        who: &Proprietor<T>,
        reason: &Self::Reason,
    ) -> Result<Self::Asset, DispatchError> {
        let digest = Self::get_commit_digest(who, reason)?;
        let digest_model = Self::determine_digest(&digest, reason);
        debug_assert!(
            digest_model.is_ok(),
            "proprietor {:?} commit-exists in digest {:?} for reason {:?}, 
            but its model cannot be determined",
            who,
            digest,
            reason
        );
        let digest_model = digest_model?;
        CommitHelpers::<T, I>::commit_value_of(who, reason, &digest_model)
    }

    /// Retrieves the real-time total value of a specific direct-digest's
    /// default variant of [`Config::Position`] for a given reason.
    ///
    /// This doesn't provides value for index or pool digests, as callers
    /// should use [`CommitIndex::get_index_value`] or [`CommitPool::get_pool_value`]
    ///
    /// Aggregates all commitments across all proprietors who committed
    /// funds to this digest's default variant.
    ///
    /// This method delagates itself to [`CommitVariant::get_digest_variant_value`]
    /// with the default position variant.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the digest's total value
    /// - `Err(DispatchError)` if the digest does not exist
    #[inline]
    fn get_digest_value(
        reason: &Self::Reason,
        digest: &Self::Digest,
    ) -> Result<Self::Asset, DispatchError> {
        Self::get_digest_variant_value(reason, digest, &T::Position::default())
    }

    /// Derives place commit limits for the given params of the
    /// default commit-variant.
    ///
    /// This is a convenience wrapper over [`Self::place_commit_limits_of_variant`],
    /// using the default [`Config::Position`] for non-variant commitments.
    ///
    /// ## Returns
    /// - `Ok(Limits)` containing the derived constraints
    /// - `Err(DispatchError)` if the derivation fails
    #[inline]
    fn place_commit_limits(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        digest: &Self::Digest,
        qualifier: &Self::Intent,
    ) -> Result<Self::Limits, DispatchError> {
        Self::place_commit_limits_of_variant(who, reason, digest, &Default::default(), qualifier)
    }

    /// Derives limits for increasing (raising) an existing commitment under
    /// the default commit-variant.
    ///
    /// Resolves the commit's associated digest and variant for the given
    /// proprietor and reason, then delegates to
    /// [`Self::place_commit_limits_of_variant`].
    ///
    /// In the lazy balance model, raising is equivalent to performing an
    /// additional deposit on an existing commitment, so the same limit
    /// derivation logic applies.
    ///
    /// The `qualifier` influences how limits are derived.
    ///
    /// ## Returns
    /// - `Ok(Limits)` containing the derived constraints
    /// - `Err(DispatchError)` if the commitment does not exist or derivation fails
    fn raise_commit_limits(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        qualifier: &Self::Intent,
    ) -> Result<Self::Limits, DispatchError> {
        let digest = Self::get_commit_digest(who, reason)?;
        let variant = Self::get_commit_variant(who, reason)?;
        // debug_assert!()
        Self::place_commit_limits_of_variant(who, reason, &digest, &variant, qualifier)
    }

    /// Derives minting limits for a digest using the default variant.
    ///
    /// This is a convenience wrapper over [`Self::digest_mint_limits_of_variant`],
    /// using the default [`Config::Position`] for single non-variant digests.
    ///
    /// The `qualifier` influences how limits are derived (e.g. strict vs relaxed).
    ///
    /// ## Returns
    /// - `Ok(Limits)` containing the derived minting constraints
    /// - `Err(DispatchError)` if the derivation fails
    #[inline]
    fn digest_mint_limits(
        digest: &Self::Digest,
        reason: &Self::Reason,
        qualifier: &Self::Intent,
    ) -> Result<Self::Limits, DispatchError> {
        Self::digest_mint_limits_of_variant(digest, reason, &Default::default(), qualifier)
    }

    /// Derives reaping limits for a digest using the default variant.
    ///
    /// This is a convenience wrapper over [`Self::digest_reap_limits_of_variant`],
    /// using the default [`Config::Position`] for single non-variant digests.
    ///
    /// The `qualifier` influences how limits are derived (e.g. strict vs relaxed).
    ///
    /// ## Returns
    /// - `Ok(Limits)` containing the derived reaping constraints
    /// - `Err(DispatchError)` if the derivation fails
    #[inline]
    fn digest_reap_limits(
        digest: &Self::Digest,
        reason: &Self::Reason,
        qualifier: &Self::Intent,
    ) -> Result<Self::Limits, DispatchError> {
        Self::digest_reap_limits_of_variant(digest, reason, &Default::default(), qualifier)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` CONSTRUCTORS ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Generates a unique digest identifier from the given source.
    ///
    /// Uses the account nonce as a salt to ensure uniqueness across multiple
    /// digest generations for the same source.
    ///
    /// Utilizes [`KeyGenFor`] trait implementation via [`KeySeedFor`]
    ///
    /// ## Returns
    /// - `Ok(Digest)` containing the generated digest
    /// - `Err(DispatchError)` if digest generation fails
    fn gen_digest(source: &DigestSource<T>) -> Result<Self::Digest, DispatchError> {
        let target = Into::<&Self::Digest>::into(source);

        // Retrieve account nonce from the system pallet as a salt
        let salt = frame_system::Pallet::<T>::account_nonce(source);

        // Generate a digest key with salt
        let key =
            KeySeedFor::<Self::Digest, (), T::Nonce, T::Hashing, T>::gen_key(target, &(), salt)
                .ok_or(Error::<T, I>::CannotGenerateDigest)?;

        Ok(key)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Places a commitment with the variant's [`Config::Position`] default.
    ///
    /// This method delagates itself to [`CommitVariant::place_commit_of_variant`]
    /// with the default position variant.
    ///
    /// For detailed information on how placing a commitment works, refer to the
    /// called implemented method [`CommitVariant::place_commit_of_variant`].
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the actual committed amount
    /// - `Err(DispatchError)` if placement fails
    fn place_commit(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
        qualifier: &Self::Intent,
    ) -> Result<Self::Asset, DispatchError> {
        Self::place_commit_of_variant(
            who,
            reason,
            digest,
            value,
            &T::Position::default(),
            qualifier,
        )
    }

    /// Resolves and withdraws a commitment for the given proprietor and reason.
    ///
    /// Calculates the final value including any rewards or penalties, unfreezes
    /// the committed assets, and returns them to the owner (authorized-caller).
    ///
    /// The commitment record is removed upon successful resolution.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the resolved amount returned to the proprietor
    /// - `Err(DispatchError)` if no commitment exists or resolution fails
    fn resolve_commit(
        who: &Proprietor<T>,
        reason: &Self::Reason,
    ) -> Result<Self::Asset, DispatchError> {
        let digest = Self::get_commit_digest(who, reason)?;
        let digest_model = Self::determine_digest(&digest, reason);
        debug_assert!(
            digest_model.is_ok(),
            "proprietor {:?} commit-exists in digest {:?} of reason {:?}, 
            but its model cannot be determined",
            who,
            digest,
            reason
        );
        let digest_model = digest_model?;
        let resolved = CommitHelpers::<T, I>::resolve_commit_of(who, reason, &digest_model)?;
        Self::on_commit_resolve(who, reason, &digest, resolved);
        Ok(resolved)
    }

    /// Raises a commitment for the given proprietor and reason.
    ///
    /// Enforces that the proprietor must already have an active
    /// commitment for the given reason.
    ///
    /// Does not allow zero-value marker commitments.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the raised amount
    /// - `Err(DispatchError)` if no existing commitment is found or funds are insufficient
    fn raise_commit(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        value: Self::Asset,
        qualifier: &Self::Intent,
    ) -> Result<Self::Asset, DispatchError> {
        Self::commit_exists(who, reason)?;
        ensure!(!value.is_zero(), Error::<T, I>::MarkerCommitNotAllowed);
        let digest = Self::get_commit_digest(who, reason)?;
        let digest_model = Self::determine_digest(&digest, reason);
        debug_assert!(
            digest_model.is_ok(),
            "proprietor {:?} commit-exists in digest {:?} for reason {:?}, 
            but its model cannot be determined",
            who,
            digest,
            reason
        );
        let digest_model = digest_model?;
        let raised =
            CommitHelpers::<T, I>::raise_commit_of(who, reason, &digest_model, value, qualifier)?;
        Self::on_commit_raise(who, reason, &digest, raised);
        Ok(raised)
    }

    /// Sets a direct value on a digest, typically for applying rewards/inflation or
    /// penalties/deflation.
    ///
    /// **Note**: This function operates at the low-level commitment for a direct-digest. It
    /// cannot be used to directly apply rewards or penalties to indexes or pools, because
    /// those maintain their balances at a higher level through their entries and slots digests.
    ///
    /// Any value adjustment for indexes or pools should propagate via their underlying
    /// sub-systems if applicable.
    ///
    /// Internally calls [`CommitVariant::set_digest_variant_value`] with the default
    /// variant of [`Config::Position`].
    ///
    /// The `qualifier` influences how the adjustment (via the given direction which
    /// could be increase or decrease from current digest value) is applied and may
    /// affect the final value that is actually set.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the resulting value of the digest after update
    /// - `Err(DispatchError)` if the operation fails
    #[inline]
    fn set_digest_value(
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
        qualifier: &Self::Intent,
    ) -> Result<Self::Asset, DispatchError> {
        Self::set_digest_variant_value(reason, digest, value, &T::Position::default(), qualifier)
    }

    /// Removes a digest from storage after ensuring it contains no active deposits.
    ///
    /// This operation is only allowed when the digest's balance has no remaining
    /// deposits (i.e., no claimable commitments exist).
    ///
    /// **Note**: This function operates for direct digests only. It cannot be used
    /// to reap indexes or pools.
    ///
    /// Any residual value ("dust") left after all deposits are withdrawn is treated
    /// as unclaimable and reaped-accounted in [`AssetToReap`], deducted from total
    /// committed value [`crate::ReasonValue`], and considered effectively dead.
    ///
    /// The digest itself can be recreated later if a new deposit is made via
    /// [`CommitDeposit::deposit_to_digest`].
    ///
    /// ## Returns
    /// - `Ok(())` if the digest is successfully removed
    /// - `Err(DispatchError)` with `DigestHasFunds` if active deposits still exist
    fn reap_digest(digest: &Self::Digest, reason: &Self::Reason) -> DispatchResult {
        let digest_info =
            DigestMap::<T, I>::get((reason, digest)).ok_or(Error::<T, I>::DigestNotFound)?;
        let balances = digest_info.balances()?;
        let mut reap = true;
        let mut remaining = Zero::zero();
        for (variant, balance) in &balances {
            if has_deposits(balance, variant, digest).is_ok() {
                reap = false;
                break;
            }
            remaining = balance_total(balance, variant, digest)?;
        }
        // Cannot reap a digest with remaining funds
        ensure!(reap, Error::<T, I>::DigestHasFunds);

        // If unerlying balance system allows residue/dust after
        // full-withdrawal due to rounding/precision and other drifts.
        if !remaining.is_zero() {
            // Residue as it will never be claimed, but maintained for equillibrium
            // Considered dead inside commitment system, and never will be able to
            // resolved in the underlying asset (fungible) system
            AssetToReap::<T, I>::mutate(|total_to_reap| -> DispatchResult {
                *total_to_reap = total_to_reap
                    .checked_add(&remaining)
                    .ok_or(Error::<T, I>::MaxAssetReaped)?;
                Ok(())
            })?;
            // Subtract reason's total committed value since value is deflated
            CommitHelpers::<T, I>::sub_from_total_value(reason, remaining)?;
        }

        // Called earlier to determine reaped digest model.
        Self::on_reap_digest(digest, reason, remaining);

        // Remove the digest from storage
        DigestMap::<T, I>::remove((reason, digest));
        Ok(())
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Hook called when a commitment is placed.
    ///
    /// Delagates itself to [`CommitVariant::on_place_commit_on_variant`]
    /// with the default position variant.
    #[inline]
    fn on_commit_place(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
    ) {
        Self::on_place_commit_on_variant(who, reason, digest, value, &Default::default());
    }

    /// Hook called when a commitment is raised.
    ///
    /// The digest is verified and classified using
    /// [`DigestModel::determine_digest`].
    ///
    /// Emits [`Event::CommitRaised`] event if
    /// [`Config::EmitEvents`] is `true`.
    fn on_commit_raise(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
    ) {
        if T::EmitEvents::get() {
            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            {
                let Ok(digest_model) = Self::determine_digest(digest, reason) else {
                    return;
                };
                Self::deposit_event(Event::<T, I>::CommitRaised {
                    who: who.clone(),
                    reason: *reason,
                    model: digest_model,
                    value,
                });
            }

            #[cfg(not(any(feature = "dev", feature = "runtime-benchmarks")))]
            {
                Self::deposit_event(Event::<T, I>::CommitRaised {
                    who: who.clone(),
                    reason: *reason,
                    digest: digest.clone(),
                    value,
                });
            }
        }
    }

    /// Hook called when a commitment is resolved.
    ///
    /// The digest is verified and classified using
    /// [`DigestModel::determine_digest`].
    ///
    /// Emits [`Event::CommitResolved`] event if
    /// [`Config::EmitEvents`] is `true`.
    fn on_commit_resolve(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
    ) {
        if T::EmitEvents::get() {
            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            {
                let Ok(digest_model) = Self::determine_digest(digest, reason) else {
                    return;
                };
                Self::deposit_event(Event::<T, I>::CommitResolved {
                    who: who.clone(),
                    reason: *reason,
                    model: digest_model,
                    value,
                });
            }

            #[cfg(not(any(feature = "dev", feature = "runtime-benchmarks")))]
            {
                Self::deposit_event(Event::<T, I>::CommitResolved {
                    who: who.clone(),
                    reason: *reason,
                    digest: digest.clone(),
                    value,
                });
            }
        }
    }

    /// Hook called when a digest value is updated.
    ///
    /// This method delagates itself to [`CommitVariant::on_set_digest_variant`]
    /// with the default position variant.
    #[inline]
    fn on_digest_update(digest: &Self::Digest, reason: &Self::Reason, value: Self::Asset) {
        Self::on_set_digest_variant(digest, reason, value, &Default::default());
    }

    /// Hook called when a digest is successfully reaped.
    ///
    /// Emits [`Event::DigestReaped`] event if [`Config::EmitEvents`] is `true`.
    fn on_reap_digest(digest: &Self::Digest, reason: &Self::Reason, dust: Self::Asset) {
        if T::EmitEvents::get() {
            // Emit the DigestReaped event
            Self::deposit_event(Event::<T, I>::DigestReaped {
                digest: digest.clone(),
                reason: *reason,
                dust,
            });
        }
    }
}

// ===============================================================================
// ```````````````````````````````` COMMIT INDEX `````````````````````````````````
// ===============================================================================

/// Implements [`CommitIndex`] for the pallet
impl<T: Config<I>, I: 'static> CommitIndex<Proprietor<T>> for Pallet<T, I> {
    /// Index struct representing an index's internal structure.
    ///
    /// This type holds all entries and their associated shares, used for commitment
    /// calculations and value aggregations within the index.
    type Index = IndexInfo<T, I>;

    /// Shares unit defining an entry's proportional weight within the index.
    ///
    /// Shares represent each entry's relative contribution to the index capital,
    /// used for calculating value distributions and reward/penalty allocations.
    type Shares = T::Shares;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Checks whether a specific index exists for the given reason.
    ///
    /// ## Returns
    /// - `Ok(())` if the index exists
    /// - `Err(DispatchError)` if the index does not exist
    fn index_exists(reason: &Self::Reason, index_of: &Self::Digest) -> DispatchResult {
        ensure!(
            IndexMap::<T, I>::contains_key((reason, index_of)),
            Error::<T, I>::IndexNotFound
        );
        Ok(())
    }

    /// Checks whether a specific entry exists within a given index.
    ///
    /// ## Returns
    /// - `Ok(())` if the entry exists
    /// - `Err(DispatchError)` if the entry does not exist
    fn entry_exists(
        reason: &Self::Reason,
        index_of: &Self::Digest,
        entry_of: &Self::Digest,
    ) -> DispatchResult {
        let index = Self::get_index(reason, index_of)?;
        for entry in index.entries() {
            if *entry_of == entry.digest() {
                return Ok(());
            }
        }
        Err(Error::<T, I>::EntryOfIndexNotFound.into())
    }

    /// Checks whether any index exists for the given reason.
    ///
    /// ## Returns
    /// - `Ok(())` if at least one index exists
    /// - `Err(DispatchError)` if no indexes exist
    fn has_index(reason: &Self::Reason) -> DispatchResult {
        ensure!(
            IndexMap::<T, I>::iter_prefix((reason,)).next().is_some(),
            Error::<T, I>::IndexNotFound
        );
        Ok(())
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Retrieves the index information (meta-struct) for a given
    /// reason and index digest.
    ///
    /// ## Returns
    /// - `Ok(Index)` containing the index structure
    /// - `Err(DispatchError)`if the index does not exist
    fn get_index(
        reason: &Self::Reason,
        index_of: &Self::Digest,
    ) -> Result<Self::Index, DispatchError> {
        let index =
            IndexMap::<T, I>::get((reason, index_of)).ok_or(Error::<T, I>::IndexNotFound)?;
        Ok(index)
    }

    /// Retrieves all entry digests and their shares for a specific index.
    ///
    /// ## Returns
    /// - `Ok(Vec<(Digest, Shares)>)` containing each entry's digest and shares
    /// - `Err(DispatchError)` if the index does not exist
    fn get_entries_shares(
        reason: &Self::Reason,
        index_of: &Self::Digest,
    ) -> Result<Vec<(Self::Digest, Self::Shares)>, DispatchError> {
        let mut vec = Vec::new();
        let index = Self::get_index(reason, index_of)?;
        for entry in index.entries() {
            let shares = entry.shares();
            vec.push((entry.digest(), shares));
        }
        Ok(vec)
    }

    /// Computes the aggregated real-time value of a specific entry
    /// across all proprietors.
    ///
    /// Only includes commitments made via this index, not direct commitments
    /// to the entry digest.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the total committed value for the entry
    /// - `Err(DispatchError)` if the entry does not exist
    fn get_entry_value(
        reason: &Self::Reason,
        index_of: &Self::Digest,
        entry_of: &Self::Digest,
    ) -> Result<Self::Asset, DispatchError> {
        Self::entry_exists(reason, index_of, entry_of)?;
        let iter = EntryMap::<T, I>::iter_prefix((reason, index_of, entry_of));
        let mut actual = Self::Asset::zero();
        for (who, _) in iter {
            let value =
                CommitHelpers::<T, I>::index_entry_commit_value(&who, reason, index_of, entry_of)?;
            actual = actual.saturating_add(value);
        }
        Ok(actual)
    }

    /// Retrieves the real-time committed value of a specific entry
    /// for a given proprietor via an index.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the proprietor's committed value for the entry
    /// - `Err(DispatchError)` if the entry does not exist
    fn get_entry_value_for(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        index_of: &Self::Digest,
        entry_of: &Self::Digest,
    ) -> Result<Self::Asset, DispatchError> {
        let digest = Self::get_commit_digest(who, reason)?;
        Self::entry_exists(reason, index_of, entry_of)?;
        ensure!(digest == *index_of, Error::<T, I>::CommitNotFoundForEntry);
        let value =
            CommitHelpers::<T, I>::index_entry_commit_value(who, reason, index_of, entry_of)?;
        Ok(value)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` CONSTRUCTORS ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Generates a unique digest for the given index using the proprietor and reason.
    ///
    /// ## Returns
    /// - `Ok(Digest)` with the newly generated digest.
    /// - `Err(DispatchError)` if digest generation fails
    fn gen_index_digest(
        from: &Proprietor<T>,
        reason: &Self::Reason,
        index: &Self::Index,
    ) -> Result<Self::Digest, DispatchError> {
        let target = from;
        let salt = frame_system::Pallet::<T>::account_nonce(from);
        let key_gen_item = IndexOfReason::<T, I>::new(*reason, index.clone());

        let key =
            KeySeedFor::<Self::Digest, IndexOfReason<T, I>, T::Nonce, T::Hashing, T>::gen_key(
                target,
                &key_gen_item,
                salt,
            )
            .ok_or(Error::<T, I>::CannotGenerateDigest)?;

        Ok(key)
    }

    /// Prepares a new index instance from a list of entry digests and
    /// their corresponding shares.
    ///
    /// This function does **not** associate the index with a specific reason or
    /// proprietor internally. The caller is responsible for ensuring the index
    /// is correctly attached to a reason and, optionally, the creator.
    ///
    /// Entries with zero shares are silently ignored, as they carry no
    /// semantic contribution to the index.
    ///
    /// Entry digests are not validated to be direct digests. If a commitment
    /// is placed on the index, each entry digest will be funded accordingly
    /// through the normal deposit routing.
    ///
    /// Nested cases-such as index entries referencing other indexes or pools-
    /// are not supported and may be treated as new direct digests when routed
    /// through [`CommitDeposit::deposit_to_digest`]. Callers are responsible
    /// for validating such cases if required.
    ///
    /// - `who`: The proprietor creating the index.
    /// - `reason`: The reason under which the index is being prepared.
    ///
    /// ## Returns
    /// - `Ok(Index)` containing the prepared index
    /// - `Err(DispatchError)` if preparation fails
    fn prepare_index(
        _who: &Proprietor<T>,
        _reason: &Self::Reason,
        entries: &[(Self::Digest, Self::Shares)],
    ) -> Result<Self::Index, DispatchError> {
        // Initialize a new Entries collection for the index
        let mut entries_of = Vec::new();
        for (digest, shares) in entries {
            // Silently ignore non-share allocated entries
            if shares.is_zero() {
                continue;
            }
            // Create a new entry with the given variant
            let entry_info = EntryInfo::<T, I>::new(digest.clone(), *shares, Default::default())?;
            // Add entry to the index, checking for maximum capacity
            entries_of.push(entry_info);
        }
        // Construct the final IndexInfo object
        let index_info = IndexInfo::<T, I>::new(&mut Entries::<T, I>::new(entries_of)?)?;
        Ok(index_info)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Sets a new index for the given digest at the given reason.
    ///
    /// - The caller must ensure that the provided digest is **unique** i.e.,
    /// generated via [`CommitIndex::gen_index_digest`] and does not collide
    /// with existing indexes in the system.
    /// - This function is intended **only** for creating new indexes.  
    /// - Mutations to existing indexes are **not supported** here.  
    /// - For updating an index, a new index digest should be generated via
    /// [`CommitIndex::set_entry_shares`]
    ///
    /// ## Returns
    /// - `Ok(())` if the index was successfully inserted
    /// - `Err(DispatchError)` with `IndexDigestTaken` if the digest already exists
    fn set_index(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        index: &Self::Index,
        digest: &Self::Digest,
    ) -> DispatchResult {
        // Ensure the digest does not already exist
        ensure!(
            !Self::index_exists(reason, digest).is_ok(),
            Error::<T, I>::IndexDigestTaken
        );
        // Insert the new index into the storage map
        IndexMap::<T, I>::insert((reason, digest), index);
        Self::on_set_index(who, digest, reason, index);
        Ok(())
    }

    /// Updates or sets the shares for a specific entry of
    /// an index, producing a new index.
    ///
    /// - If the entry already exists in the index:
    ///   - If `shares` is zero, the entry is removed.
    ///   - Otherwise, the entry's shares are updated while preserving its existing variant.
    /// - If the entry does not exist:
    ///   - If `shares` is zero, the operation is a no-op and the original index is returned.
    ///   - Otherwise, a new entry is added with the default [`Config::Position`] variant.
    ///
    /// The newly added entry digest is not validated to be a direct digest and is
    /// accepted as provided through this function. If a commitment is placed on the
    /// index, it will be funded accordingly through normal deposit routing.
    ///
    /// Nested cases-such as entries referencing other indexes or pools-
    /// are not supported and may be treated as new direct digests when routed
    /// through [`CommitDeposit::deposit_to_digest`]. Callers are responsible
    /// for validating such cases if required.
    ///
    /// ## Returns
    /// - `Ok(Digest)` containing the resulting index digest (may be unchanged if no-op)
    /// - `Err(DispatchError)` if the operation fails
    fn set_entry_shares(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        index_of: &Self::Digest,
        entry_of: &Self::Digest,
        shares: Self::Shares,
    ) -> Result<Self::Digest, DispatchError> {
        match Self::entry_exists(reason, index_of, entry_of).is_ok() {
            true => {
                if shares.is_zero() {
                    return CommitHelpers::<T, I>::remove_index_entry(
                        who, reason, index_of, entry_of,
                    );
                }
                let variant = &Self::get_entry_variant(reason, index_of, entry_of)?;
                // debug_assert!()
                CommitHelpers::<T, I>::set_index_entry(
                    who, reason, index_of, entry_of, shares, variant,
                )
            }
            false => {
                if shares.is_zero() {
                    return Ok(index_of.clone());
                }
                CommitHelpers::<T, I>::set_index_entry(
                    who,
                    reason,
                    index_of,
                    entry_of,
                    shares,
                    &Default::default(),
                )
            }
        }
    }

    /// Removes an index if all entries have no committed funds.
    ///
    /// ## Returns
    /// - `Ok(())` if the index was successfully removed
    /// - `Err(DispatchError)` with `IndexHasFunds` if any entry still has commitments
    fn reap_index(reason: &Self::Reason, index_of: &Self::Digest) -> DispatchResult {
        let index = Self::get_index(reason, index_of)?;

        ensure!(index.principal().is_zero(), Error::<T, I>::IndexHasFunds);

        // Check that all entries are empty; otherwise, cannot reap
        for entry in index.entries() {
            let digest = &entry.digest();
            let mut iter = EntryMap::<T, I>::iter_prefix((reason, index_of, digest));
            if let Some((_, _)) = iter.next() {
                debug_assert!(
                    false,
                    "index {:?} of reason {:?} top-level principal 
                    does not have deposits but its entry-map has commits 
                    for entry {:?} found during reap-index attempt",
                    index_of, reason, digest
                );
                return Err(Error::<T, I>::IndexHasFunds.into());
            }
        }

        // Remove the index after all checks pass
        IndexMap::<T, I>::remove((reason, index_of));

        // Clean removal since entry-digests are funded as direct-digests
        // Just that commits are stored in a different layout
        Self::on_reap_index(index_of, reason, Zero::zero());
        Ok(())
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Emits an event when an index is created.
    ///
    /// - Records the index digest, reason, and entry-share mappings.
    /// - Emits [`Event::IndexInitialized`] or [`Event::IndexInitialized`]
    ///   depending on whether multiple variants are supported by [`Config::Position`]
    /// - Emits these events only if [`Config::EmitEvents`] is `true`.
    fn on_set_index(
        _who: &Proprietor<T>,
        index_of: &Self::Digest,
        reason: &Self::Reason,
        _index: &Self::Index,
    ) {
        if T::EmitEvents::get() {
            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            {
                let index = _index;
                let mut entries = Vec::new();
                for entry in index.entries() {
                    let digest = entry.digest().clone();
                    let shares = entry.shares();
                    let variant = &entry.variant();
                    entries.push((digest, shares, variant.clone()));
                }

                Self::deposit_event(Event::<T, I>::IndexInitialized {
                    index_of: index_of.clone(),
                    reason: *reason,
                    entries,
                })
            }

            #[cfg(not(any(feature = "dev", feature = "runtime-benchmarks")))]
            {
                Self::deposit_event(Event::<T, I>::IndexInitialized {
                    index_of: index_of.clone(),
                    reason: *reason,
                })
            }
        }
    }

    /// Emits an event when an index is successfully reaped.
    ///
    /// Emits a [`Event::IndexReaped`] event if [`Config::EmitEvents`] is `true`.
    fn on_reap_index(index_of: &Self::Digest, reason: &Self::Reason, dust: Self::Asset) {
        debug_assert!(
            dust.is_zero(),
            "index digest {:?} of reason {:?} reaped with non-zero dust {:?}",
            index_of,
            reason,
            dust
        );
        if T::EmitEvents::get() {
            Self::deposit_event(Event::<T, I>::IndexReaped {
                index_of: index_of.clone(),
                reason: *reason,
            });
        }
    }
}

// ===============================================================================
// ````````````````````````````````` COMMIT POOL `````````````````````````````````
// ===============================================================================

/// Implements [`CommitPool`] for the pallet
impl<T: Config<I>, I: 'static> CommitPool<Proprietor<T>> for Pallet<T, I> {
    /// Pool struct representing a pool's internal structure.
    ///
    /// Contains the pool's balance, capital, slots, commission rate, and other
    /// metadata required for pool operations and value calculations.
    type Pool = PoolInfo<T, I>;

    /// Commission rate for the pool.
    ///
    /// Represents the fraction of withdrawals collected by the pool manager
    /// as compensation for managing the pool.
    type Commission = T::Commission;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Checks whether a pool exists for the given reason and digest.
    ///
    /// ## Returns
    /// - `Ok(())` if the pool exists
    /// - `Err(DispatchError)` if the pool does not exist
    fn pool_exists(reason: &Self::Reason, pool_of: &Self::Digest) -> DispatchResult {
        ensure!(
            PoolMap::<T, I>::contains_key((reason, pool_of)),
            Error::<T, I>::PoolNotFound
        );
        Ok(())
    }

    /// Checks whether a specific slot exists within a given pool.
    ///
    /// ## Returns
    /// - `Ok(())` if the slot exists
    /// - `Err(DispatchError)` with `SlotOfPoolNotFound` if the slot does not exist
    fn slot_exists(
        reason: &Self::Reason,
        pool_of: &Self::Digest,
        slot_of: &Self::Digest,
    ) -> DispatchResult {
        let pool = Self::get_pool(reason, pool_of)?;
        for slot in pool.slots() {
            if slot.digest() == *slot_of {
                return Ok(());
            }
        }
        Err(Error::<T, I>::SlotOfPoolNotFound.into())
    }

    /// Checks whether at least one pool exists for the given reason.
    ///
    /// ## Returns
    /// - `Ok(())` if at least one pool exists
    /// - `Err(DispatchError)` with `PoolNotFound` if no pools exist
    fn has_pool(reason: &Self::Reason) -> DispatchResult {
        ensure!(
            PoolMap::<T, I>::iter_prefix((reason,)).next().is_some(),
            Error::<T, I>::PoolNotFound
        );
        Ok(())
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Retrieves the manager of a given pool.
    ///
    /// Invariant enforced that every valid pool, must have a
    /// valid manager. For unmanaged distribution of commitments,
    /// [`CommitIndex`] exists.
    ///
    /// ## Returns
    /// - `Ok(Proprietor)` containing the pool manager's account id
    /// - `Err(DispatchError)` with `PoolManagerNotFound` if no manager is set
    fn get_manager(
        reason: &Self::Reason,
        pool_of: &Self::Digest,
    ) -> Result<Proprietor<T>, DispatchError> {
        Self::pool_exists(reason, pool_of)?;
        let pool_manager = PoolManager::<T, I>::get((reason, pool_of));
        debug_assert!(
            pool_manager.is_some(),
            "pool {:?} of reason {:?} exists but manager is not",
            pool_of,
            reason
        );
        let pool_manager = pool_manager.ok_or(Error::<T, I>::PoolManagerNotFound)?;
        Ok(pool_manager)
    }

    /// Retrieves the commission rate of a given pool.
    ///
    /// ## Returns
    /// - `Ok(Commission)` containing the pool's commission rate
    /// - `Err(DispatchError)` with `PoolNotFound` if the pool does not exist
    fn get_commission(
        reason: &Self::Reason,
        pool_of: &Self::Digest,
    ) -> Result<Self::Commission, DispatchError> {
        let pool = Self::get_pool(reason, pool_of)?;
        Ok(pool.commission())
    }

    /// Retrieves the pool information for a given reason and digest.
    ///
    /// ## Returns
    /// - `Ok(Pool)` containing the pool structure
    /// - `Err(DispatchError)` with `PoolNotFound` if the pool does not exist
    fn get_pool(
        reason: &Self::Reason,
        pool_of: &Self::Digest,
    ) -> Result<Self::Pool, DispatchError> {
        let pool = PoolMap::<T, I>::get((reason, pool_of)).ok_or(Error::<T, I>::PoolNotFound)?;
        Ok(pool)
    }

    /// Retrieves all slot digests of a pool along with their respective shares.
    ///
    /// ## Returns
    /// - `Ok(Vec<(Digest, Shares)>)` containing each slot's digest and shares
    /// - `Err(DispatchError)` if the pool does not exist
    fn get_slots_shares(
        reason: &Self::Reason,
        pool_of: &Self::Digest,
    ) -> Result<Vec<(Self::Digest, Self::Shares)>, DispatchError> {
        let pool = Self::get_pool(reason, pool_of)?;
        let mut vec = Vec::new();
        for slot in pool.slots() {
            let slot_digest = &slot.digest();
            let shares = slot.shares();
            vec.push((slot_digest.clone(), shares))
        }
        Ok(vec)
    }

    /// Computes the real-time value of a specific slot digest in a pool across all proprietors.
    ///
    /// ## Returns
    /// - `Ok(Asset)`containing the aggregated slot value
    /// - `Err(DispatchError)` if the slot does not exist
    fn get_slot_value(
        reason: &Self::Reason,
        pool_of: &Self::Digest,
        slot_of: &Self::Digest,
    ) -> Result<Self::Asset, DispatchError> {
        // Ensure the slot exists
        Pallet::<T, I>::slot_exists(reason, pool_of, slot_of)?;

        // Retrieve pool info
        let pool_info = Pallet::<T, I>::get_pool(reason, pool_of)?;
        let slots = &pool_info.slots();

        // Locate the entry within the index
        let mut slot_idx = None;
        for (i, slot) in slots.iter().enumerate() {
            if slot.digest() == *slot_of {
                slot_idx = Some(i);
            }
        }
        let Some(slot_idx) = slot_idx else {
            return Err(Error::<T, I>::SlotOfPoolNotFound.into());
        };

        // Get the entry object and its variant
        let slot = slots.get(slot_idx);

        debug_assert!(
            slot.is_some(),
            "pool {:?} of reason {:?} slot {:?} is found 
            during iteration, but vector get failed",
            pool_of,
            reason,
            slot_of
        );
        let slot = slot.ok_or(Error::<T, I>::SlotOfPoolNotFound)?;
        let digest = &slot.digest();
        let variant = &slot.variant();

        let digest_info =
            DigestMap::<T, I>::get((reason, digest)).ok_or(Error::<T, I>::SlotDigestNotFound)?;

        let balance = digest_info.get_balance(variant);
        debug_assert!(
            balance.is_some(),
            "pool-digest {:?} of reason {:?} slot {:?} variant {:?} balance 
            was not initiated properly in the balance vector 
            properly during slot-value retrieval",
            pool_of,
            reason,
            slot_of,
            variant,
        );
        let balance = balance.ok_or(Error::<T, I>::DigestVariantBalanceNotFound)?;
        let slot_commit = &slot.commit();

        if *slot_commit == Default::default() {
            return Ok(Zero::zero());
        }

        let take = receipt_active_value(balance, variant, digest, &slot.commit())?;

        Ok(take)
    }

    /// Computes the real-time value of a specific slot digest in a pool for a given proprietor.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the proprietor's slot value
    /// - `Err(DispatchError)` if the slot does not exist
    fn get_slot_value_for(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        pool_of: &Self::Digest,
        slot_of: &Self::Digest,
    ) -> Result<Self::Asset, DispatchError> {
        let digest = Self::get_commit_digest(who, reason)?;
        Self::slot_exists(reason, pool_of, slot_of)?;
        ensure!(digest == *pool_of, Error::<T, I>::CommitNotFoundForSlot);
        CommitHelpers::<T, I>::pool_slot_commit_value(who, reason, pool_of, slot_of)
    }

    /// Computes the real-time value of a pool-commit for a given proprietor.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the proprietor's pool value
    /// - `Err(DispatchError)` if the slot does not exist
    fn get_pool_value_for(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        pool_of: &Self::Digest,
    ) -> Result<Self::Asset, DispatchError> {
        let digest = Self::get_commit_digest(who, reason)?;
        Self::pool_exists(reason, pool_of)?;
        ensure!(digest == *pool_of, Error::<T, I>::CommitNotFoundForSlot);
        CommitHelpers::<T, I>::pool_commit_value(who, reason, pool_of)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` CONSTRUCTORS ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Generates a unique digest for a pool derived from a given index.
    ///
    /// Pools are created from indexes, and each pool digest must be unique. This function
    /// deterministically derives a digest using:
    /// - The caller (`who`) as the target
    /// - The `reason` under which the pool is created
    /// - The pool's internal entries and specified `commission`
    /// - The caller's account nonce as a salt
    ///
    /// This digest can then be used to create or reference the pool in the system.
    ///
    /// ## Returns
    /// - `Ok(Digest)` containing the unique pool digest
    /// - `Err(DispatchError)` if digest generation fails
    fn gen_pool_digest(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        index_of: &Self::Digest,
        commission: Self::Commission,
    ) -> Result<Self::Digest, DispatchError> {
        let pool_index = Self::get_index(reason, index_of)?;
        let actual_pool = PoolInfo::<T, I>::new(pool_index.reveal_entries(), commission);
        // Since the function only accumulates capital and straight conversion
        // It should pass, unless some commission checks are implied inside
        debug_assert!(
            actual_pool.is_ok(),
            "pool-info construction for reason {:?}
            from a already valid index {:?} entries and commission {:?} has failed",
            reason,
            index_of,
            commission
        );
        let actual_pool = actual_pool?;
        let target = who;
        let salt = frame_system::Pallet::<T>::account_nonce(who);
        let key_gen_item = PoolOfReason::<T, I>::new(*reason, actual_pool.clone());
        let key = KeySeedFor::<Self::Digest, PoolOfReason<T, I>, T::Nonce, T::Hashing, T>::gen_key(
            target,
            &key_gen_item,
            salt,
        )
        .ok_or(Error::<T, I>::CannotGenerateDigest)?;
        Ok(key)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Creates a new pool based on an existing index with a specified commission rate.
    ///
    /// Pools are immutable with respect to their commission. Any changes to entries
    /// require modifying slots directly or creating a new pool.
    ///
    /// - Retrieves the index information to populate the pool slots.
    /// - Converts each index entry into a pool slot, preserving the shares.
    /// - Sets the caller as the manager of the new pool.
    ///
    /// ## Returns
    /// - `Ok(())` if the pool was successfully created
    /// - `Err(DispatchError)` with `PoolDigestTaken` if a pool with this digest already exists
    fn set_pool(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        pool_of: &Self::Digest,
        index_of: &Self::Digest,
        commission: Self::Commission,
    ) -> DispatchResult {
        // Check if the pool already exists
        ensure!(
            Self::pool_exists(reason, pool_of).is_err(),
            Error::<T, I>::PoolDigestTaken
        );

        // Retrieve index information to initialize the pool slots
        let index_info = Self::get_index(reason, index_of)?;

        // Create a new pool object from index entries and the specified commission
        let pool_info = PoolInfo::<T, I>::new(index_info.reveal_entries(), commission);

        // Since the function only accumulates capital and straight conversion
        // It should pass, unless some commission checks are implied inside
        debug_assert!(
            pool_info.is_ok(),
            "pool-info construction for new pool {:?} of reason {:?}
            from a already valid index {:?} entries and commission {:?} has failed",
            pool_of,
            reason,
            index_of,
            commission
        );

        let pool_info = pool_info?;

        // Insert the new pool into storage
        PoolMap::<T, I>::insert((reason, pool_of), &pool_info);

        // Assign the caller as the manager of the pool
        let result = Self::set_pool_manager(reason, pool_of, who);

        debug_assert!(
            result.is_ok(),
            "recently created pool {:?} info inserted but 
            later logic set poolmanager {:?} failed",
            pool_of,
            who
        );

        result?;

        Self::on_set_pool(who, pool_of, reason, &pool_info);
        Ok(())
    }

    /// Sets or updates the manager for a specific pool.
    ///
    /// The manager is responsible for handeling pool operations including risk management,
    /// applying strategies on behalf of pool participants, and earning commission
    /// based on the pool's rules.
    ///
    /// ### Returns
    /// - `Ok(())` if the manager was successfully set
    /// - `Err(DispatchError)` if the pool does not exist
    fn set_pool_manager(
        reason: &Self::Reason,
        pool_of: &Self::Digest,
        manager: &Proprietor<T>,
    ) -> DispatchResult {
        Self::pool_exists(reason, pool_of)?;
        PoolManager::<T, I>::insert((reason, pool_of), manager);
        Self::on_set_manager(pool_of, reason, manager);
        Ok(())
    }

    /// Sets or updates the shares of a specific slot within a pool.
    ///
    /// Unlike indexes, pools are mutable and managed internally, so modifying a slot
    /// does **not** produce a new digest. Each mutation releases and recovers the pool
    /// to maintain real-time balances.
    ///
    /// If the slot already exists:
    /// - A zero share value removes the slot from the pool.
    /// - A non-zero share value updates the slot's shares while keeping its variant.
    ///
    /// Nested cases-such as pool slots referencing other pools or indexes
    /// are not supported and may be treated as new direct digests when routed
    /// through [`CommitDeposit::deposit_to_digest`]. Callers are responsible for
    /// validating such cases if required.
    ///
    /// If the slot does not exist:
    /// - A zero share value returns early without any changes.
    /// - A non-zero share value creates a new slot with the default variant.
    ///
    /// DispatchError if fails
    fn set_slot_shares(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        pool_of: &Self::Digest,
        slot_of: &Self::Digest,
        shares: Self::Shares,
    ) -> DispatchResult {
        match Self::slot_exists(reason, pool_of, slot_of).is_ok() {
            true => {
                let variant = Self::get_slot_variant(reason, pool_of, slot_of);
                debug_assert!(
                    variant.is_ok(),
                    "slot {:?} exists for pool {:?} of reason {:?} 
                    but its variant (must-required) is unavailable",
                    slot_of,
                    pool_of,
                    reason
                );

                let variant = variant?;

                match shares.is_zero() {
                    true => CommitHelpers::<T, I>::remove_pool_slot(who, reason, pool_of, slot_of)?,
                    false => CommitHelpers::<T, I>::set_pool_slot(
                        who, reason, pool_of, slot_of, shares, &variant,
                    )?,
                }
            }
            false => {
                if shares.is_zero() {
                    return Ok(());
                }
                CommitHelpers::<T, I>::set_pool_slot(
                    who,
                    reason,
                    pool_of,
                    slot_of,
                    shares,
                    &T::Position::default(),
                )?
            }
        }
        Self::on_set_slot_shares(pool_of, reason, slot_of, shares);
        Ok(())
    }

    /// Removes a pool from the system if all deposits have been withdrawn.
    ///
    /// Any residual funds (dust) remaining in the pool are automatically refunded
    /// to the pool manager.
    ///
    /// This function doesn't ensures the pool's current manager as it should be
    /// validated elsewhere if required.
    ///
    /// ## Returns
    /// - `Ok(())` if the pool was successfully removed
    /// - `Err(DispatchError)` otherwise
    fn reap_pool(reason: &Self::Reason, pool_of: &Self::Digest) -> DispatchResult {
        let pool = Self::get_pool(reason, pool_of)?;
        let balance = &pool.balance();

        // Cannot reap a pool if deposits still exist
        if has_deposits(balance, &Default::default(), pool_of).is_ok() {
            return Err(Error::<T, I>::PoolHasFunds.into());
        }

        // Refund any leftover effective balance to the manager
        let effective = balance_total(balance, &Default::default(), pool_of)?;
        if !effective.is_zero() {
            let imbalance = AssetDelta::<T, I> {
                deposit: Zero::zero(),
                withdraw: effective,
            };
            let manager = Self::get_manager(reason, pool_of);
            debug_assert!(
                manager.is_ok(),
                "pool {:?} for reason {:?} exists but manager is not",
                pool_of,
                reason
            );
            let manager = manager?;
            let dust_retn = CommitHelpers::<T, I>::resolve_imbalance(&manager, imbalance)?;
            CommitHelpers::<T, I>::sub_from_total_value(reason, dust_retn)?;
        }
        // Remove pool and its manager from storage
        PoolMap::<T, I>::remove((reason, pool_of));
        PoolManager::<T, I>::remove((reason, pool_of));

        // Managers are redunded with remaining dust unlike digests which
        // doesn't have any informal nominee
        Self::on_reap_pool(pool_of, reason, Zero::zero());
        Ok(())
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Emits an event when a new pool is created or initialized.
    ///
    /// - Includes all underlying slots and the commission set for the pool manager.
    ///
    /// - Emits [`Event::PoolInitialized`] or [`Event::PoolInitialized`]
    ///   depending on whether multiple variants are supported by [`Config::Position`].
    /// - Emits these events only if [`Config::EmitEvents`] is `true`.
    fn on_set_pool(
        _who: &Proprietor<T>,
        pool_of: &Self::Digest,
        reason: &Self::Reason,
        pool: &Self::Pool,
    ) {
        if T::EmitEvents::get() {
            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            {
                let mut slots = Vec::new();
                for slot in pool.slots() {
                    let slot_digest = slot.digest().clone();
                    let shares = slot.shares();
                    let variant = &slot.variant();
                    slots.push((slot_digest, shares, variant.clone()));
                }
                let commission = pool.commission();
                Self::deposit_event(Event::<T, I>::PoolInitialized {
                    pool_of: pool_of.clone(),
                    reason: *reason,
                    commission,
                    slots,
                });
            }

            #[cfg(not(any(feature = "dev", feature = "runtime-benchmarks")))]
            {
                let commission = pool.commission();
                Self::deposit_event(Event::<T, I>::PoolInitialized {
                    pool_of: pool_of.clone(),
                    reason: *reason,
                    commission,
                });
            }
        }
    }

    /// Emits an event when the manager of a pool is set or updated.
    ///
    /// Emits a [`Event::PoolManager`] event if [`Config::EmitEvents`] is `true`.
    fn on_set_manager(pool_of: &Self::Digest, reason: &Self::Reason, manager: &Proprietor<T>) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::<T, I>::PoolManager {
                pool_of: pool_of.clone(),
                reason: *reason,
                manager: manager.clone(),
            });
        }
    }

    /// Emits an event when the shares of a specific slot in a pool are updated.
    ///
    /// This method delagates itself to [`PoolVariant::on_set_slot_of_variant`]
    /// with the default position variant.
    #[inline]
    fn on_set_slot_shares(
        pool_of: &Self::Digest,
        reason: &Self::Reason,
        slot_of: &Self::Digest,
        shares: Self::Shares,
    ) {
        Self::on_set_slot_of_variant(pool_of, reason, slot_of, Some(shares), &Default::default());
    }

    /// Emits an event when a pool is reaped (removed).
    ///
    /// Emits a [`Event::PoolReaped`] event if [`Config::EmitEvents`] is `true`.
    fn on_reap_pool(pool_of: &Self::Digest, reason: &Self::Reason, dust: Self::Asset) {
        debug_assert!(
            dust.is_zero(),
            "pool digest {:?} of reason {:?} reaped with non-zero dust {:?}",
            pool_of,
            reason,
            dust
        );
        if T::EmitEvents::get() {
            Self::deposit_event(Event::<T, I>::PoolReaped {
                pool_of: pool_of.clone(),
                reason: *reason,
            });
        }
    }
}

// ===============================================================================
// ```````````````````````````````` COMMIT VARIANT ```````````````````````````````
// ===============================================================================

/// Implements [`CommitVariant`] for the pallet
impl<T: Config<I>, I: 'static> CommitVariant<Proprietor<T>> for Pallet<T, I> {
    /// Defines the commitment position variant type for a proprietor.
    ///
    /// Acts as the logical "stance" or directional disposition of a commitment.
    type Position = T::Position;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Validates whether a digest-variant value can be set.
    ///
    /// Same as the default trait validation, but extended with additional
    /// checks against the underlying balance model to ensure minting or
    /// reaping can actually be applied.
    ///
    /// In the lazy balance model, value updates are interpreted as:
    /// - Increase -> mint
    /// - Decrease -> reap
    ///
    /// Missing digest or variant state is treated as a default
    /// (fresh) balance, but validation still depends on the underlying
    /// lazy balance model and may fail.
    ///
    /// ## Returns
    /// - `Ok(())` if validation succeeds
    /// - `Err(DispatchError)` if any constraint is violated
    fn can_set_digest_variant_value(
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
        variant: &Self::Position,
        qualifier: &Self::Intent,
    ) -> DispatchResult {
        let current = Self::get_digest_variant_value(reason, digest, variant)?;
        match current.cmp(&value) {
            Ordering::Less => {
                // Mint path (increase)
                let balance = DigestMap::<T, I>::get((reason, digest))
                    .and_then(|digest_info| digest_info.get_balance(variant).cloned())
                    .unwrap_or_default();
                let limits = mint_limits_of(&balance, variant, digest, qualifier)?;
                let mintable = value.saturating_sub(current);
                ensure!(
                    <Self::Limits as Extent>::contains(&limits, mintable),
                    Error::<T, I>::MintingOffLimits,
                );
                can_mint(&balance, variant, digest, &mintable, qualifier)?;
            }
            Ordering::Greater => {
                // Reap path (decrease)
                let balance = DigestMap::<T, I>::get((reason, digest))
                    .and_then(|digest_info| digest_info.get_balance(variant).cloned())
                    .unwrap_or_default();
                let limits = reap_limits_of(&balance, variant, digest, qualifier)?;
                let reapable = current.saturating_sub(value);
                ensure!(
                    <Self::Limits as Extent>::contains(&limits, reapable),
                    Error::<T, I>::ReapingOffLimits,
                );
                can_reap(&balance, variant, digest, &reapable, qualifier)?;
            }
            Ordering::Equal => {
                // No-op
            }
        }
        Ok(())
    }

    /// Validates whether a new commitment can be placed for a specific variant.
    ///
    /// Same as the default trait validation, but extended with an additional
    /// check against the underlying balance model to ensure the deposit can
    /// actually be applied.
    ///
    /// Missing digest or variant state is treated as a fresh balance; in such
    /// cases, a default balance is used to derive limits and validate the deposit.
    ///
    /// ## Returns
    /// - `Ok(())` if validation succeeds
    /// - `Err(DispatchError)` if any constraint is violated
    fn can_place_commit_of_variant(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        digest: &Self::Digest,
        variant: &Self::Position,
        value: Self::Asset,
        qualifier: &Self::Intent,
    ) -> DispatchResult {
        ensure!(
            Self::commit_exists(who, reason).is_err(),
            Error::<T, I>::CommitAlreadyExists
        );
        let max = Self::available_funds(who);
        ensure!(max >= value, Error::<T, I>::InsufficientFunds);
        let balance = DigestMap::<T, I>::get((reason, digest))
            .and_then(|digest_info| digest_info.get_balance(variant).cloned())
            .unwrap_or_default();

        let limits = deposit_limits_of(&balance, variant, digest, qualifier)?;
        ensure!(
            <Self::Limits as Extent>::contains(&limits, value),
            Error::<T, I>::PlacingOffLimits
        );
        can_deposit(&balance, variant, digest, &value, qualifier)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Retrieves the current commitment variant for a
    /// proprietor and reason.
    ///
    /// In-case of indexes and pools its always the default variant of
    /// [`Config::Position`] since its entries and slots carry the actual
    /// variants for which this commit is distributed.
    ///
    /// ## Returns
    /// - `Ok(Position)` containing the commitment's variant
    /// - `Err(DispatchError)`  with `CommitNotFound` if no commitment exists
    fn get_commit_variant(
        who: &Proprietor<T>,
        reason: &Self::Reason,
    ) -> Result<Self::Position, DispatchError> {
        let Some(commit_info) = CommitMap::<T, I>::get((who, reason)) else {
            return Err(Error::<T, I>::CommitNotFound.into());
        };
        Ok(commit_info.variant())
    }

    /// Retrieves the real-time effective value of a specific digest
    /// variant for a given reason.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the variant's effective value,
    /// or zero if the variant does not exist
    /// - `Err(DispatchError)` with `DigestNotFound` if the digest
    /// does not exist
    fn get_digest_variant_value(
        reason: &Self::Reason,
        digest: &Self::Digest,
        variant: &Self::Position,
    ) -> Result<Self::Asset, DispatchError> {
        let digest_info =
            DigestMap::<T, I>::get((reason, digest)).ok_or(Error::<T, I>::DigestNotFound)?;
        // Return effective balance if present, otherwise zero
        let Some(balance) = digest_info.get_balance(variant) else {
            return balance_total::<T, I>(&Default::default(), variant, digest);
        };

        balance_total(balance, variant, digest)
    }

    /// Derives minting limits for a specific digest variant.
    ///
    /// Fetches the current lazy balance of the given `(reason, digest, variant)`
    /// and computes the applicable minting limits using the underlying balance model.
    ///
    /// If the digest not exists nor the variant has an initialized balance,
    /// the limits are derived using a default (empty) balance.
    ///
    /// ## Returns
    /// - `Ok(Limits)` containing the derived minting constraints
    /// - `Err(DispatchError)` if the limit derivation fails
    fn digest_mint_limits_of_variant(
        digest: &Self::Digest,
        reason: &Self::Reason,
        variant: &Self::Position,
        qualifier: &Self::Intent,
    ) -> Result<Self::Limits, DispatchError> {
        let balance = DigestMap::<T, I>::get((reason, digest))
            .and_then(|digest_info| digest_info.get_balance(variant).cloned())
            .unwrap_or_default();
        let limits = mint_limits_of(&balance, variant, digest, qualifier)?;
        Ok(limits)
    }

    /// Derives reaping limits for a specific digest variant.
    ///
    /// Fetches the current lazy balance of the given `(reason, digest, variant)`
    /// and computes the applicable reaping limits using the underlying balance model.
    ///
    /// If the digest does not exists nor the variant has a initialized balance,
    /// limits are derived using a default (empty) balance.
    ///
    /// ## Returns
    /// - `Ok(Limits)` containing the derived reaping constraints
    /// - `Err(DispatchError)` if the limit derivation fails
    fn digest_reap_limits_of_variant(
        digest: &Self::Digest,
        reason: &Self::Reason,
        variant: &Self::Position,
        qualifier: &Self::Intent,
    ) -> Result<Self::Limits, DispatchError> {
        let balance = DigestMap::<T, I>::get((reason, digest))
            .and_then(|digest_info| digest_info.get_balance(variant).cloned())
            .unwrap_or_default();

        let limits = reap_limits_of(&balance, variant, digest, qualifier)?;
        Ok(limits)
    }

    /// Derives the place commit limits for the given commitment-params.
    ///
    /// Fetches the current lazy balance of the given `(reason, digest, variant)`
    /// and computes the applicable deposit limits using the underlying
    /// balance model.
    ///
    /// - For **Direct digests**, limits are derived based on existing digest balance.
    /// - For **Index or Pool digests**, no digest-level balance info exists,
    ///   hence limits are treated as **unbounded**.
    ///
    /// If the direct digest does not exist or the variant has no initialized balance,
    /// limits are derived using a default (empty) balance.
    ///
    /// ## Returns
    /// - `Ok(Limits)` containing the derived placement constraints
    /// - `Err(DispatchError)` if the limit derivation fails
    fn place_commit_limits_of_variant(
        _who: &Proprietor<T>,
        reason: &Self::Reason,
        digest: &Self::Digest,
        variant: &Self::Position,
        qualifier: &Self::Intent,
    ) -> Result<Self::Limits, DispatchError> {
        let balance = DigestMap::<T, I>::get((reason, digest))
            .and_then(|digest_info| digest_info.get_balance(variant).cloned())
            .unwrap_or_default();

        let limits = deposit_limits_of(&balance, variant, digest, qualifier)?;
        Ok(limits)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Sets the effective value of a specific digest variant for a given reason.
    ///
    /// It is expected that the variant is initiated via a commitment before setting it.
    ///
    /// Automatically handles minting (for increases) or reaping (for decreases) of assets,
    /// and updates the total reason value accordingly. This operation directly modifies
    /// the low-level digest variant balance without affecting other variants.
    ///
    /// The `qualifier` influences how minting/reaping is applied and may affect the
    /// final value that is actually set.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the resulting value of the digest-variant after update
    /// - `Err(DispatchError)` if the operation fails
    fn set_digest_variant_value(
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
        variant: &Self::Position,
        qualifier: &Self::Intent,
    ) -> Result<Self::Asset, DispatchError> {
        let new =
            DigestMap::<T, I>::mutate((reason, digest), |result| -> Result<_, DispatchError> {
                let digest_info = result.as_mut().ok_or(Error::<T, I>::DigestNotFound)?;
                // Get the balance of the variant via its index
                let digest_of = digest_info
                    .mut_balance(variant)
                    // A deposit is required for setting, else it will be unfair to reward a digest
                    .ok_or(Error::<T, I>::DigestVariantBalanceNotFound)?;
                let current = balance_total(digest_of, variant, digest)?;
                let new = match current.cmp(&value) {
                    Ordering::Less => {
                        // Increase value: mint additional assets
                        // Determine value to mint accurately
                        let try_actual = value.saturating_sub(current);
                        // Via the mutable lazy-balance, mint the required value
                        let actual = mint(digest_of, variant, digest, &try_actual, qualifier)?;
                        // Asset is inflated via commitment-system so reflect via `AssetToIssue`
                        // Later when commitments in this inflated variant balance is resolved this will
                        // will minted to its underlying fungible system.
                        AssetToIssue::<T, I>::mutate(|total_issued| -> DispatchResult {
                            *total_issued = total_issued
                                .checked_add(&actual)
                                .ok_or(Error::<T, I>::MaxAssetIssued)?;
                            Ok(())
                        })?;
                        // Add to reason's total committed value since value is inflated
                        CommitHelpers::<T, I>::add_to_total_value(reason, actual)?;
                        current.saturating_add(actual)
                    }
                    Ordering::Greater => {
                        // Decrease value: reap excess assets
                        // Determine value to reap accurately
                        let try_actual = current.saturating_sub(value);
                        // Via the mutable lazy-balance, reap the required value
                        let actual = reap(digest_of, variant, digest, &try_actual, qualifier)?;
                        // Asset is deflated (burned) via commitment-system so reflect via `AssetToReap`
                        // Later when commitments in this deflated variant balance is resolved this will
                        // will reap the burned balance from its underlying fungible system.
                        AssetToReap::<T, I>::mutate(|total_to_reap| -> DispatchResult {
                            *total_to_reap = total_to_reap
                                .checked_add(&actual)
                                .ok_or(Error::<T, I>::MaxAssetReaped)?;
                            Ok(())
                        })?;
                        // Subtract reason's total committed value since value is deflated
                        CommitHelpers::<T, I>::sub_from_total_value(reason, actual)?;
                        current.saturating_sub(actual)
                    }
                    core::cmp::Ordering::Equal => {
                        // No change needed
                        current
                    }
                };
                Self::on_set_digest_variant(digest, reason, new, variant);
                Ok(new)
            })?;
        Ok(new)
    }

    /// Places a commitment with a specific variant to a digest.
    ///
    /// This function validates the commitment eligibility, determines the digest model
    /// (Direct, Index, or Pool), and registers the commitment with the specified variant
    /// and amount according to the provided precision and fortitude parameters.
    ///
    /// Zero-value (marker) commitments are not allowed and will be rejected.
    ///
    /// If the digest does not yet exist in the system, this function assumes it to
    /// be a **direct digest** (since indexes and pools require prior initialization)
    /// and initializes the commitment accordingly.
    ///
    /// Callers must ensure that such digests are valid within their intended
    /// commitment context and agreement.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the actual committed amount
    /// - `Err(DispatchError)` if placement fails
    fn place_commit_of_variant(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
        variant: &Self::Position,
        qualifier: &Self::Intent,
    ) -> Result<Self::Asset, DispatchError> {
        ensure!(
            Self::commit_exists(who, reason).is_err(),
            Error::<T, I>::CommitAlreadyExists
        );
        ensure!(!value.is_zero(), Error::<T, I>::MarkerCommitNotAllowed);
        let digest_model =
            Self::determine_digest(digest, reason).unwrap_or(DigestVariant::Direct(digest.clone()));
        let actual = CommitHelpers::<T, I>::place_commit_of(
            who,
            reason,
            &digest_model,
            value,
            variant,
            qualifier,
        )?;
        Self::on_place_commit_on_variant(who, reason, digest, actual, variant);
        Ok(actual)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Emits an event indicating that a commitment has been placed
    /// for a specific digest variant and proprietor.
    ///
    /// The digest is verified and classified using
    /// [`DigestModel::determine_digest`].
    ///
    /// Emits [`Event::CommitPlaced`] event if [`Config::EmitEvents`] is `true`.
    fn on_place_commit_on_variant(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
        variant: &Self::Position,
    ) {
        if T::EmitEvents::get() {
            #[cfg(any(feature = "dev", feature = "runtime-benchmarks"))]
            {
                let Ok(digest_model) = Self::determine_digest(digest, reason) else {
                    return;
                };
                Self::deposit_event(Event::<T, I>::CommitPlaced {
                    who: who.clone(),
                    reason: *reason,
                    model: digest_model,
                    value: value,
                    variant: variant.clone(),
                })
            }

            #[cfg(not(any(feature = "dev", feature = "runtime-benchmarks")))]
            {
                Self::deposit_event(Event::<T, I>::CommitPlaced {
                    who: who.clone(),
                    reason: *reason,
                    digest: digest.clone(),
                    value: value,
                    variant: variant.clone(),
                })
            }
        }
    }

    /// Emits an event when a commit for a specific digest
    /// variant is updated.
    ///
    /// This method delagates itself to [`CommitVariant::on_place_commit_on_variant`]
    /// with the default position variant.
    ///
    /// [`Self::set_commit_variant`] is semantically similar to resolving and
    /// placing a new-commitment on a different variant internally.
    #[inline]
    fn on_set_commit_variant(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        digest: &Self::Digest,
        value: Self::Asset,
        variant: &Self::Position,
    ) {
        Self::on_place_commit_on_variant(who, reason, digest, value, variant)
    }

    /// Emits an event when a digest variant's effective value is updated.
    ///
    /// This is typically triggered after a reward, penalty, or manual
    /// adjustment applied directly to a digest variant.
    ///
    /// Emits [`Event::DigestInfo`] event if [`Config::EmitEvents`] is `true`.
    fn on_set_digest_variant(
        digest: &Self::Digest,
        reason: &Self::Reason,
        value: Self::Asset,
        variant: &Self::Position,
    ) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::<T, I>::DigestInfo {
                digest: digest.clone(),
                reason: *reason,
                value: value,
                variant: variant.clone(),
            });
        }
    }
}

// ===============================================================================
// ```````````````````````````````` INDEX VARIANT ````````````````````````````````
// ===============================================================================

/// Implements [`IndexVariant`] for the pallet
impl<T: Config<I>, I: 'static> IndexVariant<Proprietor<T>> for Pallet<T, I> {
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` CONSTRUCTORS ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Prepares a new index object from a list of entry digests, their shares,
    /// and variants.
    ///
    /// This function does **not** associate the index with a specific reason or
    /// proprietor internally. The caller is responsible for ensuring the index
    /// is correctly attached to a reason via [`CommitIndex::set_index`] and,
    /// optionally, the creator (for reap reasons since index should not have a
    /// manager).
    ///
    /// Entries with zero shares are silently ignored, as they carry no
    /// semantic contribution to the index.
    ///
    /// - `who`: The proprietor creating the index.
    /// - `reason`: The reason under which the index is being prepared (not used internally).
    /// - `entries`: A vector of tuples containing:
    ///     - `Digest`: The entry digest
    ///     - `Shares`: The number of shares assigned to the entry
    ///     - `Position`: The variant/disposition of the entry
    ///
    /// ## Returns
    /// - `Ok(Index)` containing the prepared index
    /// - `Err(DispatchError)` if preparation fails
    fn prepare_index_of_variants(
        _who: &Proprietor<T>,
        _reason: &Self::Reason,
        entries: Vec<(Self::Digest, Self::Shares, Self::Position)>,
    ) -> Result<Self::Index, DispatchError> {
        // Initialize a new Entries collection for the index
        let mut entries_of = Vec::new();
        for (digest, shares, variant) in entries {
            // Silently ignore non-share allocated entries
            if shares.is_zero() {
                continue;
            }
            // Create a new entry with the given variant
            let entry_info = EntryInfo::<T, I>::new(digest, shares, variant)?;
            // Add entry to the index, checking for maximum capacity
            entries_of.push(entry_info);
        }
        // Construct the final IndexInfo object
        let index_info = IndexInfo::<T, I>::new(&mut Entries::<T, I>::new(entries_of)?)?;

        Ok(index_info)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Retrieves the variant associated with a specific entry in an index.
    ///
    /// ## Returns
    /// - `Ok(Position)` containing the entry's variant
    /// - `Err(DispatchError)` otherwise
    fn get_entry_variant(
        reason: &Self::Reason,
        index_of: &Self::Digest,
        entry_of: &Self::Digest,
    ) -> Result<Self::Position, DispatchError> {
        let index_info = Self::get_index(reason, index_of)?;
        let entries = &index_info.entries();

        let mut idx = None;

        // Locate the entry by digest
        for (i, entry) in entries.iter().enumerate() {
            if entry.digest() == *entry_of {
                idx = Some(i);
            }
        }

        let entry_info = match idx {
            Some(i) => {
                let entry = entries.get(i);
                debug_assert!(
                    entry.is_some(),
                    "entry {:?} of index {:?} found by iterating over index, 
                    but retrieval via vector index {:?} get failed",
                    entry_of,
                    index_of,
                    i
                );
                entry.ok_or(Error::<T, I>::EntryOfIndexNotFound)?
            }
            None => return Err(Error::<T, I>::EntryOfIndexNotFound.into()),
        };
        let variant = entry_info.variant().clone();

        Ok(variant)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Sets or updates the variant (position/disposition) of a specific entry
    /// of an index, producing a new index.
    ///
    /// This function handles both existing and new entries:
    /// - If the entry exists:
    ///   - If `shares` is `Some(zero)`, the entry is removed.
    ///   - If `shares` is `Some`, both shares and variant are updated.
    ///   - If `shares` is `None`, only the variant is updated while retaining existing shares.
    /// - If the entry does not exist:
    ///   - If `shares` is `Some(zero)` or `None`, the operation is a no-op and the original index is returned.
    ///   - Otherwise, a new entry is added with the provided variant and shares.
    ///
    /// The entry digest is not validated to be a direct digest and is accepted as provided.
    /// If a commitment is placed on the index, the entry will be funded through normal deposit routing.
    ///
    /// ## Returns
    /// - `Ok(Digest)` containing the resulting index digest (may be unchanged if no-op)
    /// - `Err(DispatchError)` if the operation fails
    fn set_entry_of_variant(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        index_of: &Self::Digest,
        entry_of: &Self::Digest,
        variant: Self::Position,
        shares: Option<Self::Shares>,
    ) -> Result<Self::Digest, DispatchError> {
        match Self::entry_exists(reason, index_of, entry_of).is_ok() {
            true => {
                // Entry exists
                match shares {
                    Some(s) => {
                        if s.is_zero() {
                            return CommitHelpers::<T, I>::remove_index_entry(
                                who, reason, index_of, entry_of,
                            );
                        }
                        // Update entry with new shares and variant
                        CommitHelpers::<T, I>::set_index_entry(
                            who, reason, index_of, entry_of, s, &variant,
                        )
                    }
                    None => {
                        // Retain existing shares
                        let entries = Self::get_entries_shares(reason, index_of);
                        debug_assert!(
                            entries.is_ok(),
                            "entry {:?} of index {:?} of reason {:?} exists but cannot get 
                            all entries shares of the index",
                            entry_of,
                            index_of,
                            reason
                        );
                        let entries = entries?;
                        let mut current_shares = None;
                        for (entry_digest, share) in entries {
                            if entry_digest == *entry_of {
                                current_shares = Some(share);
                            }
                        }
                        let current_shares =
                            current_shares.ok_or(Error::<T, I>::EntryOfIndexNotFound)?;
                        CommitHelpers::<T, I>::set_index_entry(
                            who,
                            reason,
                            index_of,
                            entry_of,
                            current_shares,
                            &variant,
                        )
                    }
                }
            }
            false => {
                // Entry does not exist
                if let Some(s) = shares {
                    if s.is_zero() {
                        return Ok(index_of.clone());
                    }
                    CommitHelpers::<T, I>::set_index_entry(
                        who, reason, index_of, entry_of, s, &variant,
                    )
                } else {
                    // Cannot create a new entry without shares
                    Ok(index_of.clone())
                }
            }
        }
    }
}

// ===============================================================================
// ````````````````````````````````` POOL VARIANT ````````````````````````````````
// ===============================================================================

/// Implements [`PoolVariant`] for the pallet
impl<T: Config<I>, I: 'static> PoolVariant<Proprietor<T>> for Pallet<T, I> {
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Retrieves the variant associated with a specific slot in a pool.
    ///
    /// ## Returns
    /// - `Ok(Position)` containing the slot's variant
    /// - `Err(DispatchError)`if the slot does not exist
    fn get_slot_variant(
        reason: &Self::Reason,
        pool_of: &Self::Digest,
        slot_of: &Self::Digest,
    ) -> Result<Self::Position, DispatchError> {
        // Fetch the pool information
        let pool_info = Self::get_pool(reason, pool_of)?;
        let slots = &pool_info.slots();

        // Find the index of the requested slot
        let mut idx = None;
        for (i, slot) in slots.iter().enumerate() {
            if slot.digest() == *slot_of {
                idx = Some(i);
            }
        }

        let slot_info = match idx {
            Some(i) => {
                let slot = slots.get(i);
                debug_assert!(
                    slot.is_some(),
                    "slot {:?} of pool {:?} of reason {:?} found by iterating over 
                    pool, but later retrieval via vector index {:?} get failed",
                    slot_of,
                    pool_of,
                    reason,
                    i
                );
                slot.ok_or(Error::<T, I>::SlotOfPoolNotFound)?
            }
            None => return Err(Error::<T, I>::SlotOfPoolNotFound.into()),
        };

        let variant = slot_info.variant().clone();
        Ok(variant)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Sets or updates the variant of a specific slot within a pool.
    ///
    /// Unlike indexes, pools are mutable, so this operation modifies the
    /// pool in place without creating a new pool digest. The pool is released
    /// and recovered to maintain real-time balances after the mutation.
    ///
    /// This function handles both existing and new slots:
    /// - If the slot exists:
    ///   - Updates its variant.
    ///   - Updates shares if provided.
    ///   - Removes the slot if provided shares are zero.
    ///   - Retains existing shares if `shares` is `None`.
    /// - If the slot does not exist:
    ///   - Creates a new slot if non-zero `shares` is provided.
    ///   - Does nothing if `shares` is zero.
    ///   - Returns an error if `shares` is `None` (no slot to update and no data to create).
    ///
    /// ## Returns
    /// - `Ok(())` if the operation completes successfully
    /// - `Err(DispatchError)` if the slot is not found and cannot be created,
    ///   or if the operation fails
    fn set_slot_of_variant(
        who: &Proprietor<T>,
        reason: &Self::Reason,
        pool_of: &Self::Digest,
        slot_of: &Self::Digest,
        variant: Self::Position,
        shares: Option<Self::Shares>,
    ) -> DispatchResult {
        match Self::slot_exists(reason, pool_of, slot_of).is_ok() {
            true => {
                // Slot exists, determine shares to use
                let actual_shares = if let Some(shares) = shares {
                    if shares.is_zero() {
                        return CommitHelpers::<T, I>::remove_pool_slot(
                            who, reason, pool_of, slot_of,
                        );
                    }
                    shares
                } else {
                    let slots = Self::get_slots_shares(reason, pool_of);
                    debug_assert!(
                        slots.is_ok(),
                        "slot {:?} of pool {:?} of reason {:?} exists 
                        but cannot get all slots shares of the pool",
                        slot_of,
                        pool_of,
                        reason
                    );
                    let slots = slots?;
                    let mut found_shares = None;
                    for (slot_digest, share) in slots {
                        if slot_digest == *slot_of {
                            found_shares = Some(share);
                        }
                    }
                    found_shares.ok_or(Error::<T, I>::SlotOfPoolNotFound)?
                };
                CommitHelpers::<T, I>::set_pool_slot(
                    who,
                    reason,
                    pool_of,
                    slot_of,
                    actual_shares,
                    &variant,
                )
            }
            false => {
                // Slot does not exist
                if let Some(shares) = shares {
                    if shares.is_zero() {
                        return Ok(());
                    }
                    CommitHelpers::<T, I>::set_pool_slot(
                        who, reason, pool_of, slot_of, shares, &variant,
                    )
                } else {
                    // No shares provided, No Slot Found, No Variant to Set.
                    return Err(Error::<T, I>::SlotOfPoolNotFound.into());
                }
            }
        }
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Emits an event when a slot's variant or shares within a pool is updated.
    ///
    /// Records the pool digest, reason, slot digest, new variant, and shares (either
    /// provided or retrieved from current state).
    ///
    /// If the slot cannot be found, the function silently returns.
    ///
    /// Emits:
    /// - [`Event::PoolSlot`] if `shares > 0`
    /// - [`Event::PoolSlotRemoved`] if `shares == 0`
    ///
    /// Events are emitted only if [`Config::EmitEvents`] is `true`.
    fn on_set_slot_of_variant(
        pool_of: &Self::Digest,
        reason: &Self::Reason,
        slot_of: &Self::Digest,
        shares: Option<Self::Shares>,
        variant: &Self::Position,
    ) {
        if T::EmitEvents::get() {
            let shares = match shares {
                Some(shares) => shares,
                None => {
                    // Fallback: look up existing slot shares from the pool
                    let slots = match Self::get_slots_shares(reason, pool_of) {
                        Ok(slots) => slots,
                        Err(_) => return,
                    };

                    match slots
                        .into_iter()
                        .find(|(slot_digest, _)| slot_digest == slot_of)
                    {
                        Some((_, share)) => share,
                        None => return,
                    }
                }
            };

            match shares.is_zero() {
                true => Self::deposit_event(Event::<T, I>::PoolSlotRemoved {
                    pool_of: pool_of.clone(),
                    reason: *reason,
                    slot_of: slot_of.clone(),
                    variant: variant.clone(),
                }),
                false => Self::deposit_event(Event::<T, I>::PoolSlot {
                    pool_of: pool_of.clone(),
                    reason: *reason,
                    slot_of: slot_of.clone(),
                    variant: variant.clone(),
                    shares,
                }),
            }
        }
    }
}

// ===============================================================================
// ```````````````````````````` COMMIT ERROR HANDLER `````````````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> CommitErrorHandler for Pallet<T, I> {
    type Error = Error<T, I>;

    fn from_commit_error(e: CommitError) -> Self::Error {
        match e {
            CommitError::CommitAlreadyExists => Error::<T, I>::CommitAlreadyExists,
            CommitError::InsufficientFunds => Error::<T, I>::InsufficientFunds,
            CommitError::MintingOffLimits => Error::<T, I>::MintingOffLimits,
            CommitError::ReapingOffLimits => Error::<T, I>::ReapingOffLimits,
            CommitError::PlacingOffLimits => Error::<T, I>::PlacingOffLimits,
            CommitError::RaisingOffLimits => Error::<T, I>::RaisingOffLimits,
        }
    }
}

// ===============================================================================
// `````````````````````````````````` UNIT TESTS `````````````````````````````````
// ===============================================================================

/// Unit tests for [`commitment`](frame_suite::commitment) trait
/// implementations over [`Pallet`].
#[cfg(test)]
mod tests {

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` IMPORTS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    // --- Local crate imports ---
    use crate::{balance::*, mock::*};

    // --- FRAME Suite ---
    use frame_suite::{
        commitment::*,
        misc::{Directive, PositionIndex, Disposition},
    };

    // --- FRAME Support ---
    use frame_support::{
        assert_err, assert_ok,
        traits::{
            fungible::{Inspect, InspectFreeze, InspectHold},
            tokens::{Fortitude, Precision},
        },
    };

    // --- Substrate Primitives ---
    use sp_runtime::traits::Zero;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````` INSPECT ASSET ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn available_funds_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            let liquid_balance = AssetOf::balance(&ALICE);
            let hold_balance = AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE);
            assert_eq!(liquid_balance, INITIAL_BALANCE);
            assert_eq!(hold_balance, STANDARD_HOLD);
            let expected_available_funds = liquid_balance + hold_balance;
            let actual_available_funds = Pallet::available_funds(&ALICE);
            assert_eq!(actual_available_funds, expected_available_funds);
        })
    }

    #[test]
    fn available_funds_success_with_zero_for_uninitialized_user() {
        commit_test_ext().execute_with(|| {
            // since, NIX is uninitialized expected funds is 0
            let expected_available_funds = ZERO_VALUE;
            let actual_available_funds = Pallet::available_funds(&AMY);
            assert_eq!(actual_available_funds, expected_available_funds);
        })
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````` DIGEST MODEL `````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn determine_digest_success_for_direct_digest_model() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &ESCROW,
                &CONTRACT_FREELANCE,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_eq!(
                Pallet::determine_digest(&CONTRACT_FREELANCE, &ESCROW),
                Ok(DigestVariant::Direct(CONTRACT_FREELANCE))
            );
        })
    }

    #[test]
    fn determine_digest_success_for_index_model() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_index(BOB, STAKING, &entries, INDEX_BALANCED_STAKING).unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_BALANCED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_eq!(
                Pallet::determine_digest(&INDEX_BALANCED_STAKING, &STAKING),
                Ok(DigestVariant::Index(INDEX_BALANCED_STAKING))
            );
        })
    }

    #[test]
    fn determine_digest_success_for_pool_model() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_DOMINANT),
                (VALIDATOR_BETA, SHARE_MAJOR),
            ];
            prepare_and_initiate_pool(
                BOB,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Force),
            )
            .unwrap();
            assert_eq!(
                Pallet::determine_digest(&POOL_MANAGED_STAKING, &STAKING),
                Ok(DigestVariant::Pool(POOL_MANAGED_STAKING))
            );
        })
    }

    #[test]
    fn determine_digest_err_digest_not_found_to_determine() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_err!(
                Pallet::determine_digest(&PROPOSAL_RUNTIME_UPGRADE, &GOVERNANCE),
                Error::DigestNotFoundToDetermine
            );
        })
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` COMMITMENT ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn place_commmit_success_for_digest() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            assert_err!(
                Pallet::commit_exists(&ALICE, &GOVERNANCE),
                Error::CommitNotFound
            );
            assert_err!(
                Pallet::digest_exists(&GOVERNANCE, &PROPOSAL_TREASURY_SPEND),
                Error::DigestNotFound
            );
            assert_ok!(Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force)
            ));
            // Commit and digest enquirey
            assert_ok!(Pallet::commit_exists(&ALICE, &GOVERNANCE));
            assert_ok!(Pallet::digest_exists(&GOVERNANCE, &PROPOSAL_TREASURY_SPEND));
            // Balance and freeze enquirey
            let balace_after = AssetOf::balance(&ALICE);
            let hold_balance_after = AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE);
            let expected_balance_after = INITIAL_BALANCE;
            let expected_hold_balance_after = 250;
            assert_eq!(expected_balance_after, balace_after);
            assert_eq!(expected_hold_balance_after, hold_balance_after);
            assert_eq!(
                AssetOf::balance_frozen(&GOVERNANCE, &ALICE),
                STANDARD_COMMIT
            );
            // Digest info enquiry
            let digest_info = DigestMap::get((GOVERNANCE, PROPOSAL_TREASURY_SPEND)).unwrap();
            let digests = digest_info.reveal();
            let digest_of = digests.get(0).unwrap();
            let effective =
                balance_total(digest_of, &Default::default(), &PROPOSAL_TREASURY_SPEND).unwrap();
            assert_eq!(effective, 250);
            // Commit info enquiry
            let commit_info = CommitMap::get((ALICE, GOVERNANCE)).unwrap();
            assert_eq!(commit_info.digest(), PROPOSAL_TREASURY_SPEND);
            let commits = commit_info.commits();
            let commit = commits.get(0).unwrap();
            let principal = receipt_active_value(
                digest_of,
                &Default::default(),
                &PROPOSAL_TREASURY_SPEND,
                commit,
            )
            .unwrap();
            assert_eq!(principal, STANDARD_COMMIT);
            // Total value enquiry
            let reason_value = ReasonValue::get(GOVERNANCE).unwrap();
            assert_eq!(reason_value, 250);
        })
    }

    #[test]
    fn place_commit_success_for_index() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();

            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            prepare_and_initiate_index(
                MIKE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            assert_ok!(Pallet::index_exists(&STAKING, &INDEX_OPTIMIZED_STAKING));
            // Before placing a commit to the index
            let index_info = Pallet::get_index(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            assert_eq!(index_info.capital(), 100);
            assert_eq!(index_info.principal(), ZERO_VALUE);
            let actual_entries_value =
                Pallet::get_entries_value(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_entries_value = vec![(VALIDATOR_ALPHA, 0), (VALIDATOR_BETA, 0)];
            assert_eq!(actual_entries_value, expected_entries_value);

            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 500);

            // Place commit to an index
            assert_ok!(Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force)
            ));
            // After placing a commit to the index
            let index_value = Pallet::get_index_value(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            assert_eq!(index_value, 250);
            let actual_entries_value =
                Pallet::get_entries_value(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_entries_value = vec![(VALIDATOR_ALPHA, 100), (VALIDATOR_BETA, 150)];
            assert_eq!(actual_entries_value, expected_entries_value);
            // Entry info
            let entry_info_alpha =
                EntryMap::get((STAKING, INDEX_OPTIMIZED_STAKING, VALIDATOR_ALPHA, CHARLIE))
                    .unwrap();
            let alpha_commits = entry_info_alpha.commits();
            let alpha_derived_bal = alpha_commits.get(0).unwrap();
            assert_eq!(receipt_deposit_value(alpha_derived_bal).unwrap(), 100);
            let entry_info_beta =
                EntryMap::get((STAKING, INDEX_OPTIMIZED_STAKING, VALIDATOR_BETA, CHARLIE)).unwrap();
            let beta_commits = entry_info_beta.commits();
            let alpha_derived_bal = beta_commits.get(0).unwrap();
            assert_eq!(receipt_deposit_value(alpha_derived_bal).unwrap(), 150);
            // Total reason value
            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 750);
            // Balance and freeze enquirey
            let balace_after = AssetOf::balance(&CHARLIE);
            let hold_balance_after = AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &CHARLIE);
            let expected_balance_after = INITIAL_BALANCE;
            let expected_hold_balance_after = 250;
            assert_eq!(expected_balance_after, balace_after);
            assert_eq!(expected_hold_balance_after, hold_balance_after);
            assert_eq!(AssetOf::balance_frozen(&STAKING, &CHARLIE), STANDARD_COMMIT);
        })
    }

    #[test]
    fn place_commit_success_for_pool() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_MAJOR),
                (VALIDATOR_BETA, SHARE_DOMINANT),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            // Before placing commit to pool
            assert_eq!(
                Pallet::get_manager(&STAKING, &POOL_MANAGED_STAKING),
                Ok(MIKE)
            );
            let pool_info = PoolMap::get((STAKING, POOL_MANAGED_STAKING)).unwrap();
            let pool_balance_of = pool_info.balance();
            assert_eq!(
                balance_total(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING)
                    .unwrap(),
                0
            );
            assert!(
                has_deposits(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING).is_err()
            );
            let pool_capital = pool_info.capital();
            assert_eq!(pool_capital, 100);
            let actual_slots_value =
                Pallet::get_slots_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_slots_value = vec![(VALIDATOR_ALPHA, 0), (VALIDATOR_BETA, 0)];
            assert_eq!(actual_slots_value, expected_slots_value);
            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 500);
            // Placing commit to pool
            assert_ok!(Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force)
            ));
            // After placing commit to pool
            let pool_info = PoolMap::get((STAKING, POOL_MANAGED_STAKING)).unwrap();
            let pool_balance_of = pool_info.balance();
            assert_eq!(
                balance_total(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING)
                    .unwrap(),
                250
            );
            assert!(
                has_deposits(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING).is_ok()
            );
            let actual_slots_value =
                Pallet::get_slots_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_slots_value = vec![(VALIDATOR_ALPHA, 100), (VALIDATOR_BETA, 150)];
            assert_eq!(actual_slots_value, expected_slots_value);

            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 750);
            // Balance and freeze enquirey
            let balace_after = AssetOf::balance(&CHARLIE);
            let hold_balance_after = AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &CHARLIE);
            let expected_balance_after = INITIAL_BALANCE;
            let expected_hold_balance_after = 250;
            assert_eq!(expected_balance_after, balace_after);
            assert_eq!(expected_hold_balance_after, hold_balance_after);
            assert_eq!(AssetOf::balance_frozen(&STAKING, &CHARLIE), STANDARD_COMMIT);
        })
    }

    #[test]
    fn place_commit_marker_error_for_value_zero() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            assert_err!(
                Pallet::place_commit(
                    &ALICE,
                    &STAKING,
                    &VALIDATOR_ALPHA,
                    ZERO_VALUE,
                    &Directive::new(Precision::BestEffort, Fortitude::Force)
                ),
                Error::MarkerCommitNotAllowed
            );
            // Commit and digest enquirey
            assert_err!(
                Pallet::commit_exists(&ALICE, &STAKING),
                Error::CommitNotFound
            );
            assert_err!(
                Pallet::digest_exists(&STAKING, &VALIDATOR_ALPHA),
                Error::DigestNotFound
            );
        })
    }

    #[test]
    fn place_commit_err_commit_already_exists_for_reason() {
        commit_test_ext().execute_with(|| {
            let commit_info = CommitInfo::new(
                CONTRACT_FREELANCE,
                CommitInstance::default(),
                Default::default(),
            )
            .unwrap();
            CommitMap::insert((&ALICE, &ESCROW), commit_info);
            assert_err!(
                Pallet::place_commit(
                    &ALICE,
                    &ESCROW,
                    &CONTRACT_SUPPLY_CHAIN,
                    STANDARD_COMMIT,
                    &Directive::new(Precision::BestEffort, Fortitude::Polite)
                ),
                Error::CommitAlreadyExists
            );
        })
    }

    #[test]
    fn place_commit_err_insufficient_funds() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            assert_eq!(AssetOf::total_balance(&ALICE), 1500);
            let insufficient_commit = 1600;
            assert_err!(
                Pallet::place_commit(
                    &ALICE,
                    &ESCROW,
                    &CONTRACT_SUPPLY_CHAIN,
                    insufficient_commit,
                    &Directive::new(Precision::Exact, Fortitude::Force)
                ),
                Error::InsufficientFunds
            );
        })
    }

    #[test]
    fn raise_commit_success_for_direct() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let commit_value_before = Pallet::get_commit_value(&ALICE, &STAKING).unwrap();
            assert_eq!(commit_value_before, STANDARD_COMMIT);
            assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), STANDARD_COMMIT);
            let reason_value = Pallet::get_total_value(&STAKING);
            assert_eq!(reason_value, 250);
            assert_ok!(Pallet::raise_commit(
                &ALICE,
                &STAKING,
                SMALL_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force)
            ));
            let commit_value_after = Pallet::get_commit_value(&ALICE, &STAKING).unwrap();
            assert_eq!(commit_value_after, 350);
            assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), 350);
            let reason_value = Pallet::get_total_value(&STAKING);
            assert_eq!(reason_value, 350);
            // new commit instance added for raise commit
            let commit_info = CommitMap::get((ALICE, STAKING)).unwrap();
            let commits = commit_info.commits();
            let commit = commits.get(1).unwrap();
            assert_eq!(receipt_deposit_value(commit).unwrap(), SMALL_COMMIT);

            #[cfg(not(feature = "dev"))]
            System::assert_last_event(Event::CommitRaised {
                 who: ALICE, 
                 reason: STAKING, 
                 digest: VALIDATOR_ALPHA, 
                 value: SMALL_COMMIT
                }
                .into()
            );

            #[cfg(feature = "dev")]
            System::assert_last_event(Event::CommitRaised {
                 who: ALICE, 
                 reason: STAKING, 
                 model: DigestVariant::Direct(VALIDATOR_ALPHA), 
                 value: SMALL_COMMIT
                }
                .into()
            );            
        })
    }

    #[test]
    fn raise_commit_success_for_index() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                MIKE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
                INDEX_BALANCED_STAKING,
            )
            .unwrap();

            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_BALANCED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // Before raising the index commit value
            let index_info = Pallet::get_index(&STAKING, &INDEX_BALANCED_STAKING).unwrap();
            assert_eq!(index_info.capital(), 200);
            assert_eq!(index_info.principal(), 250);
            let actual_entries_value =
                Pallet::get_entries_value(&STAKING, &INDEX_BALANCED_STAKING).unwrap();
            let expected_entries_value = vec![(VALIDATOR_ALPHA, 125), (VALIDATOR_BETA, 125)];
            assert_eq!(actual_entries_value, expected_entries_value);
            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 250);
            // alice balance inspect
            assert_eq!(AssetOf::balance(&ALICE), INITIAL_BALANCE);
            assert_eq!(AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE), 250);
            // Raise commit value
            assert_ok!(Pallet::raise_commit(
                &ALICE,
                &STAKING,
                SMALL_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force)
            ));
            // After placing a raise commit to the index
            let index_value = Pallet::get_index_value(&STAKING, &INDEX_BALANCED_STAKING).unwrap();
            assert_eq!(index_value, 350);
            let actual_entries_value =
                Pallet::get_entries_value(&STAKING, &INDEX_BALANCED_STAKING).unwrap();
            let expected_entries_value = vec![(VALIDATOR_ALPHA, 175), (VALIDATOR_BETA, 175)];
            assert_eq!(actual_entries_value, expected_entries_value);

            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 350);
            // New commit instances added for raise commit
            // Commit info check
            let commit_info = CommitMap::get((ALICE, STAKING)).unwrap();
            let commits = commit_info.commits();
            // The raise commit instance should be at index 1, but a placeholder only for index commits
            // EntryMap only holds all individual entries commits, so querying value via this
            // returns error due to default receipt - i.e., invalid
            let raise_commit = commits.get(1).unwrap();
            assert!(receipt_deposit_value(raise_commit).is_err(),);

            // Entry info check
            // For VALIDATOR_ALPHA entry
            let alpha_entry_info =
                EntryMap::get((STAKING, INDEX_BALANCED_STAKING, VALIDATOR_ALPHA, ALICE)).unwrap();
            let alpha_commits = alpha_entry_info.commits();
            let commit = alpha_commits.get(1).unwrap();
            assert_eq!(receipt_deposit_value(commit).unwrap(), 50);
            // For VALIDATOR_BETA entry
            let beta_entry_info =
                EntryMap::get((STAKING, INDEX_BALANCED_STAKING, VALIDATOR_BETA, ALICE)).unwrap();
            let beta_commits = beta_entry_info.commits();
            let commit = beta_commits.get(1).unwrap();
            assert_eq!(receipt_deposit_value(commit).unwrap(), 50);
            // Balance and freeze enquirey
            let actual_balance = AssetOf::balance(&ALICE);
            let actual_hold_balance = AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE);
            let expected_balance = INITIAL_BALANCE;
            let expected_hold_balance = 150;
            assert_eq!(actual_balance, expected_balance);
            assert_eq!(actual_hold_balance, expected_hold_balance);
            assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), 350);

            #[cfg(not(feature = "dev"))]
            System::assert_last_event(Event::CommitRaised {
                 who: ALICE, 
                 reason: STAKING, 
                 digest: INDEX_BALANCED_STAKING, 
                 value: SMALL_COMMIT
                }
                .into()
            );

            #[cfg(feature = "dev")]
            System::assert_last_event(Event::CommitRaised {
                 who: ALICE, 
                 reason: STAKING, 
                 model: DigestVariant::Index(INDEX_BALANCED_STAKING), 
                 value: SMALL_COMMIT
                }
                .into()
            );  
        })
    }

    #[test]
    fn raise_commit_success_for_pool() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_MAJOR),
                (VALIDATOR_BETA, SHARE_DOMINANT),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_ZERO,
            )
            .unwrap();

            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // Before raising the pool commit value
            let pool_info = PoolMap::get((STAKING, POOL_MANAGED_STAKING)).unwrap();
            let pool_balance_of = pool_info.balance();
            assert_ok!(has_deposits(
                &pool_balance_of,
                &Default::default(),
                &POOL_MANAGED_STAKING
            ));
            assert_eq!(
                balance_total(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING)
                    .unwrap(),
                250
            );
            let pool_capital = pool_info.capital();
            assert_eq!(pool_capital, 100);
            let actual_slots_value =
                Pallet::get_slots_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_slots_value = vec![(VALIDATOR_ALPHA, 100), (VALIDATOR_BETA, 150)];
            assert_eq!(actual_slots_value, expected_slots_value);
            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 750);
            // Raise commit value
            assert_ok!(Pallet::raise_commit(
                &ALICE,
                &STAKING,
                SMALL_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force)
            ));
            // After raising the pool commit value
            let pool_info = PoolMap::get((STAKING, POOL_MANAGED_STAKING)).unwrap();
            let pool_balance_of = pool_info.balance();
            assert_ok!(has_deposits(
                &pool_balance_of,
                &Default::default(),
                &POOL_MANAGED_STAKING
            ));
            assert_eq!(
                balance_total(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING)
                    .unwrap(),
                350
            );
            let actual_slots_value =
                Pallet::get_slots_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_slots_value = vec![(VALIDATOR_ALPHA, 140), (VALIDATOR_BETA, 210)];
            assert_eq!(actual_slots_value, expected_slots_value);
            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 850);
            // New commit instances added for raise commit
            // Commit info check
            let commit_info = CommitMap::get((ALICE, STAKING)).unwrap();
            let commits = commit_info.commits();
            // The initial commit instance should be at index 0
            let initial_commit = commits.get(0).unwrap();
            assert_eq!(
                receipt_deposit_value(initial_commit).unwrap(),
                STANDARD_COMMIT
            );
            // The raise commit instance should be at index 1
            let raise_commit = commits.get(1).unwrap();
            assert_eq!(receipt_deposit_value(raise_commit).unwrap(), SMALL_COMMIT);

            #[cfg(not(feature = "dev"))]
            System::assert_last_event(Event::CommitRaised {
                 who: ALICE, 
                 reason: STAKING, 
                 digest: POOL_MANAGED_STAKING, 
                 value: SMALL_COMMIT
                }
                .into()
            );

            #[cfg(feature = "dev")]
            System::assert_last_event(Event::CommitRaised {
                 who: ALICE, 
                 reason: STAKING, 
                 model: DigestVariant::Pool(POOL_MANAGED_STAKING), 
                 value: SMALL_COMMIT
                }
                .into()
            );  
        })
    }

    #[test]
    fn raise_commit_err_commit_not_found() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_err!(
                Pallet::raise_commit(
                    &ALICE,
                    &ESCROW,
                    SMALL_COMMIT,
                    &Directive::new(Precision::Exact, Fortitude::Force)
                ),
                Error::CommitNotFound
            );
        });
    }

    #[test]
    fn raise_commit_err_insifficient_funds() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let insufficient_commit = SMALL_COMMIT;
            assert_err!(
                Pallet::raise_commit(
                    &ALICE,
                    &STAKING,
                    insufficient_commit,
                    &Directive::new(Precision::Exact, Fortitude::Polite)
                ),
                Error::InsufficientFunds
            );
        })
    }

    #[test]
    fn raise_commit_err_marker_commit_not_allowed() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let invalid_commit_val = ZERO_VALUE;
            assert_err!(
                Pallet::raise_commit(
                    &ALICE,
                    &STAKING,
                    invalid_commit_val,
                    &Directive::new(Precision::Exact, Fortitude::Polite)
                ),
                Error::MarkerCommitNotAllowed
            );
        })
    }

    #[test]
    fn resolve_commit_success_for_direct() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_RUNTIME_UPGRADE,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // Before resolving the commit
            assert_eq!(
                AssetOf::balance_frozen(&GOVERNANCE, &ALICE),
                STANDARD_COMMIT
            );
            assert_eq!(AssetOf::balance(&ALICE), INITIAL_BALANCE);
            let commit_value = Pallet::get_commit_value(&ALICE, &GOVERNANCE).unwrap();
            assert_eq!(commit_value, STANDARD_COMMIT);

            let reason_value = Pallet::get_total_value(&GOVERNANCE);
            assert_eq!(reason_value, 250);
            // Resolve commit
            assert_ok!(Pallet::resolve_commit(&ALICE, &GOVERNANCE));
            // After resolving the commit
            assert_eq!(AssetOf::balance(&ALICE), 1250);
            assert_eq!(AssetOf::balance_frozen(&GOVERNANCE, &ALICE), ZERO_VALUE);
            assert_err!(
                Pallet::commit_exists(&ALICE, &GOVERNANCE),
                Error::CommitNotFound
            );
            let digets_value =
                Pallet::get_digest_value(&GOVERNANCE, &PROPOSAL_RUNTIME_UPGRADE).unwrap();
            assert_eq!(digets_value, ZERO_VALUE);

            let reason_value = Pallet::get_total_value(&GOVERNANCE);
            assert_eq!(reason_value, ZERO_VALUE);

            #[cfg(not(feature = "dev"))]
            System::assert_last_event(Event::CommitResolved { 
                who: ALICE, 
                reason: GOVERNANCE, 
                digest: PROPOSAL_RUNTIME_UPGRADE, 
                value: 250
                }
                .into()
            );

            #[cfg(feature = "dev")]
            System::assert_last_event(Event::CommitResolved { 
                who: ALICE, 
                reason: GOVERNANCE, 
                model: DigestVariant::Direct(PROPOSAL_RUNTIME_UPGRADE), 
                value: 250
                }
                .into()
            );
        })
    }

    #[test]
    fn resolve_commit_for_direct_success_with_reward() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), STANDARD_COMMIT);
            assert_eq!(
                Pallet::get_commit_value(&ALICE, &STAKING),
                Ok(STANDARD_COMMIT)
            );
            assert_eq!(
                Pallet::get_digest_value(&STAKING, &VALIDATOR_ALPHA,),
                Ok(STANDARD_COMMIT)
            );
            // Reward manupulation
            let new_reward_value = STANDARD_COMMIT + STANDARD_REWARD; // 250 + 50 -> 300
            Pallet::set_digest_value(
                &STAKING,
                &VALIDATOR_ALPHA,
                new_reward_value,
                &Default::default(),
            )
            .unwrap();
            assert_eq!(
                Pallet::get_digest_value(&STAKING, &VALIDATOR_ALPHA,),
                Ok(new_reward_value)
            );
            assert_eq!(Pallet::get_commit_value(&ALICE, &STAKING), Ok(300));

            // Resolving the commit after reward accumulation
            assert_ok!(Pallet::resolve_commit(&ALICE, &STAKING));

            assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), ZERO_VALUE);
            assert_eq!(AssetOf::balance(&ALICE), 1300); // existing balance + deposit + reward = 1000 + 250 + 50 = 1300
            assert_err!(
                Pallet::commit_exists(&ALICE, &STAKING),
                Error::CommitNotFound
            );
            assert_eq!(Pallet::get_digest_value(&STAKING, &VALIDATOR_ALPHA), Ok(0));
        });
    }

    #[test]
    fn resolve_commit_withdraw_direct_with_penalty_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();

            assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), STANDARD_COMMIT);
            assert_eq!(
                Pallet::get_commit_value(&ALICE, &STAKING),
                Ok(STANDARD_COMMIT)
            );
            let digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_ALPHA).unwrap();
            // Penalty manupulation
            let new_penalty_value = digest_value - STANDARD_PENALTY; // 250 - 100 -> 150;
            Pallet::set_digest_value(
                &STAKING,
                &VALIDATOR_ALPHA,
                new_penalty_value,
                &Default::default(),
            )
            .unwrap();
            assert_eq!(Pallet::get_commit_value(&ALICE, &STAKING), Ok(150));
            // Resolving the commit after penalty
            assert_ok!(Pallet::resolve_commit(&ALICE, &STAKING));

            assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), ZERO_VALUE);
            assert_eq!(AssetOf::balance(&ALICE), 1150); // existing balance + deposit - penalty = 1000 + 250 - 100 = 1150
            assert_err!(
                Pallet::commit_exists(&ALICE, &STAKING),
                Error::CommitNotFound
            );
            assert_eq!(
                Pallet::get_digest_value(&STAKING, &VALIDATOR_ALPHA),
                Ok(ZERO_VALUE)
            );
        });
    }

    #[test]
    fn resolve_direct_commit_with_penalty_and_reward_by_entry_time() {
        commit_test_ext().execute_with(|| {
            set_user_balance_and_hold(ALICE, 10000, 5000).unwrap();
            set_user_balance_and_hold(BOB, 10000, 5000).unwrap();
            set_user_balance_and_hold(CHARLIE, 10000, 5000).unwrap();

            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                1000,
                &Directive::new(Precision::Exact, Fortitude::Polite),
            )
            .unwrap();

            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &VALIDATOR_ALPHA,
                1000,
                &Directive::new(Precision::Exact, Fortitude::Polite),
            )
            .unwrap();

            // Penalty manupulation
            Pallet::set_digest_value(&STAKING, &VALIDATOR_ALPHA, 900, &Default::default()).unwrap();

            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_ALPHA,
                400,
                &Directive::new(Precision::Exact, Fortitude::Polite),
            )
            .unwrap();

            // Reward manupulation
            Pallet::set_digest_value(&STAKING, &VALIDATOR_ALPHA, 1500, &Default::default())
                .unwrap();

            // Both, alice and charlie absorbed the penalty and reward shock according to their weight
            let alice_resolved = Pallet::resolve_commit(&ALICE, &STAKING).unwrap();
            assert_eq!(alice_resolved, 519);

            let charlie_resolved = Pallet::resolve_commit(&CHARLIE, &STAKING).unwrap();
            assert_eq!(charlie_resolved, 519);

            // Since, bob placed a commit after the penalty he dosen't
            // absorbed the penalty shock but absorbs reward shock.
            let bob_resolved = Pallet::resolve_commit(&BOB, &STAKING).unwrap();
            assert_eq!(bob_resolved, 462);
        });
    }

    #[test]
    fn resolve_commit_success_for_index() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            prepare_and_initiate_index(
                MIKE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // Alice balance state before resolving
            assert_eq!(AssetOf::balance(&ALICE), INITIAL_BALANCE);
            assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), 500);
            assert_eq!(Pallet::get_commit_value(&ALICE, &STAKING), Ok(500));
            let actual_entries_value =
                Pallet::get_entries_value(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_entries_value = vec![(VALIDATOR_ALPHA, 200), (VALIDATOR_BETA, 300)];
            assert_eq!(actual_entries_value, expected_entries_value);

            assert_ok!(Pallet::resolve_commit(&ALICE, &STAKING));
            // alice's balance state after resolving
            assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), ZERO_VALUE);
            assert_eq!(AssetOf::balance(&ALICE), 1500); // existing balance + resolved balance -> 1000 + 500 = 1500
            assert_err!(
                Pallet::commit_exists(&ALICE, &STAKING),
                Error::CommitNotFound
            );
            assert_eq!(
                Pallet::get_index_value(&STAKING, &INDEX_OPTIMIZED_STAKING),
                Ok(ZERO_VALUE)
            );
            let actual_entries_value =
                Pallet::get_entries_value(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_entries_value =
                vec![(VALIDATOR_ALPHA, ZERO_VALUE), (VALIDATOR_BETA, ZERO_VALUE)];
            assert_eq!(actual_entries_value, expected_entries_value);

            #[cfg(not(feature = "dev"))]
            System::assert_last_event(Event::CommitResolved { 
                who: ALICE, 
                reason: STAKING, 
                digest: INDEX_OPTIMIZED_STAKING, 
                value: 500
                }
                .into()
            );

            #[cfg(feature = "dev")]
            System::assert_last_event(Event::CommitResolved { 
                who: ALICE, 
                reason: STAKING, 
                model: DigestVariant::Index(INDEX_OPTIMIZED_STAKING), 
                value: 500
                }
                .into()
            );
        })
    }

    #[test]
    fn resolve_commit_index_with_rewards() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                350,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            assert_eq!(AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE), 150);
            assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), 350);
            assert_eq!(Pallet::get_commit_value(&ALICE, &STAKING), Ok(350));
            // index balance
            assert_eq!(
                Pallet::get_index_value(&STAKING, &INDEX_OPTIMIZED_STAKING),
                Ok(350)
            );
            let actual_entries_value =
                Pallet::get_entries_value(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_entries_value = vec![(VALIDATOR_ALPHA, 140), (VALIDATOR_BETA, 210)];
            assert_eq!(actual_entries_value, expected_entries_value);
            // Stimulating reward senario
            // Apply rewards: Alpha 14 -> 18 (+4), Beta 21 -> 27 (+6)
            let new_alpha_reward_value = 180;
            Pallet::set_digest_value(
                &STAKING,
                &VALIDATOR_ALPHA,
                new_alpha_reward_value,
                &Default::default(),
            )
            .unwrap();
            let new_beta_reward_value = 270;
            Pallet::set_digest_value(
                &STAKING,
                &VALIDATOR_BETA,
                new_beta_reward_value,
                &Default::default(),
            )
            .unwrap();
            // Expected index value: 180 + 270 = 450
            assert_eq!(
                Pallet::get_index_value(&STAKING, &INDEX_OPTIMIZED_STAKING),
                Ok(450)
            );

            let actual_entries_value =
                Pallet::get_entries_value(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_entries_value = vec![(VALIDATOR_ALPHA, 180), (VALIDATOR_BETA, 270)];
            assert_eq!(actual_entries_value, expected_entries_value);
            // Alice resolves commitment and gets 450 tokens
            assert_ok!(Pallet::resolve_commit(&ALICE, &STAKING));
            // Final balance: 1000 (existing) + 450 (resolved with precision loss) = 1450
            assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), 0);
            assert_eq!(AssetOf::balance(&ALICE), 1450);
            assert_err!(
                Pallet::commit_exists(&ALICE, &STAKING),
                Error::CommitNotFound
            );
            assert_eq!(Pallet::get_digest_value(&STAKING, &VALIDATOR_ALPHA), Ok(0));
            assert_eq!(Pallet::get_digest_value(&STAKING, &VALIDATOR_BETA), Ok(0));
        });
    }

    #[test]
    fn resolve_commit_withdraw_index_with_reward_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            prepare_and_initiate_index(
                MIKE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                LARGE_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            // Alice balance state before resolvig
            assert_eq!(
                AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE),
                ZERO_VALUE
            );
            assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), LARGE_COMMIT);
            assert_eq!(Pallet::get_commit_value(&ALICE, &STAKING), Ok(LARGE_COMMIT));
            assert_eq!(
                Pallet::get_index_value(&STAKING, &INDEX_OPTIMIZED_STAKING),
                Ok(500)
            );
            let actual_entries_value =
                Pallet::get_entries_value(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_entries_value = vec![(VALIDATOR_ALPHA, 200), (VALIDATOR_BETA, 300)];
            assert_eq!(actual_entries_value, expected_entries_value);
            // Stimulating reward senario
            let alpha_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_ALPHA).unwrap();
            assert_eq!(alpha_digest_value, 450);
            let new_alpha_reward_value = alpha_digest_value + STANDARD_REWARD;
            Pallet::set_digest_value(
                &STAKING,
                &VALIDATOR_ALPHA,
                new_alpha_reward_value,
                &Default::default(),
            )
            .unwrap();
            let beta_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_BETA).unwrap();
            assert_eq!(beta_digest_value, 550);
            let new_beta_reward_value = beta_digest_value + STANDARD_REWARD;
            Pallet::set_digest_value(
                &STAKING,
                &VALIDATOR_BETA,
                new_beta_reward_value,
                &Default::default(),
            )
            .unwrap();
            // Index value after reward application
            // Note: There might be slight precision loss due to fixed-point bias arithmetic
            // Alice's portion should reflect proportional rewards
            // VALIDATOR_ALPHA bias: 500/450 ~= 1.111, VALIDATOR_BETA bias: 600/550 ~= 1.091
            // Alice's new value: (200 * 1.111) + (300 * 1.091) ~= 222 + 327 = 549
            assert_eq!(
                Pallet::get_index_value(&STAKING, &INDEX_OPTIMIZED_STAKING),
                Ok(549)
            );
            let actual_entries_value =
                Pallet::get_entries_value(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_entries_value = vec![(VALIDATOR_ALPHA, 222), (VALIDATOR_BETA, 327)];
            assert_eq!(actual_entries_value, expected_entries_value);
            // Alice resolves commitment
            assert_ok!(Pallet::resolve_commit(&ALICE, &STAKING));
            assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), ZERO_VALUE);
            // Final balance = 1000 (existing) + 549 (resolved with precision loss) = 1549
            assert_eq!(AssetOf::balance(&ALICE), 1549);
            assert_err!(
                Pallet::commit_exists(&ALICE, &STAKING),
                Error::CommitNotFound
            );
            // Remaining digest balances after alice withdrawal
            assert_eq!(
                Pallet::get_digest_value(&STAKING, &VALIDATOR_ALPHA),
                Ok(278)
            );
            assert_eq!(Pallet::get_digest_value(&STAKING, &VALIDATOR_BETA), Ok(273));
        });
    }

    #[test]
    fn resolve_commit_withdraw_index_with_penalty_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            prepare_and_initiate_index(
                MIKE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), LARGE_COMMIT);
            assert_eq!(Pallet::get_commit_value(&ALICE, &STAKING), Ok(LARGE_COMMIT));
            assert_eq!(
                Pallet::get_index_value(&STAKING, &INDEX_OPTIMIZED_STAKING),
                Ok(500)
            );
            let actual_entries_value =
                Pallet::get_entries_value(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_entries_value = vec![(VALIDATOR_ALPHA, 200), (VALIDATOR_BETA, 300)];
            assert_eq!(actual_entries_value, expected_entries_value);
            // Stimulating penalty senario
            let alpha_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_ALPHA).unwrap();
            assert_eq!(alpha_digest_value, 450);
            let new_alpha_penalty_value = alpha_digest_value - STANDARD_PENALTY;
            Pallet::set_digest_value(
                &STAKING,
                &VALIDATOR_ALPHA,
                new_alpha_penalty_value,
                &Default::default(),
            )
            .unwrap();
            let beta_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_BETA).unwrap();
            assert_eq!(beta_digest_value, 550);
            let new_beta_penalty_value = beta_digest_value - STANDARD_PENALTY;
            Pallet::set_digest_value(
                &STAKING,
                &VALIDATOR_BETA,
                new_beta_penalty_value,
                &Default::default(),
            )
            .unwrap();
            // Index value after penalty application
            // Note: There might be slight precision loss due to fixed-point bias arithmetic
            // Alice's portion should reflect proportional penalty suffer
            assert_eq!(
                Pallet::get_index_value(&STAKING, &INDEX_OPTIMIZED_STAKING),
                Ok(400)
            );
            let actual_entries_value =
                Pallet::get_entries_value(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_entries_value = vec![(VALIDATOR_ALPHA, 155), (VALIDATOR_BETA, 245)];
            assert_eq!(actual_entries_value, expected_entries_value);
            // Alice resolves commitment
            assert_ok!(Pallet::resolve_commit(&ALICE, &STAKING));
            // Final balance = 1000 (existing) + 400 (resolved) = 1400
            assert_eq!(AssetOf::balance(&ALICE), 1400);
            assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), ZERO_VALUE);
            assert_err!(
                Pallet::commit_exists(&ALICE, &STAKING),
                Error::CommitNotFound
            );
            // Remaining digest balances after alice withdrawal
            assert_eq!(
                Pallet::get_digest_value(&STAKING, &VALIDATOR_ALPHA),
                Ok(195)
            );
            assert_eq!(Pallet::get_digest_value(&STAKING, &VALIDATOR_BETA), Ok(205));
        });
    }

    #[test]
    fn resolve_index_commit_with_penalty_and_reward_distribution_by_entry_time() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(ALAN).unwrap();
            set_default_user_balance_and_standard_hold(NIX).unwrap();
            set_default_user_balance_and_standard_hold(MIKE).unwrap();
            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            prepare_and_initiate_index(
                MIKE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();

            Pallet::place_commit(
                &ALAN,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();

            Pallet::place_commit(
                &MIKE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();

            let alan_entries_value =
                Pallet::get_entries_value_for(&ALAN, &STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let mike_entries_value =
                Pallet::get_entries_value_for(&MIKE, &STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_alan_entries_value = vec![(VALIDATOR_ALPHA, 200), (VALIDATOR_BETA, 300)];
            let expected_mike_entries_value = vec![(VALIDATOR_ALPHA, 100), (VALIDATOR_BETA, 150)];
            assert_eq!(expected_alan_entries_value, alan_entries_value);
            assert_eq!(expected_mike_entries_value, mike_entries_value);

            // Penalty manupulation
            Pallet::set_digest_value(&STAKING, &VALIDATOR_ALPHA, 500, &Default::default()).unwrap();

            let alan_entries_value =
                Pallet::get_entries_value_for(&ALAN, &STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let mike_entries_value =
                Pallet::get_entries_value_for(&MIKE, &STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_alan_entries_value = vec![(VALIDATOR_ALPHA, 181), (VALIDATOR_BETA, 300)];
            let expected_mike_entries_value = vec![(VALIDATOR_ALPHA, 90), (VALIDATOR_BETA, 150)];
            assert_eq!(expected_alan_entries_value, alan_entries_value);
            assert_eq!(expected_mike_entries_value, mike_entries_value);

            let charlie_digest_value = Pallet::get_commit_value(&CHARLIE, &STAKING).unwrap();
            let expected_charlie_digest_value = 227;
            assert_eq!(expected_charlie_digest_value, charlie_digest_value);

            Pallet::place_commit(
                &NIX,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();

            let nix_entries_value =
                Pallet::get_entries_value_for(&NIX, &STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_nix_entries_value = vec![(VALIDATOR_ALPHA, 99), (VALIDATOR_BETA, 150)];
            assert_eq!(expected_nix_entries_value, nix_entries_value);

            // Reward manupulation
            Pallet::set_digest_value(&STAKING, &VALIDATOR_ALPHA, 1000, &Default::default())
                .unwrap();

            let alan_entries_value =
                Pallet::get_entries_value_for(&ALAN, &STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let mike_entries_value =
                Pallet::get_entries_value_for(&MIKE, &STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let nix_entries_value =
                Pallet::get_entries_value_for(&NIX, &STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_alan_entries_value = vec![(VALIDATOR_ALPHA, 303), (VALIDATOR_BETA, 300)];
            let expected_mike_entries_value = vec![(VALIDATOR_ALPHA, 151), (VALIDATOR_BETA, 150)];
            let expected_nix_entries_value = vec![(VALIDATOR_ALPHA, 166), (VALIDATOR_BETA, 150)];
            assert_eq!(expected_alan_entries_value, alan_entries_value);
            assert_eq!(expected_mike_entries_value, mike_entries_value);
            assert_eq!(expected_nix_entries_value, nix_entries_value);

            let charlie_digest_value = Pallet::get_commit_value(&CHARLIE, &STAKING).unwrap();
            let expected_charlie_digest_value = 378;
            assert_eq!(expected_charlie_digest_value, charlie_digest_value);

            // Both, mike and alan absorbed the penalty and reward shock according to their weight
            let mike_resolved = Pallet::resolve_commit(&MIKE, &STAKING).unwrap();
            assert_eq!(mike_resolved, 301);

            let alan_resolved = Pallet::resolve_commit(&ALAN, &STAKING).unwrap();
            assert_eq!(alan_resolved, 603);

            // Since, nix placed a commit after the penalty he dosen't
            // absorbed the penalty shock but absorbs reward shock.
            let nix_resolved = Pallet::resolve_commit(&NIX, &STAKING).unwrap();
            assert_eq!(nix_resolved, 316);

            // Charlie, as the last withdrawer, receives the remaining dust from rounding.
            let charlie_resolved = Pallet::resolve_commit(&CHARLIE, &STAKING).unwrap();
            assert_eq!(charlie_resolved, 380);
        })
    }

    #[test]
    fn resolve_commit_success_for_pool() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            set_default_user_balance_and_standard_hold(MIKE).unwrap();

            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_MAJOR),
                (VALIDATOR_BETA, SHARE_DOMINANT),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_STANDARD,
            )
            .unwrap();

            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // Before resolving the commit
            let pool_info = PoolMap::get((STAKING, POOL_MANAGED_STAKING)).unwrap();
            let pool_balance_of = pool_info.balance();
            assert_ok!(has_deposits(
                &pool_balance_of,
                &Default::default(),
                &POOL_MANAGED_STAKING
            ));
            assert_eq!(
                balance_total(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING)
                    .unwrap(),
                STANDARD_COMMIT
            );
            let pool_capital = pool_info.capital();
            assert_eq!(pool_capital, 100);
            let actual_slots_value =
                Pallet::get_slots_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_slots_value = vec![(VALIDATOR_ALPHA, 100), (VALIDATOR_BETA, 150)];
            assert_eq!(actual_slots_value, expected_slots_value);

            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 750);
            // resolve the commit
            assert_ok!(Pallet::resolve_commit(&ALICE, &STAKING));
            // After resolving the commit
            let pool_info = PoolMap::get((STAKING, POOL_MANAGED_STAKING)).unwrap();
            let pool_balance_of = pool_info.balance();
            assert!(
                has_deposits(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING).is_err()
            );
            assert_eq!(
                balance_total(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING)
                    .unwrap(),
                ZERO_VALUE,
            );
            let pool_capital = pool_info.capital();
            assert_eq!(pool_capital, 100);
            let actual_slots_value =
                Pallet::get_slots_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_slots_value =
                vec![(VALIDATOR_ALPHA, ZERO_VALUE), (VALIDATOR_BETA, ZERO_VALUE)];
            assert_eq!(actual_slots_value, expected_slots_value);
            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 500);
            // Alice Balance check
            // Final balance = 1000(initial) + 250(commit) - 10%(commission) -> 1225
            let expected_balance =
                INITIAL_BALANCE + STANDARD_COMMIT - (COMMISSION_STANDARD * STANDARD_COMMIT);
            dbg!(expected_balance);
            let actual_balance = AssetOf::balance(&ALICE);
            assert_eq!(actual_balance, expected_balance);
            assert_eq!(AssetOf::balance_frozen(&STAKING, &ALICE), ZERO_VALUE);
            // Mike Balnce check
            // Final balance = 1000(initial) + 10%(commission) -> 1025
            let expected_balance = INITIAL_BALANCE + (COMMISSION_STANDARD * STANDARD_COMMIT);
            let actual_balance = AssetOf::balance(&MIKE);
            assert_eq!(actual_balance, expected_balance);

            #[cfg(not(feature = "dev"))]
            System::assert_last_event(Event::CommitResolved { 
                who: ALICE, 
                reason: STAKING, 
                digest: POOL_MANAGED_STAKING, 
                value: 225
                }
                .into()
            );

            #[cfg(feature = "dev")]
            System::assert_last_event(Event::CommitResolved { 
                who: ALICE, 
                reason: STAKING, 
                model: DigestVariant::Pool(POOL_MANAGED_STAKING), 
                value: 225
                }
                .into()
            );
        })
    }

    #[test]
    fn resolve_commit_withdraw_pool_with_reward_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            set_default_user_balance_and_standard_hold(ALAN).unwrap();
            set_default_user_balance_and_standard_hold(MIKE).unwrap();

            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_MAJOR),
                (VALIDATOR_BETA, SHARE_DOMINANT),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_STANDARD,
            )
            .unwrap();
            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // Before resolving the commit
            let actual_pool_balance =
                Pallet::get_pool_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            assert_eq!(actual_pool_balance, LARGE_COMMIT);
            let actual_slots_value =
                Pallet::get_slots_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_slots_value = vec![(VALIDATOR_ALPHA, 200), (VALIDATOR_BETA, 300)];
            assert_eq!(actual_slots_value, expected_slots_value);

            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 1000);
            // Simulating reward senario
            let alpha_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_ALPHA).unwrap();
            assert_eq!(alpha_digest_value, 450);
            let new_alpha_reward_value = alpha_digest_value + STANDARD_REWARD; //450 + 50(reward) -> 500
            Pallet::set_digest_value(
                &STAKING,
                &VALIDATOR_ALPHA,
                new_alpha_reward_value,
                &Default::default(),
            )
            .unwrap();
            let beta_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_BETA).unwrap();
            assert_eq!(beta_digest_value, 550);
            let new_beta_reward_value = beta_digest_value + STANDARD_REWARD; //550 + 50(reward) -> 600
            Pallet::set_digest_value(
                &STAKING,
                &VALIDATOR_BETA,
                new_beta_reward_value,
                &Default::default(),
            )
            .unwrap();
            // Underlying digests value after reward senario
            let alpha_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_ALPHA).unwrap();
            assert_eq!(alpha_digest_value, 500);
            let beta_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_BETA).unwrap();
            assert_eq!(beta_digest_value, 600);
            // Pool value updated
            let pool_value = Pallet::get_pool_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            // precision loss catered
            assert_eq!(pool_value, 549);
            let actual_slots_value =
                Pallet::get_slots_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_slots_value = vec![(VALIDATOR_ALPHA, 222), (VALIDATOR_BETA, 327)];
            assert_eq!(actual_slots_value, expected_slots_value);

            // resolve the commit
            assert_ok!(Pallet::resolve_commit(&CHARLIE, &STAKING));

            // After resolving the commit
            let pool_info = PoolMap::get((STAKING, POOL_MANAGED_STAKING)).unwrap();
            let pool_balance_of = pool_info.balance();
            assert!(
                has_deposits(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING).is_err()
            );
            assert_eq!(
                balance_total(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING)
                    .unwrap(),
                0,
            );

            let actual_slots_value =
                Pallet::get_slots_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_slots_value = vec![(VALIDATOR_ALPHA, 0), (VALIDATOR_BETA, 0)];
            assert_eq!(actual_slots_value, expected_slots_value);

            // Underlying digests value after resolving pool commit
            let alpha_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_ALPHA).unwrap();
            assert_eq!(alpha_digest_value, 278);
            let beta_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_BETA).unwrap();
            assert_eq!(beta_digest_value, 273);

            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 551);

            // Charlies balance check
            let actual_balance = AssetOf::balance(&CHARLIE);
            let expected_balance = INITIAL_BALANCE + 495; // initial_balance + resolved_blance with commission deduction

            assert_eq!(actual_balance, expected_balance);
            let actual_freeze_balance = AssetOf::balance_frozen(&STAKING, &CHARLIE);
            assert_eq!(actual_freeze_balance, ZERO_VALUE);

            // Mike balance check for commison settlement
            let actual_balance = AssetOf::balance(&MIKE);
            let expected_balance = INITIAL_BALANCE + 54; // initial_balance + commission for the resolved commit
            assert_eq!(actual_balance, expected_balance);
        });
    }

    #[test]
    fn resolve_commit_withdraw_pool_with_penalty_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            set_default_user_balance_and_standard_hold(ALAN).unwrap();
            set_default_user_balance_and_standard_hold(MIKE).unwrap();

            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_MAJOR),
                (VALIDATOR_BETA, SHARE_DOMINANT),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_STANDARD,
            )
            .unwrap();
            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // Before resolving the commit
            let actual_pool_balance =
                Pallet::get_pool_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            assert_eq!(actual_pool_balance, LARGE_COMMIT);
            let actual_slots_value =
                Pallet::get_slots_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_slots_value = vec![(VALIDATOR_ALPHA, 200), (VALIDATOR_BETA, 300)];
            assert_eq!(actual_slots_value, expected_slots_value);

            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 1000);
            // Simulating penalty senario
            let alpha_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_ALPHA).unwrap();
            assert_eq!(alpha_digest_value, 450);
            let new_alpha_reward_value = alpha_digest_value - STANDARD_PENALTY; //450 - 100(penalty) -> 350
            Pallet::set_digest_value(
                &STAKING,
                &VALIDATOR_ALPHA,
                new_alpha_reward_value,
                &Default::default(),
            )
            .unwrap();
            let beta_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_BETA).unwrap();
            assert_eq!(beta_digest_value, 550);
            let new_beta_reward_value = beta_digest_value - SMALL_PENALTY; //550 - 10(reward) -> 540
            Pallet::set_digest_value(
                &STAKING,
                &VALIDATOR_BETA,
                new_beta_reward_value,
                &Default::default(),
            )
            .unwrap();
            // Underlying digests value after penalty senario
            let alpha_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_ALPHA).unwrap();
            assert_eq!(alpha_digest_value, 350);
            let beta_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_BETA).unwrap();
            assert_eq!(beta_digest_value, 540);
            // Pool value updated
            let pool_value = Pallet::get_pool_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            assert_eq!(pool_value, 449);
            let actual_slots_value =
                Pallet::get_slots_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_slots_value = vec![(VALIDATOR_ALPHA, 155), (VALIDATOR_BETA, 294)];
            assert_eq!(actual_slots_value, expected_slots_value);

            // resolve the commit
            assert_ok!(Pallet::resolve_commit(&CHARLIE, &STAKING));

            // After resolving the commit
            let pool_info = PoolMap::get((STAKING, POOL_MANAGED_STAKING)).unwrap();
            let pool_balance_of = pool_info.balance();
            assert!(
                has_deposits(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING).is_err()
            );
            assert_eq!(
                balance_total(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING)
                    .unwrap(),
                0,
            );

            let actual_slots_value =
                Pallet::get_slots_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_slots_value = vec![(VALIDATOR_ALPHA, 0), (VALIDATOR_BETA, 0)];
            assert_eq!(actual_slots_value, expected_slots_value);

            // Underlying digests value after resolving pool commit
            let alpha_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_ALPHA).unwrap();
            assert_eq!(alpha_digest_value, 195);
            let beta_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_BETA).unwrap();
            assert_eq!(beta_digest_value, 246);

            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 441);

            // Charlies balance check
            let actual_balance = AssetOf::balance(&CHARLIE);
            let expected_balance = INITIAL_BALANCE + 405; // initial_balance + resolved_blance with commission deduction

            assert_eq!(actual_balance, expected_balance);
            let actual_freeze_balance = AssetOf::balance_frozen(&STAKING, &CHARLIE);
            assert_eq!(actual_freeze_balance, ZERO_VALUE);

            // Mike balance check for commison settlement
            let actual_balance = AssetOf::balance(&MIKE);
            let expected_balance = INITIAL_BALANCE + 44; // initial_balance + commission for the resolved commit
            assert_eq!(actual_balance, expected_balance);
        });
    }

    #[test]
    fn resolve_pool_commit_with_penalty_and_reward_distribution_by_entry_time() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(ALAN).unwrap();
            set_default_user_balance_and_standard_hold(NIX).unwrap();
            set_default_user_balance_and_standard_hold(MIKE).unwrap();
            set_default_user_balance_and_standard_hold(DAVE).unwrap();

            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();

            prepare_and_initiate_pool(
                DAVE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_STANDARD,
            )
            .unwrap();

            Pallet::place_commit(
                &ALAN,
                &STAKING,
                &POOL_MANAGED_STAKING,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();

            Pallet::place_commit(
                &MIKE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();

            let pool_value = Pallet::get_pool_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            assert_eq!(pool_value, 750);
            let alan_slots_value =
                Pallet::get_slots_value_for(&ALAN, &STAKING, &POOL_MANAGED_STAKING).unwrap();
            let mike_slots_value =
                Pallet::get_slots_value_for(&MIKE, &STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_alan_slots_value = vec![(VALIDATOR_ALPHA, 200), (VALIDATOR_BETA, 300)];
            let expected_mike_slots_value = vec![(VALIDATOR_ALPHA, 100), (VALIDATOR_BETA, 150)];
            assert_eq!(expected_alan_slots_value, alan_slots_value);
            assert_eq!(expected_mike_slots_value, mike_slots_value);

            // Penalty manupulation
            Pallet::set_digest_value(&STAKING, &VALIDATOR_ALPHA, 500, &Default::default()).unwrap();

            let pool_value = Pallet::get_pool_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            assert_eq!(pool_value, 722);

            let alan_slots_value =
                Pallet::get_slots_value_for(&ALAN, &STAKING, &POOL_MANAGED_STAKING).unwrap();
            let mike_slots_value =
                Pallet::get_slots_value_for(&MIKE, &STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_alan_slots_value = vec![(VALIDATOR_ALPHA, 192), (VALIDATOR_BETA, 288)];
            let expected_mike_slots_value = vec![(VALIDATOR_ALPHA, 96), (VALIDATOR_BETA, 144)];
            assert_eq!(expected_alan_slots_value, alan_slots_value);
            assert_eq!(expected_mike_slots_value, mike_slots_value);

            let charlie_digest_value = Pallet::get_commit_value(&CHARLIE, &STAKING).unwrap();
            let expected_charlie_digest_value = 227;
            assert_eq!(expected_charlie_digest_value, charlie_digest_value);

            Pallet::place_commit(
                &NIX,
                &STAKING,
                &POOL_MANAGED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();

            let nix_slots_value =
                Pallet::get_slots_value_for(&NIX, &STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_nix_slots_value = vec![(VALIDATOR_ALPHA, 99), (VALIDATOR_BETA, 148)];
            assert_eq!(expected_nix_slots_value, nix_slots_value);

            // Reward manupulation
            Pallet::set_digest_value(&STAKING, &VALIDATOR_ALPHA, 1000, &Default::default())
                .unwrap();

            let pool_value = Pallet::get_pool_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            assert_eq!(pool_value, 1213);
            let alan_slots_value =
                Pallet::get_slots_value_for(&ALAN, &STAKING, &POOL_MANAGED_STAKING).unwrap();
            let mike_slots_value =
                Pallet::get_slots_value_for(&MIKE, &STAKING, &POOL_MANAGED_STAKING).unwrap();
            let nix_slots_value =
                Pallet::get_slots_value_for(&NIX, &STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_alan_slots_value = vec![(VALIDATOR_ALPHA, 240), (VALIDATOR_BETA, 360)];
            let expected_mike_slots_value = vec![(VALIDATOR_ALPHA, 120), (VALIDATOR_BETA, 180)];
            let expected_nix_slots_value = vec![(VALIDATOR_ALPHA, 124), (VALIDATOR_BETA, 186)];
            assert_eq!(expected_alan_slots_value, alan_slots_value);
            assert_eq!(expected_mike_slots_value, mike_slots_value);
            assert_eq!(expected_nix_slots_value, nix_slots_value);

            let charlie_digest_value = Pallet::get_commit_value(&CHARLIE, &STAKING).unwrap();
            let expected_charlie_digest_value = 369;
            assert_eq!(expected_charlie_digest_value, charlie_digest_value);

            // Both, mike and alan absorbed the penalty and reward shock according to their weight
            let mike_resolved = Pallet::resolve_commit(&MIKE, &STAKING).unwrap();
            assert_eq!(mike_resolved, 270);

            let alan_resolved = Pallet::resolve_commit(&ALAN, &STAKING).unwrap();
            assert_eq!(alan_resolved, 540);

            // Since, nix placed a commit after the penalty he dosen't
            // absorbed the penalty shock but absorbs reward shock.
            let nix_resolved = Pallet::resolve_commit(&NIX, &STAKING).unwrap();
            assert_eq!(nix_resolved, 278);

            // Charlie, as the last withdrawer, receives the remaining dust from rounding.
            let charlie_resolved = Pallet::resolve_commit(&CHARLIE, &STAKING).unwrap();
            assert_eq!(charlie_resolved, 374);

            // dev gets his commission from all above resolutions
            let dev_balance = AssetOf::balance(&DAVE);
            assert_eq!(dev_balance, 1122); // 1000 -> 1122 (commission)
        })
    }

    #[test]
    fn resolve_commit_withdraw_pool_with_hundred_percent_commisison() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            set_default_user_balance_and_standard_hold(ALAN).unwrap();
            set_default_user_balance_and_standard_hold(MIKE).unwrap();

            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_MAJOR),
                (VALIDATOR_BETA, SHARE_DOMINANT),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_MAX,
            )
            .unwrap();
            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // Before resolving the commit
            let actual_pool_balance =
                Pallet::get_pool_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            assert_eq!(actual_pool_balance, LARGE_COMMIT);
            let actual_slots_value =
                Pallet::get_slots_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_slots_value = vec![(VALIDATOR_ALPHA, 200), (VALIDATOR_BETA, 300)];
            assert_eq!(actual_slots_value, expected_slots_value);

            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 1000);

            // resolve the commit
            assert_ok!(Pallet::resolve_commit(&CHARLIE, &STAKING));

            // After resolving the commit
            let pool_info = PoolMap::get((STAKING, POOL_MANAGED_STAKING)).unwrap();
            let pool_balance_of = pool_info.balance();
            assert!(
                has_deposits(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING).is_err()
            );
            assert_eq!(
                balance_total(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING)
                    .unwrap(),
                0,
            );

            let actual_slots_value =
                Pallet::get_slots_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_slots_value = vec![(VALIDATOR_ALPHA, 0), (VALIDATOR_BETA, 0)];
            assert_eq!(actual_slots_value, expected_slots_value);

            // Underlying digests value after resolving pool commit
            let alpha_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_ALPHA).unwrap();
            assert_eq!(alpha_digest_value, 250);
            let beta_digest_value = Pallet::get_digest_value(&STAKING, &VALIDATOR_BETA).unwrap();
            assert_eq!(beta_digest_value, 250);

            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 500);

            // Charlies balance check (resolving balance = 0, since the commisison is 100% of the total resolved balance)
            let actual_balance = AssetOf::balance(&CHARLIE);
            let expected_balance = INITIAL_BALANCE + 0; // initial_balance + resolved_blance with commission deduction

            assert_eq!(actual_balance, expected_balance);
            let actual_freeze_balance = AssetOf::balance_frozen(&STAKING, &CHARLIE);
            assert_eq!(actual_freeze_balance, ZERO_VALUE);

            // Mike balance check for commison settlement (100% commission)
            let actual_balance = AssetOf::balance(&MIKE);
            let expected_balance = INITIAL_BALANCE + 500; // initial_balance + commission for the resolved commit
            assert_eq!(actual_balance, expected_balance);
        });
    }

    #[test]
    fn resolve_commit_err_commit_not_found() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_RUNTIME_UPGRADE,
                STANDARD_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            assert_err!(
                Pallet::resolve_commit(&ALICE, &ESCROW,),
                Error::CommitNotFound
            );
        })
    }

    #[test]
    fn gen_digest_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            let gen_digest_1 = Pallet::gen_digest(&ALICE);
            assert!(gen_digest_1.is_ok());
            let gen_digest_2 = Pallet::gen_digest(&ALICE);
            assert!(gen_digest_2.is_ok());
            assert_eq!(gen_digest_1, gen_digest_2); // deterministic key generation

            // manual mutation of account nonce
            Account::mutate(&ALICE, |info| {
                info.nonce = 2;
            });
            let gen_digest_3 = Pallet::gen_digest(&ALICE);
            assert!(gen_digest_3.is_ok());
            assert_ne!(gen_digest_2, gen_digest_3); // unique key generation accross different nonce

            // manual mutation of account nonce
            Account::mutate(&ALICE, |info| {
                info.nonce = 4;
            });
            let gen_digest_4 = Pallet::gen_digest(&ALICE);
            assert!(gen_digest_4.is_ok());
            assert_ne!(gen_digest_3, gen_digest_4); // unique key generation accross same source with different nonce
            let gen_digest_5 = Pallet::gen_digest(&BOB);
            assert!(gen_digest_5.is_ok());
            assert_ne!(gen_digest_5, gen_digest_4); // unique key generation accross different source
            let gen_digest_6 = Pallet::gen_digest(&CHARLIE);
            assert!(gen_digest_6.is_ok());
            assert_ne!(gen_digest_5, gen_digest_6); // unique key generation accross different source with same nonce
        })
    }

    #[test]
    fn commit_exists_ok() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_ok!(Pallet::commit_exists(&ALICE, &STAKING));
        })
    }

    #[test]
    fn commit_exists_err_commit_not_found() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Force),
            )
            .unwrap();
            assert_err!(
                Pallet::commit_exists(&ALICE, &GOVERNANCE),
                Error::CommitNotFound
            );
        })
    }

    #[test]
    fn digest_exists_ok() {
        commit_test_ext().execute_with(|| {
            initiate_digest_with_default_balance(GOVERNANCE, PROPOSAL_RUNTIME_UPGRADE).unwrap();
            assert_ok!(Pallet::digest_exists(
                &GOVERNANCE,
                &PROPOSAL_RUNTIME_UPGRADE,
            ));
        });
    }

    #[test]
    fn digest_exists_err_digest_not_found() {
        commit_test_ext().execute_with(|| {
            initiate_digest_with_default_balance(GOVERNANCE, PROPOSAL_RUNTIME_UPGRADE).unwrap();
            assert_err!(
                Pallet::digest_exists(&GOVERNANCE, &PROPOSAL_TREASURY_SPEND),
                Error::DigestNotFound
            );
        })
    }

    #[test]
    fn get_commit_digest_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &ESCROW,
                &CONTRACT_FREELANCE,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let commit_digest = Pallet::get_commit_digest(&ALICE, &ESCROW).unwrap();
            assert_eq!(commit_digest, CONTRACT_FREELANCE);
        })
    }

    #[test]
    fn get_commit_digest_err_commit_not_found() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &ESCROW,
                &CONTRACT_FREELANCE,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_err!(
                Pallet::get_commit_digest(&ALICE, &STAKING),
                Error::CommitNotFound
            );
        })
    }

    #[test]
    fn get_total_value_works() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();

            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let total_value = Pallet::get_total_value(&STAKING);
            assert_eq!(total_value, 250);
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_BETA,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // total commited value for staking reason
            let total_value = Pallet::get_total_value(&STAKING);
            assert_eq!(total_value, 750); // 250 + 500 -> 750

            // total commited value for bet reason
            let total_value = Pallet::get_total_value(&ESCROW);
            assert_eq!(total_value, 0); // no commits for escrow reason yet
            Pallet::place_commit(
                &BOB,
                &ESCROW,
                &CONTRACT_FREELANCE,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // total commited value for escrow reason
            let total_value = Pallet::get_total_value(&ESCROW);
            assert_eq!(total_value, 500);
        })
    }

    #[test]
    fn get_commit_value_for_direct_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let commit_value = Pallet::get_commit_value(&ALICE, &STAKING).unwrap();
            assert_eq!(commit_value, STANDARD_COMMIT);
        })
    }

    #[test]
    fn get_commit_value_for_index_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                MIKE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
                INDEX_BALANCED_STAKING,
            )
            .unwrap();
            assert_ok!(Pallet::index_exists(&STAKING, &INDEX_BALANCED_STAKING));
            // Place commit to an index
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_BALANCED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let actual_commit_value = Pallet::get_commit_value(&ALICE, &STAKING).unwrap();
            assert_eq!(actual_commit_value, STANDARD_COMMIT);
        })
    }

    #[test]
    fn get_commit_value_for_pool_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &POOL_MANAGED_STAKING,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let actual_commit_value = Pallet::get_commit_value(&BOB, &STAKING).unwrap();
            assert_eq!(actual_commit_value, LARGE_COMMIT);
        })
    }

    #[test]
    fn get_digets_value_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();

            Pallet::place_commit(
                &ALICE,
                &ESCROW,
                &CONTRACT_FREELANCE,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let digets_value = Pallet::get_digest_value(&ESCROW, &CONTRACT_FREELANCE).unwrap();
            assert_eq!(digets_value, STANDARD_COMMIT);
            Pallet::place_commit(
                &BOB,
                &ESCROW,
                &CONTRACT_FREELANCE,
                SMALL_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let digets_value = Pallet::get_digest_value(&ESCROW, &CONTRACT_FREELANCE).unwrap();
            assert_eq!(digets_value, 350); // 250 (alice's commit) + 100 (bob's commit) -> 350
        })
    }

    #[test]
    fn set_digest_value_mint_ok() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &ESCROW,
                &CONTRACT_FREELANCE,
                SMALL_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Polite),
            )
            .unwrap();
            // before set_digest_value
            assert_eq!(
                Pallet::get_digest_value(&ESCROW, &CONTRACT_FREELANCE),
                Ok(SMALL_COMMIT)
            );
            let asset_to_issue = AssetToIssue::get();
            assert_eq!(asset_to_issue, ZERO_VALUE);
            let reason_value = Pallet::get_total_value(&ESCROW);
            assert_eq!(reason_value, 100);
            // setting a new digest value > current digest value
            assert_ok!(Pallet::set_digest_value(
                &ESCROW,
                &CONTRACT_FREELANCE,
                STANDARD_COMMIT,
                &Default::default(),
            ));
            // after set_digest_value (minting senario)
            let asset_to_issue = AssetToIssue::get();
            assert_eq!(asset_to_issue, 150);
            let reason_value = Pallet::get_total_value(&ESCROW);
            assert_eq!(reason_value, 250);
            assert_eq!(
                Pallet::get_digest_value(&ESCROW, &CONTRACT_FREELANCE),
                Ok(250)
            );
        })
    }

    #[test]
    fn set_digest_value_equal_ok() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &ESCROW,
                &CONTRACT_FREELANCE,
                STANDARD_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            // before set_digest_value
            assert_eq!(
                Pallet::get_digest_value(&ESCROW, &CONTRACT_FREELANCE),
                Ok(STANDARD_COMMIT)
            );
            let asset_to_reap = AssetToReap::get();
            assert_eq!(asset_to_reap, ZERO_VALUE);
            let reason_value = Pallet::get_total_value(&ESCROW);
            assert_eq!(reason_value, 250);
            // setting a new digest value == current digest value
            assert_ok!(Pallet::set_digest_value(
                &ESCROW,
                &CONTRACT_FREELANCE,
                STANDARD_COMMIT,
                &Default::default(),
            ));
            // after set_digest_value (no changes)
            let asset_to_reap = AssetToReap::get();
            assert_eq!(asset_to_reap, ZERO_VALUE);
            let reason_value = Pallet::get_total_value(&ESCROW);
            assert_eq!(reason_value, 250);
            assert_eq!(
                Pallet::get_digest_value(&ESCROW, &CONTRACT_FREELANCE),
                Ok(STANDARD_COMMIT)
            );
        })
    }

    #[test]
    fn set_digest_value_reap_ok() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &ESCROW,
                &CONTRACT_FREELANCE,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // before set_digest_value
            assert_eq!(
                Pallet::get_digest_value(&ESCROW, &CONTRACT_FREELANCE),
                Ok(STANDARD_COMMIT)
            );
            let asset_to_reap = AssetToReap::get();
            assert_eq!(asset_to_reap, ZERO_VALUE);
            let reason_value = Pallet::get_total_value(&ESCROW);
            assert_eq!(reason_value, 250);
            // setting a new digest value < current digest value
            assert_ok!(Pallet::set_digest_value(
                &ESCROW,
                &CONTRACT_FREELANCE,
                SMALL_COMMIT,
                &Default::default(),
            ));
            // after set_digest_value (reaping senario)
            let asset_to_reap = AssetToReap::get();
            assert_eq!(asset_to_reap, 150);
            let reason_value = Pallet::get_total_value(&ESCROW);
            assert_eq!(reason_value, 100);
            assert_eq!(
                Pallet::get_digest_value(&ESCROW, &CONTRACT_FREELANCE),
                Ok(100)
            );
        })
    }

    #[test]
    fn on_commit_place_event_emmission_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            System::set_block_number(BLOCK_EARLY);
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_RUNTIME_UPGRADE,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // event emmitted
            System::assert_last_event(
                Event::CommitPlaced {
                    who: ALICE,
                    reason: GOVERNANCE,
                    #[cfg(feature = "dev")]
                    model: DigestVariant::Direct(PROPOSAL_RUNTIME_UPGRADE),
                    #[cfg(not(feature = "dev"))]
                    digest: PROPOSAL_RUNTIME_UPGRADE,
                    value: STANDARD_COMMIT,
                    variant: Position::default(),
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_commit_raise_event_emmission_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::raise_commit(
                &ALICE,
                &STAKING,
                SMALL_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            System::assert_last_event(
                Event::CommitRaised {
                    who: ALICE,
                    reason: STAKING,
                    #[cfg(feature = "dev")]
                    model: DigestVariant::Direct(VALIDATOR_ALPHA),
                    #[cfg(not(feature = "dev"))]
                    digest: VALIDATOR_ALPHA,
                    value: SMALL_COMMIT,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_commit_resolve_event_emmission_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            System::set_block_number(3);
            Pallet::resolve_commit(&ALICE, &STAKING).unwrap();
            // event emmitted
            System::assert_last_event(
                Event::CommitResolved {
                    who: ALICE,
                    reason: STAKING,
                    #[cfg(feature = "dev")]
                    model: DigestVariant::Direct(VALIDATOR_ALPHA),
                    #[cfg(not(feature = "dev"))]
                    digest: VALIDATOR_ALPHA,
                    value: STANDARD_COMMIT,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_digest_update_event_emmission_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            System::set_block_number(3);
            Pallet::set_digest_value(
                &STAKING,
                &VALIDATOR_ALPHA,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // event emmitted
            System::assert_last_event(
                Event::DigestInfo {
                    digest: VALIDATOR_ALPHA,
                    reason: STAKING,
                    value: LARGE_COMMIT,
                    variant: Position::default(),
                }
                .into(),
            );
        })
    }

    #[test]
    fn reap_digest_ok() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &ALICE,
                &ESCROW,
                &CONTRACT_FREELANCE,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::resolve_commit(&ALICE, &ESCROW).unwrap();
            // before reaping
            assert_ok!(Pallet::digest_exists(&ESCROW, &CONTRACT_FREELANCE));
            // reaping the digest which has no funds
            assert_ok!(Pallet::reap_digest(&CONTRACT_FREELANCE, &ESCROW));
            // after reaping
            assert_err!(
                Pallet::digest_exists(&ESCROW, &CONTRACT_FREELANCE),
                Error::DigestNotFound
            )
        })
    }

    #[test]
    fn reap_digest_err_digest_has_funds() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &ALICE,
                &ESCROW,
                &CONTRACT_FREELANCE,
                STANDARD_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            assert_ok!(Pallet::digest_exists(&ESCROW, &CONTRACT_FREELANCE));
            // since, digest has a commited funds, it cannot be reaped
            assert_err!(
                Pallet::reap_digest(&CONTRACT_FREELANCE, &ESCROW),
                Error::DigestHasFunds
            );
        })
    }

    #[test]
    fn on_reap_digest_event_emmission_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &ALICE,
                &ESCROW,
                &CONTRACT_FREELANCE,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            System::set_block_number(3);
            Pallet::resolve_commit(&ALICE, &ESCROW).unwrap();
            System::set_block_number(4);
            Pallet::reap_digest(&CONTRACT_FREELANCE, &ESCROW).unwrap();
            System::assert_last_event(
                Event::DigestReaped {
                    digest: CONTRACT_FREELANCE,
                    reason: ESCROW,
                    dust: Zero::zero(),
                }
                .into(),
            );
        })
    }

    #[test]
    fn can_place_commit_ok() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            assert_ok!(Pallet::can_place_commit(
                &ALICE,
                &ESCROW,
                &ALPHA_DIGEST,
                STANDARD_COMMIT,
                &Default::default()
            ));
        })
    }

    #[test]
    fn can_place_commit_err_commit_already_exists_for_reason() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_err!(
                Pallet::can_place_commit(
                    &ALICE,
                    &STAKING,
                    &VALIDATOR_ALPHA,
                    SMALL_COMMIT,
                    &Directive::new(Precision::Exact, Fortitude::Force)
                ),
                Error::CommitAlreadyExists
            );
        })
    }

    #[test]
    fn can_place_commit_err_insufficient_funds() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            assert_err!(
                Pallet::can_place_commit(
                    &ALICE,
                    &GOVERNANCE,
                    &ALPHA_DIGEST,
                    1600,
                    &Default::default()
                ),
                Error::InsufficientFunds
            );
        })
    }

    #[test]
    fn can_raise_commit_ok() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_ok!(Pallet::can_raise_commit(
                &ALICE,
                &STAKING,
                SMALL_COMMIT,
                &Default::default()
            ));
        })
    }

    #[test]
    fn can_raise_commit_err_insufficient_funds() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &ESCROW,
                &CONTRACT_SUPPLY_CHAIN,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_err!(
                Pallet::can_raise_commit(&ALICE, &ESCROW, 1200, &Default::default()),
                Error::InsufficientFunds
            );
        })
    }

    #[test]
    fn can_resolve_commit_ok() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_ok!(Pallet::can_resolve_commit(&ALICE, &GOVERNANCE));
        })
    }

    #[test]
    fn can_resolve_commit_index_ok() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                BOB,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
                INDEX_BALANCED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_BALANCED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            assert_ok!(Pallet::can_resolve_commit(&ALICE, &STAKING));
        })
    }

    #[test]
    fn can_resolve_commit_pool_ok() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_DOMINANT),
                (VALIDATOR_BETA, SHARE_MAJOR),
            ];
            prepare_and_initiate_pool(
                BOB,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            assert_ok!(Pallet::can_resolve_commit(&ALICE, &STAKING));
        })
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````` COMMIT VARIANT `````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn get_commit_variant_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit_of_variant(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                STANDARD_COMMIT,
                &Position::position_of(2).unwrap(),
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let actual_commit_variant = Pallet::get_commit_variant(&ALICE, &GOVERNANCE).unwrap();
            let expected_commit_variant = Position::position_of(2).unwrap();
            assert_eq!(actual_commit_variant, expected_commit_variant);
        })
    }

    #[test]
    fn get_commit_variant_fail_commit_not_found() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_err!(
                Pallet::get_commit_variant(&ALICE, &ESCROW),
                Error::CommitNotFound
            );
        })
    }

    #[test]
    fn get_digest_variant_value_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_RUNTIME_UPGRADE,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let digest_variant_value = Pallet::get_digest_variant_value(
                &GOVERNANCE,
                &PROPOSAL_RUNTIME_UPGRADE,
                &Position::default(),
            )
            .unwrap();
            assert_eq!(digest_variant_value, STANDARD_COMMIT);
            // zero returned when variant does not exists
            let digest_variant_value = Pallet::get_digest_variant_value(
                &GOVERNANCE,
                &PROPOSAL_RUNTIME_UPGRADE,
                &Position::position_of(1).unwrap(),
            )
            .unwrap();
            assert_eq!(digest_variant_value, ZERO_VALUE);
            Pallet::place_commit_of_variant(
                &BOB,
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                LARGE_COMMIT,
                &Position::position_of(2).unwrap(),
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            let digest_variant_value = Pallet::get_digest_variant_value(
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                &Position::position_of(2).unwrap(),
            )
            .unwrap();
            assert_eq!(digest_variant_value, LARGE_COMMIT);
            // raising the commit value of commit with variant Affirmative
            Pallet::raise_commit(
                &ALICE,
                &GOVERNANCE,
                SMALL_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Force),
            )
            .unwrap();
            // raised value refected in the digets variant
            let digest_variant_value = Pallet::get_digest_variant_value(
                &GOVERNANCE,
                &PROPOSAL_RUNTIME_UPGRADE,
                &Position::default(),
            )
            .unwrap();
            let raised_value = STANDARD_COMMIT + SMALL_COMMIT;
            assert_eq!(digest_variant_value, raised_value);
        })
    }

    #[test]
    fn get_digest_variant_value_fail_digest_not_found() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_RUNTIME_UPGRADE,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_err!(
                Pallet::get_digest_variant_value(
                    &GOVERNANCE,
                    &PROPOSAL_TREASURY_SPEND,
                    &Position::default(),
                ),
                Error::DigestNotFound
            );
        })
    }

    #[test]
    fn set_digest_variant_value_mint_ok() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // before set_digest_variant_value
            assert_eq!(
                Pallet::get_digest_value(&GOVERNANCE, &PROPOSAL_TREASURY_SPEND),
                Ok(STANDARD_COMMIT)
            );
            let asset_to_issue = AssetToIssue::get();
            assert_eq!(asset_to_issue, ZERO_VALUE);
            let total_value = Pallet::get_total_value(&GOVERNANCE);
            assert_eq!(total_value, 250);
            // setting a new digest value with specific variant > current digest value
            assert_ok!(Pallet::set_digest_variant_value(
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                LARGE_COMMIT,
                &Position::default(),
                &Default::default(),
            ));
            // after set_digest_variant_value (minting senario)
            let asset_to_issue = AssetToIssue::get();
            assert_eq!(asset_to_issue, 250);
            let total_value = Pallet::get_total_value(&GOVERNANCE);
            assert_eq!(total_value, 500);
            assert_eq!(
                Pallet::get_digest_value(&GOVERNANCE, &PROPOSAL_TREASURY_SPEND),
                Ok(LARGE_COMMIT)
            );

            System::assert_last_event(Event::DigestInfo { 
                    digest: PROPOSAL_TREASURY_SPEND, 
                    reason: GOVERNANCE, 
                    value: total_value, 
                    variant: Disposition::default() 
                }
                .into()
            );
        })
    }

    #[test]
    fn set_digest_variant_value_equal_ok() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // before set_digest_variant_value
            assert_eq!(
                Pallet::get_digest_value(&GOVERNANCE, &PROPOSAL_TREASURY_SPEND),
                Ok(STANDARD_COMMIT)
            );
            let asset_to_reap = AssetToReap::get();
            assert_eq!(asset_to_reap, ZERO_VALUE);
            let total_value = Pallet::get_total_value(&GOVERNANCE);
            assert_eq!(total_value, 250);
            // setting a new digest value with specific variant == current digest value
            assert_ok!(Pallet::set_digest_variant_value(
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                STANDARD_COMMIT,
                &Position::default(),
                &Default::default(),
            ));
            // after set_digest_variant_value (no changes)
            let asset_to_reap = AssetToReap::get();
            assert_eq!(asset_to_reap, ZERO_VALUE);
            let total_value = Pallet::get_total_value(&GOVERNANCE);
            assert_eq!(total_value, 250);
            assert_eq!(
                Pallet::get_digest_value(&GOVERNANCE, &PROPOSAL_TREASURY_SPEND),
                Ok(STANDARD_COMMIT)
            );
        })
    }

    #[test]
    fn set_digest_variant_value_reap_ok() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // before set_digest_variant_value
            assert_eq!(
                Pallet::get_digest_value(&GOVERNANCE, &PROPOSAL_TREASURY_SPEND),
                Ok(STANDARD_COMMIT)
            );
            let asset_to_reap = AssetToReap::get();
            assert_eq!(asset_to_reap, ZERO_VALUE);
            let total_value = Pallet::get_total_value(&GOVERNANCE);
            assert_eq!(total_value, 250);
            // setting a new digest value with specific variant < current digest value
            assert_ok!(Pallet::set_digest_variant_value(
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                SMALL_COMMIT,
                &Position::default(),
                &Default::default(),
            ));
            // after set_digest_variant_value (reaping senario)
            let asset_to_reap = AssetToReap::get();
            assert_eq!(asset_to_reap, 150);
            let total_value = Pallet::get_total_value(&GOVERNANCE);
            assert_eq!(total_value, 100);
            assert_eq!(
                Pallet::get_digest_value(&GOVERNANCE, &PROPOSAL_TREASURY_SPEND),
                Ok(SMALL_COMMIT)
            );
        })
    }

    #[test]
    fn set_digest_variant_value_err_cannot_mint_asset() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            AssetToIssue::put(MAX_VALUE);
            assert_err!(
                Pallet::set_digest_variant_value(
                    &GOVERNANCE,
                    &PROPOSAL_TREASURY_SPEND,
                    LARGE_COMMIT,
                    &Position::default(),
                    &Default::default(),
                ),
                Error::MaxAssetIssued
            );
        });
    }

    #[test]
    fn set_digest_variant_value_err_cannot_reap_asset() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            AssetToReap::put(MAX_VALUE);
            assert_err!(
                Pallet::set_digest_variant_value(
                    &GOVERNANCE,
                    &PROPOSAL_TREASURY_SPEND,
                    SMALL_COMMIT,
                    &Position::default(),
                    &Default::default(),
                ),
                Error::MaxAssetReaped
            );
        });
    }

    #[test]
    fn set_digest_variant_value_err_digest_not_found() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_err!(
                Pallet::set_digest_variant_value(
                    &GOVERNANCE,
                    &PROPOSAL_RUNTIME_UPGRADE,
                    SMALL_COMMIT,
                    &Position::default(),
                    &Default::default(),
                ),
                Error::DigestNotFound
            );
        });
    }

    #[test]
    fn place_commit_of_variant_success_for_digest() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            assert_err!(
                Pallet::commit_exists(&ALICE, &GOVERNANCE),
                Error::CommitNotFound
            );
            assert_err!(
                Pallet::digest_exists(&GOVERNANCE, &PROPOSAL_TREASURY_SPEND),
                Error::DigestNotFound
            );
            assert_ok!(Pallet::place_commit_of_variant(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                STANDARD_COMMIT,
                &Position::position_of(1).unwrap(),
                &Directive::new(Precision::Exact, Fortitude::Force)
            ));
            // Variant enquiry
            let actual_variant = Pallet::get_commit_variant(&ALICE, &GOVERNANCE).unwrap();
            let expected_variant = Position::position_of(1).unwrap();
            assert_eq!(expected_variant, actual_variant);
            let actual_varinat_value = Pallet::get_digest_variant_value(
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                &Position::position_of(1).unwrap(),
            )
            .unwrap();
            assert_eq!(actual_varinat_value, STANDARD_COMMIT);
            // Commit and digest enquirey
            assert_ok!(Pallet::commit_exists(&ALICE, &GOVERNANCE));
            assert_ok!(Pallet::digest_exists(&GOVERNANCE, &PROPOSAL_TREASURY_SPEND));
            // Balance and freeze enquirey
            let balace_after = AssetOf::balance(&ALICE);
            let hold_balance_after = AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &ALICE);
            let expected_balance_after = INITIAL_BALANCE;
            let expected_hold_balance_after = 250;
            assert_eq!(expected_balance_after, balace_after);
            assert_eq!(expected_hold_balance_after, hold_balance_after);
            assert_eq!(
                AssetOf::balance_frozen(&GOVERNANCE, &ALICE),
                STANDARD_COMMIT
            );
            // Commit info enquiry
            let commit_info = CommitMap::get((ALICE, GOVERNANCE)).unwrap();
            assert_eq!(commit_info.digest(), PROPOSAL_TREASURY_SPEND);
            let variant = commit_info.variant();
            let index = variant.index();
            let commits = commit_info.commits();
            let commit = commits.get(0).unwrap();
            assert_eq!(receipt_deposit_value(commit).unwrap(), STANDARD_COMMIT);
            // Digest info enquiry
            let digest_info = DigestMap::get((GOVERNANCE, PROPOSAL_TREASURY_SPEND)).unwrap();
            let digests = digest_info.reveal();
            let digest_of = digests.get(index).unwrap();
            assert_ok!(has_deposits(digest_of, &variant, &PROPOSAL_TREASURY_SPEND));
            assert_eq!(
                balance_total(digest_of, &variant, &PROPOSAL_TREASURY_SPEND).unwrap(),
                STANDARD_COMMIT,
            );
            // Total value enquiry
            let reason_value = ReasonValue::get(GOVERNANCE).unwrap();
            assert_eq!(reason_value, 250);

            #[cfg(not(feature = "dev"))]
            System::assert_last_event(Event::CommitPlaced { 
                    who: ALICE, 
                    reason: GOVERNANCE, 
                    digest: PROPOSAL_TREASURY_SPEND, 
                    value: STANDARD_COMMIT, 
                    variant: Position::position_of(1).unwrap() 
                }
                .into()
            );

            #[cfg(feature = "dev")]
            System::assert_last_event(Event::CommitPlaced { 
                    who: ALICE, 
                    reason: GOVERNANCE, 
                    model: DigestVariant::Direct(PROPOSAL_TREASURY_SPEND), 
                    value: STANDARD_COMMIT, 
                    variant: Position::position_of(1).unwrap() 
                }
                .into()
            );
        })
    }

    #[test]
    fn place_commit_of_variant_marker_error_for_value_zero() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            assert_err!(
                Pallet::place_commit_of_variant(
                    &ALICE,
                    &GOVERNANCE,
                    &PROPOSAL_RUNTIME_UPGRADE,
                    ZERO_VALUE,
                    &Position::default(),
                    &Directive::new(Precision::Exact, Fortitude::Force)
                ),
                Error::MarkerCommitNotAllowed
            );
            // Commit and digest enquirey
            assert_err!(
                Pallet::commit_exists(&ALICE, &GOVERNANCE),
                Error::CommitNotFound
            );
            assert_err!(
                Pallet::digest_exists(&GOVERNANCE, &PROPOSAL_RUNTIME_UPGRADE),
                Error::DigestNotFound
            );
        })
    }

    #[test]
    fn place_commit_of_variant_success_for_index() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();

            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            prepare_and_initiate_index(
                MIKE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            assert_ok!(Pallet::index_exists(&STAKING, &INDEX_OPTIMIZED_STAKING));
            // Before placing a commit to the index
            let index_info = Pallet::get_index(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            assert_eq!(index_info.capital(), 100);
            assert_eq!(index_info.principal(), ZERO_VALUE);
            let actual_entries_value =
                Pallet::get_entries_value(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_entries_value = vec![(VALIDATOR_ALPHA, 0), (VALIDATOR_BETA, 0)];
            assert_eq!(actual_entries_value, expected_entries_value);

            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 500);
            // Place commit to an index with variant
            assert_ok!(Pallet::place_commit_of_variant(
                &CHARLIE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Position::position_of(2).unwrap(),
                &Directive::new(Precision::Exact, Fortitude::Force)
            ));
            // After placing a commit to the index with a variant
            let index_value = Pallet::get_index_value(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            assert_eq!(index_value, 250);
            let actual_entries_value =
                Pallet::get_entries_value(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_entries_value = vec![(VALIDATOR_ALPHA, 100), (VALIDATOR_BETA, 150)];
            assert_eq!(actual_entries_value, expected_entries_value);

            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 750);
            // Variant check
            assert_eq!(
                Pallet::get_commit_variant(&CHARLIE, &STAKING),
                Ok(Position::position_of(2).unwrap())
            );

            #[cfg(not(feature = "dev"))]
            System::assert_last_event(Event::CommitPlaced { 
                    who: CHARLIE, 
                    reason: STAKING, 
                    digest: INDEX_OPTIMIZED_STAKING, 
                    value: STANDARD_COMMIT, 
                    variant: Position::position_of(2).unwrap() 
                }
                .into()
            );

            #[cfg(feature = "dev")]
            System::assert_last_event(Event::CommitPlaced { 
                    who: CHARLIE, 
                    reason: STAKING, 
                    model: DigestVariant::Index(INDEX_OPTIMIZED_STAKING), 
                    value: STANDARD_COMMIT, 
                    variant: Position::position_of(2).unwrap() 
                }
                .into()
            );
        })
    }

    #[test]
    fn place_commit_of_variant_success_for_pool() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_MAJOR),
                (VALIDATOR_BETA, SHARE_DOMINANT),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            // Before placing commit to pool
            assert_eq!(
                Pallet::get_manager(&STAKING, &POOL_MANAGED_STAKING),
                Ok(MIKE)
            );
            let pool_info = PoolMap::get((STAKING, POOL_MANAGED_STAKING)).unwrap();
            let pool_balance_of = pool_info.balance();
            assert!(
                has_deposits(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING).is_err()
            );
            assert_eq!(
                balance_total(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING)
                    .unwrap(),
                0,
            );
            let pool_capital = pool_info.capital();
            assert_eq!(pool_capital, 100);
            let actual_slots_value =
                Pallet::get_slots_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_slots_value = vec![(VALIDATOR_ALPHA, 0), (VALIDATOR_BETA, 0)];
            assert_eq!(actual_slots_value, expected_slots_value);
            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 500);
            // Placing commit to pool with variant
            assert_ok!(Pallet::place_commit_of_variant(
                &CHARLIE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                STANDARD_COMMIT,
                &Position::position_of(1).unwrap(),
                &Directive::new(Precision::Exact, Fortitude::Force)
            ));
            // After placing commit to pool
            let pool_info = PoolMap::get((STAKING, POOL_MANAGED_STAKING)).unwrap();
            let pool_balance_of = pool_info.balance();
            assert_ok!(has_deposits(
                &pool_balance_of,
                &Default::default(),
                &POOL_MANAGED_STAKING
            ));
            assert_eq!(
                balance_total(&pool_balance_of, &Default::default(), &POOL_MANAGED_STAKING)
                    .unwrap(),
                250,
            );
            let actual_slots_value =
                Pallet::get_slots_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_slots_value = vec![(VALIDATOR_ALPHA, 100), (VALIDATOR_BETA, 150)];
            assert_eq!(actual_slots_value, expected_slots_value);

            let reason_value = ReasonValue::get(STAKING).unwrap();
            assert_eq!(reason_value, 750);
            // Balance and freeze enquirey
            let balace_after = AssetOf::balance(&CHARLIE);
            let hold_balance_after = AssetOf::balance_on_hold(&PREPARE_FOR_COMMIT, &CHARLIE);
            let expected_balance_after = INITIAL_BALANCE;
            let expected_hold_balance_after = 250;
            assert_eq!(expected_balance_after, balace_after);
            assert_eq!(expected_hold_balance_after, hold_balance_after);
            assert_eq!(AssetOf::balance_frozen(&STAKING, &CHARLIE), STANDARD_COMMIT);
            // Variant check
            assert_eq!(
                Pallet::get_commit_variant(&CHARLIE, &STAKING),
                Ok(Position::position_of(1).unwrap())
            );

            #[cfg(not(feature = "dev"))]
            System::assert_last_event(Event::CommitPlaced { 
                    who: CHARLIE, 
                    reason: STAKING, 
                    digest: POOL_MANAGED_STAKING, 
                    value: STANDARD_COMMIT, 
                    variant: Position::position_of(1).unwrap() 
                }
                .into()
            );

            #[cfg(feature = "dev")]
            System::assert_last_event(Event::CommitPlaced { 
                    who: CHARLIE, 
                    reason: STAKING, 
                    model: DigestVariant::Pool(POOL_MANAGED_STAKING), 
                    value: STANDARD_COMMIT, 
                    variant: Position::position_of(1).unwrap() 
                }
                .into()
            );
        })
    }

    #[test]
    fn on_place_commit_on_variant_event_emmission_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_RUNTIME_UPGRADE,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            System::assert_last_event(
                Event::CommitPlaced {
                    who: ALICE,
                    reason: GOVERNANCE,
                    #[cfg(feature = "dev")]
                    model: DigestVariant::Direct(PROPOSAL_RUNTIME_UPGRADE),
                    #[cfg(not(feature = "dev"))]
                    digest: PROPOSAL_RUNTIME_UPGRADE,
                    value: STANDARD_COMMIT,
                    variant: Position::default(),
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_set_commit_variant_event_emmission_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            System::set_block_number(2);
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_RUNTIME_UPGRADE,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            System::assert_last_event(
                Event::CommitPlaced {
                    who: ALICE,
                    reason: GOVERNANCE,
                    #[cfg(feature = "dev")]
                    model: DigestVariant::Direct(PROPOSAL_RUNTIME_UPGRADE),
                    #[cfg(not(feature = "dev"))]
                    digest: PROPOSAL_RUNTIME_UPGRADE,
                    value: STANDARD_COMMIT,
                    variant: Position::position_of(0).unwrap(),
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_set_digest_variant_event_emmission_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            System::set_block_number(2);
            Pallet::place_commit_of_variant(
                &ALICE,
                &STAKING,
                &PROPOSAL_RUNTIME_UPGRADE,
                STANDARD_COMMIT,
                &Position::position_of(1).unwrap(),
                &Directive::new(Precision::BestEffort, Fortitude::Force),
            )
            .unwrap();
            System::set_block_number(3);
            Pallet::set_digest_variant_value(
                &STAKING,
                &PROPOSAL_RUNTIME_UPGRADE,
                SMALL_COMMIT,
                &Position::position_of(1).unwrap(),
                &Directive::new(Precision::BestEffort, Fortitude::Force),
            )
            .unwrap();
            System::assert_last_event(
                Event::DigestInfo {
                    digest: PROPOSAL_RUNTIME_UPGRADE,
                    reason: STAKING,
                    value: SMALL_COMMIT,
                    variant: Position::position_of(1).unwrap(),
                }
                .into(),
            );
        })
    }

    #[test]
    fn set_commit_variant_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let current_commit_variant = Pallet::get_commit_variant(&ALICE, &GOVERNANCE).unwrap();
            assert_eq!(current_commit_variant, Position::default());
            // setting a new variant
            let new_commit_variant = Position::position_of(1).unwrap();
            assert_ok!(Pallet::set_commit_variant(
                &ALICE,
                &GOVERNANCE,
                &new_commit_variant,
                &Default::default(),
            ));
            // new variant updated
            let current_commit_variant = Pallet::get_commit_variant(&ALICE, &GOVERNANCE).unwrap();
            assert_eq!(current_commit_variant, Position::position_of(1).unwrap());
        })
    }

    #[test]
    fn set_commit_variant_same_variant_safe_return() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &GOVERNANCE,
                &PROPOSAL_TREASURY_SPEND,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let current_commit_variant = Pallet::get_commit_variant(&ALICE, &GOVERNANCE).unwrap();
            assert_eq!(current_commit_variant, Position::default());
            // setting the same variant
            assert_ok!(Pallet::set_commit_variant(
                &ALICE,
                &GOVERNANCE,
                &current_commit_variant,
                &Default::default(),
            ));
        })
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````` COMMIT INDEX `````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn index_exists_ok() {
        commit_test_ext().execute_with(|| {
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
                INDEX_BALANCED_STAKING,
            )
            .unwrap();
            assert_ok!(Pallet::index_exists(&STAKING, &INDEX_BALANCED_STAKING));
        })
    }

    #[test]
    fn index_exists_err_index_not_found() {
        commit_test_ext().execute_with(|| {
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
                INDEX_BALANCED_STAKING,
            )
            .unwrap();
            assert_err!(
                Pallet::index_exists(&STAKING, &INDEX_OPTIMIZED_STAKING),
                Error::IndexNotFound
            );
        })
    }

    #[test]
    fn entry_exists_ok() {
        commit_test_ext().execute_with(|| {
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
                INDEX_BALANCED_STAKING,
            )
            .unwrap();
            assert_ok!(Pallet::entry_exists(
                &STAKING,
                &INDEX_BALANCED_STAKING,
                &VALIDATOR_ALPHA
            ));
            assert_ok!(Pallet::entry_exists(
                &STAKING,
                &INDEX_BALANCED_STAKING,
                &VALIDATOR_BETA
            ));
        })
    }

    #[test]
    fn entry_exists_err_entry_not_found() {
        commit_test_ext().execute_with(|| {
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
                INDEX_BALANCED_STAKING,
            )
            .unwrap();
            assert_err!(
                Pallet::entry_exists(&STAKING, &INDEX_BALANCED_STAKING, &VALIDATOR_GAMMA),
                Error::EntryOfIndexNotFound
            );
        })
    }

    #[test]
    fn has_index_ok() {
        commit_test_ext().execute_with(|| {
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
                INDEX_BALANCED_STAKING,
            )
            .unwrap();
            assert_ok!(Pallet::has_index(&STAKING));
        })
    }

    #[test]
    fn has_index_err_index_not_found() {
        commit_test_ext().execute_with(|| {
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
                INDEX_BALANCED_STAKING,
            )
            .unwrap();
            assert_err!(Pallet::has_index(&GOVERNANCE), Error::IndexNotFound);
        })
    }

    #[test]
    fn get_index_sucess() {
        commit_test_ext().execute_with(|| {
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
                INDEX_BALANCED_STAKING,
            )
            .unwrap();
            let expected_index = IndexMap::get((STAKING, INDEX_BALANCED_STAKING)).unwrap();
            let actual_index = Pallet::get_index(&STAKING, &INDEX_BALANCED_STAKING).unwrap();
            assert_eq!(expected_index.principal(), actual_index.principal());
            assert_eq!(expected_index.capital(), actual_index.capital());
            assert_eq!(
                expected_index.entries().get(0),
                actual_index.entries().get(0)
            );
            assert_eq!(
                expected_index.entries().get(1),
                actual_index.entries().get(1)
            );
        })
    }

    #[test]
    fn get_entries_shares_success() {
        commit_test_ext().execute_with(|| {
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            let expected_entries_shares = vec![
                (VALIDATOR_ALPHA, SHARE_MAJOR),
                (VALIDATOR_BETA, SHARE_DOMINANT),
            ];
            let actual_entries_shares =
                Pallet::get_entries_shares(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            assert_eq!(actual_entries_shares, expected_entries_shares);
        })
    }

    #[test]
    fn get_entry_value_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let entry_value_alpha =
                Pallet::get_entry_value(&STAKING, &INDEX_OPTIMIZED_STAKING, &VALIDATOR_ALPHA)
                    .unwrap();
            let entry_value_beta =
                Pallet::get_entry_value(&STAKING, &INDEX_OPTIMIZED_STAKING, &VALIDATOR_BETA)
                    .unwrap();
            assert_eq!(entry_value_alpha, 100);
            assert_eq!(entry_value_beta, 150);
            // placing another commit to the same index
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // aggregated value of specific entries accross different proprietors
            let entry_value_alpha =
                Pallet::get_entry_value(&STAKING, &INDEX_OPTIMIZED_STAKING, &VALIDATOR_ALPHA)
                    .unwrap();
            let entry_value_beta =
                Pallet::get_entry_value(&STAKING, &INDEX_OPTIMIZED_STAKING, &VALIDATOR_BETA)
                    .unwrap();
            assert_eq!(entry_value_alpha, 200);
            assert_eq!(entry_value_beta, 300);
        })
    }

    #[test]
    fn get_entry_value_for_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                SMALL_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // Index entry balance of alice
            let alpha_entry_value = Pallet::get_entry_value_for(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                &VALIDATOR_ALPHA,
            )
            .unwrap();
            let beta_entry_value = Pallet::get_entry_value_for(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                &VALIDATOR_BETA,
            )
            .unwrap();
            assert_eq!(alpha_entry_value, 100);
            assert_eq!(beta_entry_value, 150);
            // Index entry balance of bob
            let alpha_entry_value = Pallet::get_entry_value_for(
                &BOB,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                &VALIDATOR_ALPHA,
            )
            .unwrap();
            let beta_entry_value = Pallet::get_entry_value_for(
                &BOB,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                &VALIDATOR_BETA,
            )
            .unwrap();
            assert_eq!(alpha_entry_value, 40);
            assert_eq!(beta_entry_value, 60);
        })
    }

    #[test]
    fn prepare_index_success() {
        commit_test_ext().execute_with(|| {
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            let index = Pallet::prepare_index(
                &ALICE,
                &STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
            )
            .unwrap();
            assert_eq!(index.principal(), ZERO_VALUE);
            assert_eq!(index.capital(), 200);
            let expected_alpha_entry_info =
                EntryInfo::new(VALIDATOR_ALPHA, SHARE_EQUAL, Default::default()).unwrap();
            let expected_beta_entry_info =
                EntryInfo::new(VALIDATOR_BETA, SHARE_EQUAL, Default::default()).unwrap();
            let entries = index.entries();
            let actual_alice_entry_info = entries.get(0).unwrap();
            let actual_beta_entry_info = entries.get(1).unwrap();
            assert_eq!(expected_alpha_entry_info, *actual_alice_entry_info);
            assert_eq!(expected_beta_entry_info, *actual_beta_entry_info);
        })
    }

    #[test]
    fn prepare_index_err_duplicate_entry() {
        commit_test_ext().execute_with(|| {
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            let actual_err = Pallet::prepare_index(
                &ALICE,
                &STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                ],
            )
            .unwrap_err();
            let expected_err = Error::DuplicateEntry.into();
            assert_eq!(actual_err, expected_err);
        })
    }

    #[test]
    fn prepare_index_err_max_index_reached() {
        commit_test_ext().execute_with(|| {
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_GAMMA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_DELTA).unwrap();
            let actual_err = Pallet::prepare_index(
                &ALICE,
                &STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                    (VALIDATOR_GAMMA, SHARE_EQUAL),
                    (VALIDATOR_DELTA, SHARE_EQUAL),
                ],
            )
            .unwrap_err();
            // since MaxEntries is set to 3, adding a fourth entry results in err
            assert_eq!(actual_err, Error::MaxEntriesReached.into());
        })
    }

    #[test]
    fn set_index_ok() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            let index = Pallet::prepare_index(
                &ALICE,
                &STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
            )
            .unwrap();
            assert_err!(
                Pallet::index_exists(&STAKING, &INDEX_BALANCED_STAKING),
                Error::IndexNotFound
            );
            assert_ok!(Pallet::set_index(
                &ALICE,
                &STAKING,
                &index,
                &INDEX_BALANCED_STAKING
            ));
            assert_ok!(Pallet::index_exists(&STAKING, &INDEX_BALANCED_STAKING));

            #[cfg(not(feature = "dev"))]
            System::assert_last_event(Event::IndexInitialized {
                    index_of: INDEX_BALANCED_STAKING, 
                    reason: STAKING,
                }
                .into()
            );

            #[cfg(feature = "dev")]
            {
                let entries = vec![(VALIDATOR_ALPHA, SHARE_EQUAL, Disposition::default()), (VALIDATOR_BETA, SHARE_EQUAL, Disposition::default())];
                System::assert_last_event(Event::IndexInitialized { 
                        index_of: INDEX_BALANCED_STAKING, 
                        reason: STAKING,
                        entries: entries
                    }
                    .into()
                );  
            }
        })
    }

    #[test]
    fn set_index_err_index_already_exists() {
        commit_test_ext().execute_with(|| {
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            let index = Pallet::prepare_index(
                &ALICE,
                &STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
            )
            .unwrap();
            assert_ok!(Pallet::set_index(
                &ALICE,
                &STAKING,
                &index,
                &INDEX_BALANCED_STAKING
            ));
            assert_ok!(Pallet::index_exists(&STAKING, &INDEX_BALANCED_STAKING));
            // Error while creating an existing index
            assert_err!(
                Pallet::set_index(&ALICE, &STAKING, &index, &INDEX_BALANCED_STAKING),
                Error::IndexDigestTaken
            );
        })
    }

    #[test]
    fn set_entry_shares_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_DOMINANT),
                    (VALIDATOR_BETA, SHARE_MAJOR),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_ok!(Pallet::index_exists(&STAKING, &INDEX_OPTIMIZED_STAKING));
            let new_shares = SHARE_MAJOR;
            let new_index_digest = Pallet::set_entry_shares(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                &VALIDATOR_ALPHA,
                new_shares,
            )
            .unwrap();
            // new index created with updated shares
            assert_ok!(Pallet::index_exists(&STAKING, &new_index_digest));
            let actual_index_entries =
                Pallet::get_entries_shares(&STAKING, &new_index_digest).unwrap();
            let expected_index_entries = vec![
                (VALIDATOR_BETA, SHARE_MAJOR),
                (VALIDATOR_ALPHA, SHARE_MAJOR),
            ];
            assert_eq!(actual_index_entries, expected_index_entries);
            // old index exists and unchanged
            assert_ok!(Pallet::index_exists(&STAKING, &INDEX_OPTIMIZED_STAKING));
            let actual_index_entries =
                Pallet::get_entries_shares(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_index_entries = vec![
                (VALIDATOR_ALPHA, SHARE_DOMINANT),
                (VALIDATOR_BETA, SHARE_MAJOR),
            ];
            assert_eq!(actual_index_entries, expected_index_entries);
        })
    }

    #[test]
    fn set_entry_shares_success_removing_entry_when_shares_set_to_zero() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_DOMINANT),
                    (VALIDATOR_BETA, SHARE_MAJOR),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_ok!(Pallet::index_exists(&STAKING, &INDEX_OPTIMIZED_STAKING));
            let new_shares = ZERO_SHARE;
            let new_index_digest = Pallet::set_entry_shares(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                &VALIDATOR_ALPHA,
                new_shares,
            )
            .unwrap();
            // new index created without the entry of which shares is set to 0
            assert_ok!(Pallet::index_exists(&STAKING, &new_index_digest));
            // Entry removed
            assert_err!(
                Pallet::entry_exists(&STAKING, &new_index_digest, &VALIDATOR_ALPHA),
                Error::EntryOfIndexNotFound
            );
            let actual_index_entries =
                Pallet::get_entries_shares(&STAKING, &new_index_digest).unwrap();
            let expected_index_entries = vec![(VALIDATOR_BETA, SHARE_MAJOR)];
            assert_eq!(actual_index_entries, expected_index_entries);
            // old index exists and unchanged
            assert_ok!(Pallet::index_exists(&STAKING, &INDEX_OPTIMIZED_STAKING));
            let actual_index_entries =
                Pallet::get_entries_shares(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_index_entries = vec![
                (VALIDATOR_ALPHA, SHARE_DOMINANT),
                (VALIDATOR_BETA, SHARE_MAJOR),
            ];
            assert_eq!(actual_index_entries, expected_index_entries);
        })
    }

    #[test]
    fn set_entry_shares_success_adding_new_entry() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_GAMMA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
                INDEX_BALANCED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_BALANCED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            assert_ok!(Pallet::index_exists(&STAKING, &INDEX_BALANCED_STAKING));
            let new_shares = SHARE_EQUAL;
            let new_index_digest = Pallet::set_entry_shares(
                &ALICE,
                &STAKING,
                &INDEX_BALANCED_STAKING,
                &VALIDATOR_GAMMA,
                new_shares,
            )
            .unwrap();
            // new index created with an addition of new entry
            assert_ok!(Pallet::index_exists(&STAKING, &new_index_digest));
            let actual_entries = Pallet::get_entries_shares(&STAKING, &new_index_digest).unwrap();
            let expected_entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
                (VALIDATOR_GAMMA, SHARE_EQUAL),
            ];
            assert_eq!(expected_entries, actual_entries);
            // old index exists and unchanged
            assert_ok!(Pallet::index_exists(&STAKING, &INDEX_BALANCED_STAKING));
            let actual_entries =
                Pallet::get_entries_shares(&STAKING, &INDEX_BALANCED_STAKING).unwrap();
            let expected_entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            assert_eq!(expected_entries, actual_entries);
        })
    }

    #[test]
    fn set_entry_shares_when_share_zero_for_entry() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_GAMMA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
                INDEX_BALANCED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_BALANCED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            assert_ok!(Pallet::index_exists(&STAKING, &INDEX_BALANCED_STAKING));
            // Expected to pass when tried to remove a non-entry digest
            // by giving shares zero
            assert_ok!(Pallet::set_entry_shares(
                &ALICE,
                &STAKING,
                &INDEX_BALANCED_STAKING,
                &VALIDATOR_GAMMA,
                ZERO_SHARE,
            ));

            // While success to remove a existing entry by setting share to zero
            Pallet::set_entry_shares(
                &ALICE,
                &STAKING,
                &INDEX_BALANCED_STAKING,
                &VALIDATOR_BETA,
                ZERO_SHARE,
            )
            .unwrap();
        })
    }

    #[test]
    fn set_entry_shares_err_max_entries_reached() {
        commit_test_ext().execute_with(|| {
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_GAMMA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_DELTA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                    (VALIDATOR_GAMMA, SHARE_EQUAL),
                ],
                INDEX_BALANCED_STAKING,
            )
            .unwrap();
            assert_err!(
                Pallet::set_entry_shares(
                    &ALICE,
                    &STAKING,
                    &INDEX_BALANCED_STAKING,
                    &VALIDATOR_DELTA,
                    SHARE_EQUAL
                ),
                Error::MaxEntriesReached
            );
        })
    }

    #[test]
    fn reap_index_ok() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
                INDEX_BALANCED_STAKING,
            )
            .unwrap();
            assert_ok!(Pallet::index_exists(&STAKING, &INDEX_BALANCED_STAKING));
            assert_ok!(Pallet::reap_index(&STAKING, &INDEX_BALANCED_STAKING));
            assert_err!(
                Pallet::index_exists(&STAKING, &INDEX_BALANCED_STAKING),
                Error::IndexNotFound
            );

            System::assert_last_event(Event::IndexReaped { 
                    index_of: INDEX_BALANCED_STAKING, 
                    reason: STAKING
                }
                .into()
            );
        })
    }

    #[test]
    fn reap_index_err_index_has_funds() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
                INDEX_BALANCED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_BALANCED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_ok!(Pallet::index_exists(&STAKING, &INDEX_BALANCED_STAKING));
            assert_err!(
                Pallet::reap_index(&STAKING, &INDEX_BALANCED_STAKING),
                Error::IndexHasFunds
            );
        })
    }

    #[test]
    fn gen_index_digest_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            let index_a = Pallet::prepare_index(
                &ALICE,
                &STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
            )
            .unwrap();
            let gen_index_digest_1 = Pallet::gen_index_digest(&ALICE, &STAKING, &index_a);
            assert!(gen_index_digest_1.is_ok());
            let gen_index_digest_2 = Pallet::gen_index_digest(&ALICE, &STAKING, &index_a);
            assert!(gen_index_digest_2.is_ok());
            assert_eq!(gen_index_digest_1, gen_index_digest_2); // deterministic key generation for same input

            // new index with small change in shares
            let index_b = Pallet::prepare_index(
                &ALICE,
                &STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
            )
            .unwrap();
            let gen_index_digest_3 = Pallet::gen_index_digest(&ALICE, &STAKING, &index_b);
            assert!(gen_index_digest_3.is_ok());
            assert_ne!(gen_index_digest_2, gen_index_digest_3); // Unique key generation with even a small change in the input

            // same index and propritor with different reason
            let gen_index_digest_4 = Pallet::gen_index_digest(&ALICE, &ESCROW, &index_b);
            assert!(gen_index_digest_4.is_ok());
            assert_ne!(gen_index_digest_2, gen_index_digest_4);
            assert_ne!(gen_index_digest_3, gen_index_digest_4);
        })
    }

    #[test]
    fn on_create_index_event_emmision_success() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(2);
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_MAJOR),
                (VALIDATOR_BETA, SHARE_DOMINANT),
            ];
            let index = Pallet::prepare_index(&ALICE, &STAKING, &entries).unwrap();
            Pallet::set_index(&ALICE, &STAKING, &index, &INDEX_OPTIMIZED_STAKING).unwrap();
            #[cfg(feature = "dev")]
            let entries_defaults = vec![
                (VALIDATOR_ALPHA, SHARE_MAJOR, Position::default()),
                (VALIDATOR_BETA, SHARE_DOMINANT, Position::default()),
            ];
            System::assert_last_event(
                Event::IndexInitialized {
                    index_of: INDEX_OPTIMIZED_STAKING,
                    reason: STAKING,
                    #[cfg(feature = "dev")]
                    entries: entries_defaults,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_reap_index_event_emmision_success() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(2);
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_MAJOR),
                (VALIDATOR_BETA, SHARE_DOMINANT),
            ];
            let index = Pallet::prepare_index(&ALICE, &STAKING, &entries).unwrap();
            Pallet::set_index(&ALICE, &STAKING, &index, &INDEX_OPTIMIZED_STAKING).unwrap();
            System::set_block_number(3);
            Pallet::reap_index(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            System::assert_last_event(
                Event::IndexReaped {
                    index_of: INDEX_OPTIMIZED_STAKING,
                    reason: STAKING,
                }
                .into(),
            );
        })
    }

    #[test]
    fn get_index_value_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // index value after alice commited
            let expected_index_value = STANDARD_COMMIT;
            let actual_index_value =
                Pallet::get_index_value(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            assert_eq!(expected_index_value, actual_index_value);
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // index value after alice and bob commited
            let expected_index_value = STANDARD_COMMIT + LARGE_COMMIT;
            let actual_index_value =
                Pallet::get_index_value(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            assert_eq!(expected_index_value, actual_index_value);
        })
    }

    #[test]
    fn get_entries_value_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let actual_entries_value =
                Pallet::get_entries_value(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_entries_value = vec![(VALIDATOR_ALPHA, 100), (VALIDATOR_BETA, 150)];
            assert_eq!(expected_entries_value, actual_entries_value);
        })
    }

    #[test]
    fn get_entries_value_for_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                SMALL_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // Index entries balance of alice
            let actual_alice_entries_value =
                Pallet::get_entries_value_for(&ALICE, &STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_alice_entries_value = vec![(VALIDATOR_ALPHA, 100), (VALIDATOR_BETA, 150)];
            assert_eq!(actual_alice_entries_value, expected_alice_entries_value);
            // Index entries balance of bob
            let actual_bob_entries_value =
                Pallet::get_entries_value_for(&BOB, &STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_bob_entries_value = vec![(VALIDATOR_ALPHA, 40), (VALIDATOR_BETA, 60)];
            assert_eq!(actual_bob_entries_value, expected_bob_entries_value);
        })
    }

    #[test]
    fn get_index_value_for_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                SMALL_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // Index value of alice
            let alice_index_value =
                Pallet::get_index_value_for(&ALICE, &STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            assert_eq!(alice_index_value, STANDARD_COMMIT);
            // Index value of bob
            let bob_index_value =
                Pallet::get_index_value_for(&BOB, &STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            assert_eq!(bob_index_value, SMALL_COMMIT);
        })
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````` INDEX VARIANT ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn get_entry_variant_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            let bob_commit_variant = Position::position_of(1).unwrap();
            Pallet::place_commit_of_variant(
                &BOB,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &bob_commit_variant,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let charlie_commit_varinat = Position::position_of(2).unwrap();
            Pallet::place_commit_of_variant(
                &CHARLIE,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &charlie_commit_varinat,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            prepare_and_initiate_index(
                MIKE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // Since all entries are initialized with the default variant (regardless of the actual commit variant),
            // below entries should have the default variant at this point.
            let default_varinat = Position::default();
            let alpha_entry_variant =
                Pallet::get_entry_variant(&STAKING, &INDEX_OPTIMIZED_STAKING, &VALIDATOR_ALPHA)
                    .unwrap();
            assert_ne!(alpha_entry_variant, bob_commit_variant);
            assert_eq!(alpha_entry_variant, default_varinat);
            let beta_entry_variant =
                Pallet::get_entry_variant(&STAKING, &INDEX_OPTIMIZED_STAKING, &VALIDATOR_BETA)
                    .unwrap();
            assert_ne!(beta_entry_variant, charlie_commit_varinat);
            assert_eq!(beta_entry_variant, default_varinat);
        })
    }

    #[test]
    fn get_entry_variant_err_entry_not_found() {
        commit_test_ext().execute_with(|| {
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ],
                INDEX_BALANCED_STAKING,
            )
            .unwrap();
            assert_err!(
                Pallet::get_entry_variant(&STAKING, &INDEX_BALANCED_STAKING, &VALIDATOR_GAMMA,),
                Error::EntryOfIndexNotFound
            );
        })
    }

    #[test]
    fn set_entry_of_variant_success_only_variant() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let current_variant =
                Pallet::get_entry_variant(&STAKING, &INDEX_OPTIMIZED_STAKING, &VALIDATOR_ALPHA)
                    .unwrap();
            assert_eq!(current_variant, Position::default());
            // updating the variant of an existing entry
            let new_variant = Position::position_of(2).unwrap();
            let new_index_digest = Pallet::set_entry_of_variant(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                &VALIDATOR_ALPHA,
                new_variant,
                None,
            )
            .unwrap();
            assert_ne!(INDEX_OPTIMIZED_STAKING, new_index_digest);
            // variant updated in new index
            let actual_variant =
                Pallet::get_entry_variant(&STAKING, &new_index_digest, &VALIDATOR_ALPHA).unwrap();
            assert_eq!(actual_variant, new_variant);
            // variant unaffected in old index
            let actual_variant =
                Pallet::get_entry_variant(&STAKING, &INDEX_OPTIMIZED_STAKING, &VALIDATOR_ALPHA)
                    .unwrap();
            assert_eq!(actual_variant, current_variant);
        })
    }

    #[test]
    fn set_entry_of_variant_success_variant_with_shares() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let new_shares = SHARE_DOMINANT;
            let new_variant = Position::position_of(2).unwrap();
            let new_index_digest = Pallet::set_entry_of_variant(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                &VALIDATOR_BETA,
                new_variant,
                Some(new_shares),
            )
            .unwrap();
            assert_ne!(INDEX_OPTIMIZED_STAKING, new_index_digest);
            // variant and shares updated in new index
            let actual_variant =
                Pallet::get_entry_variant(&STAKING, &new_index_digest, &VALIDATOR_BETA).unwrap();
            assert_eq!(actual_variant, new_variant);
            let actual_shares = Pallet::get_entries_shares(&STAKING, &new_index_digest).unwrap();
            let expected_shares =
                vec![(VALIDATOR_ALPHA, SHARE_MAJOR), (VALIDATOR_BETA, new_shares)];
            assert_eq!(actual_shares, expected_shares);
            // variant and shares unaffected in old index
            let default_variant = Position::default();
            let actual_variant =
                Pallet::get_entry_variant(&STAKING, &INDEX_OPTIMIZED_STAKING, &VALIDATOR_BETA)
                    .unwrap();
            assert_eq!(actual_variant, default_variant);
            let actual_shares =
                Pallet::get_entries_shares(&STAKING, &INDEX_OPTIMIZED_STAKING).unwrap();
            let expected_shares = vec![
                (VALIDATOR_ALPHA, SHARE_MAJOR),
                (VALIDATOR_BETA, SHARE_DOMINANT),
            ];
            assert_eq!(actual_shares, expected_shares);
        })
    }

    #[test]
    fn set_entry_of_variant_new_entry_shares_cannot_be_none() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let new_variant = Position::position_of(2).unwrap();
            let new_index_digest = Pallet::set_entry_of_variant(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                &VALIDATOR_GAMMA,
                new_variant,
                None,
            )
            .unwrap();
            // Since, the new entry share is set to `None`, same index digest is returned.
            assert_eq!(new_index_digest, INDEX_OPTIMIZED_STAKING);
        })
    }

    #[test]
    fn prepare_index_of_variants_success() {
        commit_test_ext().execute_with(|| {
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            let entries = vec![
                (
                    VALIDATOR_ALPHA,
                    SHARE_DOMINANT,
                    Position::position_of(2).unwrap(),
                ),
                (
                    VALIDATOR_BETA,
                    SHARE_MAJOR,
                    Position::position_of(1).unwrap(),
                ),
            ];
            let index = Pallet::prepare_index_of_variants(&ALICE, &STAKING, entries).unwrap();
            assert_eq!(index.principal(), ZERO_VALUE);
            assert_eq!(index.capital(), 100);
            let expected_entry_alpha = EntryInfo::new(
                VALIDATOR_ALPHA,
                SHARE_DOMINANT,
                Position::position_of(2).unwrap(),
            )
            .unwrap();
            let expected_entry_beta = EntryInfo::new(
                VALIDATOR_BETA,
                SHARE_MAJOR,
                Position::position_of(1).unwrap(),
            )
            .unwrap();
            assert_eq!(index.entries().get(0).unwrap(), &expected_entry_alpha);
            assert_eq!(index.entries().get(1).unwrap(), &expected_entry_beta);
        })
    }

    #[test]
    fn prepare_index_of_variants_err_duplicate_entry() {
        commit_test_ext().execute_with(|| {
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            let actual_err = Pallet::prepare_index_of_variants(
                &ALICE,
                &STAKING,
                vec![
                    (VALIDATOR_ALPHA, SHARE_EQUAL, Position::default()),
                    (VALIDATOR_BETA, SHARE_EQUAL, Position::default()),
                    (VALIDATOR_ALPHA, SHARE_EQUAL, Position::default()),
                ],
            )
            .unwrap_err();
            let expected_err = Error::DuplicateEntry.into();
            assert_eq!(actual_err, expected_err);
        })
    }

    #[test]
    fn prepare_index_of_variants_err_max_index_reached() {
        commit_test_ext().execute_with(|| {
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_GAMMA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_DELTA).unwrap();
            let actual_err = Pallet::prepare_index_of_variants(
                &ALICE,
                &STAKING,
                vec![
                    (VALIDATOR_ALPHA, SHARE_EQUAL, Position::default()),
                    (
                        VALIDATOR_BETA,
                        SHARE_EQUAL,
                        Position::position_of(2).unwrap(),
                    ),
                    (VALIDATOR_GAMMA, SHARE_EQUAL, Position::default()),
                    (VALIDATOR_DELTA, SHARE_EQUAL, Position::default()),
                ],
            )
            .unwrap_err();
            // since MaxEntries is set to 3, adding a fourth entry results in err
            assert_eq!(actual_err, Error::MaxEntriesReached.into());
        })
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` COMMIT POOL `````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn pool_exists_ok() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            assert_ok!(Pallet::pool_exists(&STAKING, &POOL_MANAGED_STAKING));
        })
    }

    #[test]
    fn pool_exists_err_pool_not_found() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            assert_err!(
                Pallet::pool_exists(&ESCROW, &POOL_PROFESSIONAL_ESCROW),
                Error::PoolNotFound
            );
        })
    }

    #[test]
    fn slot_exists_ok() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            assert_ok!(Pallet::slot_exists(
                &STAKING,
                &POOL_MANAGED_STAKING,
                &VALIDATOR_ALPHA
            ));
        })
    }

    #[test]
    fn slot_exists_err_slot_not_found() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            assert_err!(
                Pallet::slot_exists(&STAKING, &POOL_MANAGED_STAKING, &VALIDATOR_GAMMA),
                Error::SlotOfPoolNotFound
            );
        })
    }

    #[test]
    fn has_pool_ok() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            assert_ok!(Pallet::has_pool(&STAKING));
        })
    }

    #[test]
    fn has_pool_err_pool_not_found() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            assert_err!(Pallet::has_pool(&GOVERNANCE), Error::PoolNotFound);
        })
    }

    #[test]
    fn get_manager_success() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            assert_eq!(
                Pallet::get_manager(&STAKING, &POOL_MANAGED_STAKING),
                Ok(ALICE)
            );
        })
    }

    #[test]
    fn get_manager_err_pool_not_found() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            // checks pool first
            assert_err!(
                Pallet::get_manager(&ESCROW, &POOL_PROFESSIONAL_ESCROW),
                Error::PoolNotFound
            );
        })
    }

    #[test]
    fn get_pool_success() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            let expected_pool = PoolMap::get((STAKING, POOL_MANAGED_STAKING)).unwrap();
            let actual_pool = Pallet::get_pool(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            assert_eq!(expected_pool.balance(), actual_pool.balance());
            assert_eq!(expected_pool.capital(), actual_pool.capital());
            assert_eq!(expected_pool.commission(), actual_pool.commission());
            assert_eq!(
                expected_pool.slots().get(0).unwrap(),
                actual_pool.slots().get(0).unwrap()
            );
            assert_eq!(
                expected_pool.slots().get(1).unwrap(),
                actual_pool.slots().get(1).unwrap()
            );
        })
    }

    #[test]
    fn get_pool_err_pool_not_found() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            let err = Pallet::get_pool(&ESCROW, &POOL_PROFESSIONAL_ESCROW).unwrap_err();
            assert_eq!(err, Error::PoolNotFound.into());
        })
    }

    #[test]
    fn get_commission_success() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_STANDARD,
            )
            .unwrap();
            let actual_commission =
                Pallet::get_commission(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            assert_eq!(actual_commission, COMMISSION_STANDARD);
        })
    }

    #[test]
    fn get_commission_err_pool_not_found() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_STANDARD,
            )
            .unwrap();
            let err = Pallet::get_commission(&GOVERNANCE, &POOL_EXPERT_GOVERNANCE).unwrap_err();
            assert_eq!(err, Error::PoolNotFound.into());
        })
    }

    #[test]
    fn set_pool_success() {
        commit_test_ext().execute_with(|| {
            System::set_block_number(10);
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[
                    (VALIDATOR_ALPHA, SHARE_MAJOR),
                    (VALIDATOR_BETA, SHARE_DOMINANT),
                ],
                INDEX_OPTIMIZED_STAKING,
            )
            .unwrap();
            assert_ok!(Pallet::index_exists(&STAKING, &INDEX_OPTIMIZED_STAKING));
            assert_err!(
                Pallet::pool_exists(&STAKING, &POOL_MANAGED_STAKING),
                Error::PoolNotFound
            );
            let commission = COMMISSION_STANDARD;
            Pallet::set_pool(
                &ALICE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                &INDEX_OPTIMIZED_STAKING,
                commission,
            )
            .unwrap();
            assert_ok!(Pallet::pool_exists(&STAKING, &POOL_MANAGED_STAKING));
            let pool_info = PoolMap::get((STAKING, POOL_MANAGED_STAKING)).unwrap();
            assert_eq!(pool_info.capital(), 100);
            assert_eq!(pool_info.commission(), commission);
            assert_eq!(
                Pallet::get_manager(&STAKING, &POOL_MANAGED_STAKING),
                Ok(ALICE)
            );

            #[cfg(not(feature = "dev"))]
            System::assert_last_event(Event::PoolInitialized {
                    pool_of: POOL_MANAGED_STAKING,
                    reason: STAKING,
                    commission: commission
                }
                .into()
            );

            #[cfg(feature = "dev")]
            {            
                let slots = vec![(VALIDATOR_ALPHA, SHARE_MAJOR, Disposition::default()), (VALIDATOR_BETA, SHARE_DOMINANT, Disposition::default())];
                System::assert_last_event(Event::PoolInitialized {
                        pool_of: POOL_MANAGED_STAKING,
                        reason: STAKING,
                        commission: commission,
                        slots
                    }
                    .into()
                );
            }
        })
    }

    #[test]
    fn set_pool_err_pool_already_exists() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            assert_err!(
                Pallet::set_pool(
                    &ALICE,
                    &STAKING,
                    &POOL_MANAGED_STAKING,
                    &INDEX_BALANCED_STAKING,
                    COMMISSION_STANDARD,
                ),
                Error::PoolDigestTaken
            );
        })
    }

    #[test]
    fn set_pool_manager_ok() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            assert_eq!(
                Pallet::get_manager(&STAKING, &POOL_MANAGED_STAKING),
                Ok(ALICE)
            );
            // change manager from alice -> charlie
            assert_ok!(Pallet::set_pool_manager(
                &STAKING,
                &POOL_MANAGED_STAKING,
                &CHARLIE
            ));
            // manager changed
            assert_eq!(
                Pallet::get_manager(&STAKING, &POOL_MANAGED_STAKING),
                Ok(CHARLIE)
            );
        })
    }

    #[test]
    fn set_slot_shares_success_updating_existing_slot_shares() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_DOMINANT),
                (VALIDATOR_BETA, SHARE_MAJOR),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();

            assert_ok!(Pallet::set_slot_shares(
                &ALICE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                &VALIDATOR_ALPHA,
                SHARE_EQUAL
            ));
            assert_ok!(Pallet::set_slot_shares(
                &ALICE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                &VALIDATOR_BETA,
                SHARE_EQUAL
            ));
            let expected_slots_shares = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            let actual_slots_shares =
                Pallet::get_slots_shares(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            assert_eq!(actual_slots_shares, expected_slots_shares)
        })
    }

    #[test]
    fn set_slot_shares_success_removing_slot_with_zero_share() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_DOMINANT),
                (VALIDATOR_BETA, SHARE_MAJOR),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &INDEX_OPTIMIZED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();

            assert_ok!(Pallet::set_slot_shares(
                &MIKE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                &VALIDATOR_ALPHA,
                SHARE_EQUAL
            ));
            assert_ok!(Pallet::set_slot_shares(
                &MIKE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                &VALIDATOR_BETA,
                ZERO_SHARE,
            ));
            // VALIDATOR_BETA is removed
            let expected_slots_shares = vec![(VALIDATOR_ALPHA, SHARE_EQUAL)];
            let actual_slots_shares =
                Pallet::get_slots_shares(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            assert_eq!(actual_slots_shares, expected_slots_shares)
        })
    }

    #[test]
    fn set_slot_shares_success_creating_new_slot_with_non_zero_shares() {
        commit_test_ext().execute_with(|| {
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_GAMMA).unwrap();

            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();

            assert_ok!(Pallet::set_slot_shares(
                &MIKE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                &VALIDATOR_GAMMA,
                SHARE_EQUAL
            ));
            // VALIDATOR_GAMMA is added to the existing slots
            let expected_slots_shares = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
                (VALIDATOR_GAMMA, SHARE_EQUAL),
            ];
            let actual_slots_shares =
                Pallet::get_slots_shares(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            assert_eq!(actual_slots_shares, expected_slots_shares)
        })
    }

    #[test]
    fn gen_pool_digest_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(ESCROW, CONTRACT_FREELANCE).unwrap();
            prepare_and_initiate_index(
                ALICE,
                STAKING,
                &[(VALIDATOR_ALPHA, SHARE_EQUAL)],
                INDEX_BALANCED_STAKING,
            )
            .unwrap();
            prepare_and_initiate_index(
                CHARLIE,
                ESCROW,
                &[(CONTRACT_FREELANCE, SHARE_EQUAL)],
                INDEX_ESCROW_DISTRIBUTION,
            )
            .unwrap();
            let gen_pool_diget_1 =
                Pallet::gen_pool_digest(&ALICE, &STAKING, &INDEX_BALANCED_STAKING, COMMISSION_LOW);
            assert!(gen_pool_diget_1.is_ok());
            let gen_pool_diget_2 =
                Pallet::gen_pool_digest(&ALICE, &STAKING, &INDEX_BALANCED_STAKING, COMMISSION_LOW);
            assert!(gen_pool_diget_2.is_ok());
            assert_eq!(gen_pool_diget_1, gen_pool_diget_2); // deterministic key generation for same inputs

            // Different index digest, manager and reason
            let gen_pool_diget_3 = Pallet::gen_pool_digest(
                &CHARLIE,
                &ESCROW,
                &INDEX_ESCROW_DISTRIBUTION,
                COMMISSION_LOW,
            );
            assert!(gen_pool_diget_3.is_ok());
            assert_ne!(gen_pool_diget_2, gen_pool_diget_3);
            // Same proprietor, reason, index_digest with different commission
            let gen_pool_diget_4 = Pallet::gen_pool_digest(
                &CHARLIE,
                &ESCROW,
                &INDEX_ESCROW_DISTRIBUTION,
                COMMISSION_STANDARD,
            );
            assert!(gen_pool_diget_4.is_ok());
            assert_ne!(gen_pool_diget_3, gen_pool_diget_4);
            assert_ne!(gen_pool_diget_2, gen_pool_diget_4);
        })
    }

    #[test]
    fn get_slot_shares_success() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_DOMINANT),
                (VALIDATOR_BETA, SHARE_MAJOR),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            let expected_slots_shares = vec![
                (VALIDATOR_ALPHA, SHARE_DOMINANT),
                (VALIDATOR_BETA, SHARE_MAJOR),
            ];
            let actual_slots_shares =
                Pallet::get_slots_shares(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            assert_eq!(actual_slots_shares, expected_slots_shares)
        })
    }

    #[test]
    fn get_slot_value_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            set_default_user_balance_and_standard_hold(ALAN).unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &ALAN,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();

            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_DOMINANT),
                (VALIDATOR_BETA, SHARE_MAJOR),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();

            let expected_alpha_slot_value = 150;
            let actual_alpha_slot_value =
                Pallet::get_slot_value(&STAKING, &POOL_MANAGED_STAKING, &VALIDATOR_ALPHA).unwrap();
            assert_eq!(expected_alpha_slot_value, actual_alpha_slot_value);

            let expected_beta_slot_value = 100;
            let actual_beta_slot_value =
                Pallet::get_slot_value(&STAKING, &POOL_MANAGED_STAKING, &VALIDATOR_BETA).unwrap();
            assert_eq!(expected_beta_slot_value, actual_beta_slot_value);

            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();

            let expected_alpha_slot_value = 450;
            let actual_alpha_slot_value =
                Pallet::get_slot_value(&STAKING, &POOL_MANAGED_STAKING, &VALIDATOR_ALPHA).unwrap();
            assert_eq!(expected_alpha_slot_value, actual_alpha_slot_value);

            let expected_beta_slot_value = 300;
            let actual_beta_slot_value =
                Pallet::get_slot_value(&STAKING, &POOL_MANAGED_STAKING, &VALIDATOR_BETA).unwrap();
            assert_eq!(expected_beta_slot_value, actual_beta_slot_value);
        })
    }

    #[test]
    fn get_slot_value_for_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            set_default_user_balance_and_standard_hold(ALAN).unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &ALAN,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();

            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_DOMINANT),
                (VALIDATOR_BETA, SHARE_MAJOR),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();

            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                SMALL_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // pool slot value of alice
            let expected_alpha_slot_value = 150;
            let actual_alpha_slot_value = Pallet::get_slot_value_for(
                &ALICE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                &VALIDATOR_ALPHA,
            )
            .unwrap();
            assert_eq!(expected_alpha_slot_value, actual_alpha_slot_value);

            let expected_beta_slot_value = 100;
            let actual_beta_slot_value = Pallet::get_slot_value_for(
                &ALICE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                &VALIDATOR_BETA,
            )
            .unwrap();
            assert_eq!(expected_beta_slot_value, actual_beta_slot_value);
            // pool slot value of charlie
            let expected_alpha_slot_value = 60;
            let actual_alpha_slot_value = Pallet::get_slot_value_for(
                &CHARLIE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                &VALIDATOR_ALPHA,
            )
            .unwrap();
            assert_eq!(expected_alpha_slot_value, actual_alpha_slot_value);

            let expected_beta_slot_value = 40;
            let actual_beta_slot_value = Pallet::get_slot_value_for(
                &CHARLIE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                &VALIDATOR_BETA,
            )
            .unwrap();
            assert_eq!(expected_beta_slot_value, actual_beta_slot_value);
        })
    }

    #[test]
    fn reap_pool_success() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_MAJOR),
                (VALIDATOR_BETA, SHARE_DOMINANT),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            assert_ok!(Pallet::pool_exists(&STAKING, &POOL_MANAGED_STAKING));
            assert_ok!(Pallet::reap_pool(&STAKING, &POOL_MANAGED_STAKING));
            assert_err!(
                Pallet::pool_exists(&STAKING, &POOL_MANAGED_STAKING),
                Error::PoolNotFound
            );
        })
    }

    #[test]
    fn reap_pool_err_pool_has_funds() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            assert_err!(
                Pallet::reap_pool(&STAKING, &POOL_MANAGED_STAKING),
                Error::PoolHasFunds
            );
        })
    }

    #[test]
    fn on_set_pool_event_emmission_success() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            System::set_block_number(BLOCK_EARLY);
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            #[cfg(feature = "dev")]
            let entries_defaults = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL, Position::default()),
                (VALIDATOR_BETA, SHARE_EQUAL, Position::default()),
            ];
            System::assert_last_event(
                Event::PoolInitialized {
                    pool_of: POOL_MANAGED_STAKING,
                    reason: STAKING,
                    commission: COMMISSION_LOW,
                    #[cfg(feature = "dev")]
                    slots: entries_defaults,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_set_manager_event_emmission_success() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            System::set_block_number(BLOCK_EARLY);
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            Pallet::set_pool_manager(&STAKING, &POOL_MANAGED_STAKING, &BOB).unwrap();
            System::assert_last_event(
                Event::PoolManager {
                    pool_of: POOL_MANAGED_STAKING,
                    reason: STAKING,
                    manager: BOB,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_set_slot_shares_event_emmission_success() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            System::set_block_number(BLOCK_EARLY);
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            Pallet::set_slot_shares(
                &ALICE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                &VALIDATOR_ALPHA,
                SHARE_DOMINANT,
            )
            .unwrap();
            System::assert_last_event(
                Event::PoolSlot {
                    pool_of: POOL_MANAGED_STAKING,
                    reason: STAKING,
                    slot_of: VALIDATOR_ALPHA,
                    variant: Position::default(),
                    shares: SHARE_DOMINANT,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_reap_pool_event_emmission_success() {
        commit_test_ext().execute_with(|| {
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            System::set_block_number(2);
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            Pallet::reap_pool(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            System::assert_last_event(
                Event::PoolReaped {
                    pool_of: POOL_MANAGED_STAKING,
                    reason: STAKING,
                }
                .into(),
            );
        })
    }

    #[test]
    fn get_pool_value_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            set_default_user_balance_and_standard_hold(ALAN).unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &ALAN,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_DOMINANT),
                (VALIDATOR_BETA, SHARE_MAJOR),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                LARGE_COMMIT,
                &Directive::new(Precision::BestEffort, Fortitude::Polite),
            )
            .unwrap();
            let actual_pool_value =
                Pallet::get_pool_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            assert_eq!(actual_pool_value, LARGE_COMMIT);
        })
    }

    #[test]
    fn get_pool_value_for_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            set_default_user_balance_and_standard_hold(ALAN).unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &ALAN,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_DOMINANT),
                (VALIDATOR_BETA, SHARE_MAJOR),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // Alice pool value
            let alice_pool_value =
                Pallet::get_pool_value_for(&ALICE, &STAKING, &POOL_MANAGED_STAKING).unwrap();
            assert_eq!(alice_pool_value, LARGE_COMMIT);

            // Charlie pool value
            let mike_pool_value =
                Pallet::get_pool_value_for(&CHARLIE, &STAKING, &POOL_MANAGED_STAKING).unwrap();
            assert_eq!(mike_pool_value, STANDARD_COMMIT);
        })
    }

    #[test]
    fn get_slots_value_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            set_default_user_balance_and_standard_hold(ALAN).unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &ALAN,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_DOMINANT),
                (VALIDATOR_BETA, SHARE_MAJOR),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let actual_pool_slots_value =
                Pallet::get_slots_value(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_pool_slots_value = vec![(VALIDATOR_ALPHA, 300), (VALIDATOR_BETA, 200)];
            assert_eq!(actual_pool_slots_value, expected_pool_slots_value);
        })
    }

    #[test]
    fn get_slots_value_for_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            set_default_user_balance_and_standard_hold(ALAN).unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &ALAN,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_DOMINANT),
                (VALIDATOR_BETA, SHARE_MAJOR),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                LARGE_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &CHARLIE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                SMALL_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            // Pool slots value for alice
            let alice_pool_slots_value =
                Pallet::get_slots_value_for(&ALICE, &STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_alice_pool_slots_value =
                vec![(VALIDATOR_ALPHA, 300), (VALIDATOR_BETA, 200)];
            assert_eq!(expected_alice_pool_slots_value, alice_pool_slots_value);
            // Pool slots value for charlie
            let alan_pool_slots_value =
                Pallet::get_slots_value_for(&CHARLIE, &STAKING, &POOL_MANAGED_STAKING).unwrap();
            let expected_alan_pool_slots_value = vec![(VALIDATOR_ALPHA, 60), (VALIDATOR_BETA, 40)];
            assert_eq!(expected_alan_pool_slots_value, alan_pool_slots_value);
        })
    }

    #[test]
    fn set_commission_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            set_default_user_balance_and_standard_hold(ALAN).unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &ALAN,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();

            let commission = COMMISSION_LOW;
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_DOMINANT),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                commission,
            )
            .unwrap();

            // Settign a new commission which generated a new pool digest
            let new_commission = COMMISSION_STANDARD;
            let new_pool_digest =
                Pallet::set_commission(&ALICE, &STAKING, &INDEX_OPTIMIZED_STAKING, new_commission)
                    .unwrap();
            // new pool created with updated commission
            assert_ok!(Pallet::pool_exists(&STAKING, &new_pool_digest));
            let actual_commission = Pallet::get_commission(&STAKING, &new_pool_digest).unwrap();
            assert_eq!(actual_commission, new_commission);
            // old pool commission left unaffected
            let old_pool_commission =
                Pallet::get_commission(&STAKING, &POOL_MANAGED_STAKING).unwrap();
            assert_eq!(old_pool_commission, commission);
        })
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` POOL VARIANT ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn get_slot_variant_success() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            set_default_user_balance_and_standard_hold(BOB).unwrap();
            set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
            set_default_user_balance_and_standard_hold(ALAN).unwrap();
            Pallet::place_commit(
                &BOB,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            Pallet::place_commit(
                &ALAN,
                &STAKING,
                &VALIDATOR_BETA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();

            let commission = COMMISSION_LOW;
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_DOMINANT),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_OPTIMIZED_STAKING,
                POOL_MANAGED_STAKING,
                commission,
            )
            .unwrap();

            let expected_slot_variant = Position::default();
            let actual_slot_variant =
                Pallet::get_slot_variant(&STAKING, &POOL_MANAGED_STAKING, &VALIDATOR_ALPHA)
                    .unwrap();
            assert_eq!(expected_slot_variant, actual_slot_variant);
            // Affirmative -> Contrary
            Pallet::set_slot_of_variant(
                &ALICE,
                &STAKING,
                &POOL_MANAGED_STAKING,
                &VALIDATOR_ALPHA,
                Position::position_of(1).unwrap(),
                None,
            )
            .unwrap();
            let expected_slot_variant = Position::position_of(1).unwrap();
            let actual_slot_variant =
                Pallet::get_slot_variant(&STAKING, &POOL_MANAGED_STAKING, &VALIDATOR_ALPHA)
                    .unwrap();
            assert_eq!(expected_slot_variant, actual_slot_variant);
        })
    }

    #[test]
    fn get_slot_variant_err_slot_not_found() {
        commit_test_ext().execute_with(|| {
            initiate_digest_with_default_balance(STAKING, VALIDATOR_ALPHA).unwrap();
            initiate_digest_with_default_balance(STAKING, VALIDATOR_BETA).unwrap();
            let entries = vec![
                (VALIDATOR_ALPHA, SHARE_EQUAL),
                (VALIDATOR_BETA, SHARE_EQUAL),
            ];
            prepare_and_initiate_pool(
                MIKE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            assert_err!(
                Pallet::get_slot_variant(&STAKING, &POOL_MANAGED_STAKING, &VALIDATOR_GAMMA,),
                Error::SlotOfPoolNotFound
            );
        })
    }

    #[test]
    fn set_slot_of_variant_success_variant_update() {
        commit_test_ext().execute_with(|| {
            commit_test_ext().execute_with(|| {
                set_default_user_balance_and_standard_hold(ALICE).unwrap();
                set_default_user_balance_and_standard_hold(BOB).unwrap();
                Pallet::place_commit(
                    &ALICE,
                    &STAKING,
                    &VALIDATOR_ALPHA,
                    STANDARD_COMMIT,
                    &Directive::new(Precision::Exact, Fortitude::Force),
                )
                .unwrap();
                Pallet::place_commit(
                    &BOB,
                    &STAKING,
                    &VALIDATOR_BETA,
                    STANDARD_COMMIT,
                    &Directive::new(Precision::Exact, Fortitude::Force),
                )
                .unwrap();

                let entries = vec![
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ];
                prepare_and_initiate_pool(
                    MIKE,
                    STAKING,
                    &entries,
                    INDEX_BALANCED_STAKING,
                    POOL_MANAGED_STAKING,
                    COMMISSION_LOW,
                )
                .unwrap();
                let expected_slot_variant = Position::default();
                let actual_slot_variant =
                    Pallet::get_slot_variant(&STAKING, &POOL_MANAGED_STAKING, &VALIDATOR_ALPHA)
                        .unwrap();
                assert_eq!(expected_slot_variant, actual_slot_variant);
                let new_slot_variant = Position::position_of(1).unwrap();
                Pallet::set_slot_of_variant(
                    &MIKE,
                    &STAKING,
                    &POOL_MANAGED_STAKING,
                    &VALIDATOR_ALPHA,
                    new_slot_variant,
                    None,
                )
                .unwrap();

                let actual_slot_variant =
                    Pallet::get_slot_variant(&STAKING, &POOL_MANAGED_STAKING, &VALIDATOR_ALPHA)
                        .unwrap();
                assert_eq!(actual_slot_variant, new_slot_variant);
            })
        })
    }

    #[test]
    fn set_slot_of_variant_success_variant_and_shares_update() {
        commit_test_ext().execute_with(|| {
            commit_test_ext().execute_with(|| {
                set_default_user_balance_and_standard_hold(ALICE).unwrap();
                set_default_user_balance_and_standard_hold(BOB).unwrap();
                Pallet::place_commit(
                    &ALICE,
                    &STAKING,
                    &VALIDATOR_ALPHA,
                    STANDARD_COMMIT,
                    &Directive::new(Precision::Exact, Fortitude::Force),
                )
                .unwrap();
                Pallet::place_commit(
                    &BOB,
                    &STAKING,
                    &VALIDATOR_BETA,
                    STANDARD_COMMIT,
                    &Directive::new(Precision::Exact, Fortitude::Force),
                )
                .unwrap();

                let entries = vec![
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ];
                prepare_and_initiate_pool(
                    MIKE,
                    STAKING,
                    &entries,
                    INDEX_BALANCED_STAKING,
                    POOL_MANAGED_STAKING,
                    COMMISSION_LOW,
                )
                .unwrap();
                let expected_slot_variant = Position::default();
                let actual_slot_variant =
                    Pallet::get_slot_variant(&STAKING, &POOL_MANAGED_STAKING, &VALIDATOR_ALPHA)
                        .unwrap();
                let actual_slot_shares =
                    Pallet::get_slots_shares(&STAKING, &POOL_MANAGED_STAKING).unwrap();
                let expected_slot_shares = entries;
                assert_eq!(expected_slot_variant, actual_slot_variant);
                assert_eq!(expected_slot_shares, actual_slot_shares);
                let new_slot_variant = Position::position_of(2).unwrap();
                Pallet::set_slot_of_variant(
                    &ALICE,
                    &STAKING,
                    &POOL_MANAGED_STAKING,
                    &VALIDATOR_ALPHA,
                    new_slot_variant,
                    Some(SHARE_DOMINANT),
                )
                .unwrap();
                let actual_slot_variant =
                    Pallet::get_slot_variant(&STAKING, &POOL_MANAGED_STAKING, &VALIDATOR_ALPHA)
                        .unwrap();
                let actual_slot_shares =
                    Pallet::get_slots_shares(&STAKING, &POOL_MANAGED_STAKING).unwrap();
                let expected_slot_shares = vec![
                    (VALIDATOR_BETA, SHARE_EQUAL),
                    (VALIDATOR_ALPHA, SHARE_DOMINANT),
                ];
                assert_eq!(actual_slot_variant, new_slot_variant);
                assert_eq!(actual_slot_shares, expected_slot_shares);
            })
        })
    }

    #[test]
    fn set_slot_of_variant_new_slot_shares_cannot_be_none() {
        commit_test_ext().execute_with(|| {
            commit_test_ext().execute_with(|| {
                set_default_user_balance_and_standard_hold(ALICE).unwrap();
                set_default_user_balance_and_standard_hold(BOB).unwrap();
                set_default_user_balance_and_standard_hold(CHARLIE).unwrap();
                Pallet::place_commit(
                    &ALICE,
                    &STAKING,
                    &VALIDATOR_ALPHA,
                    STANDARD_COMMIT,
                    &Directive::new(Precision::Exact, Fortitude::Force),
                )
                .unwrap();
                Pallet::place_commit(
                    &BOB,
                    &STAKING,
                    &VALIDATOR_BETA,
                    STANDARD_COMMIT,
                    &Directive::new(Precision::Exact, Fortitude::Force),
                )
                .unwrap();
                let entries = vec![
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ];
                prepare_and_initiate_pool(
                    MIKE,
                    STAKING,
                    &entries,
                    INDEX_BALANCED_STAKING,
                    POOL_MANAGED_STAKING,
                    COMMISSION_LOW,
                )
                .unwrap();
                let expected_slot_variant = Position::default();
                let actual_slot_variant =
                    Pallet::get_slot_variant(&STAKING, &POOL_MANAGED_STAKING, &VALIDATOR_ALPHA)
                        .unwrap();
                let actual_slot_shares =
                    Pallet::get_slots_shares(&STAKING, &POOL_MANAGED_STAKING).unwrap();
                let expected_slot_shares = vec![
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ];
                assert_eq!(expected_slot_variant, actual_slot_variant);
                assert_eq!(expected_slot_shares, actual_slot_shares);
                Pallet::place_commit(
                    &CHARLIE,
                    &STAKING,
                    &VALIDATOR_GAMMA,
                    STANDARD_COMMIT,
                    &Directive::new(Precision::Exact, Fortitude::Force),
                )
                .unwrap();
                // Adding a new slot digest while shares is set to None returns Error
                assert_err!(
                    Pallet::set_slot_of_variant(
                        &ALICE,
                        &STAKING,
                        &POOL_MANAGED_STAKING,
                        &VALIDATOR_GAMMA,
                        Position::position_of(1).unwrap(),
                        None,
                    ),
                    Error::SlotOfPoolNotFound
                );
                // Nothing changes due to invalid shares
                let actual_slot_variant =
                    Pallet::get_slot_variant(&STAKING, &POOL_MANAGED_STAKING, &VALIDATOR_ALPHA)
                        .unwrap();
                let actual_slot_shares =
                    Pallet::get_slots_shares(&STAKING, &POOL_MANAGED_STAKING).unwrap();
                assert_eq!(actual_slot_variant, expected_slot_variant);
                assert_eq!(actual_slot_shares, expected_slot_shares);
            })
        })
    }

    #[test]
    fn set_slot_of_variant_new_slot_digest() {
        commit_test_ext().execute_with(|| {
            commit_test_ext().execute_with(|| {
                set_default_user_balance_and_standard_hold(ALICE).unwrap();
                set_default_user_balance_and_standard_hold(BOB).unwrap();
                Pallet::place_commit(
                    &ALICE,
                    &STAKING,
                    &VALIDATOR_ALPHA,
                    STANDARD_COMMIT,
                    &Directive::new(Precision::Exact, Fortitude::Force),
                )
                .unwrap();
                Pallet::place_commit(
                    &BOB,
                    &STAKING,
                    &VALIDATOR_BETA,
                    STANDARD_COMMIT,
                    &Directive::new(Precision::Exact, Fortitude::Force),
                )
                .unwrap();
                let entries = vec![
                    (VALIDATOR_ALPHA, SHARE_EQUAL),
                    (VALIDATOR_BETA, SHARE_EQUAL),
                ];
                prepare_and_initiate_pool(
                    MIKE,
                    STAKING,
                    &entries,
                    INDEX_BALANCED_STAKING,
                    POOL_MANAGED_STAKING,
                    COMMISSION_LOW,
                )
                .unwrap();
                Pallet::set_slot_of_variant(
                    &ALICE,
                    &STAKING,
                    &POOL_MANAGED_STAKING,
                    &VALIDATOR_GAMMA,
                    Position::position_of(1).unwrap(),
                    Some(SHARE_EQUAL),
                )
                .unwrap();

                assert_eq!(
                    Pallet::get_slot_variant(&STAKING, &POOL_MANAGED_STAKING, &VALIDATOR_GAMMA,)
                        .unwrap(),
                    Position::position_of(1).unwrap(),
                );
                // Without shares returns error to set a new-variant for a new-digest
                assert_err!(
                    Pallet::set_slot_of_variant(
                        &ALICE,
                        &STAKING,
                        &POOL_MANAGED_STAKING,
                        &VALIDATOR_DELTA,
                        Position::position_of(1).unwrap(),
                        None,
                    ),
                    Error::SlotOfPoolNotFound
                );
                assert_err!(
                    Pallet::get_slot_variant(&STAKING, &POOL_MANAGED_STAKING, &VALIDATOR_DELTA,),
                    Error::SlotOfPoolNotFound
                );
                // Since its semantically no-op if its zero shares hence its silently passes
                assert_ok!(Pallet::set_slot_of_variant(
                    &ALICE,
                    &STAKING,
                    &POOL_MANAGED_STAKING,
                    &VALIDATOR_DELTA,
                    Position::position_of(1).unwrap(),
                    Some(0),
                ));
            })
        })
    }

    #[test]
    fn on_set_slot_of_variant() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let entries = vec![(VALIDATOR_ALPHA, SHARE_EQUAL)];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            System::set_block_number(2);
            let new_shares = SHARE_DOMINANT;
            let new_variant = Position::position_of(1).unwrap();
            Pallet::on_set_slot_of_variant(
                &POOL_MANAGED_STAKING,
                &STAKING,
                &VALIDATOR_ALPHA,
                Some(new_shares),
                &new_variant,
            );
            System::assert_last_event(
                Event::PoolSlot {
                    pool_of: POOL_MANAGED_STAKING,
                    reason: STAKING,
                    slot_of: VALIDATOR_ALPHA,
                    variant: new_variant,
                    shares: new_shares,
                }
                .into(),
            );
        })
    }

    #[test]
    fn on_set_slot_of_variant_shares_is_zero() {
        commit_test_ext().execute_with(|| {
            set_default_user_balance_and_standard_hold(ALICE).unwrap();
            Pallet::place_commit(
                &ALICE,
                &STAKING,
                &VALIDATOR_ALPHA,
                STANDARD_COMMIT,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )
            .unwrap();
            let entries = vec![(VALIDATOR_ALPHA, SHARE_EQUAL)];
            prepare_and_initiate_pool(
                ALICE,
                STAKING,
                &entries,
                INDEX_BALANCED_STAKING,
                POOL_MANAGED_STAKING,
                COMMISSION_LOW,
            )
            .unwrap();
            System::set_block_number(2);
            let new_shares = 0;
            let new_variant = Position::position_of(1).unwrap();
            Pallet::on_set_slot_of_variant(
                &POOL_MANAGED_STAKING,
                &STAKING,
                &VALIDATOR_ALPHA,
                Some(new_shares),
                &new_variant,
            );
            System::assert_last_event(
                Event::PoolSlotRemoved {
                    pool_of: POOL_MANAGED_STAKING,
                    reason: STAKING,
                    slot_of: VALIDATOR_ALPHA,
                    variant: new_variant,
                }
                .into(),
            );
        })
    }
}
