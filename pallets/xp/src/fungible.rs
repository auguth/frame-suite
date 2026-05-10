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
// ``````````````````````````````` FUNGIBLE ADAPTER ``````````````````````````````
// ===============================================================================

//! Implementation of compatible [`fungible`](frame_support::traits::fungible)
//! traits for the [`Pallet`] Type.
//!
//! [`Pallet`] implements via calls towards [`xp`](frame_suite::xp) traits:
//! - [`Inspect`]
//! - [`Unbalanced`]
//! - [`Mutate`]
//! - [`InspectHold`]
//! - [`InspectFreeze`]
//! - [`UnbalancedHold`]
//! - [`MutateFreeze`]
//! - [`MutateHold`]
//!
//! Local Tests for these traits are covered in `tests`.

// ===============================================================================
// ```````````````````````````````````` IMPORTS ``````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{
    types::{LockReason, ReserveReason, XpId, XpValue},
    Config, Error, Pallet, XpOf,
};

// --- FRAME Suite ---
use frame_suite::xp::{XpLock, XpMutate, XpReserve, XpSystem};

// --- FRAME Support ---
use frame_support::{
    ensure,
    traits::{
        fungible::{
            Dust, Inspect, InspectFreeze, InspectHold, Mutate, MutateFreeze, MutateHold,
            Unbalanced, UnbalancedHold,
        },
        tokens::{
            DepositConsequence, Fortitude, Precision, Preservation, Provenance, WithdrawConsequence,
        },
    },
};

// --- Substrate primitives ---
use sp_runtime::{
    traits::{CheckedAdd, CheckedSub, Saturating, Zero},
    DispatchError, DispatchResult, TokenError,
};

// ===============================================================================
// ```````````````````````````````````` INSPECT ``````````````````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> Inspect<XpId<T>> for Pallet<T, I> {
    type Balance = XpValue<T, I>;

    /// **Always panics!**. XP does not support total issuance.
    ///
    /// XP does not track total issuance since it is earned based on work or intent-specific
    /// contributions.
    ///
    /// There is no inflation model, as each XP point has individual meaning tied to context.
    ///
    /// While XP units may be comparable numerically, they are not issued under the assumption
    /// of fungibility.
    /// XP providers define how XP is earned, not through a global issuance mechanism.
    ///
    /// For runtime intents or abstractions which intend to operate on Fungible implementations
    /// such as pallet_balances and pallet_xp, callers should treat this as a no-op and utilize
    /// other trait extensions such as [`Unbalanced`]
    fn total_issuance() -> Self::Balance {
        panic!("Cannot determine total_issuance if Fungible methods are derived from Xp");
    }

    /// Returns the minimum balance required for an XP to be considered alive.
    ///
    /// XP reaping is not solely determined by balance. An XP entry may still be valid
    /// even if fully consumed, since XP can be re-earned through further work or actions.
    ///
    /// Therefore, we assume **no minimum balance** is necessary to keep an XP alive.
    ///
    /// Instead, XP lifecycle management (e.g., determining dead XP) should rely on other
    /// runtime mechanisms, such as timestamps [`crate::MinTimeStamp`].
    ///
    /// Consumers of this trait may implement automated reaping by integrating with
    /// functions like `xp_exists` or by analyzing XP activity rather than static balances.
    ///
    /// This value is deliberately **zero** to support such flexible lifecycle handling.
    fn minimum_balance() -> Self::Balance {
        Self::Balance::zero()
    }

    /// Returns the total usable XP balance for the given key.
    ///
    /// If the XP entry does not exist, this function returns `zero` as a fallback.
    ///
    /// Unlike **liquid XP**, which refers only to the `free` portion, the **usable XP**
    /// includes both `free` and `reserved` portions - making this function more suited
    /// for systems that consider total accessible XP rather than just transferable XP.
    ///
    /// This method relies on [`XpSystem::get_usable_xp`].
    ///
    /// **Note**:
    /// - This is provided to conform to the `Fungible` trait expectations.
    /// - While XP is not inherently fungible, `total_balance` allows integration
    ///   in systems assuming that a balance-like arithmetic abstraction is available.
    fn total_balance(who: &XpId<T>) -> Self::Balance {
        let Ok(total_balance) = <Pallet<T, I>>::get_usable_xp(who) else {
            return Self::Balance::zero();
        };
        total_balance
    }

    /// Returns the **liquid XP** balance for the given key.
    ///
    /// If the XP does not exist, this returns `zero`.
    ///
    /// Liquid XP represents the freely accessible portion of XP - that is,
    /// XP that is not locked or reserved and is available for immediate use.
    ///
    /// This method relies on [`XpSystem::get_liquid_xp`].
    ///
    /// **Note**:
    /// - This method aligns with the `Fungible` trait's `balance` expectation, even
    ///   though XP is not strictly fungible.
    /// - It provides the free XP as a proxy for the "spendable" amount.
    fn balance(who: &XpId<T>) -> Self::Balance {
        let Ok(balance) = Self::get_liquid_xp(who) else {
            return Self::Balance::zero();
        };
        balance
    }

    /// Returns the amount of XP that can be reduced (i.e., slashed or withdrawn) for
    /// the given Xp key.
    ///
    /// XP is **not** subject to existential deposit or minimum balance preservation
    /// like standard fungible assets.
    ///
    /// If the XP does not exist, this returns `zero`.
    ///
    /// This method relies on [`XpSystem::get_liquid_xp`].
    ///
    /// The `_preservation` and `_force` parameter is ignored as XP does not implement minimum
    /// balance enforcement.
    #[inline]
    fn reducible_balance(
        who: &XpId<T>,
        _preservation: Preservation,
        _force: Fortitude,
    ) -> Self::Balance {
        Self::balance(who)
    }

    /// Determines whether XP can be deposited into the account of the given XP key.
    ///
    /// Returns a `DepositConsequence` indicating whether the XP deposit is allowed.
    ///
    /// ### Rules
    /// - XP **cannot** be minted arbitrarily. Only providers with internal logic may
    ///   assign new XP using [`XpMutate::earn_xp`].
    /// - If the provenance is [`Provenance::Minted`], the deposit is always **blocked**.
    /// - While direct deposit minting is blocked, it is always preferable to allow minting in
    ///   XP and balance systems using the safe `Balanced` trait to issue new balance and increase
    ///   the balance of an account.
    /// - If the XP does not exist for the given key ([`XpSystem::xp_exists`] returns `false`),
    ///   the deposit is **blocked**, because creating a new XP key should only be done via
    ///   the Xp Trait [`XpMutate::new_xp`] or via genesis-config xp-accounts.
    /// - A zero-amount deposit is a **success** (considered a no-op).
    /// - Deposits are allowed **only** if the new liquid XP will not overflow.
    fn can_deposit(
        who: &XpId<T>,
        amount: Self::Balance,
        provenance: Provenance,
    ) -> DepositConsequence {
        if Self::xp_exists(who).is_err() {
            return DepositConsequence::UnknownAsset;
        }
        if amount.is_zero() {
            return DepositConsequence::Success;
        }
        if provenance == Provenance::Minted {
            return DepositConsequence::Blocked;
        }
        let balance = Self::balance(who);
        if balance.checked_add(&amount).is_none() {
            return DepositConsequence::Overflow;
        }
        DepositConsequence::Success
    }

    /// Determines whether a given amount of XP can be withdrawn from the given XP key.
    ///
    /// Returns a `WithdrawConsequence` indicating whether the amount of XP can be withdrawn.
    ///
    /// ### Behavior
    /// - If the amount is `zero`, the withdrawal is trivially allowed.
    /// - If the XP key does not exist, returns `UnknownAsset`.
    /// - Checks whether the amount can be covered using the *liquid/free* XP balance.
    ///   If the balance is insufficient, returns `BalanceLow`. Otherwise, returns `Success`.
    fn can_withdraw(who: &XpId<T>, amount: Self::Balance) -> WithdrawConsequence<Self::Balance> {
        if Self::xp_exists(who).is_err() {
            return WithdrawConsequence::UnknownAsset;
        }
        if amount.is_zero() {
            return WithdrawConsequence::Success;
        }
        let balance = Self::balance(who);
        if amount > balance {
            return WithdrawConsequence::BalanceLow;
        }
        WithdrawConsequence::Success
    }

    /// **Always panics!**. XP does not maintain an active issuance count.
    ///
    /// Similar to `total_issuance`, XP is not issued in a globally managed or inflating manner.
    ///
    /// The amount of XP granted is determined by the provider, and the XP system only defines
    /// how such points are added or interpreted.
    ///
    /// Since XP is only numerically comparable (pseudo-fungible) but not truly fungible,
    /// no active issuance is tracked to prevent any notion of inflation or global supply.
    ///
    /// Callers expecting issuance metrics (e.g., for fungible traits) should treat this
    /// as a no-op and utilize other trait extensions such as `Fungible::Balanced` or
    /// `Fungible::Unbalanced`.
    fn active_issuance() -> Self::Balance {
        panic!("Cannot determine active_issuance if Fungible methods are derived from Xp");
    }
}

