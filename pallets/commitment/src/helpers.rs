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
// ```````````````````````````````` COMMIT HELPERS ```````````````````````````````
// ===============================================================================

//! Implementation of low-level [`commit-helpers`](crate::traits)
//! traits for the internal [`CommitHelpers`] Type.
//!
//! [`CommitHelpers`] implements traits:
//! - [`CommitBalance`]
//! - [`CommitDeposit`]
//! - [`CommitWithdraw`]
//! - [`CommitOps`]
//! - [`CommitInspect`]
//! - [`PoolOps`]
//! - [`IndexOps`]
//!
//! Local Tests for these traits are covered in `tests`    .

// ===============================================================================
// ```````````````````````````````````` IMPORTS ``````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{
    balance::*, traits::*, types::*, AssetToIssue, AssetToReap, CommitHelpers, CommitMap, Config,
    DigestMap, EntryMap, Error, HoldReason, IndexMap, Pallet, PoolMap, ReasonValue,
};

// --- Core ---
use core::cmp::Ordering;

// --- FRAME Suite ---
use frame_suite::{
    commitment::{CommitIndex, CommitPool, CommitVariant, Commitment, DigestModel, InspectAsset},
    misc::{Directive, PositionIndex},
};

// --- FRAME Support ---
use frame_support::{
    ensure,
    traits::{
        fungible::{Inspect, InspectHold, Mutate, MutateFreeze, Unbalanced, UnbalancedHold},
        tokens::{Fortitude, Precision, Preservation},
        VariantCount,
    },
};

// --- Substrate primitives ---
use sp_runtime::{
    traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, One, Saturating, Zero},
    DispatchError, DispatchResult, FixedPointNumber, PerThing,
};

// ===============================================================================
// `````````````````````````````` CONVENIENCE ALIASES ````````````````````````````
// ===============================================================================

/// The asset type used for `pallet-commitment`.
type CommitmentAsset<T, I> = <Pallet<T, I> as InspectAsset<Proprietor<T>>>::Asset;

/// The fungible compile-time freeze-reason (a runtime composite enum)
/// type used for `pallet-commitment`.
type CommitmentReason<T, I> = <Pallet<T, I> as Commitment<Proprietor<T>>>::Reason;

/// The digest hash (yet account-id) type used for `pallet-commitment`.
type CommitmentDigest<T, I> = <Pallet<T, I> as Commitment<Proprietor<T>>>::Digest;

/// The shares type used for `pallet-commitment` for indexes and pools.
type CommitmentShares<T, I> = <Pallet<T, I> as CommitIndex<Proprietor<T>>>::Shares;

/// The digest-classifier type used for `pallet-commitment` to reduce digest ambiguity.
type CommitmentDigestModel<T, I> = <Pallet<T, I> as DigestModel<Proprietor<T>>>::Model;

/// The commit-variant or position type used for `pallet-commitment`.
type CommitmentPosition<T, I> = <Pallet<T, I> as CommitVariant<Proprietor<T>>>::Position;

// ===============================================================================
// ``````````````````````````````` COMMIT BALANCE ````````````````````````````````
// ===============================================================================

/// Implements the [`CommitBalance`] trait for the pallet.
///
/// Provides low-level balance management.
impl<T: Config<I>, I: 'static> CommitBalance<Proprietor<T>, Pallet<T, I>> for CommitHelpers<T, I> {
    /// The structure representing a differential between deposited
    /// and withdrawn asset values - used to determine how the balance should
    /// be compensated.
    type Imbalance = AssetDelta<T, I>;

    /// Resolves an asset imbalance for a proprietor when a commitment is finalized
    /// (e.g. during withdrawal).
    ///
    /// An imbalance may arise when a commitment's effective value changes due to
    /// digest updates (for example via [`Commitment::set_digest_value`]). This
    /// function reconciles the difference between deposited and withdrawn amounts
    /// to preserve financial consistency.
    ///
    /// The reconciliation is performed through the pallet's [`Config::Asset`]
    /// fungible adapter by **minting**, **burning**, or **reissuing** assets as
    /// required.
    ///
    /// This function finalizes the pallet's internal accounting by settling
    /// [`AssetToIssue`] and [`AssetToReap`] with the underlying asset system.
    ///
    /// Notably, this function does **not** rely on balanced fungible traits.
    /// Instead, the original deposit is returned to the proprietor, and any
    /// additional minting or burning is performed explicitly and independently
    /// via unbalanced low-level fungible traits.
    ///
    /// This ensures balance adjustments occur in a controlled and safe manner
    /// without directly mutating the proprietor's balance.
    ///
    /// ## Behavior
    /// - `deposit < withdraw`: The shortfall is **minted** as a reward
    ///   (accounted via [`AssetToIssue`]).
    /// - `deposit == withdraw`: The values are balanced; no action is taken.
    /// - `deposit > withdraw`: The surplus is **burned** as a penalty
    ///   (accounted via [`AssetToReap`]).
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the **final asset value returned to the proprietor**
    ///   after resolving the imbalance.
    /// - `Err(DispatchError)` if minting or burning fails or capacity is insufficient.
    fn resolve_imbalance(
        who: &Proprietor<T>,
        imbalance: Self::Imbalance,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        let deposit = imbalance.deposit;
        let withdraw = imbalance.withdraw;
        match deposit.cmp(&withdraw) {
            // Case 1: Deposit < Withdraw => Mint shortfall as reward
            Ordering::Less => {
                let reward = withdraw.saturating_sub(deposit);
                // Reduce available issuance capacity.
                AssetToIssue::<T, I>::mutate(|total_issued| -> DispatchResult {
                    // AssetToIssue must be valid globally over all the updated digests over rewards
                    let remaining_issue = total_issued.checked_sub(&reward);
                    debug_assert!(
                        remaining_issue.is_some(),
                        "asset issuance is not in equilibrium with minting, 
                        inconsistency detected, current total issuance {:?} and tried minting {:?}",
                        total_issued,
                        reward
                    );
                    let remaining_issue =
                        remaining_issue.ok_or(Error::<T, I>::MintingMoreThanIssued)?;
                    *total_issued = remaining_issue;
                    Ok(())
                })?;
                // Top up the depositor first, if any base deposit exists.
                if !deposit.is_zero() {
                    T::Asset::increase_balance(who, deposit, Precision::Exact)?;
                }
                // Mint the shortfall as a reward to balance the commitment.
                T::Asset::mint_into(who, reward)?;
                let total_taken = deposit.saturating_add(reward);
                Ok(total_taken)
            }

            // Case 2: Deposit == Withdraw  ->  Perfectly balanced
            Ordering::Equal => {
                T::Asset::increase_balance(who, withdraw, Precision::Exact)?;
                Ok(withdraw)
            }

            // Case 3: Deposit > Withdraw  ->  Burn surplus as penalty
            Ordering::Greater => {
                let penalty = deposit.saturating_sub(withdraw);
                // Reduce available reaping capacity.
                AssetToReap::<T, I>::mutate(|total_to_reap| -> DispatchResult {
                    let remaining_reap = total_to_reap.checked_sub(&penalty);
                    debug_assert!(
                        remaining_reap.is_some(),
                        "asset-to-reap is not in equilibrium with burning, 
                        inconsistency detected, current total-to-reap {:?} and tried burning {:?}",
                        total_to_reap,
                        penalty
                    );
                    let remaining_reap =
                        remaining_reap.ok_or(Error::<T, I>::BurningMoreThanReapable)?;
                    *total_to_reap = remaining_reap;
                    Ok(())
                })?;
                // Always credit base deposit before applying the burn.
                T::Asset::increase_balance(who, deposit, Precision::Exact)?;
                // Burn any excess value to restore balance.
                if !penalty.is_zero() {
                    T::Asset::burn_from(
                        who,
                        penalty,
                        Preservation::Expendable,
                        Precision::Exact,
                        Fortitude::Force,
                    )?;
                }
                let total_taken = deposit.saturating_sub(penalty);
                Ok(total_taken)
            }
        }
    }

    /// Deducts a specified asset value from the proprietor's balance.
    ///
    /// Held funds (under [`HoldReason::PrepareForCommit`]) are applied first,
    /// followed by liquid funds if required. The deduction behavior is governed
    /// by the provided precision and fortitude parameters:
    ///
    /// - **Precision**: Determines whether an **exact** amount is required or a
    ///   **best-effort** deduction is acceptable.
    /// - **Fortitude**: Determines whether deduction should be **polite** or
    ///   **forceful**.
    ///
    /// When `Fortitude::Polite` is used, only held (commit-reserved) funds are
    /// deducted. When `Fortitude::Force` is used, deduction first consumes any
    /// available held funds and then continues from the liquid balance if needed.
    ///
    /// All balance operations are performed under a **preservation** context and
    /// therefore never risk account closure.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the actual amount deducted.
    /// - `Err(DispatchError)` if funds are insufficient or the deduction fails.
    fn deduct_balance(
        who: &Proprietor<T>,
        value: CommitmentAsset<T, I>,
        qualifier: &DispatchPolicy,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        let hold_reason = Into::<T::AssetHold>::into(HoldReason::PrepareForCommit);

        // Get reserved balance for our commitment's hold reason
        let reserve = T::Asset::balance_on_hold(&hold_reason, who);
        let actual;

        // Case 1 - Reserve covers the whole commitment
        if reserve >= value {
            T::Asset::decrease_balance_on_hold(&hold_reason, who, value, Precision::Exact)?;
            actual = value;
            return Ok(actual);
        }

        let force = qualifier.fortitude();
        let precision = qualifier.precision();

        // Case 2 - Polite mode: reject if exact precision required and reserves insufficient
        if force == Fortitude::Polite {
            if precision == Precision::Exact || reserve.is_zero() {
                let total_reserve = T::Asset::total_balance_on_hold(who);
                // Notify there are funds in other reserves which can be deposited
                // to our commitment reserve since the caller needs only from reserve
                // A safe way to avoid risking account-reaping
                ensure!(total_reserve < value, Error::<T, I>::ExpectsHoldWithdrawal);
                return Err(Error::<T, I>::InsufficientFunds.into());
            }
        }

        // Check liquid balance availability without risking account-closure
        let liquid = T::Asset::reducible_balance(who, Preservation::Preserve, Fortitude::Force);
        let total = reserve
            .checked_add(&liquid)
            .ok_or(Error::<T, I>::ReserveLiquidOverflow)?;

        // Case 3 - Liquid Funds and Commit-reserve insufficient
        if total < value {
            // Not enough funds, reject if exact precision required
            if precision == Precision::Exact || total.is_zero() {
                // This is total balance including reducible if its greater than
                // the value, then other holds and locks are holding such amount.
                let total_balance = T::Asset::total_balance(who);
                ensure!(
                    !(total_balance > value),
                    Error::<T, I>::ExpectsFreezeAndHoldWithdrawal
                );
                return Err(Error::<T, I>::InsufficientFunds.into());
            }

            // Deduct all liquid and reserved funds since we require only Best Effort
            T::Asset::decrease_balance(
                who,
                liquid,
                Precision::Exact,
                Preservation::Preserve,
                Fortitude::Force,
            )?;
            T::Asset::decrease_balance_on_hold(&hold_reason, who, reserve, Precision::Exact)?;
            actual = total;
            return Ok(actual);
        }

        // Case 4 - Enough funds available via liquid + commit-reserve, deduct proportionally
        let excess = total.saturating_sub(value);
        let free = liquid.saturating_sub(excess);
        T::Asset::decrease_balance(
            who,
            free,
            Precision::Exact,
            Preservation::Preserve,
            Fortitude::Force,
        )?;
        T::Asset::decrease_balance_on_hold(&hold_reason, who, reserve, Precision::Exact)?;
        actual = value;
        Ok(actual)
    }

    /// Deducts a specified value from an existing imbalance, mutating it in-place.
    ///
    /// This function extracts `value` from that differential while preserving
    /// correct economic accounting for rewards or penalties.
    ///
    /// ## Semantics
    /// - If `withdraw > deposit` (reward state): the deduction first consumes
    ///   the reward portion. Any excess deduction converts into a penalty,
    ///   increasing [`AssetToReap`].
    /// - If `withdraw == deposit` (neutral state): the deduction is treated as a
    ///   penalty and directly increases [`AssetToReap`].
    /// - If `withdraw < deposit` (penalty state): the deduction deepens the
    ///   penalty, further increasing [`AssetToReap`].
    ///
    /// In all cases, the imbalance holder remains the sole party whose economic
    /// position is adjusted, avoiding double mint/burn effects for both parties.
    ///
    /// ## Returns
    /// Returns a neutral imbalance (`deposit == withdraw == value`). This allows
    /// the caller to credit the deducted value directly to the underlying
    /// fungible system without introducing additional reward or penalty logic.
    ///
    /// - `DispatchError` otherwise
    fn deduct_from_imbalance(
        imbalance: &mut Self::Imbalance,
        value: CommitmentAsset<T, I>,
    ) -> Result<Self::Imbalance, DispatchError> {
        let given = &mut imbalance.deposit;
        let taken = &mut imbalance.withdraw;
        match given.cmp(&taken) {
            // If the imbalance's withdrawal includes a reward, determine if the reward can cover the deduction
            Ordering::Less => {
                let reward = taken.saturating_sub(*given);
                match value.cmp(&reward) {
                    // Part of the imbalance's reward is retained by the proprietor after deduction
                    Ordering::Less => {
                        let actual_reward = reward.saturating_sub(value);
                        *taken = given.saturating_add(actual_reward);
                    }
                    // The reward is fully collected by the deduction
                    Ordering::Equal => {
                        *taken = *given;
                    }
                    // Reward + extra from withdrawal incurs a more penalty to the imbalance holder
                    Ordering::Greater => {
                        let remaining = value.saturating_sub(reward);
                        // We simulate that the imbalance holder incurs a penalty, so that the deduction
                        // can be a simple increase balance to underlying fungible system instead of equating
                        // asset mints and reaps doubly for both imbalance holder and deducter, by this method, we keep
                        // accurate imbalance resolving only towards the imbalance holder
                        AssetToReap::<T, I>::mutate(|total_to_reap| -> DispatchResult {
                            *total_to_reap = total_to_reap
                                .checked_add(&remaining)
                                .ok_or(Error::<T, I>::MaxAssetReaped)?;
                            Ok(())
                        })?;
                        *taken = given.saturating_sub(remaining);
                    }
                }
            }
            // If no reward or penalty, reduce the withdrawal to deduct
            Ordering::Equal => {
                AssetToReap::<T, I>::mutate(|total_to_reap| -> DispatchResult {
                    *total_to_reap = total_to_reap
                        .checked_add(&value)
                        .ok_or(Error::<T, I>::MaxAssetReaped)?;
                    Ok(())
                })?;
                *taken = taken.saturating_sub(value);
            }
            // If penalty, paying from the withdrawal incurs additional penalty to the imbalance holder
            Ordering::Greater => {
                AssetToReap::<T, I>::mutate(|total_to_reap| -> DispatchResult {
                    *total_to_reap = total_to_reap
                        .checked_add(&value)
                        .ok_or(Error::<T, I>::MaxAssetReaped)?;
                    Ok(())
                })?;
                *taken = taken.saturating_sub(value);
            }
        }

        // Here we give nill imbalance-delta so the deduction's imbalance
        // shall be a simple increase balance in the underlying system
        Ok(AssetDelta {
            deposit: value,
            withdraw: value,
        })
    }
}

