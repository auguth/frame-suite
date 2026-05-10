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
// ``````````````````````````````` XP TRAITS IMPLS ```````````````````````````````
// ===============================================================================

//! Implementations of [`XP`](frame_suite::xp) traits for
//! the [`Pallet`] Type.
//!
//! [`Pallet`] implements:
//! - [`XpSystem`]
//! - [`XpOwner`]
//! - [`XpMutate`]
//! - [`XpReap`]
//! - [`XpReserve`]
//! - [`XpLock`]
//! - and other helper traits include
//!     - [`DiscreteAccumulator`]
//!     - [`XpErrorHandler`]
//!
//! Local Tests for these traits are covered in `tests`.

// ===============================================================================
// `````````````````````````````````` IMPORTS ````````````````````````````````````
// ===============================================================================

// --- Core ---
use core::cmp::Ordering;

// --- Local crate imports ---
use crate::{
    types::{Accumulator, IdXp, Stepper, Xp, XpId},
    Config, Error, Event, InitXp, LockedXpOf, MinPulse, MinTimeStamp, Pallet, PulseFactor,
    ReapedXp, ReservedXpOf, XpOf, XpOwners,
};

// --- FRAME Suite ---
use frame_suite::{
    accumulators::DiscreteAccumulator,
    keys::{KeyGenFor, KeySeedFor},
    xp::{
        XpError, XpErrorHandler, XpLock, XpLockListener, XpMutate, XpMutateListener, XpOwner,
        XpOwnerListener, XpReap, XpReapListener, XpReserve, XpReserveListener, XpSystem,
    },
};

// --- FRAME Support ---
use frame_support::{dispatch::DispatchResult, ensure, traits::VariantCountOf};

// --- FRAME System ---
use frame_system::pallet_prelude::BlockNumberFor;

// --- Substrate primitives ---
use sp_core::Get;
use sp_runtime::{
    traits::{CheckedAdd, CheckedMul, CheckedSub, One, Zero},
    BoundedVec, DispatchError, Saturating, Vec,
};

// ===============================================================================
// ````````````````````````````````` XP SYSTEM ```````````````````````````````````
// ===============================================================================

/// Implementation of the `XpSystem` trait for the XP pallet.
///
/// This provides the core, read-only interface for querying XP state, metadata,
/// and key management. All methods are implemented in terms of the pallet's storage
/// items and types.
impl<T: Config<I>, I: 'static> XpSystem for Pallet<T, I> {
    /// The primary data structure for XP accounts in this pallet.
    ///
    /// It encapsulates all metadata information for an XP entry,
    /// including liquid, reserved, and locked XP, as well as reputation pulse
    /// and timestamp.
    type Xp = Xp<T, I>;

    /// The scalar type representing XP points (the main XP balance unit).
    type Points = T::Xp;

    /// The unique key type for XP entries (distinct from the owner).
    ///
    /// Same as [`frame_system::Config::AccountId`]
    type XpKey = XpId<T>;

    /// The type representing the timestamp (block number) for XP lifecycle tracking.
    type TimeStamp = BlockNumberFor<T>;

    /// Pallet Extensions includes external listeners and their triggers.
    type Extension = T::Extensions;

    /// Checks if an XP entry exists for the given key.
    ///
    /// This function verifies the existence of an XP entry in storage by checking
    /// if the provided key exists in the `XpOf` storage map.
    ///
    /// ## Returns
    /// - `Ok(())` if the XP entry exists for the given key
    /// - `Err(DispatchError)` if the entry does not exist
    fn xp_exists(key: &Self::XpKey) -> DispatchResult {
        ensure!(XpOf::<T, I>::contains_key(key), Error::<T, I>::XpNotFound);
        Ok(())
    }

    /// Retrieves the complete XP struct for the given key.
    ///
    /// This function fetches the full XP data structure from storage,
    /// containing all metadata including liquid, reserved, locked XP,
    /// reputation pulse, and timestamp.
    ///
    /// ## Returns
    /// - `Ok(Xp)` containing the complete XP struct if found
    /// - `Err(DispatchError)` if the entry does not exist
    fn get_xp(key: &Self::XpKey) -> Result<Self::Xp, DispatchError> {
        let Some(xp) = XpOf::<T, I>::get(key) else {
            return Err(Error::<T, I>::XpNotFound.into());
        };
        Ok(xp)
    }

    /// Validates if the XP entry meets the minimum timestamp threshold.
    ///
    /// This function checks whether an XP entry's timestamp satisfies the
    /// minimum timestamp requirement, which is used for XP liveness validation
    /// and reaping logic.
    ///
    /// ## Returns
    /// - `Ok(())` if the XP entry meets the minimum timestamp threshold
    /// - `Err(DispatchError)` if the timestamp is below the minimum
    fn has_minimum_xp(key: &Self::XpKey) -> DispatchResult {
        let xp = Self::get_xp(key)?;
        // Instead of asserting scalar xp points, we enforce
        // minimum timestamp as criteria
        ensure!(
            xp.timestamp >= MinTimeStamp::<T, I>::get(),
            Error::<T, I>::LowTimeStamp
        );
        Ok(())
    }

    /// Retrieves the liquid (free) XP balance for the given key.
    ///
    /// This function returns liquid XP points, which represents the freely
    /// spendable XP balance that is not reserved or locked for any specific purpose.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the liquid XP balance if found
    /// - `Err(DispatchError)` if the entry does not exist
    fn get_liquid_xp(key: &Self::XpKey) -> Result<Self::Points, DispatchError> {
        let xp = Self::get_xp(key)?;
        Ok(xp.free)
    }

    /// Retrieves the total usable XP (liquid + reserved) for the given key.
    ///
    /// This function calculates and returns the sum of liquid and reserved XP,
    /// representing the total amount of XP that can be utilized by the account.
    /// Locked XP is excluded as it cannot be spent or transferred.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the total usable XP balance if found
    /// - `Err(DispatchError)` if the entry does not exist
    fn get_usable_xp(key: &Self::XpKey) -> Result<Self::Points, DispatchError> {
        let xp = Self::get_xp(key)?;
        Ok(xp.free.saturating_add(xp.reserve))
    }
}