// ===============================================================================
// `````````````````````````````````` UNBALANCED `````````````````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> Unbalanced<XpId<T>> for Pallet<T, I> {
    /// XP operations may generate imprecise or saturating side-effects
    /// (e.g., dust due to overflow control), which are handled internally by the XP system.
    /// XP accounts can exist at zero points, so it is assumed no such dust will be created.
    ///
    /// Therefore, this implementation is a no-op.
    fn handle_dust(_dust: Dust<XpId<T>, Self>) {}

    /// Writes the free XP balance for the given key.
    ///
    /// This bypasses XP earning mechanisms and directly sets the XP to the specified value.
    ///
    /// We return `None` intentionally to indicate no dust may exist.
    fn write_balance(
        who: &XpId<T>,
        amount: Self::Balance,
    ) -> Result<Option<Self::Balance>, DispatchError> {
        Self::set_xp(who, amount)?;
        Ok(None)
    }

    /// The XP system does not support active or total issuance.
    ///
    /// Therefore, this implementation is a no-op.
    fn set_total_issuance(_amount: Self::Balance) {}

    /// This implementation is a no-op.
    ///  
    fn handle_raw_dust(_amount: Self::Balance) {}

    /// Increases the balance of `who` by `amount`.
    ///
    /// If the balance cannot be increased by that amount for any reason,
    /// returns `Err` and does not increase it at all.
    ///
    /// If successful, returns the amount by which the balance was
    /// increased (the imbalance).
    fn increase_balance(
        who: &XpId<T>,
        amount: Self::Balance,
        precision: Precision,
    ) -> Result<Self::Balance, DispatchError> {
        Self::xp_exists(who)?;
        let current_balance = Self::balance(who);
        let increased_balance = match precision {
            Precision::BestEffort => current_balance.saturating_add(amount),
            Precision::Exact => current_balance
                .checked_add(&amount)
                .ok_or(Error::<T, I>::XpCapOverflowed)?,
        };
        let result = Self::write_balance(who, increased_balance);
        debug_assert!(
            result.is_ok(),
            "xp-key {who:?} exists but fungible's increase balance
            throws error, for writing balance {increased_balance:?}, where current balance {current_balance:?}"
        );
        result?;
        let imbalance = increased_balance.saturating_sub(current_balance);
        Ok(imbalance)
    }

    /// Decreases the balance of `who` by `amount`.
    ///
    /// - If `precision` is `Exact` and the balance cannot be reduced by
    ///   that amount, returns `Err` and does not reduce it at all.
    /// - If `precision` is `BestEffort`, reduces the balance by as much as
    ///   possible, up to `amount`.
    ///
    /// In either case, if `Ok` is returned, the inner value is the amount by
    /// which the balance was reduced.
    fn decrease_balance(
        who: &XpId<T>,
        mut amount: Self::Balance,
        precision: Precision,
        preservation: Preservation,
        force: Fortitude,
    ) -> Result<Self::Balance, DispatchError> {
        Self::xp_exists(who)?;
        let reducible_balance = Self::reducible_balance(who, preservation, force);
        let decreased_balance = match precision {
            Precision::BestEffort => {
                amount = amount.min(reducible_balance);
                reducible_balance.saturating_sub(amount)
            }
            Precision::Exact => reducible_balance
                .checked_sub(&amount)
                .ok_or(Error::<T, I>::XpCapUnderflowed)?,
        };
        let result = Self::write_balance(who, decreased_balance);
        debug_assert!(
            result.is_ok(),
            "xp-key {who:?} exists but fungible's decrease balance
            throws error, for writing balance {decreased_balance:?}, where reducible balance {reducible_balance:?}"
        );
        result?;
        let imbalance = reducible_balance.saturating_sub(decreased_balance);
        Ok(imbalance)
    }

    /// This implementation is a no-op.
    ///  
    fn deactivate(_: Self::Balance) {}

    /// This implementation is a no-op.
    ///  
    fn reactivate(_: Self::Balance) {}
}