// ===============================================================================
// ``````````````````````````````` COMMIT DEPOSIT ````````````````````````````````
// ===============================================================================

/// Implements the [`CommitDeposit`] trait for the pallet, providing low-level deposit
/// functionality for digests, indexes, and pools within the commitment system.
impl<T: Config<I>, I: 'static> CommitDeposit<Proprietor<T>, Pallet<T, I>> for CommitHelpers<T, I> {
    /// Represents the "Receipt" of a digest at the time of a deposit.
    ///
    /// Reference [`CommitInstance`]'s generic alias documentation, where it
    /// holds the deposit receipt from the digest's lazy balance
    /// ([`LazyBalanceOf`]) when the deposit occurs.
    ///
    /// It ensures that later commitment resolution can account for the digest's
    /// state accurately, even if the underlying digest values changes over time.
    type Receipt = CommitInstance<T, I>;

    /// Deposits a value for a given digest model and its specified variant.
    ///
    /// This function centralizes the dispatch logic to the appropriate handler
    /// depending on the type of the digest model (Direct, Index, or Pool).
    ///
    /// ## Returns
    /// - `Ok((DerivedBalance, Asset))` containing the deposit's receipt and the actual
    /// depositted value.
    /// - `Err(DispatchError)` if the deposit fails
    fn deposit_to(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        digest_model: &CommitmentDigestModel<T, I>,
        value: CommitmentAsset<T, I>,
        variant: &CommitmentPosition<T, I>,
        qualifier: &DispatchPolicy,
    ) -> Result<(Self::Receipt, CommitmentAsset<T, I>), DispatchError> {
        match digest_model {
            DigestVariant::Direct(dir) => {
                Self::deposit_to_digest(who, reason, dir, value, variant, qualifier)
            }
            DigestVariant::Index(index) => {
                Self::deposit_to_index(who, reason, index, value, variant, qualifier)
            }
            DigestVariant::Pool(pool) => {
                Self::deposit_to_pool(who, reason, pool, value, variant, qualifier)
            }
            _ => {
                debug_assert!(
                    false,
                    "digest-model marker variants {:?} are constructed, 
                    captured during deposit for proprietor {:?} 
                    of reason {:?} are explicitly dis-allowed",
                    digest_model, who, reason
                );
                return Err(Error::<T, I>::InvalidDigestModel.into());
            }
        }
    }

    /// Deposits a given asset value into a specific direct-digest's variant's balance
    /// for a reason.
    ///
    /// This is a **low-level internal function** - it:
    /// - Directly deposits value into the specified digest and its variant.
    /// - Does not inspect or deduct balance, as argument.
    ///
    /// Digest variants represent different commitment positions (e.g., affirmative,
    /// contrary, etc). This function ensures deposits go to the correct variant's
    /// balance via [`PositionIndex`], as digest-balances will be stored as a vector
    /// of variant balances.
    ///
    /// ## Returns
    /// - `Ok((DerivedBalance, Asset))` containing the deposit's receipt and the actual
    /// depositted value.
    /// - `Err(DispatchError)` if the digest variant cannot be found or mutated
    fn deposit_to_digest(
        _who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        digest: &CommitmentDigest<T, I>,
        value: CommitmentAsset<T, I>,
        variant: &CommitmentPosition<T, I>,
        qualifier: &DispatchPolicy,
    ) -> Result<(Self::Receipt, CommitmentAsset<T, I>), DispatchError> {
        // Mutate the digest map to update the balance for the given reason -> digest -> variant
        let actual =
            DigestMap::<T, I>::mutate((reason, digest), |result| -> Result<_, DispatchError> {
                // Retrieve digest information; error if digest does not exist
                let digest_info = result.as_mut().ok_or(Error::<T, I>::DigestNotFound)?;

                // Attempt to retrieve the digest variant balance
                let Some(digest_of) = digest_info.mut_balance(variant) else {
                    digest_info.init_balance(variant)?;
                    // Deposit value into the newly created variant balance
                    let digest_of_new_variant = digest_info.mut_balance(variant);
                    debug_assert!(
                        digest_of_new_variant.is_some(),
                        "recently initiated digest {:?} of reason {:?} variant {:?} 
                    balance not accessible via vector",
                        digest,
                        reason,
                        variant,
                    );
                    let digest_of_new_variant =
                        digest_of_new_variant.ok_or(Error::<T, I>::DigestVariantBalanceNotFound)?;
                    let (depositted, receipt) =
                        deposit(digest_of_new_variant, variant, digest, &value, qualifier)?;
                    // Early exit from mutate closure after depositing to newly created variant
                    return Ok((receipt, depositted));
                };

                // If the variant exists, deposit directly into its balance
                let (depositted, receipt) = deposit(digest_of, variant, digest, &value, qualifier)?;
                Ok((receipt, depositted))
            })?;

        // Return the deposit (receipt, amount)
        Ok(actual)
    }

    /// Deposits a given asset value into an index and distributes it across
    /// its entries' digests.
    ///
    /// This is a **low-level internal function** - it:
    /// - Splits the value proportionally across each entry based on its share/capital ratio.
    /// - Delegates deposits into each entry's digest via [`Self::deposit_to_digest`].
    /// - Updates the index's own top-level balance.
    ///
    /// Expected invariants:
    /// - Total capital must be non-zero (no stale or invalid indexes).
    /// - Total index capital must be **at least** the share value of each entry.
    /// - Entry shares must be non-zero (no stale or invalid entries).
    ///
    /// The caller's `variant` is ignored because each entry already defines its own variant.
    ///
    /// If the index deposit has any remaining assets, they are refunded to the
    /// proprietor here. Other commitment types (such as direct digests or pools)
    /// do not require refunding, so the refund logic is handled exclusively at
    /// this level.
    ///
    /// Since Index maintains entry-commit receipts in their own high-level structures
    /// the receipt returned is a placeholder receipt (default), although the depositted
    /// amount is valid.
    ///
    /// ## Returns
    /// - `Ok((DerivedBalance, Asset))` containing a placeholder receipt and the actual
    /// depositted value.
    /// - `Err(DispatchError)` if the deposit cannot be conducted
    fn deposit_to_index(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        index_of: &CommitmentDigest<T, I>,
        value: CommitmentAsset<T, I>,
        _variant: &CommitmentPosition<T, I>,
        qualifier: &DispatchPolicy,
    ) -> Result<(Self::Receipt, CommitmentAsset<T, I>), DispatchError> {
        // Track the total successfully deposited value across all entries in the index
        let mut actual = CommitmentAsset::<T, I>::zero();

        // Mutate the index map for the given reason and index digest
        IndexMap::<T, I>::mutate((reason, index_of), |result| -> DispatchResult {
            // Retrieve the index information; error if index does not exist
            let index_info = result.as_mut().ok_or(Error::<T, I>::IndexNotFound)?;
            // Capital is used to compute proportional commitment values for each entry
            let index_capital = index_info.capital();
            debug_assert!(
                !index_capital.is_zero(),
                "index {:?} of reason {:?} capital is constructed at zero",
                index_of,
                reason
            );
            ensure!(!index_capital.is_zero(), Error::<T, I>::CapitalCannotBeZero);
            // Iterate through each entry within the index
            let entries = &index_info.entries();
            for i in 0..entries.len() {
                let entry = &entries[i];
                // Entry's share value used to compute proportional commitment value to it individually.
                let entry_share = entry.shares();
                debug_assert!(
                    !entry_share.is_zero(),
                    "index {:?} of reason {:?} entry {:?} 
                    share is found to be zero",
                    index_of,
                    reason,
                    entry.digest()
                );
                ensure!(!entry_share.is_zero(), Error::<T, I>::ShareCannotBeZero);
                debug_assert!(
                    index_capital >= entry_share,
                    "index {:?} of reason {:?}  capital of value {:?} is constructed 
                    as lesser than the entry {:?} share {:?}",
                    index_of,
                    reason,
                    index_capital,
                    entry.digest(),
                    entry_share
                );
                ensure!(
                    index_capital >= entry_share,
                    Error::<T, I>::ShareGreaterThanCapital
                );
                // Compute this entry's proportional factor (share / capital), use bias type as precision
                let factor = T::Bias::checked_from_rational(entry_share, index_capital)
                    .ok_or(Error::<T, I>::TooSmallShareValue)?;
                // Convert committing value to its fixed-point equivalent representation
                let value_fixed = T::Bias::saturating_from_integer(value);
                // Compute scaled deposit value for this entry
                let deposit_val_fixed = value_fixed
                    .checked_mul(&factor)
                    .ok_or(Error::<T, I>::DepositDeriveOverflowed)?;
                // Skip this entry if the computed allocation rounds below 1 unit
                // So its expected, that some entries of much lower shares may not get
                // commitments if the deposited value is very lower
                if deposit_val_fixed < One::one() {
                    continue;
                }
                // Convert from fixed-point back to asset units
                let deposit_val_scaled = deposit_val_fixed
                    .into_inner()
                    // ensured no NAN possibility
                    .checked_div(&T::Bias::DIV)
                    .ok_or(Error::<T, I>::DerivedLessThanZeroValue)?;
                let deposit_val: CommitmentAsset<T, I> = deposit_val_scaled.into();

                // Each entry has its own digest and variant; the caller's `_variant` is ignored
                let entry_digest = &entry.digest();
                let entry_variant = &entry.variant();
                // Perform the actual deposit to the entry's digest
                let (receipt, amount) = Self::deposit_to_digest(
                    who,
                    reason,
                    entry_digest,
                    deposit_val,
                    entry_variant,
                    qualifier,
                )?;
                // This stores the commitments to indexes in a higher structure for later retrieval,
                // for raising commits and resolving it, since base commitments only allow a single
                // digest per reason, whereas this map stores multiple digest, each for each entry of
                // index, essentially a parallel storage map akin to `CommitMap`
                // Accumulates a new commit-instance if existing commit exists, else inserts
                // A single entry point for placing or raising commits belong to an entry of index
                match EntryMap::<T, I>::contains_key((reason, index_of, &entry_digest, who)) {
                    true => {
                        // Mutate if its existing and add a new commit-instance i.e., raise-commit
                        EntryMap::<T, I>::mutate(
                            (reason, index_of, &entry_digest, who),
                            |result| -> DispatchResult {
                                debug_assert!(
                                    result.is_some(),
                                    "proprietor {:?} commit under index {:?} of entry {:?} exists, 
                                    but cannot mutate its balance",
                                    who,
                                    index_of,
                                    entry_digest
                                );
                                // Already checked if key contains, hence the try operation is dead code
                                let value =
                                    result.as_mut().ok_or(Error::<T, I>::EntryCommitNotFound)?;

                                // New commit instance i.e., receipt to add to existing commits
                                value.add_commit(receipt)?;
                                Ok(())
                            },
                        )?;
                    }
                    false => {
                        // Insert the first commit i.e., place commit
                        EntryMap::<T, I>::insert(
                            (reason, index_of, &entry_digest, who),
                            Commits::<T, I>::new(receipt)?,
                        );
                    }
                }
                // Accumulate the deposited value across all entries
                let try_accum = actual.checked_add(&amount);
                debug_assert!(
                    try_accum.is_some(),
                    "found an invariant broken due to overflow during deposit to index
                    by proprietor {:?} for index {:?} for entry {:?}, when 
                    entry's share > total capital, only overflows when Share {:?} /Capital {:?} 
                    ratio produced a factor greater than 1.",
                    who,
                    index_of,
                    entry_digest,
                    entry_share,
                    index_capital
                );
                // Overflow here signals a proportional factor > 1, not a simple arithmetic bug
                let try_accum = try_accum.ok_or(Error::<T, I>::FactorGreaterThanOne)?;
                actual = try_accum;
            }

            // Refund any remaining difference (dust) directly to the caller
            // Cannot use `resolve_imbalance` here because it interacts with issuance/reaping
            let refund = value.saturating_sub(actual);
            if !refund.is_zero() {
                T::Asset::increase_balance(who, refund, Precision::Exact)?;
            }

            // Update the index's top-level balance for quick queries (total deposits only).
            // A receipt is not required here, as balance sets are applied at the digest
            // level only. Indexes and pools do not hold commitments directly; they act as
            // convenience aggregation layers over base (direct) commitments.
            let mut index_balance_of = index_info.principal();
            index_balance_of = index_balance_of
                .checked_add(&actual)
                .ok_or(Error::<T, I>::MaxIndexCapacityReached)?;
            index_info.set_balance(index_balance_of);

            Ok(())
        })?;

        // Safe to return default receipt; individual entry receipts are tracked internally in
        // `EntryMap`. The One reason One digest Commit invariant is enforced globally via this
        // indirection. Although a base commitment always references a single qualified digest,
        // that digest may represent an index. In such cases, the actual commit data
        // is resolved via `EntryMap`, with the base commitment acting only as an
        // indirection (pointer) to the underlying structure.
        Ok((Self::Receipt::default(), actual))
    }

    /// Deposits a given asset value into a pool for a reason.
    ///
    /// This is a **low-level internal function** - it:
    /// - Directly deposits value into the specified pool.
    /// - Does not inspect or deduct balance, as argument `_who` is unused.
    ///
    /// The caller's `variant` is ignored because each pool's slot already
    /// defines its own variant.
    ///
    /// At this level the logic is straightforward: the pool itself acts as a
    /// **proprietor**. It receives funds and commits them to individual slot
    /// digests, while external proprietors hold shares of the pool.
    ///
    /// As a result, every deposit into or withdrawal from the pool requires the
    /// pool to temporarily release and then re-acquire the corresponding funds.
    /// This mirrors how an index commitment is resolved and re-placed by a
    /// proprietor.
    ///
    /// ### Returns
    /// - `Ok((DerivedBalance, Asset))` containing the pool's deposit receipt
    /// and the actual depositted value of the proprietor.
    /// - `Err(DispatchError)` if the deposit cannot be conducted
    fn deposit_to_pool(
        _who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        pool_of: &CommitmentDigest<T, I>,
        value: CommitmentAsset<T, I>,
        _variant: &CommitmentPosition<T, I>,
        qualifier: &DispatchPolicy,
    ) -> Result<(Self::Receipt, CommitmentAsset<T, I>), DispatchError> {
        // Release the pool's current balance to work with a mutable copy
        let mut released_balance = Self::release_pool(reason, pool_of)?;

        // Deposit the incoming value into the released pool balance
        let (depositted, receipt) = deposit(
            &mut released_balance,
            &Default::default(),
            pool_of,
            &value,
            qualifier,
        )?;

        // Recover the pool by writing the updated balance back into storage
        Self::recover_pool(reason, pool_of, &released_balance)?;

        // Unlike indexes, a pool behaves as a **pseudo-digest**: a proprietor deposits
        // funds into the pool itself, effectively committing to the pool as a whole.
        // The pool manager can access these funds (which direct commitment digests
        // cannot) and, acting as a proprietor, commits the funds onward to individual
        // slot digests according to their configured shares.
        //
        // Because the pool serves as the commitment target for the depositing
        // proprietor, the correct balance receipt must reflect the pool's
        // state at the moment the funds were handed over for management.
        Ok((receipt, depositted))
    }
}