// ===============================================================================
// ``````````````````````````````````` XP OWNER ``````````````````````````````````
// ===============================================================================

/// Implementation of the `XpOwner` trait for the XP pallet.
///
/// This provides the interface for XP ownership and access control, including
/// checking ownership, enumerating all XP keys owned by an account, transferring
/// ownership, and emitting ownership events.
///
/// All methods are implemented in terms of the pallet's storage items and types.
impl<T: Config<I>, I: 'static> XpOwner for Pallet<T, I> {
    /// The account ID type representing the owner of an XP entry.
    type Owner = T::AccountId;

    /// Checks if the given owner possesses ownership of the specified XP key.
    ///
    /// This function verifies ownership by checking if the owner-key pair exists
    /// in the [`XpOwners`] storage map.
    ///
    /// ## Returns
    /// - `Ok(())` if the owner possesses ownership of the XP key
    /// - `Err(DispatchError)` if the owner does not have ownership rights
    fn is_owner(owner: &Self::Owner, key: &Self::XpKey) -> DispatchResult {
        ensure!(
            XpOwners::<T, I>::contains_key((owner, key)),
            Error::<T, I>::InvalidXpOwner
        );
        Ok(())
    }

    /// Retrieves all XP keys currently owned by the given owner.
    ///
    /// ## Returns
    /// - `Ok(Vec<XpKey>)` containing all valid XP keys owned by the account
    /// - `Err(DispatchError)` if there are issues accessing storage
    fn xp_of_owner(owner: &Self::Owner) -> Result<Vec<Self::XpKey>, DispatchError> {
        let mut vec = Vec::new();
        // Direct iteration on the owner, hence carries no wasted compute
        let iter = XpOwners::<T, I>::iter_prefix((owner,));
        for (key, _) in iter {
            vec.push(key)
        }
        Ok(vec)
    }

    /// Sets the owner of the given XP key.
    ///
    /// ## Note
    /// This is a low-level primitive that directly mutates storage without
    /// performing access control checks.
    ///
    /// It should generally only be used internally. Prefer higher-level
    /// APIs such as [`Self::transfer_owner`] for safe ownership transitions.
    ///
    /// ## Returns
    /// - `Ok(())` if the owner is successfully updated
    /// - `Err(DispatchError)` if the operation fails
    fn set_owner(
        owner: &Self::Owner,
        key: &Self::XpKey,
        new_owner: &Self::Owner,
    ) -> DispatchResult {
        XpOwners::<T, I>::remove((owner, key));
        XpOwners::<T, I>::insert((new_owner, key), ());
        Ok(())
    }
    /// Generates a deterministic XP key from the provided owner and XP data.
    ///
    /// This function creates a unique XP key using the owner's account ID, the XP struct,
    /// and the owner's current nonce as salt to ensure uniqueness and prevent collisions.
    /// The key generation is deterministic for the same inputs and state-variables.
    ///
    /// ## Returns
    /// - `Ok(XpKey)` containing the generated XP key if successful
    /// - `Err(DispatchError)` if the key generation process fails
    fn xp_key_gen(owner: &Self::Owner, xp: &Self::Xp) -> Result<Self::XpKey, DispatchError> {
        let target: &Self::XpKey = owner;
        let salt = frame_system::Pallet::<T>::account_nonce(owner);
        let Some(key) =
            KeySeedFor::<Self::XpKey, Self::Xp, T::Nonce, T::Hashing, T>::gen_key(target, xp, salt)
        else {
            return Err(Error::<T, I>::CannotGenerateXpKey.into());
        };
        Ok(key)
    }

    /// Hook invoked after a successful XP ownership transfer.
    ///
    /// Emits an `XpOwner` event with the new owner and XP key.
    fn on_xp_transfer(key: &Self::XpKey, new_owner: &Self::Owner) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::XpOwner {
                id: key.clone(),
                owner: new_owner.clone(),
            });
        }
        Self::Extension::xp_transferred(key, new_owner)
    }
}