// ===============================================================================
// ```````````````````````````````````` MUTATE ```````````````````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> Mutate<XpId<T>> for Pallet<T, I> {
    // Note: In all default implementations, no-op operations such as querying total
    // issuance are provided.
    // If arithmetic operations are performed on these defaults, it may result in errors.
    // Therefore, we reimplemented the defaults to produce deterministic errors, since XP does
    // not have a total issuance and its default value is not meaningful.

    /// Mints (adds) `amount` XP to the given XP key.
    ///
    /// - Fails if the XP key does not exist.
    /// - Fails on overflow.
    /// - Calls `done_mint_into` after successful mint.
    /// - Returns the actual amount minted (the imbalance).
    fn mint_into(who: &XpId<T>, amount: Self::Balance) -> Result<Self::Balance, DispatchError> {
        let actual = Self::increase_balance(who, amount, Precision::Exact)?;
        Self::done_mint_into(who, amount);
        Ok(actual)
    }

    /// Burns (removes) up to `amount` XP from the given XP key.
    ///
    /// - Fails if the XP key does not exist.
    /// - Fails if funds are unavailable and precision is `Exact`.
    /// - Calls `done_burn_from` after successful burn.
    /// - Returns the actual amount burned (the imbalance).
    fn burn_from(
        who: &XpId<T>,
        amount: Self::Balance,
        preservation: Preservation,
        precision: Precision,
        force: Fortitude,
    ) -> Result<Self::Balance, DispatchError> {
        let actual = Self::reducible_balance(who, preservation, force).min(amount);
        ensure!(
            actual == amount || precision == Precision::BestEffort,
            TokenError::FundsUnavailable
        );
        let actual =
            Self::decrease_balance(who, actual, Precision::BestEffort, preservation, force);
        debug_assert!(
            actual.is_ok(),
            "xp-key {who:?} tried burning {amount:?} from reducible balance {actual:?} with
            BestEffort precision, yet-failed"
        );
        let actual = actual?;
        Self::done_burn_from(who, actual);
        Ok(actual)
    }

    /// Shelves (removes) up to `amount` XP from the given XP key.
    ///
    /// - Fails if funds are unavailable.
    /// - Returns the actual amount shelved (the imbalance).
    fn shelve(who: &XpId<T>, amount: Self::Balance) -> Result<Self::Balance, DispatchError> {
        let actual =
            Self::reducible_balance(who, Preservation::Expendable, Fortitude::Polite).min(amount);
        frame_support::ensure!(actual == amount, TokenError::FundsUnavailable);
        let actual = Self::decrease_balance(
            who,
            actual,
            Precision::BestEffort,
            Preservation::Expendable,
            Fortitude::Polite,
        );
        debug_assert!(
            actual.is_ok(),
            "xp-key {who:?} tried shelving (burning) {amount:?} from reducible balance {actual:?} with
            BestEffort precision, yet-failed"
        );
        let actual = actual?;
        Ok(actual)
    }

    /// Restores (adds) `amount` XP to the given XP key.
    ///
    /// - Fails if the XP key does not exist.
    /// - Fails on overflow.
    /// - Returns the actual amount restored (the imbalance).
    fn restore(who: &XpId<T>, amount: Self::Balance) -> Result<Self::Balance, DispatchError> {
        let actual = Self::increase_balance(who, amount, Precision::Exact)?;
        Ok(actual)
    }

    /// Transfers XP between keys is not supported.
    ///
    /// Always returns [`Error::CannotTransferXp`].
    fn transfer(
        _source: &XpId<T>,
        _dest: &XpId<T>,
        _amount: Self::Balance,
        _preservation: Preservation,
    ) -> Result<Self::Balance, DispatchError> {
        Err(Error::<T, I>::CannotTransferXp.into())
    }

    /// Sets the free XP balance for the given XP key.
    ///
    /// - Returns `zero` if the XP key does not exist.
    /// - Otherwise, sets the free balance and returns the new balance.
    fn set_balance(who: &XpId<T>, amount: Self::Balance) -> Self::Balance {
        if Self::xp_exists(who).is_err() {
            return Self::Balance::zero();
        }
        let _ = XpOf::<T, I>::mutate(who, |result| -> DispatchResult {
            let value = result.as_mut();
            debug_assert!(
                value.is_some(),
                "xp-key {who:?} exists but meta unaccesssible for 
                setting new liquid balance {amount:?}"
            );

            let value = value.ok_or(Error::<T, I>::XpNotFound)?;
            value.free = amount;
            Ok(())
        });
        Self::balance(who)
    }

    /// Called after a successful burn operation.
    ///
    /// Triggers XP update hook.
    #[inline]
    fn done_burn_from(who: &XpId<T>, amount: Self::Balance) {
        Self::on_xp_update(who, amount);
    }

    /// Called after a successful mint operation.
    ///
    /// Triggers XP update hook.
    #[inline]
    fn done_mint_into(who: &XpId<T>, amount: Self::Balance) {
        Self::on_xp_update(who, amount);
    }

    /// Called after a successful restore operation.
    ///
    /// Triggers XP update hook.
    #[inline]
    fn done_restore(who: &XpId<T>, amount: Self::Balance) {
        Self::on_xp_update(who, amount);
    }

    /// Called after a successful shelve operation.
    ///
    /// Triggers XP update hook.
    #[inline]
    fn done_shelve(who: &XpId<T>, amount: Self::Balance) {
        Self::on_xp_update(who, amount);
    }

    /// This implementation is a no-op.
    fn done_transfer(_source: &XpId<T>, _dest: &XpId<T>, _amount: Self::Balance) {}
}

// ===============================================================================
// ````````````````````````````````` INSPECT HOLD ````````````````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> InspectHold<XpId<T>> for Pallet<T, I> {
    /// The reserve reason identifier used to categorize reserved XP points.
    type Reason = ReserveReason<T, I>;

    /// Returns the total reserved XP for the given XP key.
    ///
    /// - If the XP does not exist, returns `zero`.
    ///
    /// Note: This function cannot definitively determine whether an XP exists solely
    /// based on the returned value, since inactive or uninitialized reserves on an
    /// active XP will also return `zero`.
    fn total_balance_on_hold(who: &XpId<T>) -> Self::Balance {
        if Self::has_reserve(who).is_err() {
            return Self::Balance::zero();
        }
        let total_reserved = Self::total_reserved(who);
        debug_assert!(
            total_reserved.is_ok(),
            "xp-key {who:?} has reserves but cannot get its total-reserve"
        );
        let Ok(on_hold) = total_reserved else {
            return Self::Balance::zero();
        };
        on_hold
    }

    /// Returns the reserved XP held for the given reason by the specified XP key.
    ///
    /// - Returns `zero` if the XP key does not have an active reserve for the given reason,
    ///   or if the reserve exists but has been fully reduced (i.e., balance is zero).
    ///
    /// Note: Due to the design of the Fungible Traits, a reserve may still technically exist
    /// even if its balance is `zero`. Therefore, this method does not distinguish between
    /// a fully depleted reserve and a non-existent one.
    fn balance_on_hold(reason: &Self::Reason, who: &XpId<T>) -> Self::Balance {
        if Self::reserve_exists(who, reason).is_err() {
            return Self::Balance::zero();
        }
        let reserve_of = Self::get_reserve_xp(who, reason);
        debug_assert!(
            reserve_of.is_ok(),
            "xp-key {who:?} reserve of reason {reason:?} exists but cannot get its value"
        );
        let Ok(on_hold) = reserve_of else {
            return Self::Balance::zero();
        };
        on_hold
    }
}