// ===============================================================================
// ``````````````````````````````` COMMIT WITHDRAW ```````````````````````````````
// ===============================================================================

/// Implements the [`CommitWithdraw`] trait for the pallet
///
/// Provides low-level withdraw functionality for digests, indexes,
/// and pools within the commitment system.
impl<T: Config<I>, I: 'static> CommitWithdraw<Proprietor<T>, Pallet<T, I>> for CommitHelpers<T, I> {
    /// Withdraws the proprietor's total committed value for a given digest
    /// model and reason.
    ///
    /// This function centralizes the dispatches to the appropriate underlying
    /// handler depending on the type of the digest model (Direct, Index, or Pool).
    ///
    /// Digest models represent different
    /// commitment structures:
    /// - **Direct**: a single digest commitment.
    /// - **Index**: a grouped commitment of multiple entry digests.
    /// - **Pool**: a shared resource i.e., a collective single commitment.
    ///
    /// ## Imbalance Semantics
    /// The returned imbalance captures the difference between the original deposit
    /// made when the commitment was placed and the value withdrawn at resolution
    /// time. Since digests may accrue rewards or penalties over time, the withdrawn
    /// amount may differ from the original deposit; this difference is expected and
    /// is represented explicitly in the imbalance.
    ///
    /// ## Notes
    /// - Callers are expected to resolve this imbalance probably
    /// via [`CommitBalance::resolve_imbalance`]
    /// - The caller's `variant` is ignored because each commit already defines
    /// its own variant during deposit operation.
    ///
    /// ## Returns
    /// - `Ok(Imbalance)` containing:
    ///   - `deposit`: the original amount deposited when the commitment was placed.
    ///   - `withdraw`: the amount withdrawn at resolution time.
    /// - `Err(DispatchError)` if any underlying withdrawal operation fails.
    fn withdraw_for(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        digest_model: &CommitmentDigestModel<T, I>,
        variant: &CommitmentPosition<T, I>,
    ) -> Result<Self::Imbalance, DispatchError> {
        match digest_model {
            DigestVariant::Direct(dir) => Self::withdraw_from_digest(who, reason, dir, variant),
            DigestVariant::Index(index) => Self::withdraw_from_index(who, reason, index, variant),
            DigestVariant::Pool(pool) => Self::withdraw_from_pool(who, reason, pool, variant),
            _ => {
                debug_assert!(
                    false,
                    "digest-model marker variants {:?} are constructed, 
                    captured during withdraw for proprietor {:?} 
                    of reason {:?} are explicitly dis-allowed",
                    digest_model, who, reason
                );
                return Err(Error::<T, I>::InvalidDigestModel.into());
            }
        }
    }

    /// Withdraws the committed value for a single direct digest and variant.
    ///
    /// This low-level function calculates the real-time effective value for all
    /// commit instances associated with this proprietor and digest, then updates
    /// the digest's balance accordingly.
    ///
    /// ## Imbalance Semantics
    /// The returned imbalance captures the difference between the original deposit
    /// made when the commitment was placed and the value withdrawn at resolution
    /// time. Since digests may accrue rewards or penalties over time, the withdrawn
    /// amount may differ from the original deposit; this difference is expected and
    /// is represented explicitly in the imbalance.
    ///
    /// ## Notes
    /// - Callers are expected to resolve this imbalance probably
    /// via [`CommitBalance::resolve_imbalance`]
    /// - The caller's `variant` is ignored because each commit already defines
    /// its own variant during deposit operation.
    ///
    /// ## Returns
    /// - `Ok(Imbalance)` containing both the deposited and the withdrawn value
    /// - `Err(DispatchError)` if the digest or variant is not found
    fn withdraw_from_digest(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        digest: &CommitmentDigest<T, I>,
        _variant: &CommitmentPosition<T, I>,
    ) -> Result<Self::Imbalance, DispatchError> {
        // Initialize zero values. `taken` tracks total withdrawn, `given` tracks total deposited.
        let mut given = CommitmentAsset::<T, I>::zero();
        let mut taken = CommitmentAsset::<T, I>::zero();

        // We take the variant of the commit - the actual variant
        //
        let variant = Pallet::<T, I>::get_commit_variant(who, reason)?;

        // Mutate the digest map to update the new balance (after withdrawal value)
        // for the given reason and digest
        DigestMap::<T, I>::mutate((reason, digest), |result| -> DispatchResult {
            // Retrieve digest information; error if digest does not exist
            let digest_info = result.as_mut().ok_or(Error::<T, I>::DigestNotFound)?;

            // Access the specific variant balance
            let balance = digest_info.mut_balance(&variant);

            debug_assert!(
                balance.is_some(),
                "digest {:?} of reason {:?} variant {:?} balance 
                was not initiated properly in the balance vector 
                during the deposit for proprietor {:?}",
                digest,
                reason,
                variant,
                who
            );

            let balance = balance.ok_or(Error::<T, I>::DigestVariantBalanceNotFound)?;

            CommitMap::<T, I>::mutate((who, reason), |result| -> DispatchResult {
                // Retrieve the proprietor's commit information for this reason
                let commit_info = result.as_mut().ok_or(Error::<T, I>::CommitNotFound)?;
                // Iterate over each commit instance associated with this proprietor
                let commits = &commit_info.commits();
                for commit in commits {
                    // Deposit accumulation
                    given = given
                        .checked_add(&receipt_deposit_value(commit)?)
                        .ok_or(Error::<T, I>::DepositAccumulationExhausted)?;
                    // Withdraw Accumulation
                    taken = taken
                        .checked_add(&withdraw(balance, &variant, digest, commit)?)
                        .ok_or(Error::<T, I>::WithdrawAccumulationExhausted)?;
                }

                Ok(())
            })?;

            Ok(())
        })?;

        // Return the withdrawn and deposited amounts as the imbalance for this proprietor
        Ok(AssetDelta {
            deposit: given,
            withdraw: taken,
        })
    }

    /// Withdraws the committed value for a given index digest.
    ///
    /// Iterates over all entries in the index, calculates each entry's effective
    /// withdrawal, and updates entry digest balances while
    /// removing commitment records from storage.
    ///
    /// ## Imbalance Semantics
    /// The returned imbalance captures the difference between the original deposit
    /// made when the commitment was placed and the value withdrawn at resolution
    /// time. Since digests may accrue rewards or penalties over time, the withdrawn
    /// amount may differ from the original deposit; this difference is expected and
    /// is represented explicitly in the imbalance.
    ///
    /// ## Notes
    /// - Callers are expected to resolve this imbalance probably
    /// via [`CommitBalance::resolve_imbalance`]
    /// - The caller's `variant` is ignored because each commit belongs to an entry already
    /// defines its own variant during deposit operation.
    /// - Since an index is an immutable structure-where entry updates result in a
    /// new index, no inconsistencies can arise.
    ///
    /// ## Returns
    /// - `Ok(Imbalance)` representing total withdrawn and deposited values across all entries
    /// - `Err(DispatchError)` if the index or any entry is not found
    fn withdraw_from_index(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        index_of: &CommitmentDigest<T, I>,
        _variant: &CommitmentPosition<T, I>,
    ) -> Result<Self::Imbalance, DispatchError> {
        // Initialize zero values. `taken` tracks total withdrawn, `given` tracks total deposited.
        // Since indexes composed of multiple entry digests we have to iterate over each digest in
        // the assumption of a single commit per entry digest (high-level structure) and track balances
        let mut taken = CommitmentAsset::<T, I>::zero();
        let mut given = CommitmentAsset::<T, I>::zero();

        // Mutate the index map to for the given index digest and reason, to
        // retrieve entry infos and update its balance for high-level queries
        IndexMap::<T, I>::mutate((reason, index_of), |result| -> DispatchResult {
            let index_info = result.as_mut().ok_or(Error::<T, I>::IndexNotFound)?;

            // Iterate over each entry within the index
            let entries = &index_info.entries();
            for entry in entries {
                let entry_digest = entry.digest();
                let entry_variant = entry.variant();

                // Retrieve all commit instances for this proprietor and entry
                let commits_of = EntryMap::<T, I>::get((reason, index_of, &entry_digest, who))
                    .ok_or(Error::<T, I>::CommitNotFoundForEntry)?;

                for commit in &commits_of.commits() {
                    // Commit value represents the amount deposited previously.
                    // Each commit instance is stored as a receipt, so this value reflects
                    // the original deposit.
                    let commit_value = receipt_deposit_value(commit)?;

                    // Accumulate total deposited amount for this entry
                    given = given
                        .checked_add(&commit_value)
                        .ok_or(Error::<T, I>::DepositAccumulationExhausted)?;

                    DigestMap::<T, I>::mutate(
                        (reason, &entry_digest),
                        |result| -> DispatchResult {
                            let digest_info =
                                result.as_mut().ok_or(Error::<T, I>::EntryDigestNotFound)?;
                            let digest_of = digest_info.mut_balance(&entry_variant);
                            debug_assert!(
                                digest_of.is_some(),
                                "entry-digest {:?} of index {:?} of reason {:?} variant {:?} balance 
                                was not initiated properly in the balance vector 
                                during the deposit for proprietor {:?}",
                                entry_digest,
                                index_of,
                                reason,
                                entry_variant,
                                who
                            );
                            let digest_of =
                                digest_of.ok_or(Error::<T, I>::DigestVariantBalanceNotFound)?;

                            // Perform withdrawal from the entry's digest
                            let take = withdraw(digest_of, &entry_variant, &entry_digest, commit)?;

                            // Accumulate total withdrawn value across all entries
                            taken = taken
                                .checked_add(&take)
                                .ok_or(Error::<T, I>::WithdrawAccumulationExhausted)?;

                            Ok(())
                        },
                    )?;
                }

                // Remove all commit instances for this proprietor for the current entry
                EntryMap::<T, I>::remove((reason, index_of, entry_digest, who));
            }

            // Subtract total deposits from the index's top-level simple balance for noting deposits
            let index_balance = &index_info.principal();
            let new_balance = index_balance.checked_sub(&given);
            debug_assert!(
                new_balance.is_some(),
                "previous deposit value {:?} of proprietor {:?} for index {:?} 
                of reason {:?} is subtracted from index principal balance {:?} 
                during withdrawal underflowed, should not fail",
                given,
                who,
                index_of,
                reason,
                index_balance
            );
            let new_balance = new_balance.ok_or(Error::<T, I>::IndexBalanceUnderflow)?;
            index_info.set_balance(new_balance);
            Ok(())
        })?;

        // Return the accumulated deposits and withdrawals
        // as the proprietor's imbalance
        Ok(AssetDelta {
            deposit: given,
            withdraw: taken,
        })
    }

    /// Withdraws the committed value for a pool digest.
    ///
    /// The pool balance is released first, then each commitment within the pool is resolved.
    /// Withdrawn values are accumulated, pool state is recovered,
    /// and commissions are handled (provided to manager internally itself) according
    /// to the pool's configuration.
    ///
    /// This low-level operation ensures the proprietor receives their imbalance while also
    /// minting, burning, or reallocating tokens due to commission configuration for the
    /// manager only.
    ///
    /// ## Returns
    /// - `Ok(Imbalance)` containing total deposited and withdrawn amounts after commissions
    /// - `Err(DispatchError)` if the pool is not found or withdrawal fails
    fn withdraw_from_pool(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        pool_of: &CommitmentDigest<T, I>,
        _variant: &CommitmentPosition<T, I>,
    ) -> Result<Self::Imbalance, DispatchError> {
        // Initialize zero values. For each slot, these will accumulate total deposits and withdrawals.
        let mut taken = CommitmentAsset::<T, I>::zero();
        let mut given = CommitmentAsset::<T, I>::zero();

        // Pools do not have the same indirection as indexes. We maintain the proprietor's balance
        // as a local commitment to the pool (similar to a direct digest commitment).
        let commits_of =
            CommitMap::<T, I>::get((who, reason)).ok_or(Error::<T, I>::CommitNotFound)?;
        let commits = commits_of.commits();

        // Release the pool to determine the withdrawal value. Since pools are collective funds,
        // we update them every time by releasing and then recovering.
        // Retrieve the mutable pool balance structure, analogous to a digest's
        // balance (just as digests track proprietor commitments, pools track
        // funders' deposits). This balance is temporarily exposed for mutation
        // during the current operation and must be recovered back to the pool
        // immediately afterward via `recover_pool()`.
        let mut pool_balance = Self::release_pool(reason, pool_of)?;

        // Iterate through the commit instances depositted to the pool
        for commit in &commits {
            // The commit value was depositted earlier; it represents the deposited amount,
            // not the real-time value
            let commit_value = receipt_deposit_value(commit)?;

            // Accumulate total deposits to each slot of the digest
            given = given
                .checked_add(&commit_value)
                .ok_or(Error::<T, I>::DepositAccumulationExhausted)?;

            // Accumulate the withdrawal values
            let take = withdraw(&mut pool_balance, &Default::default(), pool_of, commit)?;
            taken = taken
                .checked_add(&take)
                .ok_or(Error::<T, I>::WithdrawAccumulationExhausted)?;
        }

        // Recover the pool's state to the new state after withdrawal
        Self::recover_pool(reason, pool_of, &pool_balance)?;

        let pool = PoolMap::<T, I>::get((reason, pool_of));
        debug_assert!(
            pool.is_some(),
            "recently recovered pool {:?} of reason {:?} after 
            withdrawal for proprietor {:?} not accessible",
            pool_of,
            reason,
            who
        );

        let pool = pool.ok_or(Error::<T, I>::PoolNotFound)?;
        let commission = pool.commission();

        let mut imbalance = AssetDelta {
            deposit: given,
            withdraw: taken,
        };

        // Pools may have a commission, which is handled internally and only returned to the proprietor
        if commission.is_zero() {
            return Ok(imbalance);
        }

        // Get the pool's manager
        let pay_to = Pallet::<T, I>::get_manager(reason, pool_of);
        debug_assert!(
            pay_to.is_ok(),
            "pool {:?} of reason {:?} exists but manager is not",
            pool_of,
            reason
        );
        let pay_to = pay_to?;

        // Calculate the amount to pay the manager
        let parts = CommitmentAsset::<T, I>::from(commission.deconstruct());
        let accuracy = CommitmentAsset::<T, I>::from(T::Commission::ACCURACY);

        // Calculate the amount to pay the manager
        let to_pay = taken
            .checked_mul(&parts)
            .ok_or(Error::<T, I>::CommissionOverflow)?
            .checked_div(&accuracy)
            .ok_or(Error::<T, I>::DerivedLessThanZeroValue)?;

        // Early return if the payment is zero
        if to_pay.is_zero() {
            return Ok(imbalance);
        }

        // Derive commission payout imbalance for pool manager internally done with equillibrium
        // maintained via safe deduction over an existing imbalance
        let manager_imbalance = Self::deduct_from_imbalance(&mut imbalance, to_pay)?;

        // Expected to be a simple increase balance since the earlier call maintained equillibrium
        // and the `manager_imbalance` is expected to be nill imbalance-delta
        let manager_withdraw = Self::resolve_imbalance(&pay_to, manager_imbalance)?;

        // Since commission payout is to the underlying system, not reinvested to
        // commitment system itself as the caller only expects proprietor's imbalance
        // Hence to maintain per reason total value maintained for quick queries
        Self::sub_from_total_value(reason, manager_withdraw)?;

        // Mutated imbalance when deducted for commission payout
        Ok(imbalance)
    }
}