/// Implementation of the `XpMutate` trait for the XP pallet.
///
/// This provides the interface for mutating XP entries, including creation,
/// earning (with reputation effects), direct setting, and lifecycle hooks
/// for XP changes.
///
/// All methods are implemented in terms of the pallet's storage items and types.
impl<T: Config<I>, I: 'static> XpMutate for Pallet<T, I> {
    /// Returns the configured initial XP value for new entries.
    ///
    /// This value is retrieved from runtime storage ([`InitXp`]) and is used
    /// during [`Self::create_xp`] to initialize newly created XP records.
    fn init_xp() -> Self::Points {
        InitXp::<T, I>::get()
    }

    /// Creates and initializes a new XP entry for the given key and owner.
    ///
    /// **Use with caution!** as this bypasses typical XP flow and permission
    /// checks. Overwrites any existing XP entry without validation.
    ///
    /// For absolute safety, utilize [`frame_suite::xp::BeginXp::begin_xp`]
    fn new_xp(owner: &Self::Owner, key: &Self::XpKey) {
        let xp = Xp::<T, I>::default();
        XpOf::<T, I>::insert(key, xp);
        XpOwners::<T, I>::insert((&owner, &key), ());
    }

    /// **Use with caution!** This function bypasses typical XP flow and
    /// permission checks.
    ///
    /// Directly sets the liquid XP (`free`) for the given key.
    ///
    /// Unlike [`Self::earn_xp`], this method does not compute or validate the
    /// provided points. It simply overwrites the current liquid XP value.
    ///
    /// Intended for low-level runtime intents (e.g., migrations or internal resets).
    ///
    /// ## Returns
    /// - `Ok(())` if the XP was successfully set
    /// - `Err(DispatchError)` if the XP entry does not exist
    fn set_xp(key: &Self::XpKey, points: Self::Points) -> DispatchResult {
        XpOf::<T, I>::mutate(key, |result| -> DispatchResult {
            let value = result.as_mut().ok_or(Error::<T, I>::XpNotFound)?;
            value.free = points;
            Ok(())
        })?;
        Ok(())
    }

    /// Increments the liquid XP of a given key, applying pulse-based reputation mechanics.
    ///
    /// This function is the primary entry point for awarding XP from user-driven
    /// runtime actions such as task completion, participation events, or other
    /// domain-specific intents.
    ///
    /// Instead of directly crediting raw XP on every call, this method integrates
    /// a pulse-based reputation system that:
    /// - Prevents inflation from repeated calls within the same block
    /// - Gradually builds reputation (pulse) before scaling XP rewards
    /// - Multiplies earned XP once sufficient reputation is achieved
    /// - Provides accelerated reputation growth for locked (committed/staked) accounts
    ///
    /// ### Core Mechanics
    ///
    /// 1. **Same-block protection**
    ///    - If XP is earned multiple times within the same block and the pulse
    ///      is already above the minimum threshold, only raw XP is added.
    ///    - Pulse is intentionally NOT incremented to discourage rapid intra-block spamming.
    ///
    /// 2. **Pulse warm-up phase**
    ///    - If the pulse reputation is below [`MinPulse`], XP is not granted yet.
    ///    - Instead, the pulse accumulator is incremented, encouraging consistent
    ///      long-term participation rather than burst activity.
    ///
    /// 3. **Scaled XP phase**
    ///    - Once `MinPulse` is reached, earned XP is multiplied by the current
    ///      pulse value, rewarding reputable accounts with higher returns.
    ///
    /// 4. **Lock-based acceleration**
    ///    - If a lock exists on the XP key (e.g., staking or commitment),
    ///      the pulse is incremented again to accelerate future reputation growth.
    ///
    /// ### Note
    ///
    /// `MinPulse` is dynamic-storage value to support a live, gamified XP economy.
    /// As the ecosystem evolves, the required reputation tier can be
    /// adjusted to maintain fair progression, prevent early-stage farming,
    /// and keep long-term engagement meaningful without resetting user progress.
    ///
    /// ### Returns
    /// - `Ok(Points)` containing the actual XP credited after pulse scaling
    /// - `Err(DispatchError)` if computation or storage mutation fails
    fn earn_xp(key: &Self::XpKey, points: Self::Points) -> Result<Self::Points, DispatchError> {
        // Tracks the actual XP credited after all pulse scaling and checks.
        let mut actual = Self::Points::zero();

        XpOf::<T, I>::mutate(key, |result| -> DispatchResult {
            // Fetch the XP entry; fail if it does not exist.
            let value = result.as_mut().ok_or(Error::<T, I>::XpNotFound)?;

            // Current block number used for anti-spam and time-bound pulse logic.
            let current_block_height = <frame_system::Pallet<T>>::block_number();

            // -----------------------------------------------------------------
            // Same-block protection:
            // If XP earning is attempted again within the same block AND the
            // pulse reputation is already above the minimum threshold, we only
            // add raw XP without increasing pulse.
            //
            // This prevents artificial inflation of reputation from repeated
            // calls within a single block while still allowing XP crediting.
            // -----------------------------------------------------------------
            if current_block_height <= value.timestamp
                && value.pulse.value >= MinPulse::<T, I>::get()
            {
                let old_points = value.free;

                let new_points = old_points
                    .checked_add(&points)
                    .ok_or(Error::<T, I>::XpCapOverflowed)?;

                // Actual credited XP (safe difference computation).
                actual = new_points.saturating_sub(old_points);
                value.free = new_points;

                return Ok(());
            }

            // Update timestamp to indicate XP processing for this block.
            value.timestamp = current_block_height;

            // -----------------------------------------------------------------
            // Pulse warm-up phase:
            // If the pulse reputation has not yet reached the minimum threshold,
            // we do not grant XP. Instead, we increment the pulse accumulator
            // to gradually build reputation over time.
            // -----------------------------------------------------------------
            if value.pulse.value < MinPulse::<T, I>::get() {
                <Pallet<T, I> as DiscreteAccumulator>::increment(
                    &mut value.pulse,
                    &PulseFactor::<T, I>::get(),
                );
                return Ok(());
            }

            // -----------------------------------------------------------------
            // Scaled XP phase:
            // Once the pulse meets the minimum threshold, XP is multiplied by
            // the pulse value to reward reputable and consistent participants.
            // -----------------------------------------------------------------
            let multiplied = points
                .checked_mul(&value.pulse.value.into())
                .ok_or(Error::<T, I>::ReputationDeriveOverflowed)?;

            let new_points = multiplied
                .checked_add(&value.free)
                .ok_or(Error::<T, I>::XpCapOverflowed)?;

            let old_points = value.free;

            // Compute actual credited XP after scaling.
            actual = new_points
                .checked_sub(&old_points)
                .ok_or(Error::<T, I>::XpComputationError)?;

            value.free = new_points;

            // -----------------------------------------------------------------
            // Lock-based pulse acceleration:
            // If the account has an active lock (e.g., staked or committed),
            // increment pulse again to accelerate future reputation growth.
            //
            // This incentivizes stronger long-term participation by allowing
            // locked accounts to climb reputation tiers faster.
            // -----------------------------------------------------------------
            if <Self as XpLock>::has_lock(key).is_ok() {
                <Pallet<T, I> as DiscreteAccumulator>::increment(
                    &mut value.pulse,
                    &PulseFactor::<T, I>::get(),
                );
            }

            Ok(())
        })?;
        Self::on_xp_earn(key, actual);

        Ok(actual)
    }

    /// Determines the effective XP that would be earned for a given key,
    /// applying pulse-based reputation mechanics.
    ///
    /// This method mirrors the logic of [`XpMutate::earn_xp`] but does not mutate state.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the actual XP that would be credited after pulse scaling
    /// - `Err(DispatchError)` if computation fails or the XP key does not exist
    fn quote_earn_xp(
        key: &Self::XpKey,
        points: Self::Points,
    ) -> Result<Self::Points, DispatchError> {
        let value = XpOf::<T, I>::get(key).ok_or(Error::<T, I>::XpNotFound)?;

        let current_block_height = <frame_system::Pallet<T>>::block_number();

        // Same-block protection
        if current_block_height <= value.timestamp && value.pulse.value >= MinPulse::<T, I>::get() {
            return Ok(points);
        }

        // Pulse warm-up phase
        if value.pulse.value < MinPulse::<T, I>::get() {
            return Ok(Self::Points::zero());
        }

        // Scaled XP phase
        let multiplied = points
            .checked_mul(&value.pulse.value.into())
            .ok_or(Error::<T, I>::ReputationDeriveOverflowed)?;

        Ok(multiplied)
    }

    /// Hook invoked after an XP entry is updated reflecting
    /// currently available XP Points.
    ///
    /// Emits an `Xp` event with the XP key and liquid points if
    /// [`Config::EmitEvents`] is `true`.
    /// - Calls the Listener [`XpMutateListener::xp_updated`]
    fn on_xp_update(key: &Self::XpKey, points: Self::Points) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::Xp {
                id: key.clone(),
                xp: points,
            });
        }
        Self::Extension::xp_updated(key, points)
    }

    /// Hook invoked after a XP is earned.
    ///
    /// Emits an `XpEarn` event with the XP key and earned points if
    /// [`Config::EmitEvents`] is `true`.
    /// - Calls the Listener [`XpMutateListener::xp_earned`]
    fn on_xp_earn(key: &Self::XpKey, points: Self::Points) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::XpEarn {
                id: key.clone(),
                xp: points,
            });
        }
        Self::Extension::xp_earned(key, points);
    }

    /// Hook invoked after a new XP entry is created.
    ///
    /// Emits an `XpCreate` event with the XP key and owner if
    /// [`Config::EmitEvents`] is `true`.
    /// - Calls the listener [`XpMutateListener::xp_created`]
    fn on_xp_create(key: &Self::XpKey, owner: &Self::Owner) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::XpOwner {
                id: key.clone(),
                owner: owner.clone(),
            });
        }
        T::Extensions::xp_created(key, owner);
    }

    /// Hook invoked after XP points are slashed.
    ///
    /// Emits an `XpSlash` event with the XP key and slashed points if
    /// [`Config::EmitEvents`] is `true`.
    /// - Calls the listener [`XpMutateListener::xp_slashed`]
    fn on_xp_slash(key: &Self::XpKey, slashed_points: Self::Points) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::XpSlash {
                id: key.clone(),
                xp: slashed_points,
            });
        }
        T::Extensions::xp_slashed(key, slashed_points);
    }
}