// ===============================================================================
// ```````````````````````````````` INSPECT FREEZE ```````````````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> InspectFreeze<XpId<T>> for Pallet<T, I> {
    type Id = LockReason<T, I>;

    /// Returns the locked (frozen) XP of the given lock `id` of XP Key.
    ///
    /// Returns `zero` if no lock is found.
    fn balance_frozen(id: &Self::Id, who: &XpId<T>) -> Self::Balance {
        if Self::lock_exists(who, id).is_err() {
            return Self::Balance::zero();
        }
        let lock_of = Self::get_lock_xp(who, id);
        debug_assert!(
            lock_of.is_ok(),
            "xp-key {who:?} lock of reason {id:?} exists but cannot get its value"
        );
        let Ok(frozen) = lock_of else {
            return Self::Balance::zero();
        };
        frozen
    }

    /// Checks if XP can be locked (frozen) for the given lock `id` and XP key.
    ///
    /// Returns `true` if:
    /// - The XP key exists.
    /// - No lock currently exists for the given `id`.
    ///
    /// Returns `false` otherwise.
    fn can_freeze(id: &Self::Id, who: &XpId<T>) -> bool {
        if Self::xp_exists(who).is_err() {
            return false;
        }
        if Self::lock_exists(who, id).is_ok() {
            return false;
        }
        true
    }
}

// ===============================================================================
// ```````````````````````````````` UNBALANCED HOLD ``````````````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> UnbalancedHold<XpId<T>> for Pallet<T, I> {
    /// Sets or updates the reserved XP (`balance_on_hold`) for a given `reason`
    /// of XP key.
    ///
    /// - If `amount` is zero, the function does not create or modify any reserve.
    /// - If the reserve exists, [`XpReserve::can_reserve_mutate`] must return `Ok(())` to allow
    ///   the update.
    /// - If the reserve does not exist, [`XpReserve::can_reserve_new`] must return `Ok(())` to
    ///   allow creating a new reserve.
    ///
    /// Returns `Ok(())` on success, or an error if the operation fails.
    fn set_balance_on_hold(
        reason: &Self::Reason,
        who: &XpId<T>,
        amount: Self::Balance,
    ) -> DispatchResult {
        if Self::reserve_exists(who, reason).is_err() && amount.is_zero() {
            return Ok(());
        }
        // Usually passes, but edge-cases such as total-reserve checked-arithmetic may
        // return errors, hence we are not debug-asserting well-known op-result
        Self::set_reserve(who, reason, amount)?;
        Ok(())
    }
}

// ===============================================================================
// ````````````````````````````````` MUTATE FREEZE ```````````````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> MutateFreeze<XpId<T>> for Pallet<T, I> {
    /// Sets or updates the locked XP (`freeze`) for the given `id` and XP Key.
    ///
    /// - If `amount` is `zero` and lock exists for the given `key` and `id` this
    ///   operation is treated as a **thaw** (i.e., burn/remove the lock).
    /// - If the lock exists, [`XpLock::can_lock_mutate`] must return `Ok(())` to allow
    ///   the update.
    /// - If the lock does not exist, [`XpLock::can_lock_new`] must return `Ok(())` to
    ///   allow creating a new lock.
    ///
    /// Returns `Ok(())` on success, or an error if the operation fails.
    fn set_freeze(id: &Self::Id, who: &XpId<T>, amount: Self::Balance) -> DispatchResult {
        if Self::lock_exists(who, id).is_ok() && amount.is_zero() {
            Self::thaw(id, who)?;
            return Ok(());
        }
        // Usually passes, but edge-cases such as total-locks checked-arithmetic may
        // return errors, hence we are not debug-asserting well-known op-result
        Self::set_lock(who, id, amount)?;
        Ok(())
    }

    /// Extends (or sets) a lock (freeze) for the given lock `id` of the
    /// specified XP key.
    ///
    /// - If the lock exists, increases the locked amount to the greater of the
    ///   current or requested value.
    /// - If the lock does not exist, returns an error (`XpLockNotFound`).
    /// - If `amount` is `zero`, this is a no-op and returns `Ok(())`.
    fn extend_freeze(id: &Self::Id, who: &XpId<T>, amount: Self::Balance) -> DispatchResult {
        if amount.is_zero() {
            return Ok(());
        }
        let freeze_balance = Self::get_lock_xp(who, id)?;
        let extend_amount = freeze_balance.max(amount);

        // Usually passes, but edge-cases such as total-locks checked-arithmetic may
        // return errors, hence we are not debug-asserting well-known op-result
        Self::set_lock(who, id, extend_amount)?;
        Ok(())
    }

    /// Thaws (removes) the XP lock for the given lock `id` of the specified XP key.
    ///
    /// This is effectively a lock **burn** as it permanently removes the lock.
    ///
    /// - Fails if the XP key does not exist.
    /// - Fails if the lock does not exist.
    fn thaw(id: &Self::Id, who: &XpId<T>) -> DispatchResult {
        Self::xp_exists(who)?;
        Self::burn_lock(who, id)?;
        Ok(())
    }

    /// Increase the amount which is being frozen for a particular freeze, failing
    /// in the case that too little balance is available for being frozen.
    fn increase_frozen(id: &Self::Id, who: &XpId<T>, amount: Self::Balance) -> DispatchResult {
        let a = Self::balance_frozen(id, who)
            .checked_add(&amount)
            .ok_or(Error::<T, I>::XpCapOverflowed)?;
        Self::set_frozen(id, who, a, Fortitude::Force)
    }
}

// ===============================================================================
// `````````````````````````````````` MUTATE HOLD ````````````````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> MutateHold<XpId<T>> for Pallet<T, I> {}

// ===============================================================================
// `````````````````````````````````` UNIT TESTS `````````````````````````````````
// ===============================================================================