// ===============================================================================
// `````````````````````````````` COMMIT OPERATIONS ``````````````````````````````
// ===============================================================================

/// Implements the [`CommitOps`] trait for the pallet
///
/// Provides low-level (not-lowest, but still unchecked) write functionalities for digests, indexes,
/// and pools within the commitment system. This may utilize other commit-helper trait methods
impl<T: Config<I>, I: 'static> CommitOps<Proprietor<T>, Pallet<T, I>> for CommitHelpers<T, I> {
    /// Places (creates) a commitment for a given digest model with the specified variant.
    ///
    /// This function centralizes the dispatches to the appropriate underlying handler
    /// depending on the type of the digest model (Direct, Index, or Pool).
    ///
    /// Digest models represent different
    /// commitment structures:
    /// - **Direct**: a single digest commitment.
    /// - **Index**: a grouped commitment of multiple entry digests.
    /// - **Pool**: a shared resource i.e., a collective single commitment.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the committed amount
    /// - `Err(DispatchError)` if placement fails
    fn place_commit_of(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        digest_model: &CommitmentDigestModel<T, I>,
        value: CommitmentAsset<T, I>,
        variant: &CommitmentPosition<T, I>,
        qualifier: &DispatchPolicy,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        match digest_model {
            DigestVariant::Direct(dir) => {
                Self::place_digest_commit(who, reason, dir, value, variant, qualifier)
            }
            DigestVariant::Index(index) => {
                Self::place_index_commit(who, reason, index, value, variant, qualifier)
            }
            DigestVariant::Pool(pool) => {
                Self::place_pool_commit(who, reason, pool, value, variant, qualifier)
            }
            _ => {
                debug_assert!(
                    false,
                    "digest-model marker variants {:?} are constructed, 
                    captured during place commit for proprietor {:?} 
                    of reason {:?} are explicitly dis-allowed",
                    digest_model, who, reason
                );
                return Err(Error::<T, I>::InvalidDigestModel.into());
            }
        }
    }

    /// Places (creates) a direct commitment for a specific digest of a given variant under a reason.
    ///
    /// This function is the base implementation for placing commitments when the digest
    /// represents a **single, direct item** (not an index or pool).
    ///
    /// Allows placing a commitment even if the digest does not already exist in the system.
    ///
    /// ### Returns
    /// - `Ok(Asset)` containing the actual committed amount
    /// - `Err(DispatchError)` if placement fails
    fn place_digest_commit(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        digest: &CommitmentDigest<T, I>,
        value: CommitmentAsset<T, I>,
        variant: &CommitmentPosition<T, I>,
        qualifier: &DispatchPolicy,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        // If the digest does not exist yet, initialize it with default DigestInfo
        // Callers should ensure that the digest must have a deterministic way to get
        // into the system, else the commitment will be dormant
        if let Err(_) = Pallet::<T, I>::digest_exists(reason, digest) {
            DigestMap::<T, I>::insert((reason, digest), DigestInfo::default());
        }
        let try_actual = Self::deduct_balance(who, value, qualifier)?;
        let (receipt, amount) =
            Self::deposit_to_digest(who, reason, digest, try_actual, &variant, qualifier)?;
        finalize_place_commit::<T, I>(who, reason, digest, variant, &receipt, amount)?;
        Ok(amount)
    }

    /// Places a commitment to an index digest under the specified reason.
    ///
    /// Distributes the committed value proportionally across index entries based on their
    /// shares, creates entry-level commitment records, and updates the index balance.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the actual committed amount
    /// - `Err(DispatchError)` if placement fails
    fn place_index_commit(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        index_of: &CommitmentDigest<T, I>,
        value: CommitmentAsset<T, I>,
        variant: &CommitmentPosition<T, I>,
        qualifier: &DispatchPolicy,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        let try_actual = Self::deduct_balance(who, value, qualifier)?;
        let (receipt, amount) =
            Self::deposit_to_index(who, reason, index_of, try_actual, variant, qualifier)?;
        finalize_place_commit::<T, I>(who, reason, index_of, variant, &receipt, amount)?;
        Ok(amount)
    }

    /// Places a commitment to a pool digest under the specified reason.
    ///
    /// Deposits to the pool's collective balance, updates slot digests proportionally,
    /// and maintains pool-level commitment records.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the actual committed amount
    /// - `Err(DispatchError)` if placement fails
    fn place_pool_commit(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        pool_of: &CommitmentDigest<T, I>,
        value: CommitmentAsset<T, I>,
        variant: &CommitmentPosition<T, I>,
        qualifier: &DispatchPolicy,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        let try_actual = Self::deduct_balance(who, value, qualifier)?;
        let (receipt, amount) =
            Self::deposit_to_pool(who, reason, pool_of, try_actual, variant, qualifier)?;
        finalize_place_commit::<T, I>(who, reason, pool_of, variant, &receipt, amount)?;
        Ok(amount)
    }

    /// Raises (increases) an existing commitment for a given digest model.
    ///
    /// This function centralizes the dispatches to the appropriate underlying handler
    /// depending on the type of the digest model (Direct, Index, or Pool).
    ///
    /// Digest models represent different
    /// commitment structures:
    /// - **Direct**: a single digest commitment.
    /// - **Index**: a grouped commitment of multiple entry digests.
    /// - **Pool**: a shared resource i.e., a collective single commitment.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the raised amount
    /// - `Err(DispatchError)` if the raise operation fails
    fn raise_commit_of(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        digest_model: &CommitmentDigestModel<T, I>,
        value: CommitmentAsset<T, I>,
        qualifier: &DispatchPolicy,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        match digest_model {
            DigestVariant::Direct(dir) => {
                Self::raise_digest_commit(who, reason, dir, value, qualifier)
            }
            DigestVariant::Index(index) => {
                Self::raise_index_commit(who, reason, index, value, qualifier)
            }
            DigestVariant::Pool(pool) => {
                Self::raise_pool_commit(who, reason, pool, value, qualifier)
            }
            _ => {
                debug_assert!(
                    false,
                    "digest-model marker variants {:?} are constructed, 
                    captured during raise commit for proprietor {:?} 
                    of reason {:?} are explicitly dis-allowed",
                    digest_model, who, reason
                );
                return Err(Error::<T, I>::InvalidDigestModel.into());
            }
        }
    }

    /// Raises (increases) a commitment for a direct digest under a given reason.
    ///
    /// This function is the base implementation for raising commitments when the target
    /// is an **direct digest** rather than a index or pool.
    ///
    /// Unlike [`Self::place_digest_commit`] this adds a additional commit instance to the list
    /// of commits to the same direct digest enforcing the **One Reason One Digest Per
    /// Proprietor** invariant.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the raised amount
    /// - `Err(DispatchError)` if the raise operation fails
    fn raise_digest_commit(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        digest: &CommitmentDigest<T, I>,
        value: CommitmentAsset<T, I>,
        qualifier: &DispatchPolicy,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        let commit_variant = Pallet::<T, I>::get_commit_variant(who, reason)?;
        let try_actual = Self::deduct_balance(who, value, qualifier)?;
        let (receipt, raised) =
            Self::deposit_to_digest(who, reason, digest, try_actual, &commit_variant, qualifier)?;
        finalize_raise_commit::<T, I>(who, reason, &receipt, raised)?;
        Ok(raised)
    }

    /// Raises (increases) an existing commitment for a specific index under a given reason.
    ///
    /// This function is the base implementation for raising commitments when the target
    /// is an **index** rather than a digest or pool.
    ///
    /// Unlike [`Self::place_index_commit`] this adds a additional commit instance to the list
    /// of commits to the same index digest enforcing the **One Reason One Digest Per
    /// Proprietor** invariant.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the raised amount
    /// - `Err(DispatchError)` if the raise operation fails
    fn raise_index_commit(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        index_of: &CommitmentDigest<T, I>,
        value: CommitmentAsset<T, I>,
        qualifier: &DispatchPolicy,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        let try_actual = Self::deduct_balance(who, value, qualifier)?;
        let (receipt, raised) = Self::deposit_to_index(
            who,
            reason,
            index_of,
            try_actual,
            &T::Position::default(),
            qualifier,
        )?;
        finalize_raise_commit::<T, I>(who, reason, &receipt, raised)?;
        Ok(raised)
    }

    /// Raises (increases) an existing commitment for a specific pool digest under a given reason.
    ///
    /// This function is the base implementation for raising commitments when the target
    /// is an **pool** rather than a digest or index.
    ///
    /// Unlike [`Self::place_pool_commit`] this adds a additional commit instance to the list
    /// of commits to the same pool digest enforcing the **One Reason One Digest Per
    /// Proprietor** invariant.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the raised amount
    /// - `Err(DispatchError)` if the raise operation fails
    fn raise_pool_commit(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        pool_of: &CommitmentDigest<T, I>,
        value: CommitmentAsset<T, I>,
        qualifier: &DispatchPolicy,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        let try_actual = Self::deduct_balance(who, value, qualifier)?;
        let (receipt, raised) = Self::deposit_to_pool(
            who,
            reason,
            pool_of,
            try_actual,
            &T::Position::default(),
            qualifier,
        )?;
        finalize_raise_commit::<T, I>(who, reason, &receipt, raised)?;
        Ok(raised)
    }

    /// Resolves (withdraws) and finalizes a commitment for a given digest model.
    ///
    /// This function centralizes the dispatches to the appropriate underlying handler
    /// depending on the type of the digest model (Direct, Index, or Pool).
    ///
    /// Digest models represent different
    /// commitment structures:
    /// - **Direct**: a single digest commitment.
    /// - **Index**: a grouped commitment of multiple entry digests.
    /// - **Pool**: a shared resource i.e., a collective single commitment.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the resolved commitment value
    /// - `Err(DispatchError)` if resolution fails
    fn resolve_commit_of(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        digest_model: &CommitmentDigestModel<T, I>,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        match digest_model {
            DigestVariant::Direct(dir) => Self::resolve_digest_commit(who, reason, dir),
            DigestVariant::Index(index) => Self::resolve_index_commit(who, reason, index),
            DigestVariant::Pool(pool) => Self::resolve_pool_commit(who, reason, pool),
            _ => {
                debug_assert!(
                    false,
                    "digest-model marker variants {:?} are constructed, 
                    captured during resolve commit for proprietor {:?} 
                    of reason {:?} are explicitly dis-allowed",
                    digest_model, who, reason
                );
                return Err(Error::<T, I>::InvalidDigestModel.into());
            }
        }
    }

    /// Resolves (withdraws) and finalizes a commitment for a direct digest under a given reason.
    ///
    /// This function is the base implementation for resolving commitments when the target
    /// is an **direct digest** rather than a index or pool.
    ///
    /// It resolves all commit instances tied to a direct digest of a reason.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the resolved commitment value
    /// - `Err(DispatchError)` if resolution fails
    fn resolve_digest_commit(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        digest: &CommitmentDigest<T, I>,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        let variant = Pallet::<T, I>::get_commit_variant(who, reason)?;
        let imbalance = Self::withdraw_from_digest(who, reason, digest, &variant)?;
        finalize_resolve_commit::<T, I>(who, reason, imbalance)
    }

    /// Resolves (withdraws) and finalizes a commitment for a index digest under a given reason.
    ///
    /// This function is the base implementation for resolving commitments when the target
    /// is an **index** rather than a direct digest or pool.
    ///
    /// It resolves all commit instances tied to a index digest of a reason.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the resolved commitment value
    /// - `Err(DispatchError)` if resolution fails
    fn resolve_index_commit(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        index_of: &CommitmentDigest<T, I>,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        let variant = Pallet::<T, I>::get_commit_variant(who, reason)?;
        let imbalance = Self::withdraw_from_index(who, reason, index_of, &variant)?;
        finalize_resolve_commit::<T, I>(who, reason, imbalance)
    }

    /// Resolves (withdraws) and finalizes a commitment for a pool digest under a given reason.
    ///
    /// This function is the base implementation for resolving commitments when the target
    /// is an **pool** rather than a direct digest or index.
    ///
    /// It resolves all commit instances tied to a pool digest of a reason.
    ///
    /// ### Returns
    /// - `Ok(Asset)` containing the resolved commitment value
    /// - `Err(DispatchError)` if resolution fails
    fn resolve_pool_commit(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        pool_of: &CommitmentDigest<T, I>,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        let variant = Pallet::<T, I>::get_commit_variant(who, reason)?;
        let imbalance = Self::withdraw_from_pool(who, reason, pool_of, &variant)?;
        finalize_resolve_commit::<T, I>(who, reason, imbalance)
    }

    /// Sets the total asset value for a given reason.
    ///
    /// **Use with caution!** This is a low-level storage operation
    /// that directly updates the reason's total value without validation.
    ///
    /// For safe usage [`Self::add_to_total_value`] and [`Self::sub_from_total_value`].
    ///
    /// Used internally during deposit and withdrawal operations to maintain
    /// accurate aggregate tracking.
    ///
    /// This value should not be treated like digest values,
    /// rewards/inflation, or penalties/deflation.
    fn set_total_value(reason: &CommitmentReason<T, I>, value: CommitmentAsset<T, I>) {
        ReasonValue::<T, I>::set(reason, Some(value));
    }
}