// ===============================================================================
// `````````````````````````````````` XP RESERVE `````````````````````````````````
// ===============================================================================

/// Implementation of the `XpReserve` trait for the XP pallet.
///
/// This provides the interface for managing XP reserves, including
/// creation, mutation, querying, and event emission for reserved XP.
/// All methods are implemented in terms of the pallet's storage items and types.
///
impl<T: Config<I>, I: 'static> XpReserve for Pallet<T, I> {
    /// The structure representing reserve metadata (reason and reserved XP amount).
    type Reserve = IdXp<T::ReserveReason, T::Xp>;

    /// The lock reason identifier used to categorize locked XP points.
    type ReserveReason = T::ReserveReason;

    /// Checks if a reserve exists for the given XP key and reserve reason.
    ///
    /// ## Returns
    /// - `Ok(())` if the reserve exists for the given key and reason
    /// - `Err(DispatchError)` if the reserve does not exist
    fn reserve_exists(key: &Self::XpKey, reason: &Self::ReserveReason) -> DispatchResult {
        let Some(reserves) = ReservedXpOf::<T, I>::get(key) else {
            return Err(Error::<T, I>::XpReserveNotFound.into());
        };
        if !(reserves.iter().any(|reserve| reserve.id == *reason)) {
            return Err(Error::<T, I>::XpReserveNotFound.into());
        }
        Ok(())
    }

    /// Retrieves the XP points reserved under the specified reserve reason.
    ///
    /// This function returns the amount of XP points currently reserved for a specific
    /// reason on the given XP key.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the reserved XP points if found
    /// - `Err(DispatchError)` if the XP key or reserve reason does not exist
    fn get_reserve_xp(
        key: &Self::XpKey,
        reason: &Self::ReserveReason,
    ) -> Result<Self::Points, DispatchError> {
        let Some(reserves) = ReservedXpOf::<T, I>::get(key) else {
            return Err(Error::<T, I>::XpReserveNotFound.into());
        };
        let Some(reserve) = reserves.iter().find(|reserve| reserve.id == *reason) else {
            return Err(Error::<T, I>::XpReserveNotFound.into());
        };
        Ok(reserve.points)
    }

    /// Retrieves the total XP points actively reserved for the given key.
    ///
    /// This function returns the sum of all reserved XP across all reserve reasons
    /// for the specified XP key.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the total reserved XP points if found
    /// - `Err(DispatchError)` if the XP key does not exist
    fn total_reserved(key: &Self::XpKey) -> Result<Self::Points, DispatchError> {
        let Some(xp) = XpOf::<T, I>::get(key) else {
            return Err(Error::<T, I>::XpNotFound.into());
        };
        Ok(xp.reserve)
    }

    /// Checks if the given XP key has at least one active reserve.
    ///
    /// This function verifies that the XP key has one or more active reserves by
    /// checking if the reserves vector is non-empty.
    ///
    /// ## Returns
    /// - `Ok(())` if the XP key has at least one active reserve
    /// - `Err(DispatchError)` if no reserves exist for the XP key
    fn has_reserve(key: &Self::XpKey) -> DispatchResult {
        let Some(reserve) = ReservedXpOf::<T, I>::get(key) else {
            return Err(Error::<T, I>::XpReserveNotFound.into());
        };
        if reserve.is_empty() {
            return Err(Error::<T, I>::XpReserveNotFound.into());
        }
        Ok(())
    }

    /// Retrieves all active reserve reasons associated with the XP key.
    ///
    /// This function returns a list of all reserve reason identifiers currently
    /// active for the specified XP key.
    ///
    /// ## Returns
    /// - `Ok(Vec<Self::ReserveReason>)` containing all active reserve reasons
    /// - Empty vector if no reserves exist for the XP key
    fn get_all_reserves(key: &Self::XpKey) -> Result<Vec<Self::ReserveReason>, DispatchError> {
        let all_reserves = ReservedXpOf::<T, I>::get(key)
            .map(|reserves| reserves.iter().map(|reserve| reserve.id).collect())
            .unwrap_or_default();
        Ok(all_reserves)
    }

    /// Forcefully sets the reserved XP for a specific reserve reason.
    ///
    /// This function bypasses typical XP flow and permission checks, directly
    /// modifying reserve state without enforcing invariants.
    ///
    /// Creates a new reserve if none exists for the given reason, or updates an existing reserve.
    ///
    /// Use with caution as this is intended for internal runtime operations such
    /// as migrations, resets, or exceptional administrative flows.
    ///
    /// ## Returns
    /// - `Ok(())` if the reserve was successfully set
    /// - `Err(DispatchError)` if operation fails due to overflow or other constraints
    fn set_reserve(
        key: &Self::XpKey,
        reason: &Self::ReserveReason,
        points: Self::Points,
    ) -> DispatchResult {
        // Creates a new reserve if no reserve exist for the given key and reason.
        if Self::reserve_exists(key, reason).is_err() {
            // Permission and overflow checks are performed before creation to avoid inconsistent state.
            Self::can_reserve_new(key, points)?;
            let reserve = Self::Reserve::new(*reason, points);

            ReservedXpOf::<T, I>::mutate(key, |result| -> DispatchResult {
                let value = result.get_or_insert_with(|| {
                    BoundedVec::<Self::Reserve, VariantCountOf<Self::ReserveReason>>::default()
                });
                let result = value.try_push(reserve);

                debug_assert!(
                    result.is_ok(),
                    "reserves vector already bounded by reason, hence 
                    additional reserves cannot be attempted itself, inconsistency detected 
                    at set new reserve of points {points:?} for xp-key {key:?} for reason {reason:?}"
                );

                result.map_err(|_| Error::<T, I>::TooManyReserves)?;

                Ok(())
            })?;

            XpOf::<T, I>::mutate(key, |result| -> DispatchResult {
                let value = result.as_mut();
                debug_assert!(
                    value.is_some(),
                    "xp-key {key:?} reserve of reason {reason:?} newly created but Xp 
                    Meta not available to update high-level storage"
                );
                let value = value.ok_or(Error::<T, I>::XpNotFound)?;
                value.reserve = value
                    .reserve
                    .checked_add(&points)
                    .ok_or(Error::<T, I>::XpReserveCapOverflowed)?;

                Ok(())
            })?;
            return Ok(());
        }

        // Update an existing reserve
        // Permission and overflow checks are performed before mutation to avoid inconsistent state.
        Self::can_reserve_mutate(key, reason, points)?;
        ReservedXpOf::<T, I>::mutate(key, |result| -> DispatchResult {
            let value = result.as_mut();
            debug_assert!(
                value.is_some(),
                "can mutate reserve of xp-key {key:?} for reason {reason:?} but 
                cannot access the specific reserve-meta"
            );
            let value = value.ok_or(Error::<T, I>::XpReserveNotFound)?;
            let reserve = value
                .iter_mut()
                .find(|reserve| reserve.id == *reason)
                .ok_or(Error::<T, I>::XpReserveNotFound)?;
            let current_reserved = reserve.points;
            reserve.points = points;

            XpOf::<T, I>::mutate(key, |result| -> DispatchResult {
                let value = result.as_mut();
                debug_assert!(
                    value.is_some(),
                    "xp-key {key:?} reserve of reason {reason:?} recently mutated, but now Xp-meta 
                    not available to mutate"
                );
                let value = value.ok_or(Error::<T, I>::XpNotFound)?;

                let total_reserved = value.reserve;

                match current_reserved.cmp(&points) {
                    Ordering::Greater => {
                        let decrease = current_reserved.saturating_sub(points);
                        value.reserve = total_reserved.saturating_sub(decrease);
                    }
                    Ordering::Less => {
                        let increase = points.saturating_sub(current_reserved);
                        value.reserve = total_reserved.saturating_add(increase);
                    }
                    Ordering::Equal => return Ok(()),
                }
                Ok(())
            })?;
            Ok(())
        })?;
        Ok(())
    }

    /// Hook invoked after a new reservation is created or mutated.
    ///
    /// Emits an `XpReserve` event with the XP key, reserve reason,
    /// and reserve points if [`Config::EmitEvents`] is `true`.
    /// - Calls the Listener [`XpReserveListener::reserve_updated`]
    fn on_reserve_update(
        key: &Self::XpKey,
        reason: &Self::ReserveReason,
        reserve_points: Self::Points,
    ) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::XpReserve {
                of: key.clone(),
                reason: *reason,
                xp: reserve_points,
            });
        }
        Self::Extension::reserve_updated(key, reason, reserve_points);
    }

    /// Hook invoked after reserved XP points are slashed.
    ///
    /// Emits an `XpReserveSlash` event with the XP key, reserve reason,
    /// and slashed points if [`Config::EmitEvents`] is `true`.
    /// - Calls the listener [`XpReserveListener::reserve_slashed`]
    fn on_reserve_slash(
        key: &Self::XpKey,
        reason: &Self::ReserveReason,
        slashed_points: Self::Points,
    ) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::XpReserveSlash {
                of: key.clone(),
                reason: *reason,
                xp: slashed_points,
            });
        }
        T::Extensions::reserve_slashed(key, reason, slashed_points);
    }
}