/// Unit tests for [`fungible`](frame_support::traits::fungible) trait
/// implementations over [`Pallet`].
#[cfg(test)]
pub mod tests {
        
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````````` IMPORTS ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    // --- Local crate imports ---
    use crate::mock::*;

    // --- FRAME Suite ---
    use frame_suite::xp::{XpLock, XpMutate, XpReserve, XpSystem};

    // --- FRAME Support ---
    use frame_support::{
        assert_err, assert_ok,
        traits::{
            fungible::{
                Inspect, InspectFreeze, InspectHold, Mutate, MutateFreeze, Unbalanced,
                UnbalancedHold,
            },
            tokens::{
                DepositConsequence, Fortitude, Precision, Preservation, Provenance,
                WithdrawConsequence,
            },
        },
    };

    // --- Substrate primitives ---
    use sp_runtime::TokenError;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` INSPECT ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    #[should_panic]
    fn total_issuance_panic() {
        xp_test_ext().execute_with(|| {
            Pallet::total_issuance();
        });
    }

    #[test]
    fn minimum_balance_success() {
        xp_test_ext().execute_with(|| {
            let actual = Pallet::minimum_balance();
            let expected = 0;
            assert_eq!(expected, actual);
        });
    }

    #[test]
    fn total_balance_fail_uninitalized_xp() {
        xp_test_ext().execute_with(|| {
            let actual = Pallet::total_balance(&ALICE);
            let expected = 0;
            assert_eq!(expected, actual);
        });
    }

