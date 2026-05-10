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
// ````````````````````````` XP (EXPERIENCE POINTS) SUITE ````````````````````````
// ===============================================================================

//! Defines a interface for managing "Experience Points" (XP) as an
//! abstract economic primitive or a constrained resource.
//!
//! XP can serve as a non-monetary metric representing a wide range of contextual
//! values such as reputation, skill progression, contribution points, influence weight,
//! or any form of quantified domain-specific value.
//!
//! ## Experience Points (XP): A Formal Abstraction of Progress
//!
//! Experience Points or XP, represent a quantifiable measure of progress,
//! participation, or value within a system.
//!
//!  - Originally popularized in games to track advancement, now a known primitive
//! for measuring engagement and achievement.
//!  - Encodes effort, contribution, or status into a visible, programmable metric.
//!
//! ### Key Properties
//!
//! - **Non-transferable**: Linked to a specific user or role, cannot be freely
//! moved like currency.
//! - **Earned**: Collected through effort or activity, cannot be bought or
//! arbitrarily inflated.
//! - **Contextual**: Interpreted according to the domain's rules and objectives.
//! - **Comparable**: Can be ranked or measured, but is not fungible.
//!
//! These properties make XP an economic primitive for designing systems that value engagement,
//! merit, and progression over purely financial or fungible incentives.
//!
//! ## Overview
//!
//! The system is composed of modular-traits that define behaviors such as:
//!
//! - XP creation, mutation, and ownership
//! - Locking and reserving XP for runtime-intents
//! - Burning/slashing XP as penalties or resets
//! - Emitting events to reflect lifecycle changes
//!
//! Implementers of this interface can define their own internal mechanics while providing
//! a standardized API. This trait-oriented design supports flexible integration across any
//! system that needs to quantify and govern non-monetary value.
//!
//! ## Use Cases
//!
//! XP can be used anywhere progress, participation, or contribution needs to be measured
//! or rewarded.
//!
//! Common scenarios include:
//!
//! - **Governance**: Reputation, voting influence, contribution tracking.
//! - **Gaming**:  Player progression, unlockable content, skill-based gating.
//! - **Workplace**: Skill development, training milestones, peer recognition.
//! - **Communities**: Engagement scores, moderation trust, contributor incentives.
//! - **Supply Chains**: Performance metrics, reliability scoring,  compliance history.
//!

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Local crate imports ---
use crate::{
    base::{Asset, Delimited, Keyed, RuntimeEnum, RuntimeError, Time},
    misc::Ignore,
};
// --- Core ---
use core::cmp::Ordering;

// --- FRAME Support ---
use frame_support::{
    pallet_prelude::*,
    traits::{tokens::Precision, VariantCount, VariantCountOf},
};

// --- Substrate primitives ---
use sp_runtime::traits::Saturating;
use sp_std::vec::Vec;

// ===============================================================================
// `````````````````````````````````` XP ERRORS ``````````````````````````````````
// ===============================================================================

/// XP-related error types.
///
/// `XpError` defines all possible error conditions that can occur during XP operations,
/// such as querying, mutation, locking, reserving, or lifecycle transitions.
///
/// Each variant represents a specific failure scenario, allowing for precise error handling
/// and reporting throughout the XP trait system.
pub enum XpError {
    /// The specified XP entry does not exist.
    XpNotFound,
    /// The specified XP reserve does not exist.
    XpReserveNotFound,
    /// The specified XP lock does not exist.
    XpLockNotFound,
    /// Not enough liquid XP is available to complete the operation.
    InsufficientLiquidXp,
    /// The maximum number of reserves for this XP entry has been reached.
    TooManyReserves,
    /// The maximum number of locks for this XP entry has been reached.
    TooManyLocks,
    /// Attempted to lock zero XP points (not allowed).
    CannotLockZero,
    /// Attempted to reserve zero XP points (not allowed).
    CannotReserveZero,
    /// The XP entry has already been reaped (finalized) and cannot be reused.
    XpAlreadyReaped,
    /// The XP entry is alive and cannot be considered `dead` to reap.
    XpNotDead,
    // The XP entry is utilized for locks (runtime intent), hence cannot be reaped.
    CannotReapLockedXp,
    /// Not enough reserve XP is available to complete the operation.
    InsufficientReserveXp,
    /// The maximum capacity of XP was exceeded due to an arithmetic operation.    
    XpCapOverflowed,
    /// An arithmetic underflow occurred while subtracting XP points.
    XpCapUnderflowed,
    /// The maximum capacity of XP reserve was exceeded due to an arithmetic operation.
    XpReserveCapOverflowed,
    /// An arithmetic underflow occurred while subtracting reserved XP points.
    XpReserveCapUnderflowed,
    /// The maximum capacity of XP lock was exceeded due to an arithmetic operation.
    XpLockCapOverflowed,
    /// An arithmetic underflow occurred while subtracting locked XP points.
    XpLockCapUnderflowed,
}

/// A trait for mapping **domain-level XP errors** into
/// **caller- or pallet-specific error types**.
///
/// This trait acts as a bridge between the generic, FRAME-agnostic
/// [`XpError`] enum and the concrete error type expected by the
/// execution context.
pub trait XpErrorHandler {
    /// Concrete error type produced by the handler.
    ///
    /// Implements conversion to [`DispatchError`].
    type Error: RuntimeError;

    /// Converts a generic [`XpError`] into the handler's
    /// concrete error type which implements `Into<DispatchError>`.
    ///
    /// This function centralizes error translation logic and ensures
    /// that all balance-related failures are surfaced consistently
    /// according to the caller's error domain.
    fn from_xp_error(e: XpError) -> Self::Error;
}

// ===============================================================================
// ``````````````````````````````````` ALIASES ```````````````````````````````````
// ===============================================================================

/// Alias for [`XpSystem::XpKey`]
pub type Key<T> = <T as XpSystem>::XpKey;

/// Alias for [`XpSystem::Points`]
pub type Points<T> = <T as XpSystem>::Points;

/// Alias for [`XpOwner::Owner`]
pub type Owner<T> = <T as XpOwner>::Owner;

/// Alias for [`XpLock::LockReason`]
pub type LockReason<T> = <T as XpLock>::LockReason;

/// Alias for [`XpReserve::ReserveReason`]
pub type ReserveReason<T> = <T as XpReserve>::ReserveReason;

// ===============================================================================
// `````````````````````````````````` XP SYSTEM ``````````````````````````````````
// ===============================================================================

/// Core trait for querying XP state and metadata.
///
/// This trait defines the foundational interface for accessing XP data
/// in a read-only manner. It does not provide mutation logic.
///
/// If this is the only trait implemented, then it is assumed that the
/// implementer manually provides an XP state, for which the runtime only
/// supports querying.
pub trait XpSystem {
    /// Represents the full XP structure, which may include metadata or flags.
    ///
    /// Typically modeled as a struct when supporting features like locking or
    /// reserving, enabling high-level state queries.
    ///
    /// For simpler implementations, it can be aliased to [`XpSystem::Points`] if
    /// only a scalar value is needed.
    type Xp: Delimited;

    /// Scalar unsigned value representing the numerical XP points.
    type Points: Asset;

    /// A unique key identifying each XP entry, distinct from the owner.
    ///
    /// Allows a single owner to hold multiple XP records. This can be a hash, UUID,
    /// or runtime-specific ID.
    ///
    /// For 1:1 mappings, `XpKey` may be aliased to [`XpOwner::Owner`], allowing
    /// owner-specific fields to be omitted.
    type XpKey: Keyed;

    /// Represents the lifecycle or context dependent timestamps for an XP entry.
    type TimeStamp: Time;

    /// An optional extension for external triggers to react, extend, modify
    /// XP implementations
    ///
    /// If implementor chooses to avoid extensions, no op [`Ignore<Self>`] can be used
    /// ```ignore
    /// type Extension = Ignore<Self>;
    /// ```
    type Extension: XpSystemExtensions<Via = Self>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Checks if an XP entry exists for the given key.
    ///
    /// This is the standard guard function for XP querying logic and serves as a prerequisite
    /// check before calling any methods that assume a given XP key exists in storage.
    ///
    /// ## Returns
    /// - `Ok(())` if the XP entry exists for the given key.
    /// - `Err(DispatchError)` if the XP entry does not exist.
    fn xp_exists(key: &Self::XpKey) -> DispatchResult;

    /// Validates if the XP entry meets the minimum domain-defined threshold.
    ///
    /// Often used for XP reaping and lifecycle management to determine entry validity.
    /// This check is not limited to a numeric value, but may include custom conditions
    /// that determine whether an XP entry remains valid within the system.
    ///
    /// ## Returns
    /// - `Ok(())` if the XP entry meets the minimum threshold requirements.
    /// - `Err(DispatchError)` if the XP entry falls below the minimum threshold.
    fn has_minimum_xp(key: &Self::XpKey) -> DispatchResult;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Retrieves the complete XP structure associated with the key.
    ///
    /// Returns the full XP data including any metadata, flags, or extended information
    /// beyond just the point value for comprehensive XP state inspection.
    ///
    /// ## Returns
    /// - `Ok(Xp)` containing the complete XP structure if the key exists.
    /// - `Err(DispatchError)` if the XP key does not exist.
    fn get_xp(key: &Self::XpKey) -> Result<Self::Xp, DispatchError>;