// ===============================================================================
// ``````````````````````````````````` XP LOCK ```````````````````````````````````
// ===============================================================================

/// Implementation of the `XpLock` trait for the XP pallet.
///
/// This provides the interface for issuing, managing, and burning XP locks, as well as querying lock state.
/// All methods are implemented in terms of the pallet's storage items and types.
///
impl<T: Config<I>, I: 'static> XpLock for Pallet<T, I> {
    /// The structure representing lock metadata (reason and locked XP amount).
    type Lock = IdXp<T::LockReason, T::Xp>;

    /// The lock reason identifier used to categorize locked XP points.
    type LockReason = T::LockReason;

    /// Checks if the given XP key has at least one active lock.
    ///
    /// This function verifies that the XP key has one or more active locks by
    /// checking if the locks vector is non-empty.
    ///
    /// ## Returns
    /// - `Ok(())` if the XP key has at least one active lock
    /// - `Err(DispatchError)` if no locks exist for the XP key
    fn has_lock(key: &Self::XpKey) -> DispatchResult {
        let Some(locks) = LockedXpOf::<T, I>::get(key) else {
            return Err(Error::<T, I>::XpLockNotFound.into());
        };
        if locks.len().is_zero() {
            return Err(Error::<T, I>::XpLockNotFound.into());
        }
        Ok(())
    }

    /// Checks if a lock exists for the given XP key and lock reason.
    ///
    /// ## Returns
    /// - `Ok(())` if the lock exists for the given key and reason
    /// - `Err(DispatchError)` if the lock does not exist
    fn lock_exists(key: &Self::XpKey, reason: &Self::LockReason) -> DispatchResult {
        let Some(locks) = LockedXpOf::<T, I>::get(key) else {
            return Err(Error::<T, I>::XpLockNotFound.into());
        };
        if !(locks.iter().any(|lock| lock.id == *reason)) {
            return Err(Error::<T, I>::XpLockNotFound.into());
        }
        Ok(())
    }

    /// Retrieves the XP points locked under the specified lock reason.
    ///
    /// This function returns the amount of XP points currently locked for a specific
    /// reason on the given XP key.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the locked XP points if found
    /// - `Err(DispatchError)` if the XP key or lock reason does not exist
    fn get_lock_xp(
        key: &Self::XpKey,
        reason: &Self::LockReason,
    ) -> Result<Self::Points, DispatchError> {
        let Some(locks) = LockedXpOf::<T, I>::get(key) else {
            return Err(Error::<T, I>::XpLockNotFound.into());
        };
        let Some(lock) = locks.iter().find(|lock| lock.id == *reason) else {
            return Err(Error::<T, I>::XpLockNotFound.into());
        };
        Ok(lock.points)
    }

    /// Retrieves the total XP points actively locked for the given key.
    ///
    /// This function returns the sum of all locked XP across all lock reasons
    /// for the specified XP key.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the total locked XP points if found
    /// - `Err(DispatchError)` if the XP key does not exist
    fn total_locked(key: &Self::XpKey) -> Result<Self::Points, DispatchError> {
        let Some(xp) = XpOf::<T, I>::get(key) else {
            return Err(Error::<T, I>::XpNotFound.into());
        };
        Ok(xp.lock)
    }

    /// Retrieves all active lock reasons associated with the XP key.
    ///
    /// This function returns a list of all lock reason identifiers currently
    /// active for the specified XP key.
    ///
    /// ## Returns
    /// - `Ok(Vec<Self::LockReason>)` containing all active lock reasons
    /// - Empty vector if no locks exist for the XP key
    fn get_all_locks(key: &Self::XpKey) -> Result<Vec<Self::LockReason>, DispatchError> {
        let all_locks = LockedXpOf::<T, I>::get(key)
            .map(|locks| locks.iter().map(|lock| lock.id).collect())
            .unwrap_or_default();
        Ok(all_locks)
    }

    /// Burns a lock and permanently removes the associated XP.
    ///
    /// This function removes both the lock entry and destroys the locked XP points.
    /// Used in scenarios like forfeiture, decay, or permanent commitment where
    /// the XP should be permanently removed from circulation.
    ///
    /// ## Returns
    /// - `Ok(())` if the lock was successfully burned
    /// - `Err(DispatchError)` for the respected error.
    fn burn_lock(key: &Self::XpKey, reason: &Self::LockReason) -> DispatchResult {
        let locked = Self::get_lock_xp(key, reason)?;
        LockedXpOf::<T, I>::mutate(key, |result| -> DispatchResult {
            let value = result.as_mut().ok_or(Error::<T, I>::XpLockNotFound)?;
            value.retain(|lock| lock.id != *reason);
            Ok(())
        })?;

        XpOf::<T, I>::mutate(key, |result| -> DispatchResult {
            let value = result.as_mut();

            debug_assert!(
                value.is_some(),
                "xp-key {key:?} lock of reason {reason:?} exists where as Xp Meta doesn't"
            );

            let value = value.ok_or(Error::<T, I>::XpNotFound)?;

            let total_locked = value.lock;
            // If proper XP management is not enforced, this may result in saturation and potentially cause
            // `xp.lock` (the total locked XP) to underflow. For example, unsafe use of `set_lock` or
            // missing pre-condition checks in the XP system can lead to this state.
            //
            // This creates "lock dust" (unrecoverable XP) that persists due to prior imprecise mutations.
            // Since each lock is burned using its stored `points` value (not derived from `total_locked`),
            // this dust is only cleaned up when *all* locks are eventually removed.
            if total_locked < locked {
                debug_assert!(
                    false,
                    "xp-key {key:?} lock of reason {reason:?} value {locked:?} is greater than xp's total lock value {total_locked:?}"
                );
                // If `total_locked < locked`, we explicitly reset `xp.lock` to zero to dispose residual dust
                // when the final lock is burned. This state is internal and not exposed to providers, so
                // external actors will not get affected by this.
                value.lock = Self::Points::zero();
                return Ok(());
            }
            value.lock = total_locked.saturating_sub(locked);
            Ok(())
        })?;
        Ok(())
    }

    /// Forcefully sets the locked XP for a specific lock reason.
    ///
    /// This function bypasses typical XP flow and permission checks, directly
    /// modifying lock state without enforcing invariants.
    ///
    /// Creates a new lock if none exists for the given reason, or updates an existing lock.
    ///
    /// Use with caution as this is intended for internal runtime operations such
    /// as migrations, resets, or exceptional administrative flows.
    ///
    /// ## Returns
    /// - `Ok(())` if the lock was successfully set
    /// - `Err(DispatchError)` if operation fails due to overflow or other constraints
    fn set_lock(
        key: &Self::XpKey,
        reason: &Self::LockReason,
        points: Self::Points,
    ) -> DispatchResult {
        // Creates a new lock if no lock exist for the given key and reason.
        if Self::lock_exists(key, reason).is_err() {
            // Permission and overflow checks are performed before creation to avoid inconsistent state.
            Self::can_lock_new(key, points)?;
            let lock = Self::Lock::new(*reason, points);

            LockedXpOf::<T, I>::mutate(key, |result| -> DispatchResult {
                let value = result.get_or_insert_with(|| {
                    BoundedVec::<Self::Lock, VariantCountOf<T::LockReason>>::default()
                });
                let result = value.try_push(lock);

                debug_assert!(
                    result.is_ok(),
                    "locks vector already bounded by reason, hence additional locks cannot be attempted itself,
                    inconsistency detected at set new lock of points {points:?} for xp-key {key:?} for reason {reason:?}"
                );

                result.map_err(|_| Error::<T, I>::TooManyLocks)?;

                Ok(())
            })?;

            XpOf::<T, I>::mutate(key, |result| -> DispatchResult {
                let value = result.as_mut();
                debug_assert!(
                    value.is_some(),
                    "xp-key {key:?} lock of reason {reason:?} newly created but Xp 
                    Meta not available to update high-level storage"
                );
                let value = value.ok_or(Error::<T, I>::XpNotFound)?;
                // May saturate. Any resulting lock dust will be cleaned up during lock
                // withdrawal, slashing, or burn operations when all lock points are
                // about to be removed.
                // Since its the provider, that sets the lock, it is not in context,
                // where XP points may come from, hence saturation is possible, but
                // recovered over time.
                value.lock = value.lock.saturating_add(points);

                Ok(())
            })?;
            return Ok(());
        }

        // Update an existing lock
        // Permission and overflow checks are performed before mutation to avoid inconsistent state.
        Self::can_lock_mutate(key, reason, points)?;
        LockedXpOf::<T, I>::mutate(key, |result| -> DispatchResult {
            let value = result.as_mut();
            debug_assert!(
                value.is_some(),
                "can mutate lock of xp-key {key:?} for reason {reason:?} but 
                cannot access the specific lock-meta",
            );
            let value = value.ok_or(Error::<T, I>::XpLockNotFound)?;
            // Convert WeakBoundedVec into a mutable slice to access its elements.
            let slice = &mut value[..];
            let lock = slice
                .iter_mut()
                .find(|lock| lock.id == *reason)
                .ok_or(Error::<T, I>::XpLockNotFound)?;
            let current_locked = lock.points;
            lock.points = points;

            XpOf::<T, I>::mutate(key, |result| -> DispatchResult {
                let value = result.as_mut();
                debug_assert!(
                    value.is_some(),
                    "xp-key {key:?} lock of reason {reason:?} recently mutated, but now Xp-meta 
                    not available to mutate"
                );
                let value = value.ok_or(Error::<T, I>::XpNotFound)?;

                let total_locked = value.lock;
                match current_locked.cmp(&points) {
                    Ordering::Greater => {
                        let decrease = current_locked.saturating_sub(points);
                        value.lock = total_locked.saturating_sub(decrease);
                    }
                    Ordering::Less => {
                        let increase = points.saturating_sub(current_locked);
                        value.lock = total_locked.saturating_add(increase);
                    }
                    Ordering::Equal => return Ok(()),
                }
                Ok(())
            })?;
            Ok(())
        })?;
        Ok(())
    }

    /// Hook invoked after a new XP lock is successfully created or mutated.
    ///
    /// Emits an `XpLock` event with the XP key, lock reason, and
    /// lock points if [`Config::EmitEvents`] is `true`.
    /// - Calls the Listener [`XpLockListener::lock_updated`]
    fn on_lock_update(key: &Self::XpKey, reason: &Self::LockReason, lock_points: Self::Points) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::XpLock {
                of: key.clone(),
                reason: *reason,
                xp: lock_points,
            });
        }
        Self::Extension::lock_updated(key, reason, lock_points);
    }

    /// Hook invoked after an XP lock is burned and permanently removed.
    ///
    /// Emits an `XpLockBurn` event with the XP key and lock reason
    /// if [`Config::EmitEvents`] is `true`.
    /// - Calls the Listener [`XpLockListener::lock_burned`]
    fn on_lock_burn(key: &Self::XpKey, reason: &Self::LockReason) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::XpLockBurn {
                of: key.clone(),
                reason: *reason,
            });
        }
        Self::Extension::lock_burned(key, reason);
    }

    /// Hook invoked after locked XP points are slashed.
    ///
    /// Emits an `XpLockSlash` event with the XP key, lock reason,
    /// and slashed points if [`Config::EmitEvents`] is `true`.
    /// - Calls the listener [`XpLockListener::lock_slashed`]
    fn on_lock_slash(key: &Self::XpKey, reason: &Self::LockReason, slashed_points: Self::Points) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::XpLockSlash {
                of: key.clone(),
                reason: *reason,
                xp: slashed_points,
            });
        }
        T::Extensions::lock_slashed(key, reason, slashed_points);
    }
}
// ===============================================================================
// ``````````````````````````````````` XP REAP ```````````````````````````````````
// ===============================================================================