// ===============================================================================
// ````````````````````````````` FINALIZATION HELPERS ````````````````````````````
// ===============================================================================

/// Low-level helper that centralizes the finalization logic for placing commits.
///
/// This function is used across direct, index, and pool digests and serves as
/// the final step of the place a new commit's operation.
///
/// Highly unchecked and assumes all invariants are already validated.
/// Therefore, it is kept private and scoped to this module.
fn finalize_place_commit<T: Config<I>, I: 'static>(
    who: &Proprietor<T>,
    reason: &CommitmentReason<T, I>,
    digest: &CommitmentDigest<T, I>,
    variant: &CommitmentPosition<T, I>,
    receipt: &CommitInstance<T, I>,
    total_commit: CommitmentAsset<T, I>,
) -> Result<(), DispatchError> {
    // New freeze for the reason; set_freeze is used because this is a fresh freeze
    // unlike raise_* methods which would increase an existing frozen amount
    T::Asset::set_freeze(reason, who, total_commit)?;

    // Add a new commit to the list of commits for this proprietor
    // Unlike raise_* methods, which add new instances to existing commits,
    // this creates a new CommitInfo (which will include the first commit-instance)
    let commit_info = CommitInfo::<T, I>::new(digest.clone(), receipt.clone(), variant.clone())?;
    CommitMap::<T, I>::insert((who, reason), commit_info);

    // Adds to the reason's total value for quick queries
    CommitHelpers::<T, I>::add_to_total_value(reason, total_commit)?;

    Ok(())
}

/// Low-level helper that centralizes the finalization logic for raising commits.
///
/// This function is used across direct, index, and pool digests and serves as
/// the final step of the raise an existing commit's operation.
///
/// Highly unchecked and assumes all invariants are already validated.
/// Therefore, it is kept private and scoped to this module.
fn finalize_raise_commit<T: Config<I>, I: 'static>(
    who: &Proprietor<T>,
    reason: &CommitmentReason<T, I>,
    receipt: &CommitInstance<T, I>,
    total_raise: CommitmentAsset<T, I>,
) -> Result<(), DispatchError> {
    // Increase the frozen balance for the proprietor for this reason (since it is a raise)
    T::Asset::increase_frozen(reason, who, total_raise)?;

    // Add a new commit instance to the existing commit info
    CommitMap::<T, I>::mutate((who, reason), |result| -> DispatchResult {
        let commit_info = result.as_mut();
        debug_assert!(
            commit_info.is_some(),
            "finalize raise commit for proprietor {:?} already commited 
            digest of reason {:?} called without assuring commit-exists for 
            raising {:?}",
            who,
            reason,
            total_raise,
        );
        let commit_info = commit_info.ok_or(Error::<T, I>::CommitNotFound)?;
        commit_info.add_commit(receipt.clone())?;
        Ok(())
    })?;

    // Adds to the reason's total value for quick queries
    CommitHelpers::<T, I>::add_to_total_value(reason, total_raise)?;

    Ok(())
}