    /// Retrieves the liquid (free or accessible) XP for the given key.
    ///
    /// This excludes XP currently locked or reserved, and represents what is immediately
    /// usable.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the liquid XP amount if the key exists.
    /// - `Err(DispatchError)` if the XP key does not exist.
    fn get_liquid_xp(key: &Self::XpKey) -> Result<Self::Points, DispatchError>;

    /// Retrieves the total usable XP for the given key.
    ///
    /// This is the sum of liquid XP and XP held in reserves, representing the complete
    /// pool of XP that could potentially be accessed or utilized by the key owner.
    ///
    /// It is functionally the same as [`XpSystem::get_liquid_xp`] if there is no
    /// reserve implementation.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the total usable XP amount if the key exists.
    /// - `Err(DispatchError)` if the XP key does not exist.
    fn get_usable_xp(key: &Self::XpKey) -> Result<Self::Points, DispatchError>;
}

// ===============================================================================
// ````````````````````````````` XP SYSTEM EXTENSIONS ````````````````````````````
// ===============================================================================

/// Root trait for XP system extensions.
///
/// Exposes the underlying XP system (`Self::Via`) to extension traits
/// (e.g., listeners) without tying them to a concrete implementation.
///
/// `Via` is only required to implement [`XpSystem`] here (the base contract).
/// Additional XP capabilities (e.g., [`XpOwner`], [`XpMutate`]) can be required
/// by further bounding `Via` in downstream traits.
///
/// ## Example
/// ```ignore
/// pub trait XpOwnerListener
/// where
///     Self: XpSystemExtensions,
///     Self::Via: XpOwner<Self>,
/// {}
/// ```
pub trait XpSystemExtensions
where
    Self: Sized,
{
    /// The concrete XP system implementation.
    /// Possibly post-bounded to provide support for additional
    /// XP trait implementations.
    type Via: XpSystem;
}

impl<T> XpSystemExtensions for Ignore<T>
where
    Self: Sized,
    T: XpSystem,
{
    type Via = T;
}

// ===============================================================================
// ``````````````````````````````````` XP OWNER ``````````````````````````````````
// ===============================================================================