/// Implementation of the `XpReap` trait for the XP pallet.
///
/// This provides the interface for finalizing (reaping) XP entries,
/// checking reaped status, and emitting reaping events. All methods
/// are implemented in terms of the pallet's storage items and types.
///
impl<T: Config<I>, I: 'static> XpReap for Pallet<T, I> {
    /// Reaps the given XP key, removing all associated data from storage.
    ///
    /// This irreversibly deletes the XP entry from [`XpOf`] and [`ReservedXpOf`],
    /// and marks the key in [`ReapedXp`] to prevent accidental recreation.
    ///
    /// Returns the total usable (liquid + reserved) XP points, which may be imprecise in
    /// edge cases involving overflow or ignored dust, since the system does not track
    /// total issuance.
    ///
    /// Reaping forcibly removes reserves regardless of their presence.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the total usable (liquid + reserved) XP points that were
    ///   reaped, which may be imprecise in edge cases involving overflow or ignored dust,
    ///   since the system does not track total issuance.
    /// - `Err(DispatchError)` if XP locks exist or the entry does not exist
    fn reap_xp(key: &Self::XpKey) -> Result<Self::Points, DispatchError> {
        // Also does early return while checking xp-key existance in the system
        let reapable = <Self as XpSystem>::get_usable_xp(key)?;
        // Shall not reap if locks are present, as it signifies
        // the XP is utilized by the runtime
        if <Self as XpLock>::has_lock(key).is_ok() {
            return Err(Error::<T, I>::XpLockExists.into());
        }
        XpOf::<T, I>::remove(key);
        ReservedXpOf::<T, I>::remove(key);
        ReapedXp::<T, I>::insert(key, ());
        Ok(reapable)
    }

    /// Checks if the given XP key has been reaped.
    ///
    /// Used as a guard against accidental recreation or mutation of finalized XP entries.
    ///
    /// ## Returns
    /// - `Ok(())` if the XP key has been reaped
    /// - `Err(DispatchError)` if the XP key has not been reaped
    fn is_reaped(key: &Self::XpKey) -> DispatchResult {
        if !ReapedXp::<T, I>::contains_key(key) {
            return Err(Error::<T, I>::XpNotReaped.into());
        }
        Ok(())
    }

    /// Hook invoked after an XP entry has been reaped.
    ///
    /// - Emits an `XpReap` event with the reaped XP key
    ///   if [`Config::EmitEvents`] is `true`.
    /// - Calls the Listener [`XpReapListener::xp_reaped`]
    fn on_xp_reap(key: &Self::XpKey) {
        if T::EmitEvents::get() {
            Self::deposit_event(Event::XpReap { id: key.clone() });
        }
        Self::Extension::xp_reaped(key);
    }
}