/// Low-level helper that centralizes the finalization logic for resolving commits.
///
/// This function is used across direct, index, and pool digests and serves as
/// the final step of the resolving an existing commit's operation.
///
/// Highly unchecked and assumes all invariants are already validated.
/// Therefore, it is kept private and scoped to this module.
fn finalize_resolve_commit<T: Config<I>, I: 'static>(
    who: &Proprietor<T>,
    reason: &CommitmentReason<T, I>,
    imbalance: AssetDelta<T, I>,
) -> Result<CommitmentAsset<T, I>, DispatchError> {
    // Remove the commitment for the reason, as a proprietor can
    // have utmost a single commitment (commit-instances) to a digest of reason
    CommitMap::<T, I>::remove((who, reason));
    // Removes the fungible freeze lock completely, we don't
    // care about the value, as its the finalization step
    T::Asset::thaw(reason, who)?;
    // Resolve the asset balance to the proprietor back
    let resolved = CommitHelpers::<T, I>::resolve_imbalance(who, imbalance)?;
    // Take from the reason's total value as its resolved from the commitment system
    // to the underlying fungible system.
    CommitHelpers::<T, I>::sub_from_total_value(reason, resolved)?;
    Ok(resolved)
}

// ===============================================================================
// ```````````````````````````````` COMMIT INSPECT ```````````````````````````````
// ===============================================================================

/// Implements the [`CommitInspect`] trait for the pallet, providing low-level
/// inspection and querying capabilities for committed values across digests,
/// indexes, and pools.
///
/// This implementation allows retrieving **real-time committed values** without
/// altering state, supporting precise accounting and auditability. All functions
/// are **low-level and unchecked**, meaning callers are responsible for ensuring
/// correctness and invariants before invoking.
impl<T: Config<I>, I: 'static> CommitInspect<Proprietor<T>, Pallet<T, I>> for CommitHelpers<T, I> {
    /// Retrieves the total committed value of a proprietor for a
    /// fully resolved digest model.
    ///
    /// This function centralizes the dispatch logic to the appropriate
    /// handler depending on the type of the digest model.
    ///
    /// Digest models represent different
    /// commitment structures:
    /// - **Direct**: a single digest commitment.
    /// - **Index**: a grouped commitment of multiple entry digests.
    /// - **Pool**: a shared resource i.e., a collective single commitment.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the real-time committed value
    /// - `Err(DispatchError)` if the digest model is invalid or any underlying query fails
    fn commit_value_of(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        digest_model: &CommitmentDigestModel<T, I>,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        match digest_model {
            DigestVariant::Direct(dir) => Self::digest_commit_value(who, reason, dir),
            DigestVariant::Index(index) => Self::index_commit_value(who, reason, index),
            DigestVariant::Pool(pool) => Self::pool_commit_value(who, reason, pool),
            _ => {
                debug_assert!(
                    false,
                    "digest-model marker variants {:?} are constructed, 
                    captured during commit value query for proprietor {:?} 
                    of reason {:?} are explicitly dis-allowed",
                    digest_model, who, reason
                );
                return Err(Error::<T, I>::InvalidDigestModel.into());
            }
        }
    }

    /// Retrieves the proprietor's committed value for a **direct digest**
    /// of a given reason.
    ///
    /// ## Returns
    /// - `Ok(Asset)` representing the real-time committed value for
    /// the digest
    /// - `Err(DispatchError)` otherwise
    fn digest_commit_value(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        digest: &CommitmentDigest<T, I>,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        // Retrieve the digest information for the given reason and digest
        let digest_info =
            DigestMap::<T, I>::get((reason, digest)).ok_or(Error::<T, I>::DigestNotFound)?;

        // Retrieve the proprietor's commit information for this reason
        let commit_info =
            CommitMap::<T, I>::get((who, reason)).ok_or(Error::<T, I>::CommitNotFound)?;

        // Initialize total committed value accumulator for commit-instances folding
        let mut total = CommitmentAsset::<T, I>::zero();

        // Determine the index of the digest variant for this commit
        let variant = commit_info.variant();

        // Retrieve the balance for the specific digest variant
        let digest_of = digest_info.get_balance(&variant);
        debug_assert!(
            digest_of.is_some(),
            "digest {:?} of reason {:?} variant {:?} balance 
            was not initiated properly in the balance vector
            during the deposit for proprietor {:?}",
            digest,
            reason,
            variant,
            who
        );
        let digest_of = digest_of.ok_or(Error::<T, I>::DigestVariantBalanceNotFound)?;

        // Iterate over each commit instance associated with this proprietor
        let commits = commit_info.commits();
        for commit in &commits {
            let take = receipt_active_value(digest_of, &variant, digest, commit)?;
            // Accumulate the real-time commit value into the total
            total = total
                .checked_add(&take)
                .ok_or(Error::<T, I>::CommitsAccumulationExhausted)?;
        }

        // Return the aggregated real-time committed value for this digest
        Ok(total)
    }

    /// Computes the total committed value of a proprietor for a
    /// given index digest.
    ///
    /// ## Returns
    /// - `Ok(Asset)` with the total committed value for the index
    /// - `Err(DispatchError)` otherwise
    fn index_commit_value(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        index_of: &CommitmentDigest<T, I>,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        // Initialize accumulator for total committed value
        let mut total = CommitmentAsset::<T, I>::zero();

        // Retrieve index info and its entries
        let index_info = Pallet::<T, I>::get_index(reason, index_of)?;
        let entries = index_info.entries();

        // Iterate through each entry in the index
        for entry in entries {
            let entry_digest = &entry.digest();
            let entry_variant = &entry.variant();

            // Retrieve all commit instances for this proprietor and entry
            let commits = EntryMap::<T, I>::get((reason, &index_of, entry_digest, who))
                .ok_or(Error::<T, I>::CommitNotFoundForEntry)?;
            // Retrieve digest info for this entry
            let digest_info = DigestMap::<T, I>::get((reason, &entry_digest))
                .ok_or(Error::<T, I>::EntryOfIndexNotFound)?;

            let digest_of = digest_info.get_balance(entry_variant);
            debug_assert!(
                digest_of.is_some(),
                "index-digest {:?} of reason {:?} entry {:?} variant {:?} balance 
                was not initiated properly in the balance vector 
                during the deposit for proprietor {:?}",
                index_of,
                reason,
                entry_digest,
                entry_variant,
                who
            );
            let digest_of = digest_of.ok_or(Error::<T, I>::DigestVariantBalanceNotFound)?;
            // Iterate through each commit to compute its effective contribution
            for commit in &commits.commits() {
                let take = receipt_active_value(digest_of, entry_variant, entry_digest, commit)?;

                // Accumulate into total committed value
                total = total
                    .checked_add(&take)
                    .ok_or(Error::<T, I>::CommitsAccumulationExhausted)?;
            }
        }

        Ok(total)
    }

    /// Computes the total committed value of a proprietor for a
    /// specific entry within an index.
    ///
    /// ## Returns
    /// - `Ok(Asset)` containing the total committed value for the entry
    /// - `Err(DispatchError)` otherwise
    fn index_entry_commit_value(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        index_of: &CommitmentDigest<T, I>,
        entry_of: &CommitmentDigest<T, I>,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        // Retrieve the index and its entries
        let index_info = Pallet::<T, I>::get_index(reason, index_of)?;
        let entries = index_info.entries();

        // Locate the entry within the index
        let mut entry_idx = None;
        for (i, entry) in entries.iter().enumerate() {
            if entry.digest() == *entry_of {
                entry_idx = Some(i);
            }
        }
        let Some(entry_idx) = entry_idx else {
            return Err(Error::<T, I>::EntryOfIndexNotFound.into());
        };

        // Get the entry object and its variant
        let entry = entries
            .get(entry_idx)
            .ok_or(Error::<T, I>::EntryOfIndexNotFound)?;
        let entry_variant = &entry.variant();

        // Retrieve all commit instances for this proprietor and entry
        let commits = EntryMap::<T, I>::get((reason, index_of, entry_of, who))
            .ok_or(Error::<T, I>::CommitNotFoundForEntry)?;

        // Initialize accumulator for total committed value
        let mut total = CommitmentAsset::<T, I>::zero();
        // Retrieve the digest info for this entry

        let digest_info =
            DigestMap::<T, I>::get((reason, entry_of)).ok_or(Error::<T, I>::EntryDigestNotFound)?;
        let digest_of = digest_info.get_balance(entry_variant);
        debug_assert!(
            digest_of.is_some(),
            "index-digest {:?} of reason {:?} entry {:?} variant {:?} balance 
            was not initiated properly in the balance vector
            during the deposit for proprietor {:?}",
            index_of,
            reason,
            entry_of,
            entry_variant,
            who
        );
        let digest_of = digest_of.ok_or(Error::<T, I>::DigestVariantBalanceNotFound)?;

        // Iterate through each commit to compute its effective contribution
        for commit in &commits.commits() {
            let take = receipt_active_value(digest_of, entry_variant, entry_of, commit)?;
            // Accumulate into total committed value
            total = total
                .checked_add(&take)
                .ok_or(Error::<T, I>::CommitsAccumulationExhausted)?;
        }

        Ok(total)
    }

    /// Computes the total committed value of a proprietor
    /// for a given pool.
    ///
    /// ## Returns
    /// - `Ok(Asset)` with the total effective committed value
    /// - `Err(DispatchError)` otherwise
    fn pool_commit_value(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        pool_of: &CommitmentDigest<T, I>,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        let pool_info = Pallet::<T, I>::get_pool(reason, pool_of)?;
        let commit_info =
            CommitMap::<T, I>::get((who, reason)).ok_or(Error::<T, I>::CommitNotFound)?;
        ensure!(
            commit_info.digest() == *pool_of,
            Error::<T, I>::CommitNotFoundForPool
        );

        // Initialize zero values. For each slot, these will accumulate total
        // deposits and withdrawals.
        let mut taken = CommitmentAsset::<T, I>::zero();

        let pool_capital = pool_info.capital();
        debug_assert!(
            !pool_capital.is_zero(),
            "pool {:?} of reason {:?} capital is constructed at zero",
            pool_of,
            reason
        );
        ensure!(!pool_capital.is_zero(), Error::<T, I>::CapitalCannotBeZero);

        let slots = &pool_info.slots();
        let first_slot = &slots.iter().next();
        debug_assert!(
            first_slot.is_some(),
            "pool {:?} of reason {:?} constructed with empty slots",
            pool_of,
            reason,
        );
        let first_slot = first_slot.ok_or(Error::<T, I>::EmptySlotsNotAllowed)?;
        if first_slot.commit() == Default::default() {
            return Ok(Zero::zero());
        }

        // Iterate through all active pool slots
        for slot in slots {
            let digest = &slot.digest();
            let slot_share = slot.shares();

            debug_assert!(
                !slot_share.is_zero(),
                "pool {:?} of reason {:?} slot {:?} 
                share is found to be zero",
                pool_of,
                reason,
                slot.digest()
            );
            ensure!(!slot_share.is_zero(), Error::<T, I>::ShareCannotBeZero);
            debug_assert!(
                pool_capital >= slot_share,
                "pool {:?} of reason {:?}  capital of value {:?} is constructed 
                as lesser than the slot {:?} share {:?}",
                pool_of,
                reason,
                pool_capital,
                slot.digest(),
                slot_share
            );

            let slot_commit = &slot.commit();

            let digest_info = DigestMap::<T, I>::get((reason, digest))
                .ok_or(Error::<T, I>::SlotOfPoolNotFound)?;

            // Take slot's variant in the pool to find the variant in the digest
            let slot_variant = &slot.variant();

            // variant balance of the slot's digest
            let balance = digest_info.get_balance(slot_variant);
            debug_assert!(
                balance.is_some(),
                "pool-digest {:?} of reason {:?} slot {:?} variant {:?} balance 
                was not initiated properly in the balance vector
                properly during pool-value operation",
                pool_of,
                reason,
                digest,
                slot_variant,
            );
            let balance = balance.ok_or(Error::<T, I>::DigestVariantBalanceNotFound)?;

            let take = receipt_active_value(balance, slot_variant, digest, slot_commit)?;

            taken = taken
                .checked_add(&take)
                .ok_or(Error::<T, I>::WithdrawAccumulationExhausted)?;
        }

        let mut pool_balance = pool_info.balance();
        let pool_effective = balance_total(&pool_balance, &Default::default(), pool_of)?;
        match pool_effective.cmp(&taken) {
            Ordering::Less => {
                let lack = taken.saturating_sub(pool_effective);
                let minted = mint(
                    &mut pool_balance,
                    &Default::default(),
                    pool_of,
                    &lack,
                    &Directive::new(Precision::Exact, Fortitude::Force),
                )?;
                ensure!(minted.eq(&lack), Error::<T, I>::PoolUnsupported);
            }
            Ordering::Greater => {
                let excess = pool_effective.saturating_sub(taken);
                let reaped = reap(
                    &mut pool_balance,
                    &Default::default(),
                    pool_of,
                    &excess,
                    &Directive::new(Precision::Exact, Fortitude::Force),
                )?;
                ensure!(reaped.eq(&excess), Error::<T, I>::PoolUnsupported);
            }
            Ordering::Equal => {}
        }

        let mut total = CommitmentAsset::<T, I>::zero();
        for commit in commit_info.commits() {
            let take = withdraw(&mut pool_balance, &Default::default(), pool_of, &commit)?;
            total = total
                .checked_add(&take)
                .ok_or(Error::<T, I>::CommitsAccumulationExhausted)?;
        }
        Ok(total)
    }

    /// Computes the commit value of a specific slot for a given proprietor
    /// within a pool.
    ///
    /// ## Returns
    /// - `Ok(Asset>)` representing the slot's effective value for the proprietor
    /// - `Err(DispatchError)` if calculation fails
    fn pool_slot_commit_value(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        pool_of: &CommitmentDigest<T, I>,
        slot_of: &CommitmentDigest<T, I>,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        let pool_commit = Self::pool_commit_value(who, reason, pool_of)?;
        let pool_info = Pallet::<T, I>::get_pool(reason, pool_of);
        debug_assert!(
            pool_info.is_ok(),
            "pool {:?} of reason {:?} commit value for proprietor 
            {:?} dervied, but underlying info cannot get",
            pool_of,
            reason,
            who
        );
        let pool_info = pool_info?;
        let mut slot_share = None;
        for slot in pool_info.slots() {
            if slot.digest() == *slot_of {
                slot_share = Some(slot.shares());
            }
        }
        let slot_share = slot_share.ok_or(Error::<T, I>::SlotOfPoolNotFound)?;
        debug_assert!(
            !slot_share.is_zero(),
            "pool {:?} of reason {:?} slot {:?} 
            share is found to be zero",
            pool_of,
            reason,
            slot_of
        );
        ensure!(!slot_share.is_zero(), Error::<T, I>::ShareCannotBeZero);
        let pool_capital = pool_info.capital();
        debug_assert!(
            pool_capital >= slot_share,
            "pool {:?} of reason {:?}  capital of value {:?} is constructed 
             as lesser than the slot {:?} share {:?}",
            pool_of,
            reason,
            pool_capital,
            slot_of,
            slot_share
        );
        let factor = T::Bias::checked_from_rational(slot_share, pool_capital)
            .ok_or(Error::<T, I>::TooSmallShareValue)?;
        let value_fixed = T::Bias::saturating_from_integer(pool_commit);
        let val_fixed = value_fixed
            .checked_mul(&factor)
            .ok_or(Error::<T, I>::WithdrawalOverflow)?;
        if val_fixed < One::one() {
            return Ok(Zero::zero());
        }
        let val_scaled = val_fixed
            .into_inner()
            // ensured no NAN possibility
            .checked_div(&T::Bias::DIV)
            .ok_or(Error::<T, I>::DerivedLessThanZeroValue)?;

        let val: CommitmentAsset<T, I> = val_scaled.into();
        Ok(val)
    }

    /// Retrieves the total value of a reason or a specific digest model.
    ///
    /// For a specific digest model, computes the total of all dispositions
    /// (`Affirmative`, `Contrary`, `Awaiting`) for direct digests, or delegates to
    /// index/pool calculations as appropriate. If no model is provided, returns the
    /// stored top-level reason value.
    ///
    /// ### Returns
    /// - `Ok(CommitmentAsset<T, I>)` representing the total value
    /// - `Err(DispatchError)` if the reason is not found or digest value calculation fails
    fn value_of(
        digest_model: Option<&CommitmentDigestModel<T, I>>,
        reason: &CommitmentReason<T, I>,
    ) -> Result<CommitmentAsset<T, I>, DispatchError> {
        if let Some(model) = digest_model {
            let mut value: CommitmentAsset<T, I> = Zero::zero();
            match model {
                // Direct digest: sum up all dispositions
                DigestVariant::Direct(dir) => {
                    let len = <T::Position as VariantCount>::VARIANT_COUNT;
                    for i in 0..len {
                        let curr = Pallet::<T, I>::get_digest_variant_value(
                            reason,
                            dir,
                            &T::Position::position_of(i as usize)
                                .ok_or(Error::<T, I>::InvalidCommitVariantIndex)?,
                        )?;
                        value = value.saturating_add(curr)
                    }
                }

                // Index digest: get the aggregated index value
                DigestVariant::Index(index) => {
                    value = Pallet::<T, I>::get_index_value(reason, index)?;
                }

                // Pool digest: get the aggregated pool value
                DigestVariant::Pool(pool) => {
                    value = Pallet::<T, I>::get_pool_value(reason, pool)?;
                }

                _ => {
                    debug_assert!(
                        false,
                        "digest-model marker variants {:?} are constructed, 
                        captured during value query for reason {:?} are explicitly 
                        dis-allowed",
                        digest_model, reason
                    );
                    return Err(Error::<T, I>::InvalidDigestModel.into());
                }
            }
            return Ok(value);
        }

        // Return top-level reason value if no model is provided
        let value =
            ReasonValue::<T, I>::get(reason).ok_or(Error::<T, I>::CommitsNotFoundForReason)?;
        Ok(value)
    }
}