/// Trait for XP ownership and access control.
///
/// This trait defines the relationship between an `Owner` and their associated XP keys,
/// enabling access control and transfer semantics for XP entries.
pub trait XpOwner
where
    Self: XpSystem<Extension: XpOwnerListener + XpSystemExtensions<Via = Self>>,
{
    /// Represents the unique identifier for the owner of an XP entry.
    ///
    /// Typically used to associate XP records with accounts or verifiable entities
    /// in the system.
    type Owner: Keyed;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Checks whether the given owner controls the specified XP key.
    ///
    /// This is the primary access control check used by mutation and permission logic
    /// throughout the XP system. All ownership-sensitive operations should use this
    /// method to verify authorization before proceeding.
    ///
    /// ## Returns
    /// - `Ok(())` if the owner controls the specified XP key.
    /// - `Err(DispatchError)` if the owner does not control the XP key.
    fn is_owner(owner: &Self::Owner, key: &Self::XpKey) -> DispatchResult;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Returns all XP keys currently owned by the given owner.
    ///
    /// This method enables enumeration and inspection of an owner's XP portfolio,
    /// useful for performing bulk operations, displaying user assets, or implementing
    /// ownership-based queries and analytics.
    ///
    /// ## Returns
    /// - `Ok(Vec<XpKey>)` containing all XP keys owned by the specified owner.
    /// - `Err(DispatchError)` if the owner lookup fails.
    fn xp_of_owner(owner: &Self::Owner) -> Result<Vec<Self::XpKey>, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` CONSTRUCTORS ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Generates a deterministic XP key from the given owner and XP metadata.
    ///
    /// Abstracts the process of deriving a unique, reproducible XP key for a specific owner
    /// and XP record.
    ///
    /// Typically leverages [`crate::keys::KeyGenFor`] or a similar deterministic key
    /// derivation utility, combining the owner identifier, XP struct (or metadata), and a
    /// generated salt value.
    ///
    /// Guarantees that each XP key is unique for every distinct combination of owner, XP
    /// metadata, and salt. This enables support for namespaced, context-specific, or
    /// multi-record XP systems, allowing a single owner to possess multiple XP entries
    /// differentiated by context or purpose.
    ///
    /// ## Returns
    /// - `Ok(XpKey)` containing the generated deterministic key.
    /// - `Err(DispatchError)` if key generation fails or cannot be deterministically derived.
    fn xp_key_gen(owner: &Self::Owner, xp: &Self::Xp) -> Result<Self::XpKey, DispatchError>;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Transfers ownership of the given XP key from the current owner to a new owner.
    ///
    /// This function updates the ownership mapping to associate the XP key with the new owner.
    /// The transfer is immediate and permanent, removing all access rights from the previous
    /// owner and granting full control to the new owner.
    ///
    /// ## Note
    /// This is a high-level operation that enforces access control via [`Self::is_owner`].
    /// The underlying ownership update is performed by [`Self::set_owner`], which acts as
    /// the low-level primitive.
    ///
    /// ## Returns
    /// - `Ok(())` if the ownership transfer completes successfully.
    /// - `Err(DispatchError)` if the transfer fails due to access control or system errors.
    fn transfer_owner(
        owner: &Self::Owner,
        key: &Self::XpKey,
        new_owner: &Self::Owner,
    ) -> DispatchResult {
        Self::is_owner(owner, key)?;
        if owner == new_owner {
            return Ok(());
        }
        Self::set_owner(owner, key, new_owner)?;
        Self::on_xp_transfer(key, new_owner);
        Ok(())
    }

    /// Sets the owner of the given XP key.
    ///
    /// This updates the ownership mapping from the current owner to the new owner.
    ///
    /// ## Note
    /// This is a low-level primitive that directly mutates ownership without
    /// performing access control checks.
    ///
    /// This method enforces that an XP key cannot exist without an owner by
    /// requiring both the current and new owner during the update.
    ///
    /// Prefer using [`transfer_owner`](Self::transfer_owner) for safe ownership changes.
    ///
    /// ## Returns
    /// - `Ok(())` if the owner is successfully updated.
    /// - `Err(DispatchError)` if the operation fails.
    fn set_owner(
        current_owner: &Self::Owner,
        key: &Self::XpKey,
        new_owner: &Self::Owner,
    ) -> DispatchResult;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    
    /// Hook invoked after a successful XP ownership transfer.
    ///
    /// This method is a no-op by default, but can be overridden to:
    /// - Emit transfer events
    /// - Update metadata or access rights tied to the XP key
    /// - Trigger side effects related to ownership changes (optionally
    /// via listener [`XpOwnerListener::xp_transferred`])
    fn on_xp_transfer(key: &Self::XpKey, new_owner: &Self::Owner) {
        Self::Extension::xp_transferred(key, new_owner);
    }
}

// ===============================================================================
// `````````````````````````````` XP OWNER LISTENER ``````````````````````````````
// ===============================================================================

/// Listener trait for XP ownership events.
///
/// This listener is invoked on ownership changes (e.g., transfers),
/// if the [`XpOwner`] implementor chooses to call it.
///
/// It allows implementors to hook into transfer events for triggering
/// external logic.
///
/// ## Note
/// Listener hooks are best-effort and should be fail-safe. Implementations
/// may choose to invoke them selectively or not at all, so triggered logic
/// must not rely on guaranteed execution.
pub trait XpOwnerListener
where
    Self: XpSystemExtensions,
    Self::Via: XpOwner,
{
    /// Called when an XP ownership transfer occurs.
    fn xp_transferred(_key: &Key<Self::Via>, _new_owner: &Owner<Self::Via>) {}
}

impl<T> XpOwnerListener for Ignore<T>
where
    Self: XpSystemExtensions<Via = T>,
    T: XpOwner,
{
}

// ===============================================================================
// ````````````````````````````````` XP MUTATION `````````````````````````````````
// ===============================================================================

/// Trait for mutating (modifying) XP entries and providing default support utilities.
///
/// This trait defines how XP is created, earned, set, reduced, and reset,
/// and provides lifecycle hooks for reacting to XP changes.
///
/// XP mutation is **non-transferable** and always scoped to a specific `XpKey`.
///
/// Ownership, locking, and reserving are handled in separate traits.
///
/// If `XpMutate` is implemented, it typically implies that the runtime supports
/// dynamic XP mutation via intents-either from trusted system actors or untrusted
/// user inputs.
///
/// Additionally, this trait includes default support methods for common mutation
/// patterns such as slashing and burning XP. These methods encapsulate reusable
/// logic for safely reducing or resetting XP balances while handling edge cases.
pub trait XpMutate
where
    Self: XpOwner
        + XpErrorHandler
        + XpSystem<Extension: XpMutateListener + XpSystemExtensions<Via = Self>>,
{
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Returns the initial XP value for a newly created XP entry.
    ///
    /// This defines the starting point assigned during [`create_xp`](Self::create_xp).
    fn init_xp() -> Self::Points;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` CONSTRUCTORS ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Creates and initializes a new XP entry under the given key and owner.
    ///
    /// This is a high-level helper that:
    /// - Initializes the XP entry via [`Self::new_xp`]
    /// - Sets the initial XP using [`Self::init_xp`] and [`Self::set_xp`]
    /// - Triggers the creation hook via [`Self::on_xp_create`]
    ///
    /// ## Note
    /// This is one of the recommended way to create XP entries (along with
    /// [`BeginXp::begin_xp`]), ensuring consistent initialization and
    /// lifecycle handling.
    fn create_xp(owner: &Self::Owner, key: &Self::XpKey) -> DispatchResult {
        Self::new_xp(owner, key);
        let init = Self::init_xp();
        Self::set_xp(key, init)?;
        Self::on_xp_create(key, owner);
        Ok(())
    }

    /// Creates a new XP entry under the given key and owner.
    ///
    /// Initializes the XP record in storage and associates it with the provided
    /// owner. This establishes the foundational XP entry that can then be mutated
    /// through other operations like earning or setting XP values.
    ///
    /// This operation must be idempotent, meaning it is safe to retry with respect
    /// to already-initialized keys without causing errors or state corruption.
    fn new_xp(owner: &Self::Owner, key: &Self::XpKey);

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// **Use with caution!** Directly sets the liquid XP for the given key.
    ///
    /// This function bypasses typical XP flow and permission checks, allowing direct
    /// manipulation of XP values. It is intended strictly for low-level runtime intents
    /// such as migrations, internal resets, or administrative operations.
    ///
    /// This method must **never** be exposed to users or XP providers, as XP is meant
    /// to reflect earned value only through controlled mechanisms like [`XpMutate::earn_xp`].
    /// Direct setting can undermine the integrity of the XP system's earned-value principle.
    ///
    /// ## Returns
    /// - `Ok(())` if the XP value is successfully set.
    /// - `Err(DispatchError)` if the XP key does not exist or the operation fails.
    fn set_xp(key: &Self::XpKey, points: Self::Points) -> DispatchResult;

    /// Increases the liquid XP associated with a given key by the specified number of points.
    ///
    /// This is the primary mechanism for XP growth, designed for use in reward systems,
    /// leveling mechanics, achievement unlocks, and other scenarios where users earn XP
    /// through legitimate activities or contributions.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the actual XP earned after applying any internal adjustments.
    /// - `Err(DispatchError)` if the XP key does not exist or the operation fails.
    fn earn_xp(key: &Self::XpKey, points: Self::Points) -> Result<Self::Points, DispatchError> {
        let quote = Self::quote_earn_xp(key, points)?;
        Self::set_xp(key, quote)?;
        Self::on_xp_earn(key, quote);
        Ok(quote)
    }

    /// Quotes the effective XP that would be earned for the given key.
    ///
    /// This method applies runtime-specific constraints such as caps, rate limits,
    /// or validation rules, and returns the final amount that will be applied if
    /// [`Self::earn_xp`] is executed.
    ///
    /// The implementation should handle overflow gracefully using saturating or checked
    /// arithmetic to prevent system instability. Runtime-specific constraints such as
    /// earning caps, rate limits, or validation rules should be enforced internally.
    ///
    /// ## Note
    /// This does not mutate state and serves as a pure computation step.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the adjusted XP to be earned.
    /// - `Err(DispatchError)` if the XP key does not exist or validation fails.
    fn quote_earn_xp(
        key: &Self::XpKey,
        points: Self::Points,
    ) -> Result<Self::Points, DispatchError>;

    /// Reduces the liquid XP for the given key by the specified points.
    ///
    /// This is the preferred method for applying penalties. It provides a safe,
    /// high-level abstraction over XP reduction.
    ///
    /// This method attempts to slash the requested amount from the liquid XP balance.
    /// If sufficient liquid XP is available, the exact amount is slashed and returned.
    /// If available liquid XP is insufficient, all available liquid XP is burned instead
    /// and the actual burned amount is returned.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the actual points slashed or burned.
    /// - `Err(DispatchError)` if the XP key does not exist or the operation fails.
    fn slash_xp(key: &Self::XpKey, points: Self::Points) -> Result<Self::Points, DispatchError> {
        <Self as XpSystem>::xp_exists(key)?;
        let liquid = Self::get_liquid_xp(key)?;

        if liquid >= points {
            let remaining = liquid.saturating_sub(points);
            Self::set_xp(key, remaining)?;
            Self::on_xp_slash(key, points);
            return Ok(points);
        }

        let burn = Self::reset_xp(key)?;
        Self::on_xp_slash(key, liquid);
        Ok(burn)
    }

    /// Resets (burns) all liquid XP for the given key, returning the points burned.
    ///
    /// This method completely resets the liquid XP balance to `zero` and returns the
    /// previous value. This is a destructive operation that cannot be undone and
    /// represents a total forfeiture of the liquid XP balance.
    ///
    /// Burning is typically used for low-level runtime operations such as internal
    /// resets or state corrections.
    ///
    /// ## Note
    /// This is a low-level primitive. For penalty logic, prefer using [`Self::slash_xp`],
    /// which provides a safer and intention-revealing abstraction.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the amount of XP that was burned.
    /// - `Err(DispatchError)` if the XP key does not exist or the operation fails.
    fn reset_xp(key: &Self::XpKey) -> Result<Self::Points, DispatchError> {
        <Self as XpSystem>::xp_exists(key)?;
        let liquid = Self::get_liquid_xp(key)?;
        let reset_points = Self::Points::zero();
        Self::set_xp(key, reset_points)?;
        Self::on_xp_update(key, reset_points);
        Ok(liquid)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    
    /// Hook invoked after a new XP identity is created.
    ///
    /// This is called once during initialization and does not reflect
    /// subsequent balance updates.
    ///
    /// This method is a no-op by default, but can be overridden to:
    /// - Emit creation events
    /// - Initialize metadata or indexes
    /// - Trigger side effects tied to XP identity creation (optionally
    /// via listener [XpMutateListener::xp_created])
    fn on_xp_create(key: &Self::XpKey, owner: &Self::Owner) {
        Self::Extension::xp_created(key, owner);
    }

    /// Hook invoked after XP is earned for a given key.
    ///
    /// This reflects XP accumulation through valid actions.
    ///
    /// This method is a no-op by default, but can be overridden to:
    /// - Emit earning events
    /// - Update metadata or statistics
    /// - Trigger side effects related to XP accrual (optionally
    /// via listener [XpMutateListener::xp_earned])
    fn on_xp_earn(key: &Self::XpKey, earned_points: Self::Points) {
        Self::Extension::xp_earned(key, earned_points);
    }

    /// Hook invoked after XP is slashed for a given key.
    ///
    /// This reflects a reduction in XP due to penalties or protocol actions.
    ///
    /// This method is a no-op by default, but can be overridden to:
    /// - Emit slashing events
    /// - Update metadata or statistics
    /// - Trigger side effects related to XP reduction (optionally
    /// via listener [XpMutateListener::xp_slashed])
    fn on_xp_slash(key: &Self::XpKey, slashed_points: Self::Points) {
        Self::Extension::xp_slashed(key, slashed_points);
    }

    /// Hook invoked after XP is updated for a given key without a specific intent.
    ///
    /// This reflects a change in XP that is not explicitly categorized as earning,
    /// slashing, or resetting. It is typically used for internal adjustments,
    /// migrations, or state corrections where the cause is not semantically
    /// meaningful at the domain level.
    ///
    /// The `current_points` parameter represents the latest liquid XP after
    /// the update.
    ///
    /// This method is a no-op by default, but can be overridden to:
    /// - Emit generic update events
    /// - Synchronize external state or indexes
    /// - Trigger side effects that depend on the current XP value (optionally
    ///   via listener [`XpMutateListener::xp_updated`])
    fn on_xp_update(key: &Self::XpKey, current_points: Self::Points) {
        Self::Extension::xp_updated(key, current_points);
    }
}

// ===============================================================================
// ```````````````````````````` XP MUTATION LISTENER `````````````````````````````
// ===============================================================================

/// Listener trait for XP mutation events.
///
/// This listener is invoked on XP mutations (e.g., create, earn, slash, burn),
/// if the [`XpMutate`] implementor chooses to call it.
///
/// It allows implementors to hook into mutation events for triggering
/// external logic.
///
/// ## Note
/// Listener hooks are best-effort and should be fail-safe. Implementations
/// may choose to invoke them selectively or not at all, so triggered logic
/// must not rely on guaranteed execution **(unless the provider guarantees it)**.
pub trait XpMutateListener
where
    Self: XpOwnerListener,
    Self::Via: XpMutate,
{
    /// Called when a new XP identity is created.
    fn xp_created(_key: &Key<Self::Via>, _owner: &Owner<Self::Via>) {}

    /// Called when XP is earned for a given key.
    ///
    /// Points reflect the amount earned in this operation.
    fn xp_earned(_key: &Key<Self::Via>, _earned_points: Points<Self::Via>) {}

    /// Called when XP is slashed for a given key.
    ///
    /// Points reflect the amount reduced in this operation.
    fn xp_slashed(_key: &Key<Self::Via>, _slashed_points: Points<Self::Via>) {}

    /// Called when XP is reset for a given key.
    ///
    /// This reflects a complete reset of the liquid XP balance.
    fn xp_resetted(_key: &Key<Self::Via>) {}

    /// Called when XP is updated for a given key without a specific intent.
    ///
    /// Points reflect the current liquid XP after the update.
    ///
    /// This is typically used for internal adjustments, migrations, or
    /// reconciliation where the change is not categorized as earning,
    /// slashing, or resetting but could be without being explicit.
    fn xp_updated(_key: &Key<Self::Via>, _current_points: Points<Self::Via>) {}
}

impl<T> XpMutateListener for Ignore<T>
where
    Self: XpOwnerListener + XpSystemExtensions<Via = T>,
    T: XpMutate,
{
}

// ===============================================================================
// ````````````````````````````````` XP RESERVE ``````````````````````````````````
// ===============================================================================

/// Trait for reserving XP under specific reasons with built-in support utilities.
///
/// Reserved XP is set aside for future intent, constraints, or commitments,
/// and is temporarily excluded from the liquid/spendable pool. Reservations are
/// keyed by `ReserveReason` to allow multiple reserved segments per XP record.
///
/// Reserved XP is inaccessible to the owner until unreserved, but may be used by
/// the runtime for specific logical intents.
///
/// Typical use cases include planned usage, cooldowns, bonding, or module isolation.
///
/// Additionally, this trait provides default support methods for common reserve-related
/// patterns, such as validation, reserving XP, withdrawing reserves, burning, and slashing
/// reserved XP.
pub trait XpReserve
where
    Self: XpMutate + XpSystem<Extension: XpReserveListener + XpSystemExtensions<Via = Self>>,
{
    /// Structure representing reserve metadata (e.g., reason and reserved XP points).
    ///
    /// It is merely given for alias and hygiene reason for the implementation
    ///
    /// Reserve entries can be exposed to users for inspection or management.
    type Reserve: Delimited;

    /// The `ReserveReason` represents *why* XP is reserved or modified within the system.
    ///
    /// It should be a lightweight, bounded identifier that classifies the context or intent
    /// of runtime-level operations-such as staking, governance, or slashing.
    ///
    /// Should be constrained to a small, enumerable set defined by the runtime to prevent
    /// storage bloat.
    ///
    /// Example use cases:
    /// - `ReserveReason::Staking` - XP reserved for block author staking.
    /// - `ReserveReason::Treasury` - XP reserved for governance or public goods.
    type ReserveReason: RuntimeEnum + VariantCount;

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Checks if a reserve exists for the given XP key and reserve reason.
    ///
    /// This method serves as a guard function to verify reserve existence before performing
    /// operations that assume a specific reserve is present.
    ///
    /// ## Returns
    /// - `Ok(())` if a reserve exists for the specified key and reason.
    /// - `Err(DispatchError)` if the reserve does not exist or the XP key is invalid.
    fn reserve_exists(key: &Self::XpKey, reason: &Self::ReserveReason) -> DispatchResult;

    /// Checks if the XP entry has any active reserves.
    ///
    /// This method provides a quick existence check for any reserves without
    /// checking a reserve's specific reason. Useful as a precondition check
    /// before performing reserve-sensitive operations.
    ///
    /// ## Returns
    /// - `Ok(())` if the XP entry has one or more active reserves.
    /// - `Err(DispatchError)` if no reserves exist for the XP key.
    fn has_reserve(key: &Self::XpKey) -> DispatchResult;

    /// Checks if the specified points of XP can be reserved for the given key.
    ///
    /// This method performs comprehensive validation before allowing reserve creation:
    /// - Verifies the XP key exists and can support new reserves
    /// - Ensures the points to reserve are non-zero
    /// - Confirms sufficient liquid XP is available
    /// - Validates that adding the reserve won't cause arithmetic overflow
    ///
    /// This validation ensures that reserve operations will succeed and maintain
    /// system invariants when performed.
    ///
    /// ## Returns
    /// - `Ok(())` if the reserve can be safely created.
    /// - `Err(DispatchError)` if any validation condition fails.
    fn can_reserve_xp(key: &Self::XpKey, points: Self::Points) -> DispatchResult {
        ensure!(
            !points.is_zero(),
            Self::from_xp_error(XpError::CannotReserveZero).into()
        );
        let reservable = <Self as XpSystem>::get_liquid_xp(key)?;
        let total_reserved = Self::total_reserved(key)?;
        if points > reservable {
            return Err(Self::from_xp_error(XpError::InsufficientLiquidXp).into());
        }
        total_reserved
            .checked_add(&points)
            .ok_or(Self::from_xp_error(XpError::XpReserveCapOverflowed).into())?;
        Ok(())
    }

    /// Checks if an existing reserve can be mutated to the new value.
    ///
    /// This method validates whether an existing reserve's value can be safely changed
    /// to the specified points. It handles both increases and decreases in reserve value,
    /// ensuring that arithmetic operations won't overflow or underflow.
    ///
    /// This is essential for reserve modification operations that need to adjust
    /// existing reserve points while maintaining system stability.
    ///
    /// ## Returns
    /// - `Ok(())` if the reserve mutation is allowed.
    /// - `Err(DispatchError)` if the mutation would cause arithmetic errors or violate constraints.
    fn can_reserve_mutate(
        key: &Self::XpKey,
        reason: &Self::ReserveReason,
        points: Self::Points,
    ) -> DispatchResult {
        let reserved = Self::get_reserve_xp(key, reason)?;
        let total_reserved = Self::total_reserved(key)?;
        match reserved.cmp(&points) {
            Ordering::Less => {
                let increase = points.saturating_sub(reserved);
                total_reserved
                    .checked_add(&increase)
                    .ok_or(Self::from_xp_error(XpError::XpReserveCapOverflowed).into())?;
                Ok(())
            }
            Ordering::Greater => {
                let decrease = reserved.saturating_sub(points);
                total_reserved
                    .checked_sub(&decrease)
                    .ok_or(Self::from_xp_error(XpError::XpReserveCapUnderflowed).into())?;
                Ok(())
            }
            Ordering::Equal => Ok(()),
        }
    }

    /// Determines if a new XP reserve can be created for the given key and points.
    ///
    /// This method validates the fundamental requirements for creating a new reserve:
    /// - The XP key must exist in storage
    /// - The number of existing reserves must be below the maximum allowed
    /// - Adding the new reserve must not cause arithmetic overflow
    ///
    /// This is a more basic validation than [`can_reserve_xp`](Self::can_reserve_xp), focusing only on
    /// the structural requirements rather than liquid balance availability.
    ///
    /// ## Returns
    /// - `Ok(())` if reserve creation is structurally allowed.
    /// - `Err(DispatchError)` if any fundamental requirement fails.
    fn can_reserve_new(key: &Self::XpKey, points: Self::Points) -> DispatchResult {
        <Self as XpSystem>::xp_exists(key)?;
        let reserves = Self::get_all_reserves(key)?;
        if reserves.len() >= Self::maximum_reserves() {
            return Err(Self::from_xp_error(XpError::TooManyReserves).into());
        }
        let total_reserved = Self::total_reserved(key)?;
        total_reserved
            .checked_add(&points)
            .ok_or(Self::from_xp_error(XpError::XpReserveCapOverflowed).into())?;
        Ok(())
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Retrieves the amount of XP reserved under the specified reserve reason.
    ///
    /// This method returns the exact number of points currently reserved for the given
    /// reason, allowing precise queries of reserve states for accounting, validation,
    /// or display purposes.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the reserved XP amount for the specified reason.
    /// - `Err(DispatchError)` if the XP key or reserve does not exist.
    fn get_reserve_xp(
        key: &Self::XpKey,
        reason: &Self::ReserveReason,
    ) -> Result<Self::Points, DispatchError>;

    /// Retrieves the total points of XP actively reserved for the given key.
    ///
    /// **Performance Tip**: If total reserved XP is available as high-level metadata
    /// in the XP structure, it is more efficient to query this value
    /// directly rather than summing individual reserves.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the total reserved XP amount.
    /// - `Err(DispatchError)` if the XP key does not exist.
    fn total_reserved(key: &Self::XpKey) -> Result<Self::Points, DispatchError>;

    /// Retrieves all active reserve reasons associated with the XP key.
    ///
    /// Returns an empty vector if no reserves exist for the XP key.
    /// Use [`has_reserve`](Self::has_reserve) as a precondition to avoid unnecessary queries when
    /// no reserves exist.
    ///
    /// ## Returns
    /// - `Ok(Vec<ReserveReason>)` containing all active reserve reasons.
    /// - `Err(DispatchError)` if the XP key does not exist or lookup fails.
    fn get_all_reserves(key: &Self::XpKey) -> Result<Vec<Self::ReserveReason>, DispatchError>;

    /// Returns the maximum number of concurrent reserves allowed per XP key.
    ///
    /// This value is determined by the number of variants in the `ReserveReason` enum,
    /// as returned by [`VariantCountOf<Self::ReserveReason>`]. Each reserve must have a
    /// unique reason, so the maximum is bounded by the available reserve reasons.
    ///
    /// ## Returns
    /// - Returns the maximum number of concurrent reserves as a `usize`.
    fn maximum_reserves() -> usize {
        VariantCountOf::<Self::ReserveReason>::get() as usize
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// **Use with caution!** Directly sets the reserved XP for the given key and reason.
    ///
    /// This function bypasses standard XP flow and permission checks, allowing direct
    /// manipulation of reserve values. It is intended strictly for low-level runtime intents
    /// such as migrations, internal state resets, or administrative operations.
    ///
    /// This method must **never** be exposed to users or XP providers, as it allows
    /// arbitrary creation or mutation of reserves, which can break system invariants.
    ///
    /// If a reserve with the given reason does not exist, it will be created with the specified points.
    ///
    /// ## Returns
    /// - `Ok(())` if a reserve with specified XP key and reason is successfully created
    /// or mutated.
    /// - `Err(DispatchError)` if the operation fails due to system constraints.
    fn set_reserve(
        key: &Self::XpKey,
        reason: &Self::ReserveReason,
        points: Self::Points,
    ) -> DispatchResult;

    /// Reserve's the specified points of XP under the given reserve reason.
    ///
    /// This method deducts the specified points from the liquid balance and creates
    /// or updates a reserve with the given reason. If a reserve with the same reason already
    /// exists, its value is increased; otherwise, a new reserve is created.
    ///
    /// The operation ensures atomic consistency by validating preconditions and
    /// updating both the liquid balance and reserve state in a coordinated manner.
    /// This prevents partial updates that could leave the XP entry in an inconsistent state.
    ///
    /// ## Returns
    /// - `Ok(())` if the reserve is successfully created or updated.
    /// - `Err(DispatchError)` if the operation fails, with an appropriate error.
    fn reserve_xp(
        key: &Self::XpKey,
        reason: &Self::ReserveReason,
        points: Self::Points,
    ) -> DispatchResult {
        <Self as XpSystem>::xp_exists(key)?;
        let liquid = <Self as XpSystem>::get_liquid_xp(key)?;
        if liquid < points {
            return Err(Self::from_xp_error(XpError::InsufficientLiquidXp).into());
        };
        if Self::reserve_exists(key, reason).is_err() {
            let remaining = liquid.saturating_sub(points);
            <Self as XpMutate>::set_xp(key, remaining)?;
            Self::set_reserve(key, reason, points)?;
            Self::on_reserve_update(key, reason, points);
            return Ok(());
        }
        let remaining = liquid.saturating_sub(points);
        <Self as XpMutate>::set_xp(key, remaining)?;
        let old_reserve_points = Self::get_reserve_xp(key, reason)?;
        let new_reserve_points = old_reserve_points
            .checked_add(&points)
            .ok_or(Self::from_xp_error(XpError::XpReserveCapOverflowed).into())?;
        Self::set_reserve(key, reason, new_reserve_points)?;
        Self::on_reserve_update(key, reason, new_reserve_points);
        Ok(())
    }

    /// Withdraws the specified reserve, returning the reserved XP to the liquid balance.
    ///
    /// This method removes the entire reserve and restores all its reserved XP to the
    /// account's liquid balance. The reserve can only be withdrawn completely because
    /// partial withdrawals of reserved points are not supported by this method,
    /// use [`withdraw_reserve_partial`](Self::withdraw_reserve_partial) instead.
    ///
    /// The withdrawal operation is atomic, ensuring that both the reserve removal and
    /// liquid balance update occur together to maintain consistency.
    ///
    /// ## Returns
    /// - `Ok(())` if the reserve is successfully withdrawn.
    /// - `Err(DispatchError)` if the XP key or reserve does not exist or any of the
    /// operation fails.
    fn withdraw_reserve(key: &Self::XpKey, reason: &Self::ReserveReason) -> DispatchResult {
        <Self as XpSystem>::xp_exists(key)?;
        let reserve_points = Self::get_reserve_xp(key, reason)?;
        Self::withdraw_reserve_partial(key, reason, reserve_points, Precision::BestEffort)?;
        Ok(())
    }

    /// Resets (permanently burns) all reserved XP points for the given reason.
    ///
    /// This method completely resets the reserved XP balance to zero for the specified
    /// reason and returns the previous value. Unlike `XpLock::burn_lock`, the reserve entry
    /// structure is preserved but its point value is reset to zero, allowing for
    /// potential future reuse of the same reserve reason.
    ///
    /// ## Note
    /// This is a low-level primitive intended for internal state resets or corrections.
    /// It does not inherently represent a penalty. For penalty-oriented reductions,
    /// prefer using [`slash_reserve`](Self::slash_reserve).
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the amount of reserved XP that was burned.
    /// - `Err(DispatchError)` if the XP key or reserve does not exist or any operation fails.
    fn reset_reserve(
        key: &Self::XpKey,
        reason: &Self::ReserveReason,
    ) -> Result<Self::Points, DispatchError> {
        <Self as XpSystem>::xp_exists(key)?;
        let reserve_xp = Self::get_reserve_xp(key, reason)?;
        let reset_points = Zero::zero();
        Self::set_reserve(key, reason, reset_points)?;
        Ok(reserve_xp)
    }

    /// Reduces or burns reserved XP under the given reserve reason.
    ///
    /// This method provides flexible slashing behavior based on the reserve's
    /// current value:
    /// - If the reserved XP points is greater than specified points, only the
    /// requested amount is slashed
    /// - If the reserved XP points is less than the requested points, the entire
    /// reserve is reset
    ///
    /// ## Note
    /// This is the preferred method for applying penalties to reserved XP. It provides
    /// a safe, high-level abstraction over reserve reduction.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the actual amount slashed or burned.
    /// - `Err(DispatchError)` if the XP key or reserve does not exist or any of
    /// the operation fails.
    fn slash_reserve(
        key: &Self::XpKey,
        reason: &Self::ReserveReason,
        points: Self::Points,
    ) -> Result<Self::Points, DispatchError> {
        <Self as XpSystem>::xp_exists(key)?;
        let reserve_xp = Self::get_reserve_xp(key, reason)?;
        if reserve_xp < points {
            let burn_reserve_xp = Self::reset_reserve(key, reason)?;
            Self::on_reserve_slash(key, reason, burn_reserve_xp);
            return Ok(burn_reserve_xp);
        }
        // Slash the requested points
        let remaining = reserve_xp.saturating_sub(points);
        Self::set_reserve(key, reason, remaining)?;
        Self::on_reserve_slash(key, reason, points);
        Ok(points)
    }

    /// Withdraws a specified amount of reserved XP, returning it to the liquid balance.
    ///
    /// This method allows for partial or full withdrawal of reserved XP depending on the
    /// specified `points` and the `precision` mode. The withdrawn XP is transferred from
    /// the reserve back to the liquid balance, making it available for normal operations
    /// again.
    ///
    /// The precision parameter controls withdrawal behavior:
    /// - **Exact**: Only succeeds if the exact amount can be withdrawn, fails otherwise
    /// - **BestEffort**: Withdraws as much as possible up to the requested amount
    ///
    /// ## Returns
    /// - `Ok(())` if the withdrawal completes successfully according to the precision mode.
    /// - `Err(DispatchError)` if the XP key or reserve does not exist, or if exact precision
    /// fails.
    fn withdraw_reserve_partial(
        key: &Self::XpKey,
        reason: &Self::ReserveReason,
        points: Self::Points,
        precision: Precision,
    ) -> DispatchResult {
        <Self as XpSystem>::xp_exists(key)?;
        if points.is_zero() {
            return Ok(());
        }
        Self::reserve_exists(key, reason)?;
        let reserve = Self::get_reserve_xp(key, reason)?;
        let liquid = <Self>::get_liquid_xp(key)?;
        let (new_reserve, new_free) = match precision {
            Precision::Exact => {
                let new_reserve = reserve
                    .checked_sub(&points)
                    .ok_or(Self::from_xp_error(XpError::InsufficientReserveXp).into())?;
                let new_free = liquid
                    .checked_add(&points)
                    .ok_or(Self::from_xp_error(XpError::XpCapOverflowed).into())?;
                (new_reserve, new_free)
            }
            Precision::BestEffort => {
                let new_reserve = reserve.saturating_sub(points);
                let new_free = liquid.saturating_add(reserve.min(points));
                (new_reserve, new_free)
            }
        };
        match new_reserve.is_zero() {
            true => {
                let zero = Self::Points::zero();
                Self::set_reserve(key, reason, zero)?;
                Self::on_reserve_update(key, reason, zero);
            }
            false => {
                Self::set_reserve(key, reason, new_reserve)?;
                Self::on_reserve_update(key, reason, new_reserve);
            }
        }
        Self::set_xp(key, new_free)?;
        Ok(())
    }
    
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    
    /// Hook invoked after a reserve is created or its value is updated.
    ///
    /// The `reserve_points` parameter reflects the current value of the
    /// reserve after the update.
    ///
    /// ## Note
    /// An update does not imply a slashing event. It may represent either:
    /// - Depositing XP into a reserve (increase), or
    /// - Withdrawing XP from a reserve (decrease).
    ///
    /// This method is a no-op by default, but can be overridden to:
    /// - Emit reserve creation or update events
    /// - Update related metadata or statistics
    /// - Trigger side effects related to reserve changes (optionally
    ///   via listener [`XpReserveListener::reserve_updated`])
    fn on_reserve_update(
        key: &Self::XpKey,
        reason: &Self::ReserveReason,
        reserve_points: Self::Points,
    ) {
        Self::Extension::reserve_updated(key, reason, reserve_points);
    }

    /// Hook invoked after a reserve is slashed.
    ///
    /// The `slashed_points` parameter reflects the slashed value of the
    /// reserve in points.
    ///
    /// This method is a no-op by default, but can be overridden to:
    /// - Emit reserve slashing events
    /// - Update related metadata or statistics
    /// - Trigger side effects related to reserve slashing (optionally
    /// via listener [`XpReserveListener::reserve_slashed`])
    fn on_reserve_slash(
        key: &Self::XpKey,
        reason: &Self::ReserveReason,
        slashed_points: Self::Points,
    ) {
        Self::Extension::reserve_slashed(key, reason, slashed_points);
    }
}

// ===============================================================================
// ```````````````````````````` XP RESERVE LISTENER ``````````````````````````````
// ===============================================================================

/// Listener trait for XP reserving events.
///
/// This listener is invoked on xp reserving (e.g., updates, slashes, burns),
/// if the [`XpReserve`] implementor chooses to call it.
///
/// It allows implementors to hook into reserve events for triggering
/// external logic.
///
/// ## Note
/// Listener hooks are best-effort and should be fail-safe. Implementations
/// may choose to invoke them selectively or not at all, so triggered logic
/// must not rely on guaranteed execution.
pub trait XpReserveListener
where
    Self: XpMutateListener,
    Self::Via: XpReserve,
{
    /// Called when an XP reserve update event occurs.
    ///
    /// Points reflect total reserved points for the runtime reserve reason.
    ///
    /// ## Note
    /// This does not imply a slashing event. An update may result from:
    /// - Depositing XP into a reserve (increase), or
    /// - Withdrawing XP from a reserve (decrease).
    fn reserve_updated(
        _key: &Key<Self::Via>,
        _reason: &ReserveReason<Self::Via>,
        _total_points: Points<Self::Via>,
    ) {
    }

    /// Called when an XP reserve burn event occurs.
    ///
    /// Points reflect total slashed points for the runtime reserve reason.
    fn reserve_slashed(
        _key: &Key<Self::Via>,
        _reason: &ReserveReason<Self::Via>,
        _slashed_points: Points<Self::Via>,
    ) {
    }
}

impl<T> XpReserveListener for Ignore<T>
where
    Self: XpMutateListener + XpSystemExtensions<Via = T>,
    T: XpReserve,
{
}

// ===============================================================================
// ````````````````````````````````` XP LOCK `````````````````````````````````````
// ===============================================================================

/// Trait for issuing and managing XP locks.
///
/// Locked XP is set aside and made temporarily inaccessible, reducing the liquid
/// (spendable) balance for the duration of the lock. Locks are typically used to
/// enforce runtime constraints, commitments, or cooldowns, and are always scoped
/// to a specific XP entry.
///
/// - Multiple locks can exist per XP entry, each identified by a unique `LockReason`.
/// - Locking is non-transferable and always local to the XP entry; locked XP cannot
/// be moved or reassigned.
/// - Locks are intended for internal runtime use (e.g., staking, governance, slashing)
/// and should not be directly controlled by end users.
///
/// Typical use cases include staking, governance participation, temporary restrictions,
/// or module isolation.
///
/// Additionally, this trait provides default support methods for common lock-related
/// patterns, such as validation, locking, withdrawing, and slashing XP locks.
pub trait XpLock
where
    Self: XpMutate + XpSystem<Extension: XpLockListener + XpSystemExtensions<Via = Self>>,
{
    /// Structure representing lock metadata (e.g., ID, locked XP points).
    ///
    /// It is merely given for alias and hygiene reason for the implementation
    ///
    /// Locking should be internally controlled by runtime intent, not exposed to end
    /// users.
    ///
    /// **Note**:
    /// - XP locks are strictly for internal use, not for direct user access (unlike
    /// fungible assets).
    /// - Allowing users direct control over locks can lead to manipulation or spam-like
    /// behavior.
    type Lock: Delimited;

    /// The `LockReason` represents *why* XP is Locked or modified within the system.
    ///
    /// It is expected to be a lightweight, bounded identifier that classifies
    /// the context or intent of runtime-level operations-such as staking, governance, or
    /// slashing.
    ///
    /// Should be constrained to a small, enumerable set defined by the runtime to prevent
    /// storage bloat.
    ///
    /// Example use cases:
    /// - `LockReason::Staking` - XP locked due to block author staking.
    /// - `LockReason::Treasury` - XP redirected for governance or public goods.
    type LockReason: RuntimeEnum + VariantCount;


    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Checks if a lock exists for the given XP key and lock reason.
    ///
    /// This method serves as a guard function to verify lock existence before performing
    /// operations that assume a specific lock is present.
    ///
    /// ## Returns
    /// - `Ok(())` if a lock exists for the specified key and reason.
    /// - `Err(DispatchError)` if the lock does not exist or the XP key is invalid.
    fn lock_exists(key: &Self::XpKey, reason: &Self::LockReason) -> DispatchResult;

    /// Checks if the XP entry has any active locks.
    ///
    /// This method provides a quick existence check for any locks without
    /// checking a lock's specific reason. Useful as a precondition check
    /// before performing lock-sensitive operations.
    ///
    /// ## Returns
    /// - `Ok(())` if the XP entry has one or more active locks.
    /// - `Err(DispatchError)` if no locks exist for the XP key.
    fn has_lock(key: &Self::XpKey) -> DispatchResult;

    /// Checks if the specified points of XP can be locked for the given key.
    ///
    /// This method performs comprehensive validation before allowing lock creation:
    /// - Verifies the XP key exists and can support new locks
    /// - Ensures the points to lock are non-zero
    /// - Confirms sufficient liquid XP is available
    /// - Validates that adding the lock won't cause arithmetic overflow
    ///
    /// This validation ensures that lock operations will succeed and maintain
    /// system invariants when performed.
    ///
    /// ## Returns
    /// - `Ok(())` if the lock can be safely created.
    /// - `Err(DispatchError)` if any validation condition fails.
    fn can_lock_xp(key: &Self::XpKey, points: Self::Points) -> DispatchResult {
        Self::can_lock_new(key, points)?;
        let lockable = <Self as XpSystem>::get_liquid_xp(key)?;
        let total_locked = Self::total_locked(key)?;
        if points > lockable {
            return Err(Self::from_xp_error(XpError::InsufficientLiquidXp).into());
        }
        match total_locked.checked_add(&points) {
            Some(_pass) => Ok(()),
            None => Err(Self::from_xp_error(XpError::XpLockCapOverflowed).into()),
        }
    }

    /// Checks if an existing lock can be mutated to the new value.
    ///
    /// This method validates whether an existing lock's value can be safely changed
    /// to the specified points. It handles both increases and decreases in lock value,
    /// ensuring that arithmetic operations won't overflow or underflow and that the
    /// new value is valid (non-zero).
    ///
    /// This is essential for lock modification operations that need to adjust
    /// existing lock points while maintaining system stability.
    ///
    /// ## Returns
    /// - `Ok(())` if the lock mutation is allowed.
    /// - `Err(DispatchError)` if the mutation would cause arithmetic errors or violate
    /// constraints.
    fn can_lock_mutate(
        key: &Self::XpKey,
        reason: &Self::LockReason,
        points: Self::Points,
    ) -> DispatchResult {
        ensure!(
            !points.is_zero(),
            Self::from_xp_error(XpError::CannotLockZero).into()
        );
        let locked = Self::get_lock_xp(key, reason)?;
        let total_locked = Self::total_locked(key)?;
        match locked.cmp(&points) {
            Ordering::Less => {
                let increase = points.saturating_sub(locked);
                total_locked
                    .checked_add(&increase)
                    .ok_or(Self::from_xp_error(XpError::XpLockCapOverflowed).into())?;
                Ok(())
            }
            Ordering::Greater => {
                let decrease = locked.saturating_sub(points);
                total_locked
                    .checked_sub(&decrease)
                    .ok_or(Self::from_xp_error(XpError::XpLockCapUnderflowed).into())?;
                Ok(())
            }
            Ordering::Equal => Ok(()),
        }
    }

    /// Determines if a new XP lock can be created for the given key and points.
    ///
    /// This method validates the fundamental requirements for creating a new lock:
    /// - The XP key must exist in storage
    /// - The points to lock must be non-zero (prevents meaningless locks)
    /// - The number of existing locks must be below the maximum allowed
    /// - Adding the new lock must not cause arithmetic overflow
    ///
    /// This is a more basic validation than [`can_lock_xp`](Self::can_lock_xp), focusing only on
    /// the structural requirements rather than liquid balance availability.
    ///
    /// ## Returns
    /// - `Ok(())` if lock creation is structurally allowed.
    /// - `Err(DispatchError)` if any fundamental requirement fails.
    fn can_lock_new(key: &Self::XpKey, points: Self::Points) -> DispatchResult {
        <Self as XpSystem>::xp_exists(key)?;
        ensure!(
            !points.is_zero(),
            Self::from_xp_error(XpError::CannotLockZero).into()
        );
        let locks = Self::get_all_locks(key)?;
        if locks.len() >= Self::maximum_locks() {
            return Err(Self::from_xp_error(XpError::TooManyLocks).into());
        };
        let total_locked = Self::total_locked(key)?;
        total_locked
            .checked_add(&points)
            .ok_or(Self::from_xp_error(XpError::XpLockCapOverflowed).into())?;
        Ok(())
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` GETTERS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Retrieves the amount of XP locked under the specified lock reason.
    ///
    /// This method returns the exact number of points currently locked for the given
    /// reason, allowing precise queries of lock states for accounting, validation,
    /// or display purposes.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the locked XP amount for the specified reason.
    /// - `Err(DispatchError)` if the XP key or lock does not exist.
    fn get_lock_xp(
        key: &Self::XpKey,
        reason: &Self::LockReason,
    ) -> Result<Self::Points, DispatchError>;

    /// Retrieves the total points of XP actively locked for the given key.
    ///
    /// **Performance Tip**: If total locked XP is available as high-level metadata
    /// in the XP structure, it is more efficient to query this value
    /// directly rather than summing individual locks.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the total locked XP amount.
    /// - `Err(DispatchError)` if the XP key does not exist.
    fn total_locked(key: &Self::XpKey) -> Result<Self::Points, DispatchError>;

    /// Retrieves all active lock reasons associated with the XP key.
    ///
    /// Returns an empty vector if no locks exist for the XP key.
    /// Use [`has_lock`](Self::has_lock) as a precondition to avoid unnecessary queries when no locks exist.
    ///
    /// ## Returns
    /// - `Ok(Vec<LockReason>)` containing all active lock reasons.
    /// - `Err(DispatchError)` if the XP key does not exist or lookup fails.
    fn get_all_locks(key: &Self::XpKey) -> Result<Vec<Self::LockReason>, DispatchError>;

    /// Returns the maximum number of concurrent locks allowed per XP key.
    ///
    /// This value is determined by the number of variants in the `LockReason` enum,
    /// as returned by [`VariantCountOf<Self::LockReason>`]. Each lock must have a
    /// unique reason, so the maximum is bounded by the available lock reasons.
    ///
    /// ### Returns
    /// - Returns the maximum number of concurrent locks as a `usize`.
    fn maximum_locks() -> usize {
        VariantCountOf::<Self::LockReason>::get() as usize
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Burns (permanently removes) a lock and its associated XP.
    ///
    /// This method completely destroys both the lock entry and the XP it contained,
    /// representing full consumption of the locked value.
    ///
    /// This is typically used for internal operations or full withdrawal of a lock,
    /// as locks cannot be partially withdrawn unlike reserves.
    ///
    /// The handling of the burned XP is left to the caller or runtime logic.
    ///
    /// ## Note
    /// This does not inherently indicate a penalty. For penalty-oriented reductions,
    /// prefer using [`Self::slash_lock`].
    ///
    /// ## Returns
    /// - `Ok(())` if the lock is successfully burned.
    /// - `Err(DispatchError)` if the XP key or lock does not exist or the operation fails.
    fn burn_lock(key: &Self::XpKey, reason: &Self::LockReason) -> DispatchResult;

    /// **Use with caution!** Directly sets the locked XP for the given key and reason.
    ///
    /// This function bypasses standard XP flow and permission checks, allowing direct
    /// manipulation of lock values. It is intended strictly for low-level runtime intents
    /// such as migrations, internal state resets, or administrative operations.
    ///
    /// This method must **never** be exposed to users or XP providers, as it allows
    /// arbitrary creation or mutation of locks, which can break system invariants.
    /// Locks should always be created and withdrawn as whole units through controlled flows.
    ///
    /// If a lock with the given reason does not exist, it will be created with the specified
    /// points.
    ///
    /// ## Returns
    /// - `Ok(())` if a lock with specified XP key and reason is successfully created or mutated.
    /// - `Err(DispatchError)` if the operation fails due to system constraints.
    fn set_lock(
        key: &Self::XpKey,
        reason: &Self::LockReason,
        points: Self::Points,
    ) -> DispatchResult;

    /// Lock's the specified points of XP under the given lock reason.
    ///
    /// This method deducts the specified points from the liquid balance and creates
    /// or updates a lock with the given reason. If a lock with the same reason already
    /// exists, its value is increased; otherwise, a new lock is created.
    ///
    /// The operation ensures atomic consistency by validating preconditions and
    /// updating both the liquid balance and lock state in a coordinated manner.
    /// This prevents partial updates that could leave the XP entry in an inconsistent state.
    ///
    /// ## Returns
    /// - `Ok(())` if the lock is successfully created or updated.
    /// - `Err(DispatchError)` if the operation fails, with an appropriate error.
    fn lock_xp(
        key: &Self::XpKey,
        reason: &Self::LockReason,
        points: Self::Points,
    ) -> DispatchResult {
        <Self as XpSystem>::xp_exists(key)?;
        ensure!(
            !points.is_zero(),
            Self::from_xp_error(XpError::CannotLockZero).into()
        );
        let liquid = <Self as XpSystem>::get_liquid_xp(key)?;
        if liquid < points {
            return Err(Self::from_xp_error(XpError::InsufficientLiquidXp).into());
        };
        if Self::lock_exists(key, reason).is_err() {
            let remaining = liquid.saturating_sub(points);
            <Self as XpMutate>::set_xp(key, remaining)?;
            Self::set_lock(key, reason, points)?;
            Self::on_lock_update(key, reason, points);
            return Ok(());
        }
        let remaining = liquid.saturating_sub(points);
        <Self as XpMutate>::set_xp(key, remaining)?;
        let old_lock_points = Self::get_lock_xp(key, reason)?;
        let new_lock_points = old_lock_points
            .checked_add(&points)
            .ok_or(Self::from_xp_error(XpError::XpLockCapOverflowed).into())?;
        Self::set_lock(key, reason, new_lock_points)?;
        Self::on_lock_update(key, reason, new_lock_points);
        Ok(())
    }

    /// Withdraws the specified lock, returning the locked XP to the liquid balance.
    ///
    /// This method removes the entire lock and restores all its locked XP to the
    /// account's liquid balance. The lock can only be withdrawn completely because
    /// partial withdrawals of locked points are not supported by this method.
    ///
    /// The withdrawal operation is atomic, ensuring that both the lock removal and
    /// liquid balance update occur together to maintain consistency.
    ///
    /// ## Returns
    /// - `Ok(())` if the lock is successfully withdrawn.
    /// - `Err(DispatchError)` if the XP key or lock does not exist or any of the
    /// operation fails.
    fn withdraw_lock(key: &Self::XpKey, reason: &Self::LockReason) -> DispatchResult {
        <Self as XpSystem>::xp_exists(key)?;
        <Self as XpLock>::lock_exists(key, reason)?;
        let lock_points = Self::get_lock_xp(key, reason)?;
        let liquid = <Self as XpSystem>::get_liquid_xp(key)?;
        let new_liquid = liquid.saturating_add(lock_points);
        <Self as XpMutate>::set_xp(key, new_liquid)?;
        <Self as XpLock>::burn_lock(key, reason)?;
        Self::on_lock_burn(key, reason);
        Ok(())
    }

    /// Reduces or slashes locked XP under the given lock reason.
    ///
    /// This method provides flexible slashing behavior based on the lock's current value:
    /// - If the locked XP points is greater than specified points, only the requested
    /// amount is slashed
    /// - If the locked XP points is less than the requested points, the entire lock is
    /// burned
    ///
    /// This is typically used for penalty enforcement, where locked XP is reduced
    /// or fully forfeited based on protocol rules.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the actual amount slashed or burned.
    /// - `Err(DispatchError)` if the XP key or lock does not exist or any of the operation
    /// fails.
    fn slash_lock(
        key: &Self::XpKey,
        reason: &Self::LockReason,
        points: Self::Points,
    ) -> Result<Self::Points, DispatchError> {
        <Self as XpSystem>::xp_exists(key)?;
        let lock_xp = Self::get_lock_xp(key, reason)?;
        if lock_xp < points {
            Self::burn_lock(key, reason)?;
            Self::on_lock_slash(key, reason, lock_xp);
            Self::on_lock_burn(key, reason);
            return Ok(lock_xp);
        }
        // Slash the requested points
        let remaining = lock_xp.saturating_sub(points);
        Self::set_lock(key, reason, remaining)?;
        Self::on_lock_slash(key, reason, points);
        Ok(points)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Hook invoked after an XP lock is created or its value is updated.
    ///
    /// The `lock_points` parameter reflects the current value of the lock
    /// after the update.
    ///
    /// This method is a no-op by default, but can be overridden to:
    /// - Emit lock creation or update events
    /// - Update related metadata or statistics
    /// - Trigger side effects related to lock changes
    fn on_lock_update(key: &Self::XpKey, reason: &Self::LockReason, lock_points: Self::Points) {
        Self::Extension::lock_updated(key, reason, lock_points);
    }

    /// Hook invoked after an XP lock is burned (permanently removed).
    ///
    /// This method is a no-op by default, but can be overridden to:
    /// - Emit lock removal or burn events
    /// - Update related metadata or statistics
    /// - Trigger side effects related to lock removal
    fn on_lock_burn(key: &Self::XpKey, reason: &Self::LockReason) {
        Self::Extension::lock_burned(key, reason);
    }

    /// Hook invoked after a lock is slashed.
    ///
    /// The `slashed_points` parameter reflects the slashed value of the
    /// lock in points.
    ///
    /// This method is a no-op by default, but can be overridden to:
    /// - Emit slashing events
    /// - Update related metadata or statistics
    /// - Trigger side effects related to lock slashing
    fn on_lock_slash(key: &Self::XpKey, reason: &Self::LockReason, slashed_points: Self::Points) {
        Self::Extension::lock_slashed(key, reason, slashed_points);
    }
}

// ===============================================================================
// ````````````````````````````` XP LOCK LISTENER ````````````````````````````````
// ===============================================================================

/// Listener trait for XP locking events.
///
/// This listener is invoked on xp locking (e.g., updates, slashes, burns),
/// if the [`XpLock`] implementor chooses to call it.
///
/// It allows implementors to hook into locking events for triggering
/// external logic.
///
/// ## Note
/// Listener hooks are best-effort and should be fail-safe. Implementations
/// may choose to invoke them selectively or not at all, so triggered logic
/// must not rely on guaranteed execution.
pub trait XpLockListener
where
    Self: XpMutateListener,
    Self::Via: XpLock,
{
    /// Called when an XP lock update event occurs.
    ///
    /// Points reflect total locked points for the runtime lock reason.
    fn lock_updated(
        _key: &Key<Self::Via>,
        _reason: &LockReason<Self::Via>,
        _total_points: Points<Self::Via>,
    ) {
    }

    /// Called when an XP lock burn event occurs for the runtime lock reason.
    fn lock_burned(_key: &Key<Self::Via>, _reason: &LockReason<Self::Via>) {}

    /// Called when an XP lock burn event occurs.
    ///
    /// Points reflect total slashed points for the runtime lock reason.
    fn lock_slashed(
        _key: &Key<Self::Via>,
        _reason: &LockReason<Self::Via>,
        _slashed_points: Points<Self::Via>,
    ) {
    }
}

impl<T> XpLockListener for Ignore<T>
where
    Self: XpMutateListener + XpSystemExtensions<Via = T>,
    T: XpLock,
{
}

// ===============================================================================
// ````````````````````````````````` XP REAP `````````````````````````````````````
// ===============================================================================

/// Trait for XP lifecycle finalization (reaping) with built-in support utilities.
///
/// `XpReap` enables explicit deactivation or invalidation of XP entries that are no longer
/// in use or have failed to exhibit expected runtime behavior.
///
/// This trait extends XP mutation and system capabilities to ensure full lifecycle control,
/// including cleanup, guarded creation, and safe finalization.
///
/// XP entries marked as "reaped" are considered finalized and cannot be reinitialized.
///
/// Additionally, this trait includes default support methods for common reaping patterns,
/// such as validating whether an XP entry can be safely reaped and performing safe,
/// atomic reaping operations.
///
/// #### Example Use Cases
/// - Invalidated quests or tasks
/// - Expired onboarding flows
/// - Cleanup of abandoned or dead XP keys
pub trait XpReap
where
    Self: XpLock + XpSystem<Extension: XpReapListener + XpSystemExtensions<Via = Self>>,
{
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` CHECKERS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    
    /// Checks if the given XP key has been reaped (finalized).
    ///
    /// This method serves as a guard against accidental recreation or mutation of
    /// finalized XP entries.
    ///
    /// ## Returns
    /// - `Ok(())` if the XP key has been reaped.
    /// - `Err(DispatchError)` if the XP key has not been reaped or does not exist.
    fn is_reaped(key: &Self::XpKey) -> DispatchResult;

    /// Checks whether the given XP key can be safely reaped (finalized).
    ///
    /// This method enforces comprehensive safety conditions before allowing reaping:
    /// - The XP entry must exist in storage
    /// - The XP entry must not meet the minimum XP threshold (i.e., is "dead")
    /// - The XP entry must not have any active locks (prevents loss of locked value)
    /// - The XP entry must not already be reaped (prevents double-finalization)
    ///
    /// These conditions ensure that reaping only occurs when an XP entry is truly
    /// abandoned, expired, or no longer viable according to system rules.
    ///
    /// ## Returns
    /// - `Ok(())` if all safety conditions are satisfied and reaping is allowed.
    /// - `Err(DispatchError)` if any condition fails, with specific error indicating the
    /// failure reason.
    fn can_reap(key: &Self::XpKey) -> DispatchResult {
        if Self::is_reaped(key).is_ok() {
            return Err(Self::from_xp_error(XpError::XpAlreadyReaped).into());
        }

        Self::xp_exists(key)?;

        if Self::has_minimum_xp(key).is_ok() {
            return Err(Self::from_xp_error(XpError::XpNotDead).into());
        }

        if <Self as XpLock>::has_lock(key).is_ok() {
            return Err(Self::from_xp_error(XpError::CannotReapLockedXp).into());
        }

        Ok(())
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` MUTATORS ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Irreversibly marks the given XP key as reaped (finalized).
    ///
    /// This operation permanently invalidates the XP entry, making it unusable for future
    /// operations. The method returns the total usable points from the XP entry, allowing
    /// the runtime to determine how to handle the recovered value.
    ///
    /// Reaped XP cannot be recreated with the same key, ensuring that finalization is
    /// truly permanent. The recovered points can be redirected toward other purposes
    /// such as governance, treasury operations, or other runtime-controlled flows.
    ///
    /// This is a destructive operation that should only be performed when an XP entry
    /// is confirmed to be no longer needed or valid according to system rules.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the total usable points from the reaped XP entry.
    /// - `Err(DispatchError)` if the XP key does not exist or reaping fails.
    fn reap_xp(key: &Self::XpKey) -> Result<Self::Points, DispatchError>;

    /// Attempts to reap (finalize) the given XP entry if all conditions are met.
    ///
    /// This method provides a safe, atomic approach to XP finalization by first
    /// validating all reaping conditions using [`can_reap`](Self::can_reap), then proceeding with
    /// the irreversible reaping operation if validation passes.
    ///
    /// This is the recommended way to perform XP reaping as it ensures all safety
    /// invariants are checked before the destructive operation occurs.
    ///
    /// ## Returns
    /// - `Ok(Points)` containing the total usable points from the reaped XP entry.
    /// - `Err(DispatchError)` if any safety condition fails or the reaping operation
    /// encounters an error.
    fn try_reap(key: &Self::XpKey) -> Result<Self::Points, DispatchError> {
        Self::can_reap(key)?;
        let p = Self::reap_xp(key)?;
        Self::on_xp_reap(key);
        Ok(p)
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````````````` HOOKS ````````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    /// Hook invoked after an XP entry has been reaped.
    ///
    /// This method is a no-op by default, but can be overridden to:
    /// - Emit reaping events
    /// - Update related metadata
    /// - Trigger side effects related to XP finalization
    fn on_xp_reap(key: &Self::XpKey) {
        Self::Extension::xp_reaped(key);
    }
}

// ===============================================================================
// ```````````````````````````` XP REAP LISTENER `````````````````````````````````
// ===============================================================================

/// Listener trait for XP reaping events.
///
/// This listener is invoked on xp reaping events if the
/// [`XpLock`] implementor chooses to call it.
///
/// It allows implementors to hook into reaping events for triggering
/// external logic.
///
/// ## Note
/// Listener hooks are best-effort and should be fail-safe. Implementations
/// may choose to invoke them selectively or not at all, so triggered logic
/// must not rely on guaranteed execution.
pub trait XpReapListener
where
    Self: XpLockListener,
    Self::Via: XpLock,
{
    /// Called when an XP reap event occurs.
    fn xp_reaped(_key: &Key<Self::Via>) {}
}

impl<T> XpReapListener for Ignore<T>
where
    Self: XpLockListener + XpSystemExtensions<Via = T>,
    T: XpLock,
{
}

// ===============================================================================
// ```````````````````````````````` BEGIN XP `````````````````````````````````````
// ===============================================================================

/// Blanket Trait for safe initialization and earning of XP entries.
///
/// `BeginXp` by default extends [`XpReap`] to provide a unified entry point
/// for initializing new XP records or earning XP on existing ones, while ensuring
/// that reaped (finalized) XP keys cannot be reused.
///
/// This trait encapsulates guarded creation logic, preventing accidental
/// re-initialization of finalized XP entries and enforcing correct lifecycle
/// transitions.
pub trait BeginXp
where
    Self: XpReap + XpSystem<Extension: XpReapListener + XpSystemExtensions<Via = Self>>,
{
    /// Initializes a new XP entry or earns XP based on the current state of the key.
    ///
    /// This method provides state-aware XP management with the following behavior:
    /// - If the XP key does not exist and has never been reaped, creates a new XP entry
    /// for the owner
    /// - If the XP key exists and is not reaped, earns (increments) XP by the specified
    /// points
    /// - If the XP key has been reaped (finalized), prevents any operation and returns
    /// an error
    ///
    /// This unified approach ensures that XP operations respect the complete lifecycle,
    /// preventing resurrection of finalized entries while enabling seamless creation and
    /// growth of valid ones. The method serves as a safe entry point that handles all edge
    /// cases.
    ///
    /// ## Returns
    /// - `Ok(())` if the XP entry is successfully created or XP is successfully earned.
    /// - `Err(DispatchError)` if the XP key has been reaped or any underlying operation fails.
    fn begin_xp(owner: &Self::Owner, key: &Self::XpKey, points: Self::Points) -> DispatchResult {
        let exists = Self::xp_exists(key).is_ok();
        let reaped = Self::is_reaped(key).is_ok();
        if reaped {
            return Err(Self::from_xp_error(XpError::XpAlreadyReaped).into());
        }
        if !exists {
            Self::create_xp(owner, key)?;
            return Ok(());
        }
        Self::earn_xp(key, points)?;
        Ok(())
    }
}

/// Blanket implementation for [`BeginXp`] extending [`XpReap`].
impl<T> BeginXp for T where
    T: XpReap + XpSystem<Extension: XpReapListener + XpSystemExtensions<Via = Self>>
{
}