// ===============================================================================
// ````````````````````````````````` ACCUMULATOR `````````````````````````````````
// ===============================================================================

/// Implementation of the `DiscreteAccumulator` trait for the XP pallet.
///
/// This trait provides an abstraction for accumulator data structures that can be incremented or decremented
/// by discrete steps, while maintaining an internal state that can be revealed as a readable value.
///
/// The accumulator increases its value when enough fractional steps have been collected to reach the threshold.
/// Similarly, it decreases its value when enough steps are removed, handling underflow and overflow gracefully.
///
impl<T: Config<I>, I: 'static> DiscreteAccumulator for Pallet<T, I> {
    /// The value type being accumulated.
    type Value = T::Pulse;

    /// The step type representing fractional progress.
    type Step = T::Pulse;

    /// The accumulator structure holding the current value and step count.
    type Accumulator = Accumulator<T, I>;

    /// The stepper configuration, defining the threshold and per-step increment.
    type Stepper = Stepper<T, I>;

    /// Increments the accumulator by the stepper's per-count value.
    ///
    /// When the accumulated step reaches or exceeds the threshold, the value is increased by one
    /// and the step is reduced accordingly. Handles overflow gracefully using saturating arithmetic.
    fn increment(accum: &mut Self::Accumulator, stepper: &Self::Stepper) {
        accum.step = accum.step.saturating_add(stepper.per_count);
        while accum.step >= stepper.threshold {
            accum.value = accum.value.saturating_add(One::one());
            accum.step = accum.step.saturating_sub(stepper.threshold);
        }
    }

    /// Decrements the accumulator by the stepper's per-count value.
    ///
    /// If the current step is greater than or equal to the per-count, it simply subtracts per-count from the step.
    /// Otherwise, it calculates the deficit needed to maintain a non-negative step.
    ///
    /// If the `value` is > 0, subtract 1, and set `step` to deficit, else set `step` to 0.
    fn decrement(accum: &mut Self::Accumulator, stepper: &Self::Stepper) {
        if accum.step >= stepper.per_count {
            accum.step = accum.step.saturating_sub(stepper.per_count);
            return;
        }
        let sub_pos = stepper.per_count.saturating_sub(accum.step);
        let deficit = stepper.threshold.saturating_sub(sub_pos);
        if accum.value.is_zero() {
            accum.step = Zero::zero();
            return;
        }
        accum.value = accum.value.saturating_sub(One::one());
        accum.step = deficit;
    }

    /// Reveals the current accumulated value from the internal state.
    ///
    /// Returns the main value of the accumulator, ignoring the fractional step.
    fn reveal(accum: &Self::Accumulator) -> Self::Value {
        accum.value
    }
}