    #[test]
    fn total_balance_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::set_reserve(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let expected_total_balance = xp.free + xp.reserve;
            let actual_total_balance = Pallet::total_balance(&XP_ALPHA);
            assert_eq!(expected_total_balance, actual_total_balance);
        });
    }

    #[test]
    fn balance_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let expected_balance = xp.free;
            let actual_balance = Pallet::balance(&XP_ALPHA);
            assert_eq!(expected_balance, actual_balance);
        });
    }

    #[test]
    fn balance_fail_uninitialized() {
        xp_test_ext().execute_with(|| {
            let actual = Pallet::balance(&ALICE);
            let expected = 0;
            assert_eq!(expected, actual);
        });
    }

    #[test]
    fn reducible_balance_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let expected_liquid = xp.free;
            let actual_reducible =
                Pallet::reducible_balance(&XP_ALPHA, Preservation::Expendable, Fortitude::Polite);
            assert_eq!(expected_liquid, actual_reducible);
        });
    }

    #[test]
    fn reducible_balance_fail_uninitialized() {
        xp_test_ext().execute_with(|| {
            let actual_reducible =
                Pallet::reducible_balance(&ALICE, Preservation::Expendable, Fortitude::Polite);
            let expected_liquid = 0;
            assert_eq!(expected_liquid, actual_reducible);
        });
    }

    #[test]
    fn can_deposit_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_eq!(
                Pallet::can_deposit(&ALICE, DEFAULT_POINTS, Provenance::Extant),
                DepositConsequence::Success
            )
        });
    }

    #[test]
    fn can_deposit_success_with_zero() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_eq!(
                Pallet::can_deposit(&ALICE, INVALID_POINTS, Provenance::Extant),
                DepositConsequence::Success
            )
        });
    }

    #[test]
    fn can_deposit_fail_uninitialized_xp() {
        xp_test_ext().execute_with(|| {
            assert_eq!(
                Pallet::can_deposit(&ALICE, DEFAULT_POINTS, Provenance::Extant),
                DepositConsequence::UnknownAsset
            )
        });
    }

    #[test]
    fn can_deposit_fail_overflow() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_eq!(
                Pallet::can_deposit(&ALICE, SATURATED_MAX, Provenance::Extant),
                DepositConsequence::Overflow
            )
        });
    }

    #[test]
    fn can_deposit_fail_minted() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_eq!(
                Pallet::can_deposit(&ALICE, DEFAULT_POINTS, Provenance::Minted),
                DepositConsequence::Blocked
            )
        });
    }

    #[test]
    fn can_withdraw_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_eq!(
                Pallet::can_withdraw(&ALICE, DEFAULT_POINTS),
                WithdrawConsequence::Success
            )
        });
    }

    #[test]
    fn can_withdraw_success_with_zero() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_eq!(
                Pallet::can_withdraw(&ALICE, INVALID_POINTS),
                WithdrawConsequence::Success
            )
        });
    }

    #[test]
    fn can_withdraw_fail_uninitialized_xp() {
        xp_test_ext().execute_with(|| {
            assert_eq!(
                Pallet::can_withdraw(&ALICE, DEFAULT_POINTS),
                WithdrawConsequence::UnknownAsset
            )
        });
    }

    #[test]
    fn can_withdraw_fail_low_balance() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let available_liquid = xp.free;
            assert_eq!(available_liquid, DEFAULT_POINTS);
            let withdraw_amount = 20;
            assert_eq!(
                Pallet::can_withdraw(&ALICE, withdraw_amount),
                WithdrawConsequence::BalanceLow
            )
        });
    }

    #[test]
    #[should_panic]
    fn active_issuance_panic() {
        xp_test_ext().execute_with(|| {
            Pallet::active_issuance();
        });
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` UNBALANCED ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn write_balance_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&ALICE).unwrap();
            let free_before = xp.free;
            assert_eq!(free_before, 10);
            let new_balance = 50;
            assert_ok!(Pallet::write_balance(&ALICE, new_balance));
            let xp = Pallet::get_xp(&ALICE).unwrap();
            let free_after = xp.free;
            assert_eq!(free_after, 50);
        });
    }

    #[test]
    fn write_balance_fail_uninitalized_xp() {
        xp_test_ext().execute_with(|| {
            assert_err!(
                Pallet::write_balance(&ALICE, DEFAULT_POINTS),
                Error::XpNotFound
            )
        });
    }

    #[test]
    fn increase_balance_success_besteffort() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_before = xp.free;
            assert_eq!(balance_before, 10);
            let increment_amount = 20;
            let imbalance =
                Pallet::increase_balance(&XP_ALPHA, increment_amount, Precision::BestEffort)
                    .unwrap();
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_after = xp.free;
            let expected_balace = balance_before.saturating_add(increment_amount);
            let expected_imbalance = balance_after.saturating_sub(balance_before);
            assert_eq!(expected_imbalance, imbalance);
            assert_eq!(expected_balace, balance_after);
        });
    }

    #[test]
    fn increase_balance_success_exact() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_before = xp.free;
            assert_eq!(balance_before, 10);
            let increment_amount = 20;
            let imbalance =
                Pallet::increase_balance(&XP_ALPHA, increment_amount, Precision::Exact).unwrap();
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_after = xp.free;
            let expected_balace = balance_before.saturating_add(increment_amount);
            let expected_imbalance = balance_after.saturating_sub(balance_before);
            assert_eq!(expected_imbalance, imbalance);
            assert_eq!(expected_balace, balance_after);
        });
    }

    #[test]
    fn increase_balance_handle_exact_overflow() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_err!(
                Pallet::increase_balance(&XP_ALPHA, SATURATED_MAX, Precision::Exact),
                Error::XpCapOverflowed
            )
        });
    }

    #[test]
    fn increase_balance_handle_besteffort_saturating() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_before = xp.free;
            assert_eq!(balance_before, 10);
            let imbalance =
                Pallet::increase_balance(&XP_ALPHA, SATURATED_MAX, Precision::BestEffort).unwrap();
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_after = xp.free;
            let expected_imbalance = balance_after.saturating_sub(balance_before);
            assert_eq!(expected_imbalance, imbalance);
            assert_eq!(balance_after, SATURATED_MAX);
        });
    }

    #[test]
    fn increase_balance_fail_uninitialized_xp() {
        xp_test_ext().execute_with(|| {
            assert_err!(
                Pallet::increase_balance(&XP_ALPHA, DEFAULT_POINTS, Precision::Exact),
                Error::XpNotFound
            )
        });
    }

    #[test]
    fn decrease_balance_success_besteffort() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_before = xp.free;
            assert_eq!(balance_before, 10);
            let decrement_amount = 5;
            let imbalance = Pallet::decrease_balance(
                &XP_ALPHA,
                decrement_amount,
                Precision::BestEffort,
                Preservation::Expendable,
                Fortitude::Polite,
            )
            .unwrap();
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_after = xp.free;
            let expected_balace = balance_before.saturating_sub(decrement_amount);
            let expected_imbalance = balance_before.saturating_sub(balance_after);
            assert_eq!(expected_imbalance, imbalance);
            assert_eq!(expected_balace, balance_after);
        });
    }

    #[test]
    fn decrease_balance_success_exact() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_before = xp.free;
            assert_eq!(balance_before, 10);
            let decrement_amount = 5;
            let imbalance = Pallet::decrease_balance(
                &XP_ALPHA,
                decrement_amount,
                Precision::Exact,
                Preservation::Expendable,
                Fortitude::Polite,
            )
            .unwrap();
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_after = xp.free;
            let expected_balace = balance_before.saturating_sub(decrement_amount);
            let expected_imbalance = balance_before.saturating_sub(balance_after);
            assert_eq!(expected_imbalance, imbalance);
            assert_eq!(expected_balace, balance_after);
        });
    }

    #[test]
    fn decrease_balance_handle_exact_underflow() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_err!(
                Pallet::decrease_balance(
                    &XP_ALPHA,
                    SATURATED_MAX,
                    Precision::Exact,
                    Preservation::Expendable,
                    Fortitude::Polite
                ),
                Error::XpCapUnderflowed
            )
        });
    }

    #[test]
    fn decrease_balance_handle_besteffort_saturating() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_before = xp.free;
            assert_eq!(balance_before, 10);
            let imbalance = Pallet::decrease_balance(
                &XP_ALPHA,
                SATURATED_MAX,
                Precision::BestEffort,
                Preservation::Expendable,
                Fortitude::Polite,
            )
            .unwrap();
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_after = xp.free;
            let expected_imbalance = balance_before.saturating_sub(balance_after);
            assert_eq!(expected_imbalance, imbalance);
            assert_eq!(balance_after, 0);
        });
    }

    #[test]
    fn decrease_balance_fail_uninitialized_xp() {
        xp_test_ext().execute_with(|| {
            assert_err!(
                Pallet::decrease_balance(
                    &XP_ALPHA,
                    DEFAULT_POINTS,
                    Precision::Exact,
                    Preservation::Expendable,
                    Fortitude::Polite
                ),
                Error::XpNotFound
            )
        });
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` MUTATE ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn mint_into_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_before = xp.free;
            System::set_block_number(2);
            let minted = Pallet::mint_into(&XP_ALPHA, DEFAULT_POINTS).unwrap();
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_after = xp.free;
            let balance_expected = balance_before.saturating_add(DEFAULT_POINTS);
            let expected_minted = balance_after.saturating_sub(balance_before);
            assert_eq!(balance_expected, balance_after);
            assert_eq!(expected_minted, minted);
            System::assert_last_event(
                Event::Xp {
                    id: XP_ALPHA,
                    xp: minted,
                }
                .into(),
            );
        });
    }

    #[test]
    fn mint_into_uninitialized_xp() {
        xp_test_ext().execute_with(|| {
            assert_err!(
                Pallet::mint_into(&XP_ALPHA, DEFAULT_POINTS),
                Error::XpNotFound
            );
        });
    }

    #[test]
    fn mint_into_fail_overflow() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_err!(
                Pallet::mint_into(&XP_ALPHA, SATURATED_MAX),
                Error::XpCapOverflowed
            )
        });
    }

    #[test]
    fn burn_from_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_before = xp.free;
            System::set_block_number(2);
            let burn_amount = 5;
            let burned = Pallet::burn_from(
                &XP_ALPHA,
                burn_amount,
                Preservation::Expendable,
                Precision::BestEffort,
                Fortitude::Polite,
            )
            .unwrap();
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_after = xp.free;
            let expected_balance = balance_before.saturating_sub(balance_after);
            assert_eq!(expected_balance, balance_after);
            assert_eq!(burned, burn_amount);
            System::assert_last_event(
                Event::Xp {
                    id: XP_ALPHA,
                    xp: burned,
                }
                .into(),
            );
        });
    }

    #[test]
    fn burn_from_fail_funds_unavailable() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_before = xp.free;
            assert_eq!(balance_before, 10);
            System::set_block_number(2);
            let burn_amount = 20;
            assert_err!(
                Pallet::burn_from(
                    &XP_ALPHA,
                    burn_amount,
                    Preservation::Expendable,
                    Precision::Exact,
                    Fortitude::Polite
                ),
                TokenError::FundsUnavailable
            )
        });
    }

    #[test]
    fn shelve_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_before = xp.free;
            let shelve_amount = 5;
            let shelved = Pallet::shelve(&XP_ALPHA, shelve_amount).unwrap();
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_after = xp.free;
            let expected_balance = balance_before.saturating_sub(balance_after);
            assert_eq!(expected_balance, balance_after);
            assert_eq!(shelve_amount, shelved);
        });
    }

    #[test]
    fn shelve_fail_funds_unavailable() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let shelve_amount = 20;
            assert_err!(
                Pallet::shelve(&XP_ALPHA, shelve_amount),
                TokenError::FundsUnavailable
            );
        });
    }

    #[test]
    fn restore_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_before = xp.free;
            let restore_amount = 15;
            let restored = Pallet::restore(&XP_ALPHA, restore_amount).unwrap();
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_after = xp.free;
            let expected_balance = balance_before.saturating_add(restore_amount);
            assert_eq!(expected_balance, balance_after);
            assert_eq!(restore_amount, restored);
        });
    }

    #[test]
    fn restore_fail_uninitalized_xp() {
        xp_test_ext().execute_with(|| {
            assert_err!(
                Pallet::restore(&XP_ALPHA, DEFAULT_POINTS),
                Error::XpNotFound
            )
        });
    }

    #[test]
    fn restore_fail_overflow() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_err!(
                Pallet::restore(&XP_ALPHA, SATURATED_MAX),
                Error::XpCapOverflowed
            )
        });
    }

    #[test]
    fn transfer_failure_success() {
        xp_test_ext().execute_with(|| {
            assert_err!(
                Pallet::transfer(
                    &XP_ALPHA,
                    &XP_BETA,
                    DEFAULT_POINTS,
                    Preservation::Expendable
                ),
                Error::CannotTransferXp
            )
        });
    }

    #[test]
    fn set_balance_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_before = xp.free;
            assert_eq!(balance_before, 10);
            let set_amount = 50;
            let new_balance = Pallet::set_balance(&XP_ALPHA, set_amount);
            let xp = Pallet::get_xp(&XP_ALPHA).unwrap();
            let balance_after = xp.free;
            assert_eq!(balance_after, set_amount);
            assert_eq!(balance_after, new_balance);
        });
    }

    #[test]
    fn set_balance_uninitialized_xp() {
        xp_test_ext().execute_with(|| {
            let new_balance = Pallet::set_balance(&XP_ALPHA, DEFAULT_POINTS);
            let expected_balance = 0;
            assert_eq!(expected_balance, new_balance);
        });
    }

    #[test]
    fn done_burn_from_success() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(2);
            Pallet::done_burn_from(&XP_ALPHA, DEFAULT_POINTS);
            System::assert_last_event(
                Event::Xp {
                    id: XP_ALPHA,
                    xp: DEFAULT_POINTS,
                }
                .into(),
            );
        });
    }

    #[test]
    fn done_mint_into_success() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(2);
            Pallet::done_mint_into(&XP_ALPHA, DEFAULT_POINTS);
            System::assert_last_event(
                Event::Xp {
                    id: XP_ALPHA,
                    xp: DEFAULT_POINTS,
                }
                .into(),
            );
        });
    }

    #[test]
    fn done_restore_success() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(2);
            Pallet::done_restore(&XP_ALPHA, DEFAULT_POINTS);
            System::assert_last_event(
                Event::Xp {
                    id: XP_ALPHA,
                    xp: DEFAULT_POINTS,
                }
                .into(),
            );
        });
    }

    #[test]
    fn done_shelve() {
        xp_test_ext().execute_with(|| {
            System::set_block_number(2);
            Pallet::done_shelve(&XP_ALPHA, DEFAULT_POINTS);
            System::assert_last_event(
                Event::Xp {
                    id: XP_ALPHA,
                    xp: DEFAULT_POINTS,
                }
                .into(),
            );
        });
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` INSPECT HOLD ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn total_balance_on_hold_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let reserve_points_1 = 20;
            Pallet::set_reserve(&XP_ALPHA, &STAKING, reserve_points_1).unwrap();
            let reserve_points_2 = 30;
            Pallet::set_reserve(&XP_ALPHA, &GOVERNANCE, reserve_points_2).unwrap();
            let actual_total_hold = Pallet::total_balance_on_hold(&XP_ALPHA);
            let expected_total_hold = reserve_points_1 + reserve_points_2;
            assert_eq!(expected_total_hold, actual_total_hold);
        });
    }

    #[test]
    fn total_balance_on_hold_uninitialized_xp() {
        xp_test_ext().execute_with(|| {
            let expected_hold = 0;
            assert_eq!(Pallet::total_balance_on_hold(&XP_ALPHA), expected_hold);
        });
    }

    #[test]
    fn total_balance_on_hold_no_reserve() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let expected_hold = 0;
            assert_eq!(Pallet::total_balance_on_hold(&XP_ALPHA), expected_hold);
        });
    }

    #[test]
    fn balance_on_hold_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let reserve_points = 30;
            Pallet::set_reserve(&XP_ALPHA, &STAKING, reserve_points).unwrap();
            let actual_hold = Pallet::balance_on_hold(&STAKING, &XP_ALPHA);
            let expected_hold = reserve_points;
            assert_eq!(expected_hold, actual_hold);
        });
    }

    #[test]
    fn balance_on_hold_uninitialized_xp() {
        xp_test_ext().execute_with(|| {
            let expected_hold = 0;
            assert_eq!(Pallet::balance_on_hold(&STAKING, &XP_ALPHA), expected_hold);
        });
    }

    #[test]
    fn balance_on_hold_no_reserve() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let expected_hold = 0;
            assert_eq!(Pallet::balance_on_hold(&STAKING, &XP_ALPHA), expected_hold);
        });
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````` INSPECT FREEZE ```````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn balance_frozen_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let lock_points = 40;
            Pallet::set_lock(&XP_ALPHA, &STAKING, lock_points).unwrap();
            let frozen = Pallet::balance_frozen(&STAKING, &XP_ALPHA);
            assert_eq!(frozen, lock_points);
        });
    }

    #[test]
    fn balance_frozen_uninitialized_xp() {
        xp_test_ext().execute_with(|| {
            let frozen = Pallet::balance_frozen(&STAKING, &XP_ALPHA);
            let expected_frozen = 0;
            assert_eq!(expected_frozen, frozen);
        });
    }

    #[test]
    fn balance_frozen_no_lock() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let frozen = Pallet::balance_frozen(&STAKING, &XP_ALPHA);
            let expected_frozen = 0;
            assert_eq!(expected_frozen, frozen);
        });
    }

    #[test]
    fn can_freeze_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert!(Pallet::can_freeze(&STAKING, &XP_ALPHA));
        });
    }

    #[test]
    fn can_freeze_fail_uninitialized_xp() {
        xp_test_ext().execute_with(|| {
            assert!(!Pallet::can_freeze(&STAKING, &XP_ALPHA));
        });
    }

    #[test]
    fn can_freeze_fail_lock_exist() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::set_lock(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();
            assert!(!Pallet::can_freeze(&STAKING, &XP_ALPHA));
        });
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````` UNBALANCED HOLD ``````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn set_balance_on_hold_success_on_zero() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let reserve_points = 0;
            assert_ok!(Pallet::set_balance_on_hold(
                &STAKING,
                &XP_ALPHA,
                reserve_points
            ));
            assert_err!(
                Pallet::reserve_exists(&XP_ALPHA, &STAKING),
                Error::XpReserveNotFound
            );
        });
    }

    #[test]
    fn set_balance_on_hold_success_new() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let reserve_points = 40;
            Pallet::set_balance_on_hold(&STAKING, &XP_ALPHA, reserve_points).unwrap();
            let reserved = Pallet::get_reserve_xp(&XP_ALPHA, &STAKING).unwrap();
            assert_eq!(reserved, reserve_points);
        });
    }

    #[test]
    fn set_balance_on_hold_success_mutate() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::set_reserve(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();
            let existing_reserve_points = Pallet::get_reserve_xp(&XP_ALPHA, &STAKING).unwrap();
            assert_eq!(existing_reserve_points, 10);
            let new_reserve_points = 40;
            Pallet::set_balance_on_hold(&STAKING, &XP_ALPHA, new_reserve_points).unwrap();
            let new_reserved = Pallet::get_reserve_xp(&XP_ALPHA, &STAKING).unwrap();
            assert_eq!(new_reserved, new_reserve_points);
        });
    }

    #[test]
    fn set_balance_on_hold_fail_mutate_overflow() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::set_reserve(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();
            Pallet::set_reserve(&XP_ALPHA, &REASON_TREASURY, DEFAULT_POINTS).unwrap();
            assert_err!(
                Pallet::set_balance_on_hold(&STAKING, &XP_ALPHA, SATURATED_MAX),
                Error::XpReserveCapOverflowed
            )
        });
    }

    #[test]
    fn set_balance_on_hold_fail_uninitialized_xp() {
        xp_test_ext().execute_with(|| {
            assert_err!(
                Pallet::set_balance_on_hold(&STAKING, &XP_ALPHA, DEFAULT_POINTS),
                Error::XpNotFound
            )
        });
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````` MUTATE FREEZE ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn set_freeze_thaw_on_zero() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::set_lock(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();
            assert_ok!(Pallet::lock_exists(&XP_ALPHA, &STAKING));
            let lock_points = 0;
            Pallet::set_freeze(&STAKING, &XP_ALPHA, lock_points).unwrap();
            assert_err!(
                Pallet::lock_exists(&XP_ALPHA, &STAKING),
                Error::XpLockNotFound
            );
        });
    }

    #[test]
    fn set_freeze_fail_on_zero() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_err!(
                Pallet::set_freeze(&STAKING, &XP_ALPHA, INVALID_POINTS),
                Error::CannotLockZero,
            );
            assert_err!(
                Pallet::lock_exists(&XP_ALPHA, &STAKING),
                Error::XpLockNotFound
            );
        });
    }

    #[test]
    fn set_freeze_success_new() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            let lock_points = 40;
            Pallet::set_freeze(&STAKING, &XP_ALPHA, lock_points).unwrap();
            let locked = Pallet::get_lock_xp(&XP_ALPHA, &STAKING).unwrap();
            assert_eq!(locked, lock_points);
        });
    }

    #[test]
    fn set_freeze_success_mutate() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::set_lock(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();
            let existing_lock_points = Pallet::get_lock_xp(&XP_ALPHA, &STAKING).unwrap();
            assert_eq!(existing_lock_points, 10);
            let new_lock_points = 40;
            Pallet::set_freeze(&STAKING, &XP_ALPHA, new_lock_points).unwrap();
            let new_locked = Pallet::get_lock_xp(&XP_ALPHA, &STAKING).unwrap();
            assert_eq!(new_locked, new_lock_points);
        });
    }

    #[test]
    fn set_freeze_fail_mutate_overflow() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::set_lock(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();
            Pallet::set_lock(&XP_ALPHA, &REASON_TREASURY, DEFAULT_POINTS).unwrap();
            assert_err!(
                Pallet::set_freeze(&STAKING, &XP_ALPHA, SATURATED_MAX),
                Error::XpLockCapOverflowed
            )
        });
    }

    #[test]
    fn set_freeze_fail_uninitialized_xp() {
        xp_test_ext().execute_with(|| {
            assert_err!(
                Pallet::set_freeze(&STAKING, &XP_ALPHA, DEFAULT_POINTS),
                Error::XpNotFound
            )
        });
    }

    #[test]
    fn thaw_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::set_lock(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();
            assert_ok!(Pallet::thaw(&STAKING, &XP_ALPHA));
            assert_err!(
                Pallet::lock_exists(&XP_ALPHA, &STAKING),
                Error::XpLockNotFound
            );
        });
    }

    #[test]
    fn thaw_fail_uninitialized_xp() {
        xp_test_ext().execute_with(|| {
            assert_err!(Pallet::thaw(&STAKING, &XP_ALPHA), Error::XpNotFound);
        });
    }

    #[test]
    fn thaw_fail_no_lock() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_err!(Pallet::thaw(&STAKING, &XP_ALPHA), Error::XpLockNotFound);
        });
    }

    #[test]
    fn extend_freeze_success_on_zero() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::set_lock(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();
            assert_ok!(Pallet::extend_freeze(&STAKING, &XP_ALPHA, INVALID_POINTS));
        });
    }

    #[test]
    fn extend_freeze_success() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::set_lock(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();
            let extend_points = 40;
            Pallet::extend_freeze(&STAKING, &XP_ALPHA, extend_points).unwrap();
            let extended_freez = Pallet::get_lock_xp(&XP_ALPHA, &STAKING).unwrap();
            assert_eq!(extended_freez, extend_points)
        });
    }

    #[test]
    fn extend_freeze_low_amount() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            Pallet::set_lock(&XP_ALPHA, &STAKING, DEFAULT_POINTS).unwrap();
            let extend_points = 9;
            Pallet::extend_freeze(&STAKING, &XP_ALPHA, extend_points).unwrap();
            let extended_freez = Pallet::get_lock_xp(&XP_ALPHA, &STAKING).unwrap();
            assert_eq!(extended_freez, DEFAULT_POINTS)
        });
    }

    #[test]
    fn extend_freeze_fail_no_lock() {
        xp_test_ext().execute_with(|| {
            Pallet::new_xp(&ALICE, &XP_ALPHA);
            assert_err!(
                Pallet::extend_freeze(&STAKING, &XP_ALPHA, DEFAULT_POINTS),
                Error::XpLockNotFound
            )
        });
    }
}
