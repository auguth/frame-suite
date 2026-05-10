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