// ===============================================================================
// ``````````````````````````````` POOL OPERATIONS ```````````````````````````````
// ===============================================================================

/// Implementation of [`PoolOps`] for the pallet, defining how pools slots
/// are queried, inserted, updated, or removed within a given index digest.
///
/// Each function operates on the *pool-slot composition layer* - maintaining
/// relationships between an pool digest and its subordinate slot digests.
impl<T: Config<I>, I: 'static> PoolOps<Proprietor<T>, Pallet<T, I>> for CommitHelpers<T, I> {
    /// Uses [`LazyBalanceOf`] to track both the *real-time effective* balance and the
    /// *total principal deposits* contributed by all proprietors of the pool.
    ///
    /// Each proprietor's deposit (commit) into the pool is recorded as a [`CommitInstance`]
    /// inside its storage.
    type PoolBalance = LazyBalanceOf<T, I>;

    /// Releases a pool's balance into a detached representation.
    ///
    /// Resolves all active slot digests belonging to the pool, withdraws their
    /// values proportionally, and resets the pool's stored balance to default.
    /// The released balance can then be modified before being recovered.
    ///
    /// ## Returns
    /// - `Ok(Balance)` containing the released pool balance
    /// - `Err(DispatchError)` if the pool is not found or release fails
    fn release_pool(
        reason: &CommitmentReason<T, I>,
        pool_of: &CommitmentDigest<T, I>,
    ) -> Result<Self::PoolBalance, DispatchError> {
        // In case if pool is empty, return early
        let pool = Pallet::<T, I>::get_pool(reason, pool_of)?;
        let mut pool_balance = pool.balance();
        if pool_balance == Self::PoolBalance::default() {
            return Ok(pool_balance);
        }

        // Initialize zero values. For each slot, these will accumulate total deposits and withdrawals.
        let mut taken = CommitmentAsset::<T, I>::zero();
        let pool_capital = pool.capital();
        debug_assert!(
            !pool_capital.is_zero(),
            "pool {:?} of reason {:?} capital is constructed at zero",
            pool_of,
            reason
        );
        ensure!(!pool_capital.is_zero(), Error::<T, I>::CapitalCannotBeZero);
        // Mutate the pool storage entry for (reason, pool_of)
        PoolMap::<T, I>::mutate((reason, pool_of), |result| -> DispatchResult {
            let pool_info = result.as_mut();
            debug_assert!(
                pool_info.is_some(),
                "pool {:?} of reason {:?} exists, but cannot mutate",
                pool_of,
                reason
            );
            let pool_info = pool_info.ok_or(Error::<T, I>::PoolNotFound)?;
            // Iterate through all active pool slots
            let slots = &pool_info.slots();
            for slot in slots {
                let digest = &slot.digest();
                let slot_share = slot.shares();

                debug_assert!(
                    !slot_share.is_zero(),
                    "pool {:?} of reason {:?} slot {:?} 
                    share is found to be zero",
                    pool_of,
                    reason,
                    slot.digest()
                );
                ensure!(!slot_share.is_zero(), Error::<T, I>::ShareCannotBeZero);
                debug_assert!(
                    pool_capital >= slot_share,
                    "pool {:?} of reason {:?}  capital of value {:?} is constructed 
                    as lesser than the slot {:?} share {:?}",
                    pool_of,
                    reason,
                    pool_capital,
                    slot.digest(),
                    slot_share
                );

                let slot_commit = &slot.commit();

                if *slot_commit == Default::default() {
                    continue;
                }

                // Release each slot via its digest
                DigestMap::<T, I>::mutate((reason, digest), |result| -> DispatchResult {
                    let digest_info = result.as_mut().ok_or(Error::<T, I>::SlotOfPoolNotFound)?;

                    // Take slot's variant in the pool to find the variant in the digest
                    let slot_variant = &slot.variant();

                    // variant balance of the slot's digest
                    let balance = digest_info.mut_balance(slot_variant);
                    debug_assert!(
                        balance.is_some(),
                        "pool-digest {:?} of reason {:?} slot {:?} variant {:?} balance 
                        was not initiated properly in the balance vector
                        properly during release-pool operation",
                        pool_of,
                        reason,
                        digest,
                        slot_variant,
                    );
                    let balance = balance.ok_or(Error::<T, I>::DigestVariantBalanceNotFound)?;

                    let take = withdraw(balance, &Default::default(), pool_of, slot_commit)?;

                    taken = taken
                        .checked_add(&take)
                        .ok_or(Error::<T, I>::WithdrawAccumulationExhausted)?;
                    Ok(())
                })?;
            }
            pool_info.balance_reset();
            Ok(())
        })?;
        let pool_effective = balance_total(&pool_balance, &Default::default(), pool_of)?;
        match pool_effective.cmp(&taken) {
            Ordering::Less => {
                let lack = taken.saturating_sub(pool_effective);
                let minted = mint(
                    &mut pool_balance,
                    &Default::default(),
                    pool_of,
                    &lack,
                    &Directive::new(Precision::Exact, Fortitude::Force),
                )?;
                ensure!(minted.eq(&lack), Error::<T, I>::PoolUnsupported);
            }
            Ordering::Greater => {
                let excess = pool_effective.saturating_sub(taken);
                let reaped = reap(
                    &mut pool_balance,
                    &Default::default(),
                    pool_of,
                    &excess,
                    &Directive::new(Precision::Exact, Fortitude::Force),
                )?;
                ensure!(reaped.eq(&excess), Error::<T, I>::PoolUnsupported);
            }
            Ordering::Equal => {}
        }
        Ok(pool_balance)
    }

    /// Restores a pool's state after a prior release.
    ///
    /// Redistributes the recovered balance among slots proportionally based on
    /// their shares, and reconciles any dust with the pool manager. This function
    /// should only be called after `release_pool`.
    ///
    /// ## Returns
    /// - `Ok(())` if the pool state was successfully recovered
    /// - `Err(DispatchError)` with `ReleasePoolToRecover` if pool is not in released state, or recovery fails
    fn recover_pool(
        reason: &CommitmentReason<T, I>,
        pool_of: &CommitmentDigest<T, I>,
        balance: &Self::PoolBalance,
    ) -> DispatchResult {
        let effective = balance_total(balance, &Default::default(), pool_of)?;

        PoolMap::<T, I>::mutate((reason, pool_of), |result| -> DispatchResult {
            let pool_info = result.as_mut().ok_or(Error::<T, I>::PoolNotFound)?;

            // Ensure the pool is in a "released" state (balance must be zero) before recovery.
            let is_released = pool_info.balance() == Self::PoolBalance::default();
            debug_assert!(
                is_released,
                "pool {:?} of reason {:?} attempted 
                recovering without being released or empty",
                pool_of, reason
            );
            ensure!(is_released, Error::<T, I>::ReleasePoolToRecover);

            let mut taken = CommitmentAsset::<T, I>::zero();
            let pool_capital = pool_info.capital();
            debug_assert!(
                !pool_capital.is_zero(),
                "pool {:?} of reason {:?} capital is constructed at zero",
                pool_of,
                reason
            );
            ensure!(!pool_capital.is_zero(), Error::<T, I>::CapitalCannotBeZero);

            // Iterate over each slot to allocate its share of the recovered balance.
            let slots = &pool_info.slots();
            for slot in slots {
                let digest = &slot.digest();
                let slot_shares = slot.shares();
                debug_assert!(
                    !slot_shares.is_zero(),
                    "pool {:?} of reason {:?} slot {:?} 
                    share is found to be zero",
                    pool_of,
                    reason,
                    slot.digest()
                );
                ensure!(!slot_shares.is_zero(), Error::<T, I>::ShareCannotBeZero);
                debug_assert!(
                    pool_capital >= slot_shares,
                    "pool {:?} of reason {:?}  capital of value {:?} is constructed 
                    as lesser than the slot {:?} share {:?}",
                    pool_of,
                    reason,
                    pool_capital,
                    slot.digest(),
                    slot_shares
                );
                let variant = &slot.variant();

                // Calculate slot's proportional factor: shares / total capital.
                let factor = T::Bias::saturating_from_rational(slot_shares, pool_capital);
                // Convert the effective balance to fixed point representation.
                let effective_fixed = T::Bias::saturating_from_integer(effective);
                // Calculate the deposit value for this slot.
                let slot_deposit_fixed = effective_fixed
                    .checked_mul(&factor)
                    .ok_or(Error::<T, I>::DepositDeriveOverflowed)?;
                let slot_deposit_scaled = slot_deposit_fixed
                    .into_inner()
                    .checked_div(&T::Bias::DIV)
                    .ok_or(Error::<T, I>::DerivedLessThanZeroValue)?;
                let slot_deposit: CommitmentAsset<T, I> = slot_deposit_scaled.into();

                let mut commit_instance = Default::default();

                if slot_deposit.is_zero() {
                    pool_info.set_slot_commit(digest, commit_instance)?;
                    continue;
                }

                // Track the slot's commit-instance after deposit.
                // Mutate the slot's digest to update the deposit.
                DigestMap::<T, I>::mutate((reason, digest), |result| -> DispatchResult {
                    let digest_info = result.as_mut().ok_or(Error::<T, I>::SlotOfPoolNotFound)?;

                    let Some(balance) = digest_info.mut_balance(variant) else {
                        digest_info.init_balance(variant)?;
                        // Deposit value into the newly created variant balance
                        let digest_of_new_variant = digest_info.mut_balance(variant);
                        debug_assert!(
                            digest_of_new_variant.is_some(),
                            "recently initiated digest {:?} of reason {:?} variant {:?} 
                            balance not accessible via vector",
                            digest,
                            reason,
                            variant,
                        );
                        let digest_of_new_variant = digest_of_new_variant
                            .ok_or(Error::<T, I>::DigestVariantBalanceNotFound)?;
                        let (depositted, receipt) = deposit(
                            digest_of_new_variant,
                            variant,
                            digest,
                            &slot_deposit,
                            &Directive::new(Precision::Exact, Fortitude::Force),
                        )?;
                        ensure!(depositted.eq(&slot_deposit), Error::<T, I>::PoolUnsupported);
                        taken = taken
                            .checked_add(&depositted)
                            .ok_or(Error::<T, I>::WithdrawAccumulationExhausted)?;
                        commit_instance = receipt;

                        // Early exit from mutate closure after depositing to newly created variant
                        return Ok(());
                    };

                    // Deposit the slot's share.
                    let (depositted, receipt) = deposit(
                        balance,
                        variant,
                        digest,
                        &slot_deposit,
                        &Directive::new(Precision::Exact, Fortitude::Force),
                    )?;
                    ensure!(depositted.eq(&slot_deposit), Error::<T, I>::PoolUnsupported);
                    taken = taken
                        .checked_add(&depositted)
                        .ok_or(Error::<T, I>::WithdrawAccumulationExhausted)?;

                    commit_instance = receipt;

                    Ok(())
                })?;

                // Now mutate the slot via mutable pool to set slot balance
                // Update the slot's commit holding deposit.
                pool_info.set_slot_commit(digest, commit_instance)?;
            }

            // Compute any leftover amount ("dust") due to rounding or proportional distribution.
            let dust = effective.saturating_sub(taken);

            if dust.is_zero() {
                // If there is no dust, simply set the recovered balance as the pool's balance.
                pool_info.set_balance(balance.clone());
                return Ok(());
            }

            // If dust exists, resolve it with the pool manager.
            let manager = Pallet::<T, I>::get_manager(reason, pool_of);
            debug_assert!(
                manager.is_ok(),
                "pool {:?} of reason {:?} exists but manager is not",
                pool_of,
                reason
            );
            let manager = manager?;

            let manager_imbalance = AssetDelta {
                deposit: dust,
                withdraw: dust,
            };

            let manager_withdraw = Self::resolve_imbalance(&manager, manager_imbalance)?;
            Self::sub_from_total_value(reason, manager_withdraw)?;

            // Adjust recovered balance to remove dust.
            let mut new_balance = balance.clone();
            // Usually reap/mint are seen in `set_digest_*` functions, we do it here, since
            // there are no structural level mutations on `LazyBalance`
            let reaped = reap(
                &mut new_balance,
                &Default::default(),
                pool_of,
                &dust,
                &Directive::new(Precision::Exact, Fortitude::Force),
            )?;
            ensure!(reaped.eq(&dust), Error::<T, I>::PoolUnsupported);

            // Update the pool's balance to the recovered balance after adjustments.
            pool_info.set_balance(new_balance);

            Ok(())
        })?;

        Ok(())
    }

    /// Removes a specific slot from a given pool.
    ///
    /// The function first *releases* the pool to safely mutate it, ensuring no
    /// lazy balance state remains. Once verified, it searches for the specified slot
    /// and removes it. The pool's capital is then adjusted to account for the
    /// removed slot's shares. Finally, the pool state is *recovered*.
    ///
    /// ## Returns
    /// - `Ok(())` if the slot is successfully removed or not found
    /// - `Err(DispatchError)` if pool or slot state is invalid
    fn remove_pool_slot(
        _who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        pool_of: &CommitmentDigest<T, I>,
        slot_of: &CommitmentDigest<T, I>,
    ) -> DispatchResult {
        // Temporarily release the pool to allow mutation.
        let balance = Self::release_pool(reason, pool_of)?;

        PoolMap::<T, I>::mutate((reason, pool_of), |result| -> DispatchResult {
            let pool_info = result.as_mut();
            debug_assert!(
                pool_info.is_some(),
                "pool-released {:?} of reason {:?} but cannot mutate during removing its slot {:?}",
                pool_of,
                reason,
                slot_of
            );
            let pool_info = pool_info.ok_or(Error::<T, I>::PoolNotFound)?;

            pool_info.remove_slot(slot_of)?;

            let slots = &pool_info.slots();

            ensure!(!slots.is_empty(), Error::<T, I>::EmptySlotsNotAllowed);

            let capital = pool_info.capital();
            debug_assert!(
                !capital.is_zero(),
                "pool {:?} of reason {:?} newly derived capital after slot {:?} removal is zero",
                pool_of,
                reason,
                slot_of
            );
            ensure!(!capital.is_zero(), Error::<T, I>::CapitalCannotBeZero);

            Ok(())
        })?;

        // Reapply pool state after mutation.
        Self::recover_pool(reason, pool_of, &balance)?;
        Ok(())
    }

    /// Inserts or updates a slot within an existing pool. For removing use
    /// [`Self::remove_pool_slot`] instead.
    ///
    /// The pool is first *released* for safe modification.  
    /// If the slot already exists, it is removed and replaced with the updated one.
    /// Capital adjustments are performed based on the delta between old and new
    /// share allocations. The pool is then *recovered* post-mutation.
    ///
    /// ## Returns
    /// - `Ok(())` if the slot is successfully set
    /// - `Err(DispatchError)` if pool is not in released state, if slot capacity exceeded, or operation fails
    fn set_pool_slot(
        _who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        pool_of: &CommitmentDigest<T, I>,
        slot_of: &CommitmentDigest<T, I>,
        shares: CommitmentShares<T, I>,
        variant: &CommitmentPosition<T, I>,
    ) -> DispatchResult {
        ensure!(!shares.is_zero(), Error::<T, I>::ShareCannotBeZero);
        // Temporarily release the pool to allow mutable access.
        let balance = Self::release_pool(reason, pool_of)?;

        PoolMap::<T, I>::mutate((reason, pool_of), |result| -> DispatchResult {
            let pool_info = result.as_mut();
            debug_assert!(
                pool_info.is_some(),
                "pool-released {:?} of reason {:?} but cannot mutate during removing its slot {:?}",
                pool_of,
                reason,
                slot_of
            );
            let pool_info = pool_info.ok_or(Error::<T, I>::PoolNotFound)?;

            if pool_info.slot_exists(slot_of).is_ok() {
                pool_info.remove_slot(slot_of)?;
            }

            let entry = EntryInfo::<T, I>::new(slot_of.clone(), shares, variant.clone())?;
            pool_info.add_slot(entry)?;

            Ok(())
        })?;

        // Restore the pool's active balance state.
        Self::recover_pool(reason, pool_of, &balance)?;
        Ok(())
    }
}