// ===============================================================================
// `````````````````````````````` XP ERROR HANDLER ```````````````````````````````
// ===============================================================================

impl<T: Config<I>, I: 'static> XpErrorHandler for Pallet<T, I> {
    type Error = Error<T, I>;

    fn from_xp_error(e: XpError) -> Self::Error {
        match e {
            XpError::XpNotFound => Error::<T, I>::XpNotFound,
            XpError::XpReserveNotFound => Error::<T, I>::XpReserveNotFound,
            XpError::XpLockNotFound => Error::<T, I>::XpLockNotFound,
            XpError::InsufficientLiquidXp => Error::<T, I>::InsufficientLiquidXp,
            XpError::TooManyReserves => Error::<T, I>::TooManyReserves,
            XpError::TooManyLocks => Error::<T, I>::TooManyLocks,
            XpError::CannotLockZero => Error::<T, I>::CannotLockZero,
            XpError::CannotReserveZero => Error::<T, I>::CannotReserveZero,
            XpError::XpAlreadyReaped => Error::<T, I>::XpAlreadyReaped,
            XpError::XpNotDead => Error::<T, I>::XpNotDead,
            XpError::CannotReapLockedXp => Error::<T, I>::CannotReapLockedXp,
            XpError::InsufficientReserveXp => Error::<T, I>::InsufficientReserveXp,
            XpError::XpCapOverflowed => Error::<T, I>::XpCapOverflowed,
            XpError::XpCapUnderflowed => Error::<T, I>::XpCapUnderflowed,
            XpError::XpReserveCapOverflowed => Error::<T, I>::XpReserveCapOverflowed,
            XpError::XpReserveCapUnderflowed => Error::<T, I>::XpReserveCapUnderflowed,
            XpError::XpLockCapOverflowed => Error::<T, I>::XpLockCapOverflowed,
            XpError::XpLockCapUnderflowed => Error::<T, I>::XpLockCapUnderflowed,
        }
    }
}