// ===============================================================================
// `````````````````````````````` INDEX OPERATIONS ```````````````````````````````
// ===============================================================================

/// Implementation of [`IndexOps`] for the pallet, defining how index entries
/// are queried, inserted, updated, or removed within a given index digest.
///
/// Each function operates on the *index-entry composition layer* - maintaining
/// relationships between an index digest and its subordinate entry digests.
impl<T: Config<I>, I: 'static> IndexOps<Proprietor<T>, Pallet<T, I>> for CommitHelpers<T, I> {
    /// Removes a specific entry from an existing index.
    ///
    /// The function retrieves the target index, searches for the specified entry,
    /// and removes it if found. The index digest is then re-generated and updated
    /// to reflect the new structure as indexes are immutable, changes may create
    /// new index digest.
    ///
    /// ## Returns
    /// - `Ok(IndexDigest)` containing the new index digest after entry removal
    /// - `Err(DispatchError)` if the operation fails
    fn remove_index_entry(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        index_of: &CommitmentDigest<T, I>,
        entry_of: &CommitmentDigest<T, I>,
    ) -> Result<CommitmentDigest<T, I>, DispatchError> {
        // Retrieve the current index and its entries.
        let index_info = Pallet::<T, I>::get_index(reason, index_of)?;
        let mut new_entries = index_info.reveal_entries();
        new_entries.remove_entry(entry_of)?;

        // Construct the updated index info and generate a new digest.
        let new_index = IndexInfo::<T, I>::new(&mut new_entries)?;
        let digest = Pallet::<T, I>::gen_index_digest(who, reason, &new_index)?;

        // Commit the updated index structure.
        Pallet::<T, I>::set_index(who, reason, &new_index, &digest)?;
        Ok(digest)
    }

    /// Inserts or updates an entry within a given index. For removing use
    /// [`Self::remove_index_entry`] instead.
    ///
    /// Since indexes are immutable, this operation creates a new index
    /// with the modified entry configuration and generates a new digest.
    ///
    /// If the entry exists, its shares and variant are updated; otherwise,
    /// a new entry is appended.
    ///
    /// ## Returns
    /// - `Ok(IndexDigest)` containing the new index digest after entry modification
    /// - `Err(DispatchError)` if the operation fails
    fn set_index_entry(
        who: &Proprietor<T>,
        reason: &CommitmentReason<T, I>,
        index_of: &CommitmentDigest<T, I>,
        entry_of: &CommitmentDigest<T, I>,
        shares: CommitmentShares<T, I>,
        variant: &CommitmentPosition<T, I>,
    ) -> Result<CommitmentDigest<T, I>, DispatchError> {
        ensure!(!shares.is_zero(), Error::<T, I>::ShareCannotBeZero);
        // Retrieve the target index and clone its entries for mutation.
        let index_info = Pallet::<T, I>::get_index(reason, index_of)?;
        let mut new_entries = index_info.reveal_entries();

        if index_info.entry_exists(entry_of).is_ok() {
            new_entries.remove_entry(entry_of)?;
        };

        new_entries.add_entry(EntryInfo::<T, I>::new(
            entry_of.clone(),
            shares,
            variant.clone(),
        )?)?;

        // Generate updated index info and its new digest.
        let new_index = IndexInfo::<T, I>::new(&mut new_entries)?;
        let digest = Pallet::<T, I>::gen_index_digest(who, reason, &new_index)?;

        // Persist the updated index.
        Pallet::<T, I>::set_index(who, reason, &new_index, &digest)?;

        Ok(digest)
    }